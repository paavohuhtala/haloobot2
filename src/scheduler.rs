use anyhow::Context;
use teloxide::prelude::*;

use crate::{
    db::DatabaseRef,
    handlers::{handle_fingerpori, handle_lasaga},
    subscriptions::{Subscription, SubscriptionType, TIME_FORMAT},
};

async fn handle_scheduled_task(
    bot: &AutoSend<Bot>,
    subscription: Subscription,
) -> anyhow::Result<()> {
    log::info!(
        "Handling scheduled task {} for chat {:?}",
        subscription.kind.as_str(),
        subscription.chat_id
    );

    match subscription.kind {
        SubscriptionType::Comics => {
            handle_fingerpori(bot, subscription.chat_id)
                .await
                .context("handle_fingerpori (scheduled)")?;

            handle_lasaga(bot, subscription.chat_id)
                .await
                .context("handle_lasaga (scheduled)")?;

            Ok(())
        }
        SubscriptionType::Events => Ok(()),
    }
}

pub async fn scheduled_event_handler(bot: AutoSend<Bot>, db: DatabaseRef) -> anyhow::Result<()> {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);
    const OUTDATED_SUBSCRIPTIONS_THRESHOLD_MINUTES: i64 = 60;

    log::info!("Task scheduler started.");

    loop {
        let now = chrono::Local::now();
        let subscriptions = db.get_pending_subscriptions(now).await?;

        for subscription in subscriptions {
            // If the scheduled time was under 15 minutes ago, handle it.
            if (now.time() - subscription.time)
                < chrono::Duration::minutes(OUTDATED_SUBSCRIPTIONS_THRESHOLD_MINUTES)
            {
                handle_scheduled_task(&bot, subscription.clone()).await?;
                log::info!(
                    "Handled scheduled task {} for chat {:?}",
                    subscription.kind.as_str(),
                    subscription.chat_id
                );
            } else {
                log::info!(
                    "Skipping scheduled task {} for chat {:?} (scheduled time was {})",
                    subscription.kind.as_str(),
                    subscription.chat_id,
                    subscription.time.format(TIME_FORMAT)
                );
            }

            db.mark_subscription_updated(&subscription).await?;
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
