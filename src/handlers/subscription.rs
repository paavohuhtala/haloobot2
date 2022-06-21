use std::str::FromStr;

use chrono::NaiveTime;
use teloxide::{adaptors::AutoSend, prelude::Requester, types::ChatId, Bot};

use crate::{
    db::DatabaseRef,
    subscriptions::{Subscription, SubscriptionType, TIME_FORMAT},
};

pub async fn handle_subscribe(
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
    db: DatabaseRef,
    subscription_type: &str,
    time: &str,
) -> anyhow::Result<()> {
    let kind = SubscriptionType::from_str(subscription_type);

    let kind = match kind {
        Ok(kind) => kind,
        Err(_) => {
            bot.send_message(
                chat_id,
                "Epäkelpo tilauksen tyyppi. Käytä jokin seuraavista: comics, events",
            )
            .await?;

            return Ok(());
        }
    };

    let time = NaiveTime::parse_from_str(time, TIME_FORMAT);

    let time = match time {
        Ok(time) => time,
        Err(_) => {
            bot.send_message(chat_id, "Epäkelpo ajankohta. Käytä muotoa HH:MM")
                .await?;

            return Ok(());
        }
    };

    let subscription = Subscription {
        chat_id,
        kind,
        time,
    };

    db.add_subscription(&subscription).await?;

    log::info!("Added subscription: {:?}", subscription);

    bot.send_message(
        chat_id,
        format!(
            "🎉 Lisätty tilaus {}, päivittäin kello {}",
            kind.as_str(),
            time.format(TIME_FORMAT)
        ),
    )
    .await?;

    Ok(())
}
