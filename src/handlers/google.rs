use teloxide::prelude::*;

use crate::google::GoogleCalendarClientFactory;

pub async fn handle_start_google_auth(
    bot: &AutoSend<Bot>,
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
) -> anyhow::Result<()> {
    let client = if google_calendar_client_factory.is_none() {
        bot.send_message(
            message.chat.id,
            "Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin.",
        )
        .await?;
        return Ok(());
    } else {
        google_calendar_client_factory
            .as_ref()
            .as_ref()
            .unwrap()
            .create_client()
    };

    if !message.chat.is_private() {
        bot.send_message(message.chat.id, "Hoidetaan tää privassa jookos 🥺👉👈")
            .await?;
        return Ok(());
    }

    let consent_url =
        client.user_consent_url(&[String::from("https://www.googleapis.com/auth/calendar")]);

    bot.send_message(message.chat.id, format!("Menes {consent_url} 👈 tonne"))
        .await?;

    Ok(())
}

pub async fn handle_finish_google_auth(
    bot: &AutoSend<Bot>,
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
    code: String,
    state: String,
) -> anyhow::Result<()> {
    let mut client = if google_calendar_client_factory.is_none() {
        bot.send_message(
            message.chat.id,
            "Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin.",
        )
        .await?;
        return Ok(());
    } else {
        google_calendar_client_factory
            .as_ref()
            .as_ref()
            .unwrap()
            .create_client()
    };

    if !message.chat.is_private() {
        bot.send_message(
            message.chat.id,
            "hupsista keikkaa :D ei kantsis postaa tota julkisesti :D",
        )
        .await?;
        return Ok(());
    }

    log::info!("Finishing user OAuth: {code} {state}");

    let access_token = client.get_access_token(&code, &state).await?;

    log::info!("Access token: {:?}", access_token);

    bot.send_message(message.chat.id, format!("Kohtalosi on sinetöity. 👌"))
        .await?;

    Ok(())
}
