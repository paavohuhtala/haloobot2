use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;
use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};
use teloxide::types::ChatId;
use tokio::sync::RwLock;

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

pub struct AutoreplySet {
    autoreplies_by_regex: HashMap<String, Autoreply>,
    autoreply_set: RegexSet,
}

#[derive(Clone, Debug)]
pub enum AddAutoreplyResult {
    Ok,
    AlreadyExists,
}

impl AutoreplySet {
    pub fn empty() -> Self {
        Self {
            autoreplies_by_regex: HashMap::new(),
            autoreply_set: RegexSet::empty(),
        }
    }

    pub fn new(autoreplies: &[Autoreply]) -> Self {
        let mut autoreplies_by_regex = HashMap::new();

        for autoreply in autoreplies {
            autoreplies_by_regex.insert(
                autoreply.pattern_regex.as_str().to_string(),
                autoreply.clone(),
            );
        }

        let autoreply_set = RegexSet::new(
            autoreplies
                .iter()
                .map(|autoreply| autoreply.pattern_regex.as_str()),
        )
        .expect("Creating regex set should never fail");

        Self {
            autoreplies_by_regex,
            autoreply_set,
        }
    }

    pub fn get_matches<'a>(&'a self, message: &str) -> Vec<&'a Autoreply> {
        let match_collection = self.autoreply_set.matches(message);

        match_collection
            .iter()
            .filter_map(|regex_index| {
                let regex = self.autoreply_set.patterns()[regex_index].as_str();
                self.autoreplies_by_regex.get(regex)
            })
            .collect()
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
