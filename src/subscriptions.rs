use std::str::FromStr;

use chrono::NaiveTime;
use teloxide::types::ChatId;

pub const TIME_FORMAT: &str = "%H:%M";

#[derive(Copy, Clone, Debug)]
pub enum SubscriptionType {
    Comics,
    Events,
}

impl FromStr for SubscriptionType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "comics" => Ok(SubscriptionType::Comics),
            "events" => Ok(SubscriptionType::Events),
            _ => Err(anyhow::anyhow!("Invalid subscription type: {}", s)),
        }
    }
}

impl SubscriptionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionType::Comics => "comics",
            SubscriptionType::Events => "events",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Subscription {
    pub chat_id: ChatId,
    pub kind: SubscriptionType,
    pub time: NaiveTime,
}
