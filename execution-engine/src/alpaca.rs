use reqwest::Client;
use tracing::{info, warn};

use crate::models::AlpacaAccount;

const PAPER_BASE_URL: &str = "https://paper-api.alpaca.markets";
const LIVE_BASE_URL: &str = "https://api.alpaca.markets";

/// Configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    pub api_key: String,
    pub secret_key: String,
    pub mode: AlpacaMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlpacaMode {
    Paper,
    Live,
}

impl std::fmt::Display for AlpacaMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlpacaMode::Paper => write!(f, "paper"),
            AlpacaMode::Live => write!(f, "live"),
        }
    }
}

impl AlpacaConfig {
    /// Load config from environment variables. Panics if keys are missing.
    pub fn from_env() -> Self {
        let api_key = std::env::var("ALPACA_API_KEY")
            .expect("ALPACA_API_KEY must be set");
        let secret_key = std::env::var("ALPACA_SECRET_KEY")
            .expect("ALPACA_SECRET_KEY must be set");

        if api_key.is_empty() || secret_key.is_empty() {
            panic!("ALPACA_API_KEY and ALPACA_SECRET_KEY must not be empty");
        }

        let mode = match std::env::var("ALPACA_MODE").as_deref() {
            Ok("live") => {
                warn!("ALPACA_MODE=live — using REAL money API");
                AlpacaMode::Live
            }
            _ => {
                info!("ALPACA_MODE=paper (default)");
                AlpacaMode::Paper
            }
        };

        Self {
            api_key,
            secret_key,
            mode,
        }
    }

    pub fn base_url(&self) -> &'static str {
        match self.mode {
            AlpacaMode::Paper => PAPER_BASE_URL,
            AlpacaMode::Live => LIVE_BASE_URL,
        }
    }
}

/// HTTP client for the Alpaca REST API.
#[derive(Debug, Clone)]
pub struct AlpacaClient {
    pub config: AlpacaConfig,
    pub(crate) http: Client,
}

impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Self {
        let http = Client::new();
        Self { config, http }
    }

    /// Fetch account details from Alpaca. Used at startup to verify auth
    /// and to read equity/buying power for risk calculations.
    pub async fn get_account(&self) -> Result<AlpacaAccount, AlpacaError> {
        let url = format!("{}/v2/account", self.config.base_url());

        let resp = self
            .http
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .map_err(AlpacaError::Network)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AlpacaError::Api {
                status: status.as_u16(),
                body,
            });
        }

        resp.json::<AlpacaAccount>()
            .await
            .map_err(AlpacaError::Deserialize)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AlpacaError {
    #[error("network error: {0}")]
    Network(reqwest::Error),

    #[error("Alpaca API error (HTTP {status}): {body}")]
    Api { status: u16, body: String },

    #[error("failed to deserialize Alpaca response: {0}")]
    Deserialize(reqwest::Error),
}
