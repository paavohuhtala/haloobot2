use std::str::FromStr;

use anyhow::Context;
use rand::Rng;
use reqwest::Url;
use scraper::{Html, Selector};
use teloxide::{prelude::*, types::InputFile};

fn extract_fingerpori_url(html: &str) -> anyhow::Result<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(".cartoon img").unwrap();
    let cartoon = document
        .select(&selector)
        .next()
        .context("failed to find cartoon element")?;

    let srcset = cartoon
        .value()
        .attr("data-srcset")
        .context("Failed to find data-srcset attribute")?;

    let src = srcset
        .split(' ')
        .next()
        .context("Failed to extract URL from data-srcset")?;
    let url = format!("https:{src}");

    Ok(url)
}

async fn fetch_and_extract_fingerpori_url(page_url: &str) -> anyhow::Result<String> {
    let html = reqwest::get(page_url)
        .await
        .context("Failed to fetch")?
        .text()
        .await
        .context("Failed to fetch (body)")?;

    let url = extract_fingerpori_url(&html).context("Failed to extract Fingerpori URL")?;

    Ok(url)
}

async fn get_random_fingerpori_page_url() -> anyhow::Result<String> {
    // The API only returns roughly this many old comics
    const PREVIOUS_FINGERPORIS: i32 = 480;

    let offset = rand::thread_rng().gen_range(0..=PREVIOUS_FINGERPORIS);
    let page_url = format!(
        "https://www.hs.fi/rest/laneitems/39221/moreItems?from={offset}&pageId=290&even=false"
    );

    Ok(page_url)
}

async fn handle_fingerpori_for_page_url(
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
    url: &str,
) -> anyhow::Result<()> {
    let url = fetch_and_extract_fingerpori_url(url).await?;
    let url = Url::from_str(&url).context("Failed to parse comic URL")?;

    bot.send_photo(chat_id, InputFile::url(url)).await?;

    Ok(())
}

pub async fn handle_fingerpori(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    handle_fingerpori_for_page_url(bot, chat_id, "https://www.hs.fi/fingerpori/").await
}

pub async fn handle_randompori(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    let page_url = get_random_fingerpori_page_url()
        .await
        .context("Failed to get random Fingerpori URL")?;

    handle_fingerpori_for_page_url(bot, chat_id, &page_url).await
}
