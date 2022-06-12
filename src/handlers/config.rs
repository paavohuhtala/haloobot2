use std::sync::Arc;

use teloxide::{adaptors::AutoSend, prelude::Requester, types::ChatId, Bot};

use crate::chat_config::ChatConfigModel;

pub async fn handle_set_autoreply_chance(
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
    chat_config_map: Arc<ChatConfigModel>,
    value: f64,
) -> anyhow::Result<()> {
    chat_config_map.set_autoreply_chance(chat_id, value).await?;

    bot.send_message(
        chat_id,
        format!(
            "🎉 Automaattisen vastauksen todennäköisyys asetettu arvoon {}",
            value
        ),
    )
    .await?;

    Ok(())
}
