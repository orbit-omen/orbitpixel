use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::redirect::Policy;
use reqwest::Client;
use url::Url;

const MAX_REDIRECTS: usize = 10;
const MAX_HTML_BYTES: usize = 5 * 1024 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone)]
pub struct FetchedPage {
    pub final_url: String,
    pub status_code: u16,
    pub html: String,
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .redirect(Policy::limited(MAX_REDIRECTS))
            .user_agent("OrbitPixel/0.1.0 (Terminal-Native Browser; +https://github.com/orbit-omen/orbitpixel)")
            .build()
            .context("Failed to initialize HTTP client")?;
        Ok(Self { client })
    }

    pub fn normalize_url(raw: &str) -> Result<Url> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            bail!("URL cannot be empty");
        }

        let candidate = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("https://{}", trimmed)
        };

        let parsed = Url::parse(&candidate)
            .with_context(|| format!("Invalid URL syntax: '{}'", trimmed))?;

        match parsed.scheme() {
            "http" | "https" => Ok(parsed),
            scheme => bail!("Blocked unsupported URL scheme: '{}'", scheme),
        }
    }

    pub async fn fetch_html(&self, raw_url: &str) -> Result<FetchedPage> {
        let url = Self::normalize_url(raw_url)?;
        let response = self
            .client
            .get(url.clone())
            .header("Accept", "text/html,application/xhtml+xml,text/plain;q=0.9,*/*;q=0.8")
            .send()
            .await
            .with_context(|| format!("Network request failed for {}", url))?;

        let status_code = response.status().as_u16();
        let final_url = response.url().to_string();

        if let Some(len) = response.content_length() {
            if len as usize > MAX_HTML_BYTES {
                return Err(anyhow!(
                    "Response size ({} bytes) exceeds {} byte safety limit",
                    len,
                    MAX_HTML_BYTES
                ));
            }
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;

        if bytes.len() > MAX_HTML_BYTES {
            return Err(anyhow!(
                "Response body ({} bytes) exceeds {} byte safety limit",
                bytes.len(),
                MAX_HTML_BYTES
            ));
        }

        let html = String::from_utf8_lossy(&bytes).into_owned();
        Ok(FetchedPage {
            final_url,
            status_code,
            html,
        })
    }
}
