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

async fn handle_fingerpori_for_url(
    bot: &AutoSend<Bot>,
    message: &Message,
    url: &str,
) -> anyhow::Result<()> {
    let html = reqwest::get(url)
        .await
        .context("Failed to fetch")?
        .text()
        .await
        .context("Failed to fetch (body)")?;

    let url = extract_fingerpori_url(&html).context("Failed to extract Fingerpori URL")?;
    let url = Url::from_str(&url).context("Failed to parse comic URL")?;

    bot.send_photo(message.chat.id, InputFile::url(url)).await?;

    Ok(())
}

pub async fn handle_fingerpori(bot: &AutoSend<Bot>, message: &Message) -> anyhow::Result<()> {
    handle_fingerpori_for_url(bot, message, "https://www.hs.fi/fingerpori/").await
}

pub async fn handle_randompori(bot: &AutoSend<Bot>, message: &Message) -> anyhow::Result<()> {
    // The API only returns roughly this many old comics
    const PREVIOUS_FINGERPORIS: i32 = 480;

    let offset = rand::thread_rng().gen_range(0..=PREVIOUS_FINGERPORIS);
    let url = format!(
        "https://www.hs.fi/rest/laneitems/39221/moreItems?from={offset}&pageId=290&even=false"
    );

    handle_fingerpori_for_url(bot, message, &url).await
}
