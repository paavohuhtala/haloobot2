use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use anyhow::Context;
use dashmap::DashMap;
use itertools::Itertools;
use multimap::MultiMap;
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};
use teloxide::types::{ChatId, Sticker};
use tokio::sync::RwLock;

use crate::{chat_config::ChatConfigModel, db::DatabaseRef};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutoreplyResponse {
    Literal(String),
    Sticker(String),
}

#[derive(Clone, Debug)]
pub struct Autoreply {
    pub chat_id: ChatId,
    pub name: String,
    pub pattern_regex: Regex,
    pub response: AutoreplyResponse,
}

#[derive(Debug)]
pub struct AutoreplySet {
    autoreplies_by_regex: MultiMap<String, Autoreply>,
    autoreply_set: RegexSet,
}

impl AutoreplySet {
    pub fn empty() -> Self {
        Self {
            autoreplies_by_regex: MultiMap::new(),
            autoreply_set: RegexSet::empty(),
        }
    }

    pub fn new(autoreplies: &[Autoreply]) -> Self {
        let mut autoreplies_by_regex = MultiMap::new();

        for autoreply in autoreplies {
            autoreplies_by_regex.insert(
                autoreply.pattern_regex.as_str().to_string(),
                autoreply.clone(),
            );
        }

        let autoreply_set = RegexSet::new(
            autoreplies
                .iter()
                .map(|autoreply| autoreply.pattern_regex.as_str())
                .unique(),
        )
        .expect("Creating regex set should never fail");

        Self {
            autoreplies_by_regex,
            autoreply_set,
        }
    }

    pub fn get_matches<'a>(&'a self, message: &str) -> Vec<&'a Autoreply> {
        let match_collection = self.autoreply_set.matches(message);
        let mut matching_autoreplies = Vec::new();

        for regex_index in match_collection {
            let regex = self.autoreply_set.patterns()[regex_index].as_str();

            if let Some(autoreplies) = self.autoreplies_by_regex.get_vec(regex) {
                matching_autoreplies.extend(autoreplies);
            }
        }

        matching_autoreplies
    }

    pub fn add_autoreply(&mut self, autoreply: Autoreply) {
        self.autoreplies_by_regex
            .insert(autoreply.pattern_regex.as_str().to_string(), autoreply);

        self.autoreply_set = RegexSet::new(
            self.autoreplies_by_regex
                .iter()
                .map(|(_, autoreply)| autoreply.pattern_regex.as_str()),
        )
        .expect("Creating regex set should never fail");
    }
}

pub type AutoreplySetMap = Arc<RwLock<HashMap<ChatId, AutoreplySet>>>;

pub fn create_autoreply_set_map(replies: Vec<Autoreply>) -> AutoreplySetMap {
    let autoreplies_by_chat_id = replies.into_iter().into_group_map_by(|reply| reply.chat_id);

    let mut autoreply_map = HashMap::new();

    for (chat_id, autoreplies) in autoreplies_by_chat_id {
        let autoreply_set = AutoreplySet::new(&autoreplies);
        autoreply_map.insert(chat_id, autoreply_set);
    }

    Arc::new(RwLock::new(autoreply_map))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StickerEntry {
    pub id: String,
    pub file_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct StickersForEmoji {
    /// An LRU cache of sticker IDs.
    stickers: VecDeque<StickerEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct ChatStickerCache {
    by_emoji: HashMap<String, StickersForEmoji>,
}

pub struct StickerCache {
    by_chat: DashMap<ChatId, ChatStickerCache>,
    db: DatabaseRef,
    config: Arc<ChatConfigModel>,
}

impl StickersForEmoji {
    pub fn new(stickers: impl Into<VecDeque<StickerEntry>>) -> Self {
        Self {
            stickers: stickers.into(),
        }
    }

    pub fn select_sticker_and_update<'a>(
        &'a mut self,
        lru_size: u32,
        posted_sticker: &Sticker,
    ) -> Option<&'a StickerEntry> {
        use rand::prelude::*;

        // Try to find the sticker ID in the cache.
        let posted_sticker_index = self
            .stickers
            .iter()
            .position(|sticker| sticker.id == posted_sticker.file_unique_id);

        match posted_sticker_index {
            // If it exists and is the first in the cache, we don't need to do anything.
            Some(0) => {}
            // If it exists, move it to the front of the queue by removing and re-inserting it.
            Some(index) => {
                let sticker = self.stickers.remove(index).unwrap();
                self.stickers.push_front(sticker);
            }
            // Otherwise just add it to the front of the queue.
            None => {
                self.stickers.push_front(StickerEntry {
                    id: posted_sticker.file_unique_id.clone(),
                    file_id: posted_sticker.file_id.clone(),
                });
            }
        }

        self.stickers.truncate(lru_size as usize);

        let selected_sticker = self
            .stickers
            .iter()
            // Skip the first sticker, which is the sticker that was just posted.
            .skip(1)
            .choose(&mut thread_rng());

        selected_sticker
    }
}

impl ChatStickerCache {
    pub fn new(by_emoji: HashMap<String, StickersForEmoji>) -> Self {
        Self { by_emoji }
    }

    pub fn select_sticker_and_update<'a>(
        &'a mut self,
        lru_size: u32,
        emoji: &str,
        posted_sticker: &Sticker,
    ) -> Option<&'a StickerEntry> {
        let entry = self.by_emoji.entry(emoji.to_string()).or_default();
        entry.select_sticker_and_update(lru_size, posted_sticker)
    }
}

impl StickerCache {
    pub fn new(db: DatabaseRef, config: Arc<ChatConfigModel>) -> Self {
        StickerCache {
            by_chat: DashMap::new(),
            db,
            config,
        }
    }

    pub async fn update_and_get_response_sticker(
        &self,
        chat_id: ChatId,
        emoji: &str,
        posted_sticker: &Sticker,
    ) -> anyhow::Result<Option<StickerEntry>> {
        let config = self.config.get(chat_id).await?;
        let lru_size = config.sticker_lru_size;

        // We could do this with just one lookup using entry API if we didn't use async.

        if !self.by_chat.contains_key(&chat_id) {
            let new_entry = self.db.get_stickers_for_chat(chat_id).await?;
            self.by_chat.insert(chat_id, new_entry);
        }

        let mut cache_entry = self
            .by_chat
            .get_mut(&chat_id)
            .context("Sticker cache key should always exist after insertion")?;

        let sticker = cache_entry
            .select_sticker_and_update(lru_size, emoji, posted_sticker)
            .cloned();

        let stickers_for_emoji = cache_entry
            .by_emoji
            .get(emoji)
            .context("Reading from cache should never fail")?;

        self.db
            .update_seen_stickers(chat_id, emoji, &stickers_for_emoji.stickers)
            .await
            .context("Failed to update seen stickers to db")?;

        Ok(sticker)
    }
}
