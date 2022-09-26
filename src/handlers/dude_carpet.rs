use anyhow::Context;
use scraper::{Html, Selector};

use crate::command_handler::{succeed_with_message, HandlerResult};

fn parse_dude_carpet(body: &str) -> anyhow::Result<String> {
    let document = Html::parse_document(body);

    let selector = Selector::parse("div").expect("Parsing selector should never fail");

    let element = document
        .select(&selector)
        .next()
        .context("Failed to find selected element")?;

    let text = element.text().next().context("Failed to find text")?;

    Ok(text.to_string())
}

pub async fn handle_dude_carpet() -> HandlerResult {
    let response = reqwest::get("https://aijamatto.herokuapp.com/")
        .await
        .context("Failed to fetch")?;

    let body = response.text().await.context("Failed to fetch (body)")?;
    let text = parse_dude_carpet(&body).context("Failed to parse dude carpet")?;

    succeed_with_message(text)
}
