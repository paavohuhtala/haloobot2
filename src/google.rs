use std::sync::Arc;

pub struct GoogleCalendarClientFactoryState {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

pub type GoogleCalendarClientFactory = Arc<Option<GoogleCalendarClientFactoryState>>;

impl GoogleCalendarClientFactoryState {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
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

    pub fn create_client_from_refresh_token(
        &self,
        refresh_token: String,
    ) -> google_calendar::Client {
        google_calendar::Client::new(
            &self.client_id,
            &self.client_secret,
            &self.redirect_uri,
            String::new(),
            refresh_token,
        )
    }
}
