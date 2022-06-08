use std::{str::FromStr, sync::Arc};

use anyhow::Context;
use chrono::NaiveTime;
use rusqlite::Connection;
use teloxide::types::ChatId;
use tokio::sync::Mutex;

use crate::subscriptions::{Subscription, SubscriptionType, TIME_FORMAT};

pub struct Database(Connection);
#[derive(Clone)]
pub struct DatabaseRef(Arc<Mutex<Database>>);

pub fn open_and_prepare_db() -> anyhow::Result<DatabaseRef> {
    let connection = Connection::open_in_memory().context("Failed to open SQLite database")?;

    connection
        .execute_batch(
            r#"
      CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY,
        chat_id INTEGER NOT NULL,
        date TEXT NOT NULL,
        countdown_days INTEGER
      );

      CREATE TABLE IF NOT EXISTS subscription_types (
        id TEXT NOT NULL PRIMARY KEY
      );

      INSERT INTO subscription_types (id) VALUES ('comics'), ('events');

      CREATE TABLE IF NOT EXISTS subscriptions (
        subscription_type TEXT NOT NULL REFERENCES subscription_types(id),
        chat_id INTEGER NOT NULL,
        time TEXT NOT NULL,

        PRIMARY KEY (chat_id, subscription_type)
      );
    "#,
        )
        .context("Failed to run database migrations")?;

    log::info!("Database prepared.");

    let db = Database(connection);
    let db_ref = DatabaseRef(Arc::new(Mutex::new(db)));

    Ok(db_ref)
}

impl DatabaseRef {
    pub async fn get_subscriptions(&self) -> anyhow::Result<Vec<Subscription>> {
        let db = self.0.lock().await;

        let rows: Vec<Subscription> =
            db.0.prepare("SELECT chat_id, subscription_type, time FROM subscriptions")?
                .query([])
                .context("Failed to query database")?
                .mapped(|row| {
                    let chat_id: i64 = row.get(0)?;
                    let subscription_type: String = row.get(1)?;
                    let time: String = row.get(2)?;

                    Ok((chat_id, subscription_type, time))
                })
                .filter_map(|row| match row {
                    Err(err) => {
                        log::error!("Failed to read subscription row: {:?}", err);
                        None
                    }
                    Ok(row) => Some(row),
                })
                .map(
                    |(chat_id, subscription_type, time)| -> anyhow::Result<Subscription> {
                        let chat_id = ChatId(chat_id);
                        let subscription_type = SubscriptionType::from_str(&subscription_type)?;
                        let time = NaiveTime::parse_from_str(&time, TIME_FORMAT)
                            .with_context(|| format!("Invalid time: {}", time))?;
                        Ok(Subscription {
                            chat_id,
                            kind: subscription_type,
                            time,
                        })
                    },
                )
                .filter_map(|maybe_row| match maybe_row {
                    Err(err) => {
                        log::error!("Failed to parse subscription row: {:?}", err);
                        None
                    }
                    Ok(row) => Some(row),
                })
                .collect();

        Ok(rows)
    }

    pub async fn add_subscription(&self, subscription: Subscription) -> anyhow::Result<()> {
        let db = self.0.lock().await;

        db.0.execute(
            "
        INSERT INTO subscriptions (chat_id, subscription_type, time) VALUES (?1, ?2, ?3)
        ON CONFLICT (chat_id, subscription_type) DO UPDATE SET time = ?3
        ",
            rusqlite::params![
                subscription.chat_id.0,
                subscription.kind.as_str(),
                subscription.time.format(TIME_FORMAT).to_string().as_str(),
            ],
        )?;

        Ok(())
    }
}
