use anyhow::Context;
use google_calendar::types::MinAccessRole;
use teloxide::prelude::*;

use crate::{db::DatabaseRef, google::GoogleCalendarClientFactory};

pub async fn handle_start_google_auth(
    bot: &AutoSend<Bot>,
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
) -> anyhow::Result<()> {
    let chat_id = message.chat.id;

    let google_calendar_client_factory = match google_calendar_client_factory.as_ref() {
        None => {
            bot.send_message(
                chat_id,
                "Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin.",
            )
            .await?;
            return Ok(());
        }
        Some(client_factory) => client_factory,
    };

    if !message.chat.is_private() {
        bot.send_message(message.chat.id, "Hoidetaan tää privassa jookos 🥺👉👈")
            .await?;
        return Ok(());
    }

    let user_id = message
        .from()
        .context("Expected message to have a sender")?
        .id;

    match google_calendar_client_factory
        .create_client_for_user(user_id)
        .await?
    {
        Some(client) => match client.refresh_access_token().await {
            Ok(_) => {
                bot.send_message(
                    chat_id,
                    "Olet jo kirjautunut sisään Google-kalenteriin, veliseni.",
                )
                .await?;
                return Ok(());
            }
            Err(_) => {}
        },
        None => {}
    }

    let client = google_calendar_client_factory.create_client();

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
    db: DatabaseRef,
) -> anyhow::Result<()> {
    let google_calendar_client_factory = match google_calendar_client_factory.as_ref() {
        None => {
            bot.send_message(
                message.chat.id,
                "Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin.",
            )
            .await?;
            return Ok(());
        }
        Some(client_factory) => client_factory,
    };

    let mut client = google_calendar_client_factory.create_client();

    if !message.chat.is_private() {
        bot.send_message(
            message.chat.id,
            "hupsista keikkaa :D ei kantsis postaa tota julkisesti :D",
        )
        .await?;
        return Ok(());
    }

    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    log::info!(
        "Finishing OAuth for user {} ({})",
        sender.full_name(),
        sender.id.0
    );

    let access_token = client.get_access_token(&code, &state).await?;

    db.set_user_google_refresh_token(sender.id, &access_token.refresh_token)
        .await?;

    bot.send_message(message.chat.id, format!("Kohtalosi on sinetöity. 👌"))
        .await?;

    let calendars = client
        .calendar_list()
        .list_all(MinAccessRole::Reader, false, false)
        .await
        .context("Expected reading calendars to succeed.")?;

    println!("Calendars: {:#?}", calendars);

    Ok(())
}

pub async fn connect_google_calendar(
    bot: &AutoSend<Bot>,
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
    db: DatabaseRef,
    calendar_id: String,
) -> anyhow::Result<()> {
    let google_calendar_client_factory = match google_calendar_client_factory.as_ref() {
        None => {
            bot.send_message(
                message.chat.id,
                "Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin.",
            )
            .await?;
            return Ok(());
        }
        Some(client_factory) => client_factory,
    };

    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    let client = google_calendar_client_factory
        .create_client_for_user(sender.id)
        .await?;

    let client = match client {
        None => {
            bot.send_message(
                message.chat.id,
                "Et ole vielä Google-tunnistautunut. Käytä /startgoogleauth -komentoa.",
            )
            .await?;
            return Ok(());
        }
        Some(client) => client,
    };

    let calendar = match client.calendars().get(&calendar_id).await {
        Ok(calendar) => calendar,
        Err(err) => {
            bot.send_message(
                message.chat.id,
                format!("Kalenteria {} ei löytynyt. Virhe: {}", calendar_id, err),
            )
            .await?;
            log::error!("Error while getting calendar: {}", err);
            return Ok(());
        }
    };

    db.add_connected_calendar(message.chat.id, sender.id, &calendar.id)
        .await?;

    bot.send_message(
        message.chat.id,
        format!(
            "Jipii, jihuu! Kalenteri {} on kytketty kanavaan.",
            calendar.summary
        ),
    )
    .await?;

    Ok(())
}

pub async fn disconnect_google_calendar(
    bot: &AutoSend<Bot>,
    message: Message,
    db: DatabaseRef,
) -> anyhow::Result<()> {
    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    db.remove_connected_calendar(message.chat.id, sender.id)
        .await?;

    bot.send_message(
        message.chat.id,
        "Kytkemäsi kalenteri on irrotettu kanavalta.",
    )
    .await?;

    Ok(())
}
