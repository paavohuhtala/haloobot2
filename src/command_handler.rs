use std::sync::Arc;

use anyhow::Context;
use teloxide::{prelude::*, utils::command::BotCommands};

use crate::{
    autoreplies::AutoreplySetMap, chat_config::ChatConfigModel, db::DatabaseRef, handlers, Command,
};

pub async fn handle_command(
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
        Command::Fokit => handlers::handle_fokit(&bot, chat_id)
            .await
            .context("handle_fokit"),
        Command::RandomFokit => handlers::handle_random_fokit(&bot, chat_id)
            .await
            .context("handle_random_fokit"),
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

async fn send_help(bot: &AutoSend<Bot>, message: &Message) -> anyhow::Result<()> {
    bot.send_message(message.chat.id, Command::descriptions().to_string())
        .await?;

    Ok(())
}
