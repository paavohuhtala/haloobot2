use std::str::FromStr;

use chrono::NaiveTime;
use teloxide::types::ChatId;

use crate::{
    command_handler::{fail, succeed_with_message, HandlerResult},
    db::DatabaseRef,
    subscriptions::{Subscription, SubscriptionType, TIME_FORMAT},
};

pub async fn handle_subscribe(
    chat_id: ChatId,
    db: DatabaseRef,
    subscription_type: &str,
    time: &str,
) -> HandlerResult {
    let kind = SubscriptionType::from_str(subscription_type);

    let kind = match kind {
        Ok(kind) => kind,
        Err(_) => {
            return fail("Epäkelpo tilauksen tyyppi. Käytä jokin seuraavista: comics, events");
        }
    };

    let time = NaiveTime::parse_from_str(time, TIME_FORMAT);

    let time = match time {
        Ok(time) => time,
        Err(_) => {
            return fail("Epäkelpo ajankohta. Käytä muotoa HH:MM");
        }
    };

    let subscription = Subscription {
        chat_id,
        kind,
        time,
    };

    db.add_subscription(&subscription).await?;

    log::info!("Added subscription: {:?}", subscription);

    succeed_with_message(format!(
        "🎉 Lisätty tilaus {}, päivittäin kello {}",
        kind.as_str(),
        time.format(TIME_FORMAT)
    ))
}
