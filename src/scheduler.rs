use std::{sync::mpsc::Receiver, thread, time::Duration};

use anyhow::Context;
use clokwerk::{Scheduler, TimeUnits};
use futures::{
    channel::{self},
    StreamExt,
};
use teloxide::prelude::*;

use crate::{
    handlers::{handle_fingerpori, handle_lasaga},
    subscriptions::{Subscription, SubscriptionType},
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

// The scheduled task handler has two main components:
// - The scheduler itself, powered by the clokwerk crate.
//   Tasks are registered using clokwerk's DSL.
//   Pending tasks are evaluated periodically by a separate thread.
//   Tasks that should be executed are pushed to a channel.
// - The update handler listens to the channel and actually executes the tasks.
//   It uses the StreamExt implementation of the channel (UnboundedReceiver) to handle the tasks as they arrive.
//
// TODO: Add tasks from db
// TODO: Support adding more tasks at runtime
pub async fn scheduled_event_handler(
    bot: AutoSend<Bot>,
    receive_new_subscription: Receiver<Subscription>,
) -> anyhow::Result<()> {
    let mut scheduler = Scheduler::new();

    let (notify_subscription, receive_subscription) = channel::mpsc::unbounded();

    let scheduler_thread = thread::spawn(move || loop {
        let new_subscription = receive_new_subscription.try_recv();

        if let Ok(new_subscription) = new_subscription {
            let notify_subscription = notify_subscription.clone();

            scheduler
                .every(1.day())
                .at_time(new_subscription.time)
                .run(move || {
                    notify_subscription
                        .unbounded_send(new_subscription.clone())
                        .expect("Expected pushing task to channel to never fail");
                });
        }

        scheduler.run_pending();
        thread::sleep(Duration::from_secs(60));
    });

    log::info!("Task scheduler thread started.");

    receive_subscription
        .for_each(|subscription| async {
            // Explicitly move to a local variable.
            let subscription = subscription;
            let result = handle_scheduled_task(&bot, subscription.clone()).await;

            if let Err(err) = result {
                log::error!(
                    "Scheduled task {} failed: {:#}",
                    subscription.kind.as_str(),
                    err
                );
            }
        })
        .await;

    scheduler_thread
        .join()
        .expect("Failed to join scheduler thread (this should never happen :D)");

    Ok(())
}
