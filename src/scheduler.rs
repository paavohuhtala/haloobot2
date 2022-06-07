use std::{thread, time::Duration};

use anyhow::Context;
use chrono::NaiveTime;
use clokwerk::{Scheduler, TimeUnits};
use futures::{channel, StreamExt};
use teloxide::prelude::*;

use crate::handlers::handle_fingerpori;

#[derive(Clone, Debug)]
enum TaskType {
    SendComics,
}

#[derive(Clone, Debug)]
struct ScheduledTask(pub ChatId, pub TaskType);

async fn handle_scheduled_task(
    bot: &AutoSend<Bot>,
    ScheduledTask(chat_id, task): ScheduledTask,
) -> anyhow::Result<()> {
    log::info!("Handling scheduled task {:?} for chat {:?}", task, chat_id);

    match task {
        TaskType::SendComics => handle_fingerpori(bot, chat_id)
            .await
            .context("handle_fingerpori (scheduled)"),
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
    chat_ids: &[ChatId],
) -> anyhow::Result<()> {
    let mut scheduler = Scheduler::new();

    let (send_task, receive_task) = channel::mpsc::unbounded();

    for chat_id in chat_ids.iter().copied() {
        let send_task = send_task.clone();

        scheduler
            .every(1.day())
            .at_time(NaiveTime::from_hms(10, 5, 0))
            .run(move || {
                send_task
                    .unbounded_send(ScheduledTask(chat_id, TaskType::SendComics))
                    .expect("Expected pushing task to channel to never fail");
            });
    }

    let scheduler_thread = thread::spawn(move || loop {
        scheduler.run_pending();
        thread::sleep(Duration::from_secs(60));
    });

    log::info!("Task scheduler thread started.");

    receive_task
        .for_each(|task| async {
            // Explicitly move the task a local variable.
            let task = task;
            let result = handle_scheduled_task(&bot, task.clone()).await;

            if let Err(err) = result {
                log::error!("Scheduled task {:?} failed: {:#}", task.1, err);
            }
        })
        .await;

    scheduler_thread
        .join()
        .expect("Failed to join scheduler thread (this should never happen :D)");

    Ok(())
}
