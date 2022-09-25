use std::sync::Arc;

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
