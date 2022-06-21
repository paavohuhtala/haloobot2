use std::{
    collections::{HashMap, VecDeque},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, Local, NaiveTime};
use regex::Regex;
use rusqlite::Connection;
use teloxide::types::ChatId;
use tokio::sync::Mutex;

use crate::{
    autoreplies::{Autoreply, ChatStickerCache, StickerEntry, StickersForEmoji},
    chat_config::ChatConfig,
    subscriptions::{Subscription, SubscriptionType, TIME_FORMAT},
};

#[derive(Debug)]
pub struct Database(Connection);
#[derive(Clone, Debug)]
pub struct DatabaseRef(Arc<Mutex<Database>>);

const SQL_TIME_FORMAT: &str = "%F %T";

pub fn open_and_prepare_db() -> anyhow::Result<DatabaseRef> {
    let connection = Connection::open("haloo.db3").context("Failed to open SQLite database")?;

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

      INSERT INTO subscription_types (id) VALUES ('comics'), ('events')
      ON CONFLICT DO NOTHING;

      CREATE TABLE IF NOT EXISTS subscriptions (
        subscription_type TEXT NOT NULL REFERENCES subscription_types(id),
        chat_id INTEGER NOT NULL,
        time TEXT NOT NULL,
        last_updated TEXT,

        PRIMARY KEY (chat_id, subscription_type)
      );

      CREATE TABLE IF NOT EXISTS autoreplies (
        id INTEGER NOT NULL PRIMARY KEY,
        chat_id INTEGER NOT NULL,
        name TEXT NOT NULL,
        pattern_regex TEXT NOT NULL,
        response_json TEXT NOT NULL,

        UNIQUE (chat_id, name)
      );

      CREATE TABLE IF NOT EXISTS chat_settings (
        chat_id INTEGER NOT NULL PRIMARY KEY,
        autoreply_chance REAL NOT NULL DEFAULT 0.5,
        sticker_lru_size INTEGER NOT NULL DEFAULT 20
      );

      CREATE TABLE IF NOT EXISTS seen_stickers (
        chat_id INTEGER NOT NULL,
        emoji TEXT NOT NULL,
        stickers_json TEXT NOT NULL,

        UNIQUE (chat_id, emoji)
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
    pub async fn set_autoreply_chance(&self, chat_id: ChatId, chance: f64) -> anyhow::Result<()> {
        let db = self.0.lock().await;

        db.0.prepare(
            "
            INSERT INTO chat_settings(chat_id, autoreply_chance) VALUES (?1, ?2)
            ON CONFLICT DO UPDATE SET autoreply_chance = ?2
            ",
        )?
        .execute(rusqlite::params![chat_id.0, chance])
        .context("Failed to update autoreply chance")?;

        Ok(())
    }

    pub async fn get_chat_config(&self, chat_id: ChatId) -> anyhow::Result<Option<ChatConfig>> {
        let db = self.0.lock().await;

        let mut statement = db.0.prepare(
            "SELECT autoreply_chance, sticker_lru_size FROM chat_settings WHERE chat_id = ?1",
        )?;

        let mut maybe_row = statement.query_and_then::<_, anyhow::Error, _, _>(
            rusqlite::params![chat_id.0],
            |row| {
                let autoreply_chance = row.get::<_, f64>(0)?;
                let sticker_lru_size = row.get::<_, u32>(1)?;

                Ok(ChatConfig {
                    chat_id,
                    autoreply_chance,
                    sticker_lru_size,
                })
            },
        )?;

        match maybe_row.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub async fn get_pending_subscriptions(
        &self,
        now: DateTime<Local>,
    ) -> anyhow::Result<Vec<Subscription>> {
        let db = self.0.lock().await;

        let formatted_time = now.format(SQL_TIME_FORMAT).to_string();

        let mut statement = db.0.prepare(
            "
            SELECT chat_id, subscription_type, time
            FROM subscriptions
            WHERE
              ((last_updated IS NULL) OR (date(last_updated) < date(?1)))
              AND time(subscriptions.time) <= time(?1)
          ",
        )?;

        let rows: Vec<Subscription> = statement
            .query(rusqlite::params![formatted_time])
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

    pub async fn mark_subscription_updated(
        &self,
        subscription: &Subscription,
        now: DateTime<Local>,
    ) -> anyhow::Result<()> {
        let db = self.0.lock().await;

        let chat_id = subscription.chat_id.0;
        let subscription_type = subscription.kind.as_str();

        let formatted_time = now.format(SQL_TIME_FORMAT).to_string();

        db.0.execute(
            "
                UPDATE subscriptions
                SET last_updated = ?3
                WHERE chat_id = ?1 AND subscription_type = ?2
            ",
            rusqlite::params![chat_id, subscription_type, formatted_time],
        )
        .context("Failed to update subscription timestamp")?;

        Ok(())
    }

    pub async fn add_subscription(&self, subscription: &Subscription) -> anyhow::Result<()> {
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

    pub async fn add_autoreply(&self, autoreply: &Autoreply) -> anyhow::Result<()> {
        let db = self.0.lock().await;

        db.0.execute(
            "
            INSERT INTO autoreplies (chat_id, name, pattern_regex, response_json) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT (chat_id, name) DO UPDATE SET pattern_regex = ?3, response_json = ?4
        ",
            rusqlite::params![
                autoreply.chat_id.0,
                autoreply.name,
                autoreply.pattern_regex.as_str(),
                serde_json::to_string(&autoreply.response).unwrap(),
            ],
        )?;

        Ok(())
    }

    pub async fn get_autoreplies(&self) -> anyhow::Result<Vec<Autoreply>> {
        let db = self.0.lock().await;

        let mut statement = db.0.prepare(
            "
            SELECT chat_id, name, pattern_regex, response_json
            FROM autoreplies
        ",
        )?;

        let rows = statement
            .query(rusqlite::params![])
            .context("Failed to query database")?;

        let mapped_rows = rows
            .mapped(|row| {
                let chat_id = row.get(0)?;
                let name: String = row.get(1)?;
                let pattern_regex: String = row.get(2)?;
                let response_json: String = row.get(3)?;

                Ok((chat_id, name, pattern_regex, response_json))
            })
            .filter_map(|row| match row {
                Err(err) => {
                    log::error!("Failed to read autoreply row: {:?}", err);
                    None
                }
                Ok(row) => Some(row),
            })
            .map(
                |(chat_id, name, pattern_regex, response_json)| -> anyhow::Result<Autoreply> {
                    let chat_id = ChatId(chat_id);
                    let name = name;
                    let pattern_regex = Regex::new(&pattern_regex)?;
                    let response = serde_json::from_str(&response_json)?;

                    Ok(Autoreply {
                        chat_id,
                        name,
                        pattern_regex,
                        response,
                    })
                },
            )
            .filter_map(|maybe_row| match maybe_row {
                Err(err) => {
                    log::error!("Failed to parse autoreply row: {:?}", err);
                    None
                }
                Ok(row) => Some(row),
            })
            .collect();

        Ok(mapped_rows)
    }

    pub async fn update_seen_stickers(
        &self,
        chat_id: ChatId,
        emoji: &str,
        stickers: &VecDeque<StickerEntry>,
    ) -> anyhow::Result<()> {
        let db = self.0.lock().await;

        let chat_id = chat_id.0;
        let emoji = emoji;

        let mut statement = db.0.prepare(
            "
            INSERT INTO seen_stickers (chat_id, emoji, stickers_json) VALUES (?1, ?2, ?3)
            ON CONFLICT (chat_id, emoji) DO UPDATE SET stickers_json = ?3
        ",
        )?;

        let stickers_json = serde_json::to_string(stickers).unwrap();

        statement
            .execute(rusqlite::params![chat_id, emoji, stickers_json])
            .context("Failed to update seen stickers")?;

        Ok(())
    }

    pub async fn get_stickers_for_chat(&self, chat_id: ChatId) -> anyhow::Result<ChatStickerCache> {
        let db = self.0.lock().await;

        let mut statement = db.0.prepare(
            "
            SELECT emoji, stickers_json
            FROM seen_stickers
            WHERE chat_id = ?1
        ",
        )?;

        let rows = statement
            .query(rusqlite::params![chat_id.0])
            .context("Failed to query database")?;

        let emoji_sticker_map: HashMap<String, StickersForEmoji> = rows
            .mapped(|row| {
                let emoji: String = row.get(0)?;
                let stickers_json: String = row.get(1)?;

                Ok((emoji, stickers_json))
            })
            .filter_map(|row| match row {
                Err(err) => {
                    log::error!("Failed to read seen stickers row: {:?}", err);
                    None
                }
                Ok(row) => Some(row),
            })
            .map(
                |(emoji, stickers_json)| -> anyhow::Result<(String, StickersForEmoji)> {
                    let stickers: VecDeque<StickerEntry> = serde_json::from_str(&stickers_json)?;
                    let stickers = StickersForEmoji::new(stickers);

                    Ok((emoji, stickers))
                },
            )
            .filter_map(|maybe_row| match maybe_row {
                Err(err) => {
                    log::error!("Failed to parse seen stickers row: {:?}", err);
                    None
                }
                Ok(row) => Some(row),
            })
            .collect();

        let cache = ChatStickerCache::new(emoji_sticker_map);

        Ok(cache)
    }
}
