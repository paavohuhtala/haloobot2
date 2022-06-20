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

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

pub async fn scheduled_event_handler(bot: AutoSend<Bot>, db: DatabaseRef) -> anyhow::Result<()> {
    let ctrl_c_signal = tokio::signal::ctrl_c();
    // This is technically a oneshot channel, but actual tokio oneshot channel cannot be be listened to in a loop.
    let (send_shutdown, mut receive_shutdown) = tokio::sync::mpsc::unbounded_channel();

    let handler_task = tokio::spawn(async move {
        let db = db.clone();

        loop {
            match handle_subscriptions(&db, &bot).await {
                Ok(()) => {}
                Err(err) => {
                    log::error!("Error while handling scheduled event{:#}", err);
                }
            }

            // Wait for the next poll interval
            // or
            // wait for the shutdown signal
            tokio::select! {
                _ = receive_shutdown.recv() => {
                    break;
                }
                _ = tokio::time::sleep(POLL_INTERVAL) => { }
            }
        }

        // Allow this unreachable code because rustc can't infer the types correctly without it.
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });

    log::info!("Task scheduler started.");

    ctrl_c_signal.await.unwrap();

    log::info!("Task scheduler received SIGINT signal.");

    send_shutdown.send(()).unwrap();
    handler_task.await??;

    log::info!("Task scheduler stopped.");

    Ok(())
}

const OUTDATED_SUBSCRIPTIONS_THRESHOLD_MINUTES: i64 = 60;

async fn handle_subscriptions(db: &DatabaseRef, bot: &AutoSend<Bot>) -> Result<(), anyhow::Error> {
    let now = chrono::Local::now();
    let subscriptions = db
        .get_pending_subscriptions(now)
        .await
        .context("Failed to read pending subscriptions")?;
    Ok(for subscription in subscriptions {
        // If the scheduled time was under 15 minutes ago, handle it.
        if (now.time() - subscription.time)
            < chrono::Duration::minutes(OUTDATED_SUBSCRIPTIONS_THRESHOLD_MINUTES)
        {
            handle_scheduled_task(bot, subscription.clone())
                .await
                .context("Failed to handle scheduled task")?;
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

        db.mark_subscription_updated(&subscription)
            .await
            .context("Failed to mark subscription as updated")?;
    })
}
