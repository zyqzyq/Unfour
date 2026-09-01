use std::time::Duration;

use async_trait::async_trait;

use crate::AppActivePayload;

#[async_trait]
pub(crate) trait TelemetryTransport: Send + Sync {
    async fn send(&self, endpoint: &str, payload: &AppActivePayload) -> Result<bool, ()>;
}

pub(crate) struct HttpTelemetryTransport {
    client: Option<reqwest::Client>,
}

impl HttpTelemetryTransport {
    pub(crate) fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(5))
                .build()
                .ok(),
        }
    }
}

#[async_trait]
impl TelemetryTransport for HttpTelemetryTransport {
    async fn send(&self, endpoint: &str, payload: &AppActivePayload) -> Result<bool, ()> {
        let client = self.client.as_ref().ok_or(())?;
        let response = client
            .post(endpoint)
            .json(payload)
            .send()
            .await
            .map_err(|_| ())?;
        Ok(response.status().is_success())
    }
}
