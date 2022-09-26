use anyhow::Context;
use serde::Deserialize;

use crate::command_handler::{succeed_with_message, HandlerResult};

#[derive(Deserialize, Debug)]
struct ExcuseResponse {
    excuse: String,
}

pub async fn handle_get_excuse() -> HandlerResult {
    let response = reqwest::get("http://ohjelmointitekosyyt.fi/.netlify/functions/excuse")
        .await
        .context("Failed to fetch")?;

    let ExcuseResponse { excuse } = response
        .json::<ExcuseResponse>()
        .await
        .context("Failed to parse JSON")?;

    succeed_with_message(excuse)
}
