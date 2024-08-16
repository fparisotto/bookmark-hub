use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadabilityResponse {
    pub title: String,
    pub content: String,
    #[serde(rename(deserialize = "textContent"))]
    pub text_content: String,
}

#[instrument(skip_all)]
pub async fn process(
    client: &Client,
    readability_endpoint: &str,
    raw_content: String,
) -> Result<ReadabilityResponse> {
    let response = client
        .post(readability_endpoint)
        .body(raw_content)
        .send()
        .await?;
    let payload = response.json::<ReadabilityResponse>().await?;
    Ok(payload)
}
