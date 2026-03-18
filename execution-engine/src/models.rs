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

/// Signal received from the strategy engine via POST /signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub symbol: String,
    pub direction: Direction,
    pub confidence: f64,
    pub reason: String,
    pub strategy_name: String,
    pub timestamp: String,
    #[serde(default)]
    pub trade_type: TradeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Direction {
    Buy,
    Sell,
    Hold,
}

/// Whether a signal/position is day trading or swing trading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TradeType {
    Day,
    Swing,
}

impl Default for TradeType {
    fn default() -> Self {
        TradeType::Day
    }
}

/// OHLCV bar from Alpaca WebSocket or strategy engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub symbol: String,
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

/// Position tracked in memory and DuckDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub qty: f64,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    #[serde(default)]
    pub trade_type: TradeType,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
}

/// Order record for DuckDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub alpaca_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub qty: f64,
    pub filled_price: Option<f64>,
    pub status: String,
    pub strategy_name: String,
    pub created_at: String,
    pub filled_at: Option<String>,
    #[serde(default)]
    pub trade_type: TradeType,
}

/// Alpaca order response from POST /v2/orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlpacaOrder {
    pub id: String,
    pub status: String,
    pub filled_avg_price: Option<String>,
    pub filled_at: Option<String>,
    pub symbol: String,
    pub side: String,
    pub qty: String,
}

/// SSE event types broadcast to the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SseEventType {
    PositionUpdate,
    OrderFill,
    TradingHalted,
    TradingResumed,
    DailyPnl,
    RiskBreach,
    RiskConfigUpdated,
}

/// Partial update request for PATCH /risk/config.
/// All fields are optional — only provided fields are updated.
#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfigUpdate {
    pub max_daily_loss_pct: Option<f64>,
    pub max_position_size_pct: Option<f64>,
    pub max_open_positions: Option<usize>,
    pub min_signal_confidence: Option<f64>,
    pub order_throttle_secs: Option<u64>,
    pub eod_flatten_time_et: Option<String>,
}

/// Response for GET /risk/config and PATCH /risk/config.
#[derive(Debug, Clone, Serialize)]
pub struct RiskConfigResponse {
    pub max_daily_loss_pct: f64,
    pub max_position_size_pct: f64,
    pub max_open_positions: usize,
    pub min_signal_confidence: f64,
    pub order_throttle_secs: u64,
    pub eod_flatten_time_et: String,
}

/// SSE event envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub event_type: SseEventType,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

/// Response from strategy engine POST /signal.
#[derive(Debug, Clone, Deserialize)]
pub struct SignalResponse {
    pub signals: Vec<Signal>,
}

/// Request body sent to strategy engine POST /signal.
#[derive(Debug, Clone, Serialize)]
pub struct SignalRequest {
    pub symbol: String,
    pub bars: Vec<Bar>,
}

/// Request body sent to strategy engine POST /signal/swing.
#[derive(Debug, Clone, Serialize)]
pub struct SwingSignalRequest {
    pub symbol: String,
    pub bars_daily: Vec<Bar>,
}

/// Response from strategy engine POST /signal/swing.
#[derive(Debug, Clone, Deserialize)]
pub struct SwingSignalResponse {
    pub composite: Signal,
    pub individual: std::collections::HashMap<String, Signal>,
}

/// Partial update for PATCH /risk/swing-config.
#[derive(Debug, Clone, Deserialize)]
pub struct SwingRiskConfigUpdate {
    pub max_swing_positions: Option<usize>,
    pub max_portfolio_heat_pct: Option<f64>,
    pub per_position_stop_loss_pct: Option<f64>,
    pub per_position_take_profit_pct: Option<f64>,
    pub min_composite_confidence: Option<f64>,
}

/// Response for GET/PATCH /risk/swing-config.
#[derive(Debug, Clone, Serialize)]
pub struct SwingRiskConfigResponse {
    pub max_swing_positions: usize,
    pub max_portfolio_heat_pct: f64,
    pub per_position_stop_loss_pct: f64,
    pub per_position_take_profit_pct: f64,
    pub min_composite_confidence: f64,
}
