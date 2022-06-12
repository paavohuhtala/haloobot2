use std::sync::Arc;

use anyhow::Context;
use autoreplies::AutoreplySetMap;
use chrono::{DateTime, Utc};
use db::DatabaseRef;
use teloxide::{
    dispatching::{UpdateFilterExt, UpdateHandler},
    prelude::*,
    types::{InputFile, Update},
    utils::command::BotCommands,
};

use crate::{
    autoreplies::{create_autoreply_set_map, AutoreplyResponse},
    chat_config::ChatConfigModel,
    db::open_and_prepare_db,
    scheduler::scheduled_event_handler,
};

mod argument_parser;
mod autoreplies;
mod chat_config;
mod db;
mod handlers;
mod scheduler;
mod subscriptions;

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "Tuetut komennot:")]
pub enum Command {
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

    #[command(description = "Aseta automaattisen vastauksen todennäköisyys")]
    SetAutoreplyChance(f64),
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
    chat_config_map: Arc<ChatConfigModel>,
) -> anyhow::Result<()> {
    let chat_id = message.chat.id;

    let result = match command {
        Command::GetExcuse => handlers::handle_get_excuse(&bot, &message)
            .await
            .context("handle_get_excuse"),
        Command::DudeCarpet => handlers::handle_dude_carpet(&bot, &message)
            .await
            .context("handle_dude_carpet"),
        Command::Help => send_help(&bot, &message).await.context("send_help"),
        Command::Fingerpori => handlers::handle_fingerpori(&bot, chat_id)
            .await
            .context("handle_fingerpori"),
        Command::Randompori => handlers::handle_randompori(&bot, chat_id)
            .await
            .context("handle_randompori"),
        Command::Lasaga => handlers::handle_lasaga(&bot, chat_id)
            .await
            .context("handle_lasaga"),
        Command::RandomLasaga => handlers::handle_random_lasaga(&bot, chat_id)
            .await
            .context("handle_random_lasaga"),
        Command::Subscribe { kind, time } => {
            handlers::handle_subscribe(&bot, chat_id, db, &kind, &time)
                .await
                .context("handle_subscribe")
        }
        Command::AddMessage(args) => {
            handlers::handle_add_message(&bot, chat_id, db, autoreply_set_map, &args)
                .await
                .context("handle_add_message")
        }
        Command::SetAutoreplyChance(value) => {
            handlers::handle_set_autoreply_chance(&bot, chat_id, chat_config_map, value)
                .await
                .context("handle_set_autoreply_chance")
        }
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            log::error!("{:#}", err);
            bot.parse_mode(teloxide::types::ParseMode::Html)
                .send_message(
                    chat_id,
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
    db: DatabaseRef,
    message: Message,
    autoreply_set_map: AutoreplySetMap,
    chat_config_map: Arc<ChatConfigModel>,
) -> anyhow::Result<()> {
    let mut is_reply_to_me = false;

    if let Some(original_message) = message.reply_to_message() {
        let me = bot
            .get_me()
            .await
            .context("Expected get_me to never fail ")?;

        match original_message.from() {
            Some(from) => {
                if from.id == me.id {
                    is_reply_to_me = true;
                }
            }
            _ => {}
        }

        let username = me.username.as_deref().unwrap_or_default();
        let original_as_cmd = Command::parse(original_message.text().unwrap_or_default(), username);

        match original_as_cmd {
            Ok(_) if message.from() != original_message.from() => {
                bot.send_message(message.chat.id, "😳").await?;
                return Ok(());
            }
            Ok(Command::AddMessage(args)) => {
                handlers::handle_add_message_reply(&bot, db, autoreply_set_map, &message, &args)
                    .await?;

                return Ok(());
            }
            _ => {}
        }
    }

    let chat_id = message.chat.id;
    let text = message.text().unwrap_or_default();

    let chat_config = chat_config_map.get(chat_id).await?;

    let autoreply_set_map = autoreply_set_map.read().await;
    let autoreply_set = autoreply_set_map.get(&chat_id);

    let autoreply_set = match autoreply_set {
        None => {
            return Ok(());
        }
        Some(autoreply_set) => autoreply_set,
    };

    let mut reply_message = String::new();
    let autoreply_chance = if is_reply_to_me {
        1.0
    } else {
        chat_config.autoreply_chance
    };

    for reply in autoreply_set.get_matches(text) {
        let p: f64 = rand::random();

        if p > autoreply_chance {
            continue;
        }

        match &reply.response {
            AutoreplyResponse::Literal(text) => {
                if !reply_message.is_empty() {
                    reply_message.push(' ');
                }
                reply_message.push_str(text);
            }
            AutoreplyResponse::Sticker(sticker_id) => {
                bot.send_sticker(chat_id, InputFile::file_id(sticker_id))
                    .await?;
                return Ok(());
            }
        }
    }

    if !reply_message.is_empty() {
        bot.send_message(message.chat.id, reply_message).await?;
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

    let chat_config_map = Arc::new(ChatConfigModel::new(db.clone()));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler(start_time))
        .default_handler(ignore_update)
        .dependencies(dptree::deps![
            db.clone(),
            autoreply_set_map,
            chat_config_map
        ])
        .build();

    let (_, event_handler_result) = futures::join!(
        dispatcher.setup_ctrlc_handler().dispatch(),
        scheduled_event_handler(bot, db.clone())
    );

    event_handler_result?;

    Ok(())
}
