mod alpaca;
mod db;
mod models;
mod orders;
mod positions;
mod risk;
mod scheduler;
mod sse;

use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

use alpaca::{AlpacaClient, AlpacaConfig};
use models::*;
use positions::PositionTracker;
use risk::{RiskConfig, RiskContext, RiskDecision, RiskEngine};
use sse::SseBroadcaster;

/// Shared application state available to all Axum handlers and background tasks.
pub struct AppState {
    pub alpaca: AlpacaClient,
    pub broadcaster: SseBroadcaster,
    pub positions: Mutex<PositionTracker>,
    pub risk_engine: Mutex<RiskEngine>,
    pub trading_halted: Mutex<bool>,
    pub daily_pnl: Mutex<f64>,
    pub account_equity: Mutex<f64>,
    pub strategy_engine_url: String,
}

#[tokio::main]
async fn main() {
    // 1. Load .env
    dotenvy::from_filename("../.env").or_else(|_| dotenvy::dotenv()).ok();

    // 2. Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "execution_engine=info".into()),
        )
        .init();

    let check_auth = std::env::args().any(|a| a == "--check-auth");

    // 3. Build Alpaca client from env vars
    let config = AlpacaConfig::from_env();
    let alpaca = AlpacaClient::new(config);

    // 4. Verify auth — fetch account. Exit on failure.
    let account = match alpaca.get_account().await {
        Ok(acct) => {
            info!(
                mode = %alpaca.config.mode,
                equity = %acct.equity,
                buying_power = %acct.buying_power,
                status = %acct.status,
                "Alpaca auth verified"
            );
            acct
        }
        Err(e) => {
            error!("Alpaca auth failed: {e}");
            std::process::exit(1);
        }
    };

    if account.trading_blocked || account.account_blocked {
        error!(
            trading_blocked = account.trading_blocked,
            account_blocked = account.account_blocked,
            "Alpaca account is blocked — exiting"
        );
        std::process::exit(1);
    }

    // --check-auth: print summary and exit
    if check_auth {
        let parse = |s: &str| s.parse::<f64>().unwrap_or(0.0);
        println!("=== Alpaca Paper Auth OK ===");
        println!("  Mode:          {}", alpaca.config.mode);
        println!("  Account:       {}", account.account_number);
        println!("  Status:        {}", account.status);
        println!("  Equity:        ${:.2}", parse(&account.equity));
        println!("  Buying Power:  ${:.2}", parse(&account.buying_power));
        println!("  Cash:          ${:.2}", parse(&account.cash));
        println!("  Currency:      {}", account.currency);
        return;
    }

    let equity: f64 = account.equity.parse().unwrap_or(0.0);

    // 5. Load positions from DuckDB
    let mut position_tracker = PositionTracker::new();
    if let Ok(con) = db::connect() {
        match db::load_positions(&con) {
            Ok(positions) => {
                info!(count = positions.len(), "Loaded positions from DuckDB");
                position_tracker.load(positions);
            }
            Err(e) => warn!("Failed to load positions: {e}"),
        }
    }

    let strategy_engine_url = std::env::var("STRATEGY_ENGINE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".into());

    // 6. Build shared state
    let state = Arc::new(AppState {
        alpaca: alpaca.clone(),
        broadcaster: SseBroadcaster::new(100),
        positions: Mutex::new(position_tracker),
        risk_engine: Mutex::new(RiskEngine::new(RiskConfig::default())),
        trading_halted: Mutex::new(false),
        daily_pnl: Mutex::new(0.0),
        account_equity: Mutex::new(equity),
        strategy_engine_url,
    });

    // 7. Spawn WebSocket bar ingestion task
    let ws_state = state.clone();
    tokio::spawn(async move {
        websocket_loop(ws_state).await;
    });

    // 8. Spawn EOD flatten scheduler
    let sched_state = state.clone();
    tokio::spawn(async move {
        scheduler::eod_flatten_loop(sched_state).await;
    });

    // 9. Build Axum router
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/account", get(get_account))
        .route("/positions", get(get_positions))
        .route("/orders", get(get_orders))
        .route("/trading/halt", post(halt_trading))
        .route("/trading/resume", post(resume_trading))
        .route("/stream/events", get(stream_events))
        .layer(cors)
        .with_state(state);

    // 10. Start server
    let addr = "0.0.0.0:8080";
    info!("execution-engine listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ---------------------------------------------------------------------------
// WebSocket bar ingestion
// ---------------------------------------------------------------------------

const SYMBOLS: &[&str] = &["SPY", "QQQ", "AAPL", "MSFT", "NVDA", "GOOGL"];
const DATA_WS_URL: &str = "wss://stream.data.alpaca.markets/v2/iex";

async fn websocket_loop(state: Arc<AppState>) {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    loop {
        info!("Connecting to Alpaca WebSocket...");
        let ws_result = tokio_tungstenite::connect_async(DATA_WS_URL).await;
        let (mut ws, _) = match ws_result {
            Ok(pair) => pair,
            Err(e) => {
                error!("WebSocket connect failed: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        // Authenticate
        let auth_msg = serde_json::json!({
            "action": "auth",
            "key": state.alpaca.config.api_key,
            "secret": state.alpaca.config.secret_key,
        });
        if ws.send(Message::Text(auth_msg.to_string())).await.is_err() {
            error!("Failed to send auth message");
            continue;
        }

        // Read auth response
        if let Some(Ok(msg)) = ws.next().await {
            info!("WS auth response: {}", msg);
        }
        // Read connected message if any
        if let Some(Ok(msg)) = ws.next().await {
            info!("WS message: {}", msg);
        }

        // Subscribe to bars
        let sub_msg = serde_json::json!({
            "action": "subscribe",
            "bars": SYMBOLS,
        });
        if ws.send(Message::Text(sub_msg.to_string())).await.is_err() {
            error!("Failed to send subscribe message");
            continue;
        }

        // Read subscription confirmation
        if let Some(Ok(msg)) = ws.next().await {
            info!("WS subscription response: {}", msg);
        }

        info!("WebSocket connected and subscribed to bars");

        // Process incoming messages
        while let Some(msg_result) = ws.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    error!("WebSocket error: {e}");
                    break;
                }
            };

            let text = match msg {
                Message::Text(t) => t,
                Message::Ping(_) => continue,
                Message::Close(_) => {
                    warn!("WebSocket closed by server");
                    break;
                }
                _ => continue,
            };

            // Parse Alpaca message — array of events
            let events: Vec<serde_json::Value> = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

            for event in events {
                let msg_type = event.get("T").and_then(|v| v.as_str()).unwrap_or("");
                if msg_type != "b" {
                    continue; // only process bar messages
                }

                let bar = Bar {
                    symbol: event.get("S").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    timestamp: event.get("t").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    open: event.get("o").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    high: event.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    low: event.get("l").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    close: event.get("c").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    volume: event.get("v").and_then(|v| v.as_i64()).unwrap_or(0),
                };

                if bar.symbol.is_empty() {
                    continue;
                }

                info!(symbol = %bar.symbol, close = bar.close, "Received bar");

                // Upsert to DuckDB
                if let Ok(con) = db::connect() {
                    if let Err(e) = db::upsert_bar(&con, &bar, "5min") {
                        error!("Failed to upsert bar: {e}");
                    }
                }

                // Send to strategy engine and process signals
                process_bar(&state, &bar).await;
            }
        }

        warn!("WebSocket disconnected — reconnecting in 5s");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// Send bar to strategy engine, evaluate signals through risk, submit orders.
async fn process_bar(state: &Arc<AppState>, bar: &Bar) {
    let http = reqwest::Client::new();

    // Fetch recent bars from DuckDB for this symbol to give strategy context
    let bars = match db::connect() {
        Ok(con) => db::get_recent_bars(&con, &bar.symbol, "5min", 50).unwrap_or_default(),
        Err(_) => vec![bar.clone()],
    };
    if bars.is_empty() {
        return;
    }

    let req = SignalRequest {
        symbol: bar.symbol.clone(),
        bars,
    };

    let url = format!("{}/signal", state.strategy_engine_url);
    let resp = match http.post(&url).json(&req).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to call strategy engine: {e}");
            return;
        }
    };

    let signal_resp: SignalResponse = match resp.json().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse strategy response: {e}");
            return;
        }
    };

    for signal in signal_resp.signals {
        if signal.direction == Direction::Hold {
            continue;
        }

        // Build risk context
        let positions = state.positions.lock().await;
        let ctx = RiskContext {
            trading_halted: *state.trading_halted.lock().await,
            account_equity: *state.account_equity.lock().await,
            daily_loss: *state.daily_pnl.lock().await,
            open_position_count: positions.count(),
            position_value_for_symbol: positions.position_value(&signal.symbol, bar.close),
        };
        drop(positions);

        let decision = {
            let engine = state.risk_engine.lock().await;
            engine.evaluate(&signal, &ctx)
        };

        match decision {
            RiskDecision::Approved => {
                let side = match signal.direction {
                    Direction::Buy => "buy",
                    Direction::Sell => "sell",
                    Direction::Hold => continue,
                };

                // Calculate qty: use max_position_size_pct of equity
                let equity = *state.account_equity.lock().await;
                let max_value = equity * 0.10; // 10% of equity
                let qty = (max_value / bar.close).floor().max(1.0);

                match state.alpaca.submit_market_order(&signal.symbol, qty, side).await {
                    Ok(alpaca_order) => {
                        let now = chrono::Utc::now().to_rfc3339();
                        let order = Order {
                            order_id: uuid::Uuid::new_v4().to_string(),
                            alpaca_id: Some(alpaca_order.id.clone()),
                            symbol: signal.symbol.clone(),
                            side: side.to_string(),
                            qty,
                            filled_price: alpaca_order
                                .filled_avg_price
                                .as_ref()
                                .and_then(|p| p.parse::<f64>().ok()),
                            status: alpaca_order.status.clone(),
                            strategy_name: signal.strategy_name.clone(),
                            created_at: now.clone(),
                            filled_at: alpaca_order.filled_at.clone(),
                        };

                        // Record in DuckDB
                        if let Ok(con) = db::connect() {
                            if let Err(e) = db::insert_order(&con, &order) {
                                error!("Failed to insert order: {e}");
                            }
                        }

                        // Record throttle
                        state.risk_engine.lock().await.record_order(&signal.symbol);

                        // Poll for fill (market orders usually fill quickly)
                        let fill_price = poll_for_fill(state, &alpaca_order.id, &order.order_id, side, qty, &signal.symbol).await;

                        // Update position tracker
                        if let Some(fp) = fill_price {
                            let mut positions = state.positions.lock().await;
                            let pos = positions.update_on_fill(&signal.symbol, side, qty, fp);
                            if let Ok(con) = db::connect() {
                                match pos {
                                    Some(ref p) => { let _ = db::upsert_position(&con, p); }
                                    None => { let _ = db::delete_position(&con, &signal.symbol); }
                                }
                            }
                        }

                        info!(
                            symbol = %signal.symbol,
                            side,
                            qty,
                            strategy = %signal.strategy_name,
                            "Order submitted and processed"
                        );
                    }
                    Err(e) => {
                        error!("Order submission failed: {e}");
                    }
                }
            }
            RiskDecision::Rejected(reason) => {
                warn!(
                    symbol = %signal.symbol,
                    reason,
                    strategy = %signal.strategy_name,
                    "Signal rejected by risk engine"
                );
                state.broadcaster.send(SseEvent {
                    event_type: SseEventType::RiskBreach,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    payload: serde_json::json!({
                        "symbol": signal.symbol,
                        "reason": reason,
                        "strategy": signal.strategy_name,
                    }),
                });
            }
            RiskDecision::HaltAll(reason) => {
                error!(reason, "DAILY LOSS LIMIT BREACHED — halting all trading");
                *state.trading_halted.lock().await = true;
                state.broadcaster.send(SseEvent {
                    event_type: SseEventType::TradingHalted,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    payload: serde_json::json!({"reason": reason}),
                });
            }
        }
    }
}

/// Poll Alpaca for order fill, update DuckDB, broadcast SSE event.
pub(crate) async fn poll_for_fill(
    state: &Arc<AppState>,
    alpaca_id: &str,
    order_id: &str,
    side: &str,
    qty: f64,
    symbol: &str,
) -> Option<f64> {
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match state.alpaca.get_order(alpaca_id).await {
            Ok(order) if order.status == "filled" => {
                let fill_price = order
                    .filled_avg_price
                    .as_ref()
                    .and_then(|p| p.parse::<f64>().ok());

                if let Ok(con) = db::connect() {
                    let _ = db::update_order_fill(
                        &con,
                        order_id,
                        "filled",
                        fill_price,
                        order.filled_at.as_deref(),
                    );
                }

                state.broadcaster.send(SseEvent {
                    event_type: SseEventType::OrderFill,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    payload: serde_json::json!({
                        "symbol": symbol,
                        "side": side,
                        "qty": qty,
                        "fill_price": fill_price,
                        "alpaca_id": alpaca_id,
                    }),
                });

                info!(symbol, side, fill_price, "Order filled");
                return fill_price;
            }
            Ok(order) if order.status == "canceled" || order.status == "expired" || order.status == "rejected" => {
                warn!(symbol, status = %order.status, "Order not filled");
                if let Ok(con) = db::connect() {
                    let _ = db::update_order_fill(&con, order_id, &order.status, None, None);
                }
                return None;
            }
            Ok(_) => continue, // still pending
            Err(e) => {
                error!("Failed to poll order: {e}");
                continue;
            }
        }
    }

    warn!(symbol, "Order fill poll timed out after 5s");
    None
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "mode": state.alpaca.config.mode.to_string()
    }))
}

async fn get_account(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AccountSummary>, (axum::http::StatusCode, String)> {
    let acct = state
        .alpaca
        .get_account()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    let parse = |s: &str| s.parse::<f64>().unwrap_or(0.0);

    Ok(Json(AccountSummary {
        equity: parse(&acct.equity),
        buying_power: parse(&acct.buying_power),
        cash: parse(&acct.cash),
        currency: acct.currency,
        status: acct.status,
        mode: state.alpaca.config.mode.to_string(),
        trading_blocked: acct.trading_blocked,
    }))
}

async fn get_positions(State(state): State<Arc<AppState>>) -> Json<Vec<Position>> {
    let positions = state.positions.lock().await;
    Json(positions.all())
}

async fn get_orders(
    State(_state): State<Arc<AppState>>,
) -> Json<Vec<Order>> {
    let orders = match db::connect_readonly() {
        Ok(con) => db::load_orders(&con, 100).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    Json(orders)
}

async fn halt_trading(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    *state.trading_halted.lock().await = true;
    state.broadcaster.send(SseEvent {
        event_type: SseEventType::TradingHalted,
        timestamp: chrono::Utc::now().to_rfc3339(),
        payload: serde_json::json!({"reason": "Manual halt via API"}),
    });
    info!("Trading halted manually");
    Json(serde_json::json!({"status": "halted"}))
}

async fn resume_trading(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    *state.trading_halted.lock().await = false;
    state.broadcaster.send(SseEvent {
        event_type: SseEventType::TradingResumed,
        timestamp: chrono::Utc::now().to_rfc3339(),
        payload: serde_json::json!({"reason": "Manual resume via API"}),
    });
    info!("Trading resumed manually");
    Json(serde_json::json!({"status": "active"}))
}

async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> axum::response::sse::Sse<impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>
{
    state.broadcaster.subscribe()
}
