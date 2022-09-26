use std::sync::Arc;

use teloxide::types::ChatId;

use crate::{
    chat_config::ChatConfigModel,
    command_handler::{succeed_with_message, HandlerResult},
};

pub async fn handle_set_autoreply_chance(
    chat_id: ChatId,
    chat_config_map: Arc<ChatConfigModel>,
    value: f64,
) -> HandlerResult {
    chat_config_map.set_autoreply_chance(chat_id, value).await?;

    succeed_with_message(format!(
        "🎉 Automaattisen vastauksen todennäköisyys asetettu arvoon {}",
        value
    ))
}
