use anyhow::Context;
use chrono::Local;
use teloxide::{prelude::*, types::ParseMode};

use crate::{
    db::DatabaseRef,
    google::{get_events_to_announce, EventExt, GoogleCalendarClientFactory, UpcomingEvent},
    telegram_utils::telegram_escape,
};

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

pub async fn print_calendar_events(
    bot: &AutoSend<Bot>,
    message: Message,
    db: DatabaseRef,
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

    let calendar_id = db.get_connected_calendar_id(chat_id).await?;

    let calendar_id = match calendar_id {
        None => {
            bot.send_message(
                chat_id,
                "Tätä kanavaa ei ole kytketty mihinkään kalenteriin. Käytä /connectgooglecalendar -komentoa.",
            )
            .await?;
            return Ok(());
        }
        Some(calendar_id) => calendar_id,
    };

    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    let client = google_calendar_client_factory
        .create_client_for_user(sender.id)
        .await?
        // TODO handle
        .expect("Expected user to have a Google client");

    let now = Local::now();
    let events_summary = get_events_to_announce(&client, &calendar_id, now).await?;

    if events_summary.today.is_empty() && events_summary.upcoming.is_empty() {
        bot.send_message(chat_id, "Ei tulevia tapahtumia kalenterissa. 😔")
            .await?;
        return Ok(());
    }

    let mut message = String::new();

    if !events_summary.today.is_empty() {
        message.push_str("*Tänään*:\n");
        for event in events_summary.today {
            let event_start = event.start.unwrap();
            let event_timestamp = if let Some(date_time) = event_start.date_time {
                date_time
                    .with_timezone(&Local)
                    .format(" (%H:%M)")
                    .to_string()
            } else {
                "".to_string()
            };
            message.push_str(&telegram_escape(&format!(
                "{}{}\n",
                event.summary, event_timestamp
            )));
        }
    }

    if !message.is_empty() {
        message.push_str("\n");
    }

    if !events_summary.upcoming.is_empty() {
        message.push_str("*Tulevat tapahtumat*:\n");
        for UpcomingEvent { event, days } in events_summary.upcoming {
            let event_local_time = event.get_start_date().unwrap();
            let event_date = event_local_time.format("%d.%m.%Y");
            let days_label = match days {
                1 => String::from("Huomenna"),
                days => format!("{} päivän päästä", days),
            };
            message.push_str(&telegram_escape(&format!(
                "{}: {} ({})\n",
                days_label, event.summary, event_date
            )));
        }
    }

    bot.send_message(chat_id, message)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}
