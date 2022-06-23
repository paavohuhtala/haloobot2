use teloxide::prelude::*;

use super::hs::{get_latest_cartoon, get_random_cartoon, HsCartoonExtractor};

struct Fingerpori;

impl HsCartoonExtractor for Fingerpori {
    const NAME: &'static str = "Fingerpori";

    const PAGED_URL: &'static str =
        "https://www.hs.fi/rest/laneitems/39221/moreItems?pageId=290&even=false";

    const PAGES: u32 = 480;
}

pub async fn handle_fingerpori(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    let photo = get_latest_cartoon::<Fingerpori>().await?;
    bot.send_photo(chat_id, photo).await?;
    Ok(())
}

pub async fn handle_randompori(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    let photo = get_random_cartoon::<Fingerpori>().await?;
    bot.send_photo(chat_id, photo).await?;
    Ok(())
}
