use reqwest::Client;
use serde_json::Value;
use tokio::time::{sleep, Duration};
use webbrowser;
use url::Url;
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use log::{info, error, debug, warn, trace};
use dotenv::dotenv;
use std::env;

#[derive(Debug, Deserialize)]
struct Config {
    firestore_url: String,
    check_interval_seconds: u64,
    collection_id: String,
    google_credentials_path: String,
}

impl Config {
    fn new() -> Result<Self, String> {
        let google_credentials_path = env::var("GOOGLE_APPLICATION_CREDENTIALS")
            .map_err(|_| "GOOGLE_APPLICATION_CREDENTIALS environment variable is not set".to_string())?;
        
        let firestore_url = env::var("FIRESTORE_URL")
            .map_err(|_| "FIRESTORE_URL environment variable is not set".to_string())?;

        Ok(Self {
            firestore_url,
            check_interval_seconds: 5,
            collection_id: "shared_urls".to_string(),
            google_credentials_path,
        })
    }
}

#[derive(Debug)]
enum UrlListenerError {
    FetchError(reqwest::Error),
    UrlParseError(url::ParseError),
    BrowserError(String),
}

impl std::error::Error for UrlListenerError {}
impl std::fmt::Display for UrlListenerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FetchError(e) => write!(f, "Failed to fetch URL: {}", e),
            Self::UrlParseError(e) => write!(f, "Failed to parse URL: {}", e),
            Self::BrowserError(e) => write!(f, "Failed to open browser: {}", e),
        }
    }
}

struct UrlListener {
    client: Client,
    last_seen_url: Option<String>,
    is_first_run: bool,
    check_interval: Duration,
    config: Config,
}

impl UrlListener {
    fn new(config: Config) -> Self {
        debug!("Creating new URL listener with config: {:?}", config);
        Self {
            client: Client::new(),
            last_seen_url: None,
            is_first_run: true,
            check_interval: Duration::from_secs(config.check_interval_seconds),
            config,
        }
    }

    async fn run(&mut self) {
        info!("Starting URL listener with check interval: {:?}", self.check_interval);
        loop {
            if let Err(e) = self.check_and_open_url().await {
                error!("Error occurred: {}", e);
            }
            trace!("Waiting for next check...");
            sleep(self.check_interval).await;
        }
    }

    async fn check_and_open_url(&mut self) -> Result<(), UrlListenerError> {
        match self.fetch_latest_url().await {
            Ok(Some(url)) => {
                debug!("Found new URL: {}", url);
                self.handle_new_url(url).await
            }
            Ok(None) => {
                trace!("No new URLs found in Firestore");
                Ok(())
            }
            Err(e) => {
                error!("Failed to fetch URL: {}", e);
                Err(e)
            }
        }
    }

    async fn handle_new_url(&mut self, url: String) -> Result<(), UrlListenerError> {
        if self.last_seen_url.as_ref() != Some(&url) {
            info!("Processing new URL: {}", url);
            if !self.is_first_run {
                self.open_url(&url).await?;
            } else {
                warn!("Skipping URL opening on first run");
            }
            self.last_seen_url = Some(url);
            self.is_first_run = false;
        } else {
            debug!("URL already processed: {}", url);
        }
        Ok(())
    }

    async fn open_url(&self, url: &str) -> Result<(), UrlListenerError> {
        debug!("Attempting to open URL: {}", url);
        let decoded = percent_decode_str(url)
            .decode_utf8()
            .map_err(|e| UrlListenerError::BrowserError(format!("Failed to decode URL: {}", e)))?;

        let parsed_url = Url::parse(&decoded.to_string())
            .map_err(UrlListenerError::UrlParseError)?;

        webbrowser::open(parsed_url.as_str())
            .map_err(|e| UrlListenerError::BrowserError(format!("Failed to open browser: {}", e)))?;

        info!("Successfully opened URL in browser: {}", url);
        Ok(())
    }

    async fn fetch_latest_url(&self) -> Result<Option<String>, UrlListenerError> {
        let query = serde_json::json!({
            "structuredQuery": {
                "from": [{
                    "collectionId": self.config.collection_id
                }],
                "orderBy": [{
                    "field": {
                        "fieldPath": "timestamp"
                    },
                    "direction": "DESCENDING"
                }],
                "limit": 1
            }
        });

        let url = format!("{}/documents:runQuery", self.config.firestore_url);
        debug!("Fetching latest URL from Firestore: {} using credentials from: {}", url, self.config.google_credentials_path);
        
        let response = self.client
            .post(&url)
            .json(&query)
            .send()
            .await
            .map_err(|e| UrlListenerError::FetchError(e))?;
        let body: Value = response.json().await.map_err(|e| UrlListenerError::FetchError(e))?;

        trace!("Received Firestore response: {}", serde_json::to_string_pretty(&body).unwrap());

        if let Some(documents) = body.as_array() {
            debug!("Found {} documents", documents.len());
            if let Some(last_doc) = documents.first() {
                if let Some(fields) = last_doc.get("document").and_then(|d| d.get("fields")) {
                    if let Some(url) = fields.get("url").and_then(|u| u.get("stringValue")).and_then(|v| v.as_str()) {
                        debug!("Extracted URL from document: {}", url);
                        return Ok(Some(url.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    
    let config = match Config::new() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to initialize config: {}", e);
            return;
        }
    };
    
    info!("Starting URL listener application");
    let mut listener = UrlListener::new(config);
    listener.run().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env::with_var;

    #[test]
    fn test_url_parsing() {
        let url = "https://example.com";
        let decoded = percent_decode_str(url).decode_utf8().unwrap();
        let parsed_url = Url::parse(&decoded.to_string()).unwrap();
        assert_eq!(parsed_url.as_str(), "https://example.com/");
    }

    #[test]
    fn test_config_creation() {
        with_var("GOOGLE_APPLICATION_CREDENTIALS", Some("test-credentials.json"), || {
            with_var("FIRESTORE_URL", Some("https://firestore.googleapis.com/v1/projects/test-project/databases/(default)"), || {
                let config = Config::new().unwrap();
                assert_eq!(config.check_interval_seconds, 5);
                assert_eq!(config.collection_id, "shared_urls");
                assert_eq!(config.google_credentials_path, "test-credentials.json");
                assert_eq!(config.firestore_url, "https://firestore.googleapis.com/v1/projects/test-project/databases/(default)");
            });
        });
    }

    #[test]
    fn test_config_creation_without_env() {
        with_var("GOOGLE_APPLICATION_CREDENTIALS", None::<String>, || {
            let result = Config::new();
            assert!(result.is_err());
        });
    }

    #[tokio::test]
    async fn test_url_listener_creation() {
        with_var("GOOGLE_APPLICATION_CREDENTIALS", Some("test-credentials.json"), || {
            with_var("FIRESTORE_URL", Some("https://firestore.googleapis.com/v1/projects/test-project/databases/(default)"), || {
                let config = Config::new().unwrap();
                let listener = UrlListener::new(config);
                assert!(listener.is_first_run);
                assert!(listener.last_seen_url.is_none());
            });
        });
    }
}
