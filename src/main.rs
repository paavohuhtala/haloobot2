use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use autoreplies::AutoreplySet;
use chrono::{DateTime, Utc};
use command_handler::handle_command;
use message_handler::handle_message;
use teloxide::{
    dispatching::{UpdateFilterExt, UpdateHandler},
    prelude::*,
    types::Update,
    utils::command::BotCommands,
};
use tokio::sync::RwLock;

use crate::{
    autoreplies::{create_autoreply_set_map, StickerCache},
    chat_config::ChatConfigModel,
    db::open_and_prepare_db,
    scheduler::scheduled_event_handler,
};

mod argument_parser;
mod autoreplies;
mod chat_config;
mod command_handler;
mod db;
mod handlers;
mod message_handler;
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

    #[command(description = "Hae uusin Fok_It")]
    Fokit,

    #[command(description = "Hae satunnainen Fok_It")]
    RandomFokit,

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

    let autoreply_set_map = load_autoreplies(&db).await?;

    let chat_config_map = Arc::new(ChatConfigModel::new(db.clone()));

    let sticker_cache = Arc::new(StickerCache::new(db.clone(), chat_config_map.clone()));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler(start_time))
        .default_handler(ignore_update)
        .dependencies(dptree::deps![
            db.clone(),
            autoreply_set_map,
            chat_config_map,
            sticker_cache
        ])
        .build();

    let (_, event_handler_result) = futures::join!(
        dispatcher.setup_ctrlc_handler().dispatch(),
        scheduled_event_handler(bot, db.clone())
    );

    event_handler_result?;

    Ok(())
}

async fn load_autoreplies(
    db: &db::DatabaseRef,
) -> anyhow::Result<Arc<RwLock<HashMap<ChatId, AutoreplySet>>>> {
    let autoreplies = db
        .get_autoreplies()
        .await
        .context("Failed to read autoreplies from DB")?;

    log::info!("Loaded {} autoreplies", autoreplies.len());

    let autoreply_set_map = create_autoreply_set_map(autoreplies);
    Ok(autoreply_set_map)
}
