use serde::{Deserialize, Serialize};

/// Alpaca account snapshot returned by GET /v2/account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlpacaAccount {
    pub id: String,
    pub account_number: String,
    pub status: String,
    pub currency: String,
    pub cash: String,
    pub equity: String,
    pub buying_power: String,
    pub portfolio_value: String,
    pub pattern_day_trader: bool,
    pub trading_blocked: bool,
    pub account_blocked: bool,
}

/// Lightweight view returned by our GET /account endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AccountSummary {
    pub equity: f64,
    pub buying_power: f64,
    pub cash: f64,
    pub currency: String,
    pub status: String,
    pub mode: String,
    pub trading_blocked: bool,
}
