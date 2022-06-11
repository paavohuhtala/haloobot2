use std::collections::HashMap;

use anyhow::Context;
use teloxide::types::ChatId;
use tokio::sync::RwLock;

use crate::db::DatabaseRef;

pub const DEFAULT_AUTOREPLY_CHANCE: f64 = 0.5;

#[derive(Debug, Clone)]
pub struct ChatConfig {
    pub chat_id: ChatId,
    pub autoreply_chance: f64,
}

impl ChatConfig {
    pub fn new(chat_id: ChatId) -> Self {
        Self {
            chat_id,
            autoreply_chance: DEFAULT_AUTOREPLY_CHANCE,
        }
    }
}

pub struct ChatConfigModel {
    db: DatabaseRef,
    cache: RwLock<HashMap<ChatId, ChatConfig>>,
}

impl ChatConfigModel {
    pub fn new(db: DatabaseRef) -> Self {
        Self {
            db,
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get(&self, chat_id: ChatId) -> anyhow::Result<ChatConfig> {
        {
            let reader = self.cache.read().await;
            let config = reader.get(&chat_id);

            if let Some(config) = config {
                return Ok(config.clone());
            }
        }

        let config = self
            .db
            .get_chat_config(chat_id)
            .await
            .context("Failed to fetch chat config")?
            .unwrap_or_else(|| ChatConfig::new(chat_id));

        {
            let mut writer = self.cache.write().await;
            writer.insert(chat_id, config.clone());
        }

        Ok(config)
    }

    pub async fn set_autoreply_chance(&self, chat_id: ChatId, chance: f64) -> anyhow::Result<()> {
        self.db
            .set_autoreply_chance(chat_id, chance)
            .await
            .context("Failed to update autoreply chance")?;

        {
            let mut writer = self.cache.write().await;
            writer
                .entry(chat_id)
                .or_insert_with(|| ChatConfig::new(chat_id))
                .autoreply_chance = chance;
        }

        Ok(())
    }
}
