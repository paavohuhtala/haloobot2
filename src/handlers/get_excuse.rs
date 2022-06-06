use anyhow::Context;
use serde::Deserialize;
use teloxide::prelude::*;

#[derive(Deserialize, Debug)]
struct ExcuseResponse {
    excuse: String,
}

pub async fn handle_get_excuse(bot: &AutoSend<Bot>, message: &Message) -> anyhow::Result<()> {
    let response = reqwest::get("http://ohjelmointitekosyyt.fi/.netlify/functions/excuse")
        .await
        .context("Failed to fetch")?;

    let ExcuseResponse { excuse } = response
        .json::<ExcuseResponse>()
        .await
        .context("Failed to parse JSON")?;

    bot.send_message(message.chat.id, excuse).await?;

    Ok(())
}
