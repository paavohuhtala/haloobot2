use teloxide::prelude::*;

use super::hs::{get_latest_cartoon, get_random_cartoon, HsCartoonExtractor};

struct Fokit;

impl HsCartoonExtractor for Fokit {
    const NAME: &'static str = "Fok_It";

    const PAGED_URL: &'static str =
        "https://www.hs.fi/rest/laneitems/39221/moreItems?pageId=295&even=false";

    const PAGES: u32 = 499;
}

pub async fn handle_fokit(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    let photo = get_latest_cartoon::<Fokit>().await?;
    bot.send_photo(chat_id, photo).await?;
    Ok(())
}

pub async fn handle_random_fokit(bot: &AutoSend<Bot>, chat_id: ChatId) -> anyhow::Result<()> {
    let photo = get_random_cartoon::<Fokit>().await?;
    bot.send_photo(chat_id, photo).await?;
    Ok(())
}
