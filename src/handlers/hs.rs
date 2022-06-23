use std::str::FromStr;

use anyhow::Context;
use rand::Rng;
use reqwest::Url;
use scraper::{Html, Selector};
use teloxide::types::InputFile;

fn extract_cartoon_url(html: &str) -> anyhow::Result<Url> {
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

    let url = Url::from_str(&url).with_context(|| format!("Failed to parse URL '{}'", url))?;

    Ok(url)
}

async fn fetch_and_extract_url(page_url: &str) -> anyhow::Result<Url> {
    let html = reqwest::get(page_url)
        .await
        .context("Failed to fetch")?
        .text()
        .await
        .context("Failed to fetch (body)")?;

    extract_cartoon_url(&html).context("Failed to extract cartoon URL")
}

pub trait HsCartoonExtractor {
    const NAME: &'static str;

    const PAGED_URL: &'static str;

    const PAGES: u32;

    fn get_latest_page_url() -> String {
        format!("{}&from=0", Self::PAGED_URL)
    }

    fn get_random_page_url() -> String {
        let offset = rand::thread_rng().gen_range(0..=Self::PAGES);
        format!("{}&from={}", Self::PAGED_URL, offset)
    }
}

// These are free functions instead of members of the trait because of trait async limitations.

pub async fn get_latest_cartoon<E: HsCartoonExtractor>() -> anyhow::Result<InputFile> {
    let url = fetch_and_extract_url(&E::get_latest_page_url())
        .await
        .with_context(|| format!("Failed to fetch latest {}", E::NAME))?;
    Ok(InputFile::url(url))
}

pub async fn get_random_cartoon<E: HsCartoonExtractor>() -> anyhow::Result<InputFile> {
    let page_url = E::get_random_page_url();
    let url = fetch_and_extract_url(&page_url)
        .await
        .with_context(|| format!("Failed to fetch random {}", E::NAME))?;
    Ok(InputFile::url(url))
}
