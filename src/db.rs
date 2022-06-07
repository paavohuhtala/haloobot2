use anyhow::Context;
use rusqlite::Connection;

pub fn open_and_prepare_db() -> anyhow::Result<Connection> {
    let connection = Connection::open_in_memory().context("Failed to open SQLite database")?;

    connection
        .execute(
            r#"
      CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY,
        chat_id TEXT NOT NULL,
        date TEXT NOT NULL,
        countdown_days INTEGER
      )
    "#,
            [],
        )
        .context("Failed to run database migrations")?;

    log::info!("Database prepared.");

    Ok(connection)
}
