use std::sync::Arc;

use anyhow::Context;
use teloxide::{prelude::*, utils::command::BotCommands, RequestError};
use thiserror::Error;

use crate::{
    autoreplies::AutoreplySetMap, chat_config::ChatConfigModel, db::DatabaseRef,
    google::GoogleCalendarClientFactory, handlers, Command,
};

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Handled error with chat message")]
    ErrorReply(String),
    #[error(transparent)]
    ActualError(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum HandlerSuccess {
    Finished,
    Message(String),
}

impl From<RequestError> for HandlerError {
    fn from(e: RequestError) -> Self {
        HandlerError::ActualError(e.into())
    }
}

pub type HandlerResult<T = HandlerSuccess> = Result<T, HandlerError>;

trait ResultExt<T> {
    fn handler_context(self, message: &'static str) -> HandlerResult<T>;
}

impl<T> ResultExt<T> for HandlerResult<T> {
    fn handler_context(self, message: &'static str) -> HandlerResult<T> {
        self.map_err(|e| match e {
            HandlerError::ErrorReply(_) => e,
            HandlerError::ActualError(e) => HandlerError::ActualError(e.context(message)),
        })
    }
}

pub fn fail<T>(message: impl Into<String>) -> HandlerResult<T> {
    Err(HandlerError::ErrorReply(message.into()))
}

pub fn succeed() -> HandlerResult {
    Ok(HandlerSuccess::Finished)
}

pub fn succeed_with_message(message: impl Into<String>) -> HandlerResult {
    Ok(HandlerSuccess::Message(message.into()))
}

pub async fn handle_command(
    bot: AutoSend<Bot>,
    message: Message,
    command: Command,
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
    chat_config_map: Arc<ChatConfigModel>,
    google_calendar_client_factory: GoogleCalendarClientFactory,
) -> anyhow::Result<()> {
    let chat_id = message.chat.id;

    let result = match command {
        Command::GetExcuse => handlers::handle_get_excuse()
            .await
            .handler_context("handle_get_excuse"),
        Command::Help => send_help(&bot, &message).await.handler_context("send_help"),
        Command::Fingerpori => handlers::handle_fingerpori(&bot, chat_id)
            .await
            .handler_context("handle_fingerpori"),
        Command::Randompori => handlers::handle_randompori(&bot, chat_id)
            .await
            .handler_context("handle_randompori"),
        Command::Fokit => handlers::handle_fokit(&bot, chat_id)
            .await
            .handler_context("handle_fokit"),
        Command::RandomFokit => handlers::handle_random_fokit(&bot, chat_id)
            .await
            .handler_context("handle_random_fokit"),
        Command::Lasaga => handlers::handle_lasaga(&bot, chat_id)
            .await
            .handler_context("handle_lasaga"),
        Command::RandomLasaga => handlers::handle_random_lasaga(&bot, chat_id)
            .await
            .handler_context("handle_random_lasaga"),
        Command::Subscribe { kind, time } => handlers::handle_subscribe(chat_id, db, &kind, &time)
            .await
            .handler_context("handle_subscribe"),
        Command::AddMessage(args) => {
            handlers::handle_add_message(chat_id, db, autoreply_set_map, &args)
                .await
                .handler_context("handle_add_message")
        }
        Command::SetAutoreplyChance(value) => {
            handlers::handle_set_autoreply_chance(chat_id, chat_config_map, value)
                .await
                .handler_context("handle_set_autoreply_chance")
        }
        Command::StartGoogleAuth => {
            handlers::handle_start_google_auth(message, google_calendar_client_factory.clone())
                .await
                .handler_context("handle_start_google_auth")
        }
        Command::FinishGoogleAuth { code, state } => handlers::handle_finish_google_auth(
            message,
            google_calendar_client_factory.clone(),
            code,
            state,
            db,
        )
        .await
        .handler_context("handle_finish_google_auth"),
        Command::ConnectGoogleCalendar(calendar_id) => handlers::connect_google_calendar(
            message,
            google_calendar_client_factory.clone(),
            db,
            calendar_id,
        )
        .await
        .handler_context("connect_google_calendar"),
        Command::DisconnectGoogleCalendar => handlers::disconnect_google_calendar(message, db)
            .await
            .handler_context("disconnect_google_calendar"),
        Command::Events => handlers::print_calendar_events(
            &bot,
            message,
            db,
            google_calendar_client_factory.clone(),
        )
        .await
        .handler_context("print_calendar_events"),
    };

    match result {
        Ok(HandlerSuccess::Finished) => {}
        Ok(HandlerSuccess::Message(message)) => {
            bot.send_message(chat_id, message).await?;
        }
        Err(HandlerError::ErrorReply(reply)) => {
            bot.send_message(chat_id, reply).await?;
        }
        Err(HandlerError::ActualError(err)) => {
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
        }
    }

    Ok(())
}

async fn send_help(bot: &AutoSend<Bot>, message: &Message) -> HandlerResult {
    bot.send_message(message.chat.id, Command::descriptions().to_string())
        .await
        .context("Failed to send message")?;

    succeed()
}
