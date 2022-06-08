use std::{
    str::FromStr,
    sync::{mpsc::Sender, Arc, Mutex},
};

use anyhow::Context;
use chrono::{DateTime, NaiveTime, Utc};
use db::DatabaseRef;
use subscriptions::{Subscription, SubscriptionType};
use teloxide::{
    dispatching::{UpdateFilterExt, UpdateHandler},
    prelude::*,
    types::Update,
    utils::command::BotCommands,
};

use crate::{
    db::open_and_prepare_db, scheduler::scheduled_event_handler, subscriptions::TIME_FORMAT,
};

mod db;
mod handlers;
mod scheduler;
mod subscriptions;

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "Tuetut komennot:")]
enum Command {
    #[command(description = "Miksi härveli ei toimi?")]
    GetExcuse,

    #[command(description = "Moro äijät mitäs äijät :D")]
    DudeCarpet,

    #[command(description = "Apuva")]
    Help,

    #[command(description = "Hae uusin Fingerpori")]
    Fingerpori,

    #[command(description = "Hae satunnainen Fingerpori")]
    Randompori,

    #[command(description = "im sorry jon")]
    Lasaga,

    #[command(description = "im sorry jon xD")]
    RandomLasaga,

    #[command(description = "Tilaa ajoitettu tapahtuma", parse_with = "split")]
    Subscribe { kind: String, time: String },
}

async fn send_help(bot: &AutoSend<Bot>, message: &Message) -> anyhow::Result<()> {
    bot.send_message(message.chat.id, Command::descriptions().to_string())
        .await?;

    Ok(())
}

async fn handle_command(
    bot: AutoSend<Bot>,
    message: Message,
    command: Command,
    add_subscription: Arc<Mutex<Sender<Subscription>>>,
    db: DatabaseRef,
) -> anyhow::Result<()> {
    let result = match command {
        Command::GetExcuse => handlers::handle_get_excuse(&bot, &message)
            .await
            .context("handle_get_excuse"),
        Command::DudeCarpet => handlers::handle_dude_carpet(&bot, &message)
            .await
            .context("handle_dude_carpet"),
        Command::Help => send_help(&bot, &message).await.context("send_help"),
        Command::Fingerpori => handlers::handle_fingerpori(&bot, message.chat.id)
            .await
            .context("handle_fingerpori"),
        Command::Randompori => handlers::handle_randompori(&bot, message.chat.id)
            .await
            .context("handle_randompori"),
        Command::Lasaga => handlers::handle_lasaga(&bot, message.chat.id)
            .await
            .context("handle_lasaga"),
        Command::RandomLasaga => handlers::handle_random_lasaga(&bot, message.chat.id)
            .await
            .context("handle_random_lasaga"),

        Command::Subscribe { kind, time } => {
            let kind = SubscriptionType::from_str(&kind);

            let kind = match kind {
                Ok(kind) => kind,
                Err(_) => {
                    bot.send_message(
                        message.chat.id,
                        "Epäkelpo tilauksen tyyppi. Käytä jokin seuraavista: comics, events",
                    )
                    .await?;

                    return Ok(());
                }
            };

            let time = NaiveTime::parse_from_str(&time, TIME_FORMAT);

            let time = match time {
                Ok(time) => time,
                Err(_) => {
                    bot.send_message(message.chat.id, "Epäkelpo ajankohta. Käytä muotoa HH:MM")
                        .await?;

                    return Ok(());
                }
            };

            let subscription = Subscription {
                chat_id: message.chat.id,
                kind,
                time,
            };

            db.add_subscription(subscription.clone()).await?;

            {
                let add_subscription = add_subscription.lock().unwrap();
                add_subscription.send(subscription).unwrap();
            }

            bot.send_message(
                message.chat.id,
                format!(
                    "🎉 Lisätty tilaus {}, päivittäin kello {}",
                    kind.as_str(),
                    time.format(TIME_FORMAT)
                ),
            )
            .await?;

            Ok(())
        }
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            log::error!("{:#}", err);
            bot.parse_mode(teloxide::types::ParseMode::Html)
                .send_message(
                    message.chat.id,
                    format!(
                        "Jotain meni pieleen :(\nEhkä tästä on apua:\n<pre>{:#}</pre>",
                        err
                    ),
                )
                .await?;
            Ok(())
        }
    }
}

async fn handle_message(bot: AutoSend<Bot>, message: Message) -> anyhow::Result<()> {
    let text = message.text().unwrap_or_default();

    if text.contains("kalja") {
        bot.send_message(message.chat.id, "oispa kaljaa").await?;
    }

    Ok(())
}

fn handler(start_time: DateTime<Utc>) -> UpdateHandler<anyhow::Error> {
    Update::filter_message()
        .chain(dptree::filter(move |message: Message| {
            // Ignore messages older than start_time to prevent massive spam
            message.date > start_time
        }))
        .branch(teloxide::filter_command::<Command, _>().chain(dptree::endpoint(handle_command)))
        .branch(dptree::endpoint(handle_message))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    log::info!("Starting haloobot2...");

    let db = open_and_prepare_db()?;

    let start_time = Utc::now();

    let token =
        std::env::var("TELEGRAM_TOKEN").context("TELEGRAM_TOKEN not found in environment")?;

    let bot = Bot::new(token).auto_send();

    bot.set_my_commands(Command::bot_commands()).await?;

    log::info!("Commands registered & Telegram connection established.");

    // https://github.com/teloxide/teloxide/blob/86657f55ffa1f10baa18a6fdca2c72c30db33519/src/dispatching/repls/commands_repl.rs#L82
    let ignore_update = |_upd| Box::pin(async {});

    let (create_subscription, receive_subscription) = std::sync::mpsc::channel();

    let create_subscription_shared = Arc::new(Mutex::new(create_subscription.clone()));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler(start_time))
        .default_handler(ignore_update)
        .dependencies(dptree::deps![create_subscription_shared, db.clone()])
        .build();

    let subscriptions = db
        .get_subscriptions()
        .await
        .context("Failed to get subscriptions")?;

    for subscription in subscriptions {
        create_subscription.send(subscription)?;
    }

    let (_, event_handler_result) = futures::join!(
        dispatcher.setup_ctrlc_handler().dispatch(),
        scheduled_event_handler(bot, receive_subscription)
    );

    event_handler_result?;

    Ok(())
}
