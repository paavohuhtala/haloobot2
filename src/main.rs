use std::str::FromStr;

use anyhow::Context;
use argument_parser::parse_arguments;
use autoreplies::{AddAutoreplyResult, Autoreply, AutoreplySet, AutoreplySetMap};
use chrono::{DateTime, NaiveTime, Utc};
use db::DatabaseRef;
use regex::Regex;
use subscriptions::{Subscription, SubscriptionType};
use teloxide::{
    dispatching::{UpdateFilterExt, UpdateHandler},
    prelude::*,
    types::Update,
    utils::command::BotCommands,
};

use crate::{
    autoreplies::{create_autoreply_set_map, AutoreplyResponse},
    db::open_and_prepare_db,
    scheduler::scheduled_event_handler,
    subscriptions::TIME_FORMAT,
};

mod argument_parser;
mod autoreplies;
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

    #[command(description = "Lisää automaattinen vastaus")]
    AddMessage(String),
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
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
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

            db.add_subscription(&subscription).await?;

            log::info!("Added subscription: {:?}", subscription);

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

        Command::AddMessage(args) => {
            let args = parse_arguments(&args);
            let chat_id = message.chat.id;

            match args {
                Err(err) => {
                    bot.send_message(
                        chat_id,
                        format!("Parametrien parsinta epäonnistui: {}", err),
                    )
                    .await?;

                    return Ok(());
                }
                Ok((_, args)) => {
                    if args.len() != 3 {
                        bot.send_message(
                            chat_id,
                            "Parametrien määrä väärin. Käytä muotoa: /addmessage <nimi> <regex> <viesti>",
                        )
                        .await?;

                        return Ok(());
                    }

                    let name = &args[0];
                    let pattern_regex = Regex::new(&args[1]);

                    let pattern_regex = match pattern_regex {
                        Ok(regex) => regex,
                        Err(err) => {
                            bot.send_message(
                                chat_id,
                                format!("Regex-lausekkeen parsinta epäonnistui: {}", err),
                            )
                            .await?;

                            return Ok(());
                        }
                    };

                    let response = &args[2];

                    let autoreply = Autoreply {
                        chat_id,
                        name: name.to_string(),
                        pattern_regex,
                        response: AutoreplyResponse::Literal(response.to_string()),
                    };

                    match db.add_autoreply(&autoreply).await? {
                        AddAutoreplyResult::Ok => {}
                        AddAutoreplyResult::AlreadyExists => {
                            bot.send_message(
                                chat_id,
                                format!("Automaattinen vastaus on jo olemassa samalle regex-lauseelle 😭"),
                            )
                            .await?;

                            return Ok(());
                        }
                    }

                    let mut autoreply_set_map = autoreply_set_map.write().await;
                    autoreply_set_map
                        .entry(chat_id)
                        .or_insert_with(|| AutoreplySet::empty())
                        .add_autoreply(autoreply);

                    bot.send_message(
                        chat_id,
                        format!("🎉 Lisätty automaattinen vastaus {}", name),
                    )
                    .await?;
                }
            }

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

async fn handle_message(
    bot: AutoSend<Bot>,
    message: Message,
    autoreply_set_map: AutoreplySetMap,
) -> anyhow::Result<()> {
    let chat_id = message.chat.id;
    let text = message.text().unwrap_or_default();

    let autoreply_set_map = autoreply_set_map.read().await;

    let autoreply_set = autoreply_set_map.get(&chat_id);

    let autoreply_set = match autoreply_set {
        None => {
            return Ok(());
        }
        Some(autoreply_set) => autoreply_set,
    };

    for reply in autoreply_set.get_matches(text) {
        bot.send_message(
            message.chat.id,
            match &reply.response {
                AutoreplyResponse::Literal(text) => text,
                AutoreplyResponse::Sticker(text) => text,
            },
        )
        .await?;
        // TODO: handle multiple triggers
        break;
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

    let autoreplies = db
        .get_autoreplies()
        .await
        .context("Failed to read autoreplies from DB")?;

    log::info!("Loaded {} autoreplies", autoreplies.len());

    let autoreply_set_map = create_autoreply_set_map(autoreplies);

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler(start_time))
        .default_handler(ignore_update)
        .dependencies(dptree::deps![db.clone(), autoreply_set_map])
        .build();

    let (_, event_handler_result) = futures::join!(
        dispatcher.setup_ctrlc_handler().dispatch(),
        scheduled_event_handler(bot, db.clone())
    );

    event_handler_result?;

    Ok(())
}
