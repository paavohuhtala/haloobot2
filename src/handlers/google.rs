use anyhow::Context;
use chrono::Local;
use teloxide::{prelude::*, types::ParseMode};

use crate::{
    command_handler::{fail, succeed, succeed_with_message, HandlerError, HandlerResult},
    db::DatabaseRef,
    google::{
        get_events_to_announce, EventExt, GoogleCalendarClientFactory,
        GoogleCalendarClientFactoryState, UpcomingEvent,
    },
    telegram_utils::telegram_escape,
};

fn get_google_calendar_client_factory<'a>(
    shared_factory: &'a GoogleCalendarClientFactory,
) -> Result<&'a GoogleCalendarClientFactoryState, HandlerError> {
    match shared_factory.as_ref() {
        None => fail("Tämä Haloobot-instanssi ei tue Google-integraatiota. Syylliset esiin."),
        Some(client_factory) => Ok(client_factory),
    }
}

async fn get_google_calendar_client_for_user(
    factory: &GoogleCalendarClientFactoryState,
    user_id: UserId,
) -> Result<google_calendar::Client, HandlerError> {
    let client = factory.create_client_for_user(user_id).await?;
    match client {
        None => fail("Et ole vielä Google-tunnistautunut. Käytä /startgoogleauth -komentoa."),
        Some(client) => Ok(client),
    }
}

pub async fn handle_start_google_auth(
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
) -> HandlerResult {
    let google_calendar_client_factory =
        get_google_calendar_client_factory(&google_calendar_client_factory)?;

    if !message.chat.is_private() {
        return fail("Hoidetaan tää privassa jookos 🥺👉👈");
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
                return fail("Olet jo kirjautunut sisään Google-kalenteriin, veliseni.");
            }
            Err(_) => {}
        },
        None => {}
    }

    let client = google_calendar_client_factory.create_client();

    let consent_url =
        client.user_consent_url(&[String::from("https://www.googleapis.com/auth/calendar")]);

    succeed_with_message(format!("Menes {consent_url} 👈 tonne"))
}

pub async fn handle_finish_google_auth(
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
    code: String,
    state: String,
    db: DatabaseRef,
) -> HandlerResult {
    let google_calendar_client_factory =
        get_google_calendar_client_factory(&google_calendar_client_factory)?;

    let mut client = google_calendar_client_factory.create_client();

    if !message.chat.is_private() {
        return fail("hupsista keikkaa :D ei kantsis postaa tota julkisesti :D");
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

    succeed_with_message("Kohtalosi on sinetöity. 👌")
}

pub async fn connect_google_calendar(
    message: Message,
    google_calendar_client_factory: GoogleCalendarClientFactory,
    db: DatabaseRef,
    calendar_id: String,
) -> HandlerResult {
    let google_calendar_client_factory =
        get_google_calendar_client_factory(&google_calendar_client_factory)?;

    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    let client =
        get_google_calendar_client_for_user(google_calendar_client_factory, sender.id).await?;

    let calendar = match client.calendars().get(&calendar_id).await {
        Ok(calendar) => calendar,
        Err(err) => {
            log::error!("Error while getting calendar: {}", err);

            return fail(format!(
                "Kalenteria {} ei löytynyt. Virhe: {}",
                calendar_id, err
            ));
        }
    };

    db.add_connected_calendar(message.chat.id, sender.id, &calendar.id)
        .await?;

    succeed_with_message(format!(
        "Jipii, jihuu! Kalenteri {} on kytketty kanavaan.",
        calendar.summary
    ))
}

pub async fn disconnect_google_calendar(message: Message, db: DatabaseRef) -> HandlerResult {
    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    db.remove_connected_calendar(message.chat.id, sender.id)
        .await?;

    succeed_with_message("Kytkemäsi kalenteri on irrotettu kanavalta.")
}

pub async fn print_calendar_events(
    bot: &AutoSend<Bot>,
    message: Message,
    db: DatabaseRef,
    google_calendar_client_factory: GoogleCalendarClientFactory,
) -> HandlerResult {
    let chat_id = message.chat.id;

    let google_calendar_client_factory =
        get_google_calendar_client_factory(&google_calendar_client_factory)?;

    let calendar_id = db.get_connected_calendar_id(chat_id).await?;

    let calendar_id = match calendar_id {
        None => {
            return fail("Tätä kanavaa ei ole kytketty mihinkään kalenteriin. Käytä /connectgooglecalendar -komentoa.");
        }
        Some(calendar_id) => calendar_id,
    };

    let sender = message
        .from()
        .context("Expected message to have a sender")?;

    let client =
        get_google_calendar_client_for_user(google_calendar_client_factory, sender.id).await?;

    let now = Local::now();
    let events_summary = get_events_to_announce(&client, &calendar_id, now).await?;

    if events_summary.today.is_empty() && events_summary.upcoming.is_empty() {
        return succeed_with_message("Ei tulevia tapahtumia kalenterissa. 😔");
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

    succeed()
}
