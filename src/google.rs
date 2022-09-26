use std::sync::Arc;

use chrono::{DateTime, Duration, NaiveDate};
use google_calendar::types::{Event, EventDateTime, OrderBy};
use itertools::{Either, Itertools};
use once_cell::sync::OnceCell;
use regex::Regex;
use teloxide::types::UserId;

use crate::db::DatabaseRef;

pub struct GoogleCalendarClientFactoryState {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    db: DatabaseRef,
}

pub type GoogleCalendarClientFactory = Arc<Option<GoogleCalendarClientFactoryState>>;

impl GoogleCalendarClientFactoryState {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        db: DatabaseRef,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            db,
        }
    }

    pub fn create_client(&self) -> google_calendar::Client {
        google_calendar::Client::new(
            &self.client_id,
            &self.client_secret,
            &self.redirect_uri,
            String::new(),
            String::new(),
        )
    }

    fn create_client_from_refresh_token(&self, refresh_token: String) -> google_calendar::Client {
        let mut client = google_calendar::Client::new(
            &self.client_id,
            &self.client_secret,
            &self.redirect_uri,
            String::new(),
            refresh_token,
        );

        client.set_auto_access_token_refresh(true);
        client
    }

    pub async fn create_client_for_user(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Option<google_calendar::Client>> {
        let refresh_token = self.db.get_user_google_refresh_token(user_id).await?;

        match refresh_token {
            Some(refresh_token) => {
                let client = self.create_client_from_refresh_token(refresh_token);
                client.refresh_access_token().await?;
                Ok(Some(client))
            }
            None => Ok(None),
        }
    }
}

#[derive(Debug, Default)]
pub struct EventConfig {
    countdown_days: Option<u32>,
}

#[derive(Debug)]
struct EventWithConfig(Event, EventConfig);

#[derive(Debug)]
enum SummaryEvent {
    Today(Event),
    Upcoming(UpcomingEvent),
}

impl EventWithConfig {
    pub fn as_summary_event(self, today: NaiveDate) -> Option<SummaryEvent> {
        let countdown_days = self.1.countdown_days.unwrap_or(0);

        let event_date = self.0.get_start_date()?;

        if event_date == today {
            return Some(SummaryEvent::Today(self.0));
        }

        let first_date_to_include = event_date - Duration::days(countdown_days as i64);
        if first_date_to_include <= today {
            let days = (event_date - today).num_days() as u32;
            Some(SummaryEvent::Upcoming(UpcomingEvent {
                event: self.0,
                days,
            }))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct UpcomingEvent {
    pub event: Event,
    pub days: u32,
}

static EVENT_CONFIG_COUNTDOWN_DAYS_REGEX: OnceCell<Regex> = OnceCell::new();

fn get_event_config(event: &Event) -> EventConfig {
    let countdown_days_regex = EVENT_CONFIG_COUNTDOWN_DAYS_REGEX.get_or_init(|| {
        Regex::new("countdown_days=(\\d+)").expect("Failed to compile countdown_days regex")
    });

    countdown_days_regex
        .captures(&event.description)
        .and_then(|captures| {
            let days = &captures[1];
            Some(EventConfig {
                countdown_days: days.parse::<u32>().ok(),
            })
        })
        .unwrap_or_default()
}

const FETCH_DAYS_IN_FUTURE: i64 = 100;

#[derive(Debug)]
pub struct EventsSummary {
    pub today: Vec<Event>,
    pub upcoming: Vec<UpcomingEvent>,
}

pub async fn get_events_to_announce(
    client: &google_calendar::Client,
    calendar_id: &str,
    now: DateTime<chrono::Local>,
) -> anyhow::Result<EventsSummary> {
    let start_time = now;
    let end_time = start_time + Duration::days(FETCH_DAYS_IN_FUTURE);

    let start_time = start_time.to_rfc3339();
    let end_time = end_time.to_rfc3339();

    let events = client
        .events()
        .list_all(
            &calendar_id,
            "",
            0,
            OrderBy::StartTime,
            &[],
            "",
            &[],
            false,
            false,
            true,
            &end_time,
            &start_time,
            "",
            "",
        )
        .await?;

    let today = now.date().naive_local();

    let events_with_config = events
        .into_iter()
        .map(|event| {
            let config = get_event_config(&event);
            EventWithConfig(event, config)
        })
        .filter_map(|event| event.as_summary_event(today));

    let (today, upcoming): (Vec<Event>, Vec<UpcomingEvent>) =
        events_with_config.partition_map(|event| match event {
            SummaryEvent::Today(event) => Either::Left(event),
            SummaryEvent::Upcoming(event) => Either::Right(event),
        });

    Ok(EventsSummary { today, upcoming })
}

pub trait EventExt {
    fn get_start_date(&self) -> Option<NaiveDate>;
}

impl EventExt for Event {
    fn get_start_date(&self) -> Option<NaiveDate> {
        match self.start {
            Some(EventDateTime {
                date: Some(date), ..
            }) => Some(date),
            Some(EventDateTime {
                date_time: Some(start_time),
                ..
            }) => Some(start_time.date_naive()),
            _ => None,
        }
    }
}
