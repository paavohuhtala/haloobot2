use std::ops::Sub;

use anyhow::Context;
use chrono::{Duration, Utc};
use reqwest::Url;
use scraper::{Html, Selector};
use teloxide::{prelude::*, types::InputFile};

fn extract_garfield_url(html: &str) -> anyhow::Result<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(".item-comic-image > img").unwrap();

    let cartoon = document
        .select(&selector)
        .next()
        .context("Failed to find comic element")?;

    let srcset = cartoon
        .value()
        .attr("data-srcset")
        .context("Failed to find data-srcset attribute")?;

    let url = srcset
        .split(' ')
        .next()
        .context("Failed to extract URL from data-srcset")?;

    Ok(String::from(url))
}

async fn handle_lasaga_for_page_url(
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
    url: &str,
) -> anyhow::Result<()> {
    let html = reqwest::get(url)
        .await
        .context("Failed to fetch")?
        .text()
        .await
        .context("Failed to fetch (body)")?;

    let url = extract_garfield_url(&html).context("Failed to extract comic URL")?;
    let url = Url::parse(&url).context("Failed to parse comic URL")?;
    bot.send_photo(chat_id, InputFile::url(url))
        .await
        .context("Failed to send Garfield")?;

    Ok(())
}

pub async fn handle_lasaga(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    // Technically fetch yesterdays comic to be safe
    let formatted_date = Utc::now().sub(Duration::days(1)).date().format("%Y/%m/%d");
    let url = format!("https://www.gocomics.com/garfield/{formatted_date}");
    handle_lasaga_for_page_url(bot, chat_id, &url)
        .await
        .context("Failed to handle lasaga")?;
    Ok(())
}

pub async fn handle_random_lasaga(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    handle_lasaga_for_page_url(bot, chat_id, "https://www.gocomics.com/random/garfield")
        .await
        .context("Failed to handle random lasaga")?;
    Ok(())
}
