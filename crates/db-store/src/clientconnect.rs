use crate::config::{
    CLIENT_CONNECTION_TIMEOUT, CLIENT_IDLE_CONNECTION_TIMEOUT, CLIENT_REQUEST_TIMEOUT,
};
use core_types::OutBoxEvent;
use reqwest::{Client, Response, Result};
use std::time::Duration;

#[derive(Clone)]
pub struct ClientConnect {
    pub client: Client,
}

impl Default for ClientConnect {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientConnect {
    fn new() -> Self {
        // Build with strict connection timeouts so unresponsive webhook endpoints
        // do not hold open sockets indefinitely.
        let client = Client::builder()
            .timeout(Duration::from_secs(CLIENT_REQUEST_TIMEOUT))
            .connect_timeout(Duration::from_secs(CLIENT_CONNECTION_TIMEOUT))
            .pool_idle_timeout(Duration::from_secs(CLIENT_IDLE_CONNECTION_TIMEOUT))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client }
    }

    pub async fn connect(&self, event: &OutBoxEvent, signature: &str) -> Result<Response> {
        let payload = event.payload.to_string();

        self.client
            .post(&event.destination_url)
            .header("Content-Type", "application/json")
            .header("X-Signature", signature)
            .header("X-Delivery-Attempt", (event.attempt_count + 1).to_string())
            .body(payload)
            .send()
            .await
    }
}
