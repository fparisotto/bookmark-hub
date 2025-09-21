use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use headless_chrome::Browser;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info};
use url::Url;

#[derive(Debug, Clone)]
pub enum ChromeConnection {
    Local,
    Remote { host: String, port: u16 },
}

pub struct ChromeClient {
    connection: ChromeConnection,
}

impl ChromeClient {
    pub fn new(connection: ChromeConnection) -> Self {
        Self { connection }
    }

    async fn connect_to_browser(&self) -> Result<Browser> {
        match &self.connection {
            ChromeConnection::Local => {
                info!("Starting local Chrome instance");
                Browser::default().context("Failed to start local Chrome instance")
            }
            ChromeConnection::Remote { host, port } => {
                info!(%host, %port, "Connecting to remote Chrome instance");
                let ws_url = self.discover_websocket_url(host, *port).await?;
                Browser::connect(ws_url)
                    .with_context(|| format!("Failed to connect to Chrome at {host}:{port}"))
            }
        }
    }

    async fn discover_websocket_url(&self, host: &str, port: u16) -> Result<String> {
        let http_url = format!("http://{}:{}/json/version", host, port);
        debug!(%http_url, "Discovering WebSocket URL");

        let client = Client::new();
        let response = client
            .get(&http_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch Chrome info from {http_url}"))?
            .error_for_status()
            .with_context(|| format!("Chrome returned error status for {http_url}"))?;

        let response_text = response
            .text()
            .await
            .context("Failed to read Chrome response as text")?;

        debug!(%response_text, "Chrome info response content");

        let json: Value = serde_json::from_str(&response_text)
            .inspect_err(|err| {
                debug!(%err, %response_text, "Failed to parse Chrome info JSON");
            })
            .context("Failed to parse Chrome info JSON")?;

        let ws_url = json["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| anyhow!("No webSocketDebuggerUrl found in Chrome info"))?;

        debug!(%ws_url, "Discovered WebSocket URL");
        Ok(ws_url.to_string())
    }

    pub async fn fetch_rendered_html(&self, url: &Url) -> Result<String> {
        debug!(%url, "Connecting to browser");
        let browser = self.connect_to_browser().await?;

        debug!(%url, "Creating new tab");
        let tab = browser
            .new_tab()
            .context("Failed to create new browser tab")?;

        debug!(%url, "Navigating to");
        tab.navigate_to(url.as_str())
            .with_context(|| format!("Failed to navigate to {url}"))?;

        debug!(%url, "Waiting for navigation to complete (network idle)");
        tab.wait_until_navigated()
            .context("Failed waiting for navigation to complete")?;

        debug!(%url, "Waiting for body element to ensure page is loaded");
        tab.wait_for_element("body")
            .context("Failed waiting for body element")?;

        // Try to wait for common content indicators, but don't fail if not found
        // This helps ensure dynamic content has loaded
        match tab.wait_for_element_with_custom_timeout(
            "article, main, [role='main'], #content, .content",
            Duration::from_secs(2),
        ) {
            Ok(_) => debug!(%url, "Found main content indicator"),
            Err(_) => debug!(%url, "No common content indicators found, proceeding anyway"),
        }

        debug!(%url, "Getting fully rendered page content");
        let html = tab.get_content().context("Failed to get page content")?;

        debug!(%url, size_bytes = %html.len(), "Successfully fetched HTML content");

        // Tab is automatically closed when dropped
        Ok(html)
    }
}
