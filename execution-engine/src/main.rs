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
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use serde::Deserialize;
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
    pub symbols: Mutex<Vec<String>>,
    /// SPY's opening price for the current trading day (set once at market open).
    pub spy_day_open: Mutex<Option<f64>>,
    /// SPY's intraday percentage change from open (updated every 15s by quote_refresh_loop).
    pub spy_day_change_pct: Mutex<f64>,
    /// True when the daily profit target has been hit — blocks new entries for the rest of the day.
    pub profit_target_hit: Mutex<bool>,
}

async fn load_symbols(strategy_engine_url: &str) -> Vec<String> {
    let env_default = "SPY,QQQ,AAPL,MSFT,NVDA,GOOGL".to_string();

    // Try fetching symbols from the strategy engine first
    let url = format!("{}/symbols", strategy_engine_url);
    let client = reqwest::Client::new();
    if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        if let Ok(data) = resp.json::<serde_json::Value>().await {
            if let Some(symbols) = data.get("symbols").and_then(|v| v.as_array()) {
                let syms: Vec<String> = symbols
                    .iter()
                    .filter_map(|v| v.as_str().map(|s: &str| s.to_uppercase()))
                    .collect();
                if !syms.is_empty() {
                    return syms;
                }
            }
        }
    }

    // Fall back to SYMBOLS env var or default
    let raw = std::env::var("SYMBOLS").unwrap_or(env_default);
    raw.split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
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

    // 5. Ensure DuckDB schema exists (self-heal after corruption / fresh DB)
    match db::connect() {
        Ok(con) => {
            if let Err(e) = db::ensure_schema(&con) {
                error!("Failed to ensure DuckDB schema: {e}");
                std::process::exit(1);
            }
            info!("DuckDB schema verified");
        }
        Err(e) => {
            error!("Failed to connect to DuckDB: {e}");
            std::process::exit(1);
        }
    }

    // 6. Load positions from DuckDB, then sync with Alpaca
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

    // Sync with Alpaca's actual holdings to fix any qty/price drift
    match alpaca.get_positions().await {
        Ok(alpaca_positions) => {
            let changed = position_tracker.sync_with_alpaca(&alpaca_positions);
            if !changed.is_empty() {
                info!(changed = ?changed, "Synced positions with Alpaca");
                // Persist synced state to DuckDB
                if let Ok(con) = db::connect() {
                    for sym in &changed {
                        if let Some(pos) = position_tracker.get(sym) {
                            let _ = db::upsert_position(&con, pos);
                        } else {
                            let _ = db::delete_position(&con, sym);
                        }
                    }
                }
            }
            info!(
                local = position_tracker.count(),
                alpaca = alpaca_positions.len(),
                "Position sync complete"
            );
        }
        Err(e) => warn!("Failed to fetch Alpaca positions for sync: {e}"),
    }

    let strategy_engine_url = std::env::var("STRATEGY_ENGINE_URL")
        .unwrap_or_else(|_| "http://localhost:9100".into());

    let symbols = load_symbols(&strategy_engine_url).await;
    info!(symbols = ?symbols, "Loaded symbol list from strategy engine");

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
        symbols: Mutex::new(symbols),
        spy_day_open: Mutex::new(None),
        spy_day_change_pct: Mutex::new(0.0),
        profit_target_hit: Mutex::new(false),
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

    // 8b. Spawn daily swing signal scanner (fires at 4:05 PM ET)
    let swing_state = state.clone();
    tokio::spawn(async move {
        scheduler::daily_swing_signal_loop(swing_state).await;
    });

    // 8c. Spawn swing stop-loss/take-profit monitor (every 60s during market hours)
    let stop_state = state.clone();
    tokio::spawn(async move {
        scheduler::swing_stop_check_loop(stop_state).await;
    });

    // 8d. Spawn quote refresh loop (updates position prices every 15s)
    let quote_state = state.clone();
    tokio::spawn(async move {
        scheduler::quote_refresh_loop(quote_state).await;
    });

    // 8e. Spawn symbol sync loop (syncs with strategy engine every 5 min)
    let sym_state = state.clone();
    tokio::spawn(async move {
        scheduler::symbol_sync_loop(sym_state).await;
    });

    // 9. Build Axum router
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9102".parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/account", get(get_account))
        .route("/positions", get(get_positions))
        .route("/orders", get(get_orders))
        .route("/trading/halt", post(halt_trading))
        .route("/trading/resume", post(resume_trading))
        .route("/flatten", post(flatten_day_positions))
        .route("/positions/:symbol/close", post(close_position))
        .route("/risk/config", get(get_risk_config).patch(patch_risk_config))
        .route("/risk/swing-config", get(get_swing_risk_config).patch(patch_swing_risk_config))
        .route("/positions/day", get(get_day_positions))
        .route("/positions/swing", get(get_swing_positions))
        .route("/db/signals", post(post_db_signals))
        .route("/db/watched-symbols", post(post_db_watched_symbol))
        .route("/db/watched-symbols/:symbol", delete(delete_db_watched_symbol))
        .route("/stream/events", get(stream_events))
        .layer(cors)
        .with_state(state);

    // 10. Start server
    let addr = "0.0.0.0:9101";
    info!("execution-engine listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ---------------------------------------------------------------------------
// WebSocket bar ingestion
// ---------------------------------------------------------------------------

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
        let current_symbols = state.symbols.lock().await.clone();
        let sub_msg = serde_json::json!({
            "action": "subscribe",
            "bars": current_symbols,
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

        // Process incoming messages (10-min read timeout detects stale connections)
        let read_timeout = std::time::Duration::from_secs(600);
        loop {
            let msg_result = match tokio::time::timeout(read_timeout, ws.next()).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    warn!("WebSocket stream ended (server closed)");
                    break;
                }
                Err(_) => {
                    warn!("WebSocket read timeout (no data for 10 min) — reconnecting");
                    break;
                }
            };
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

                // Upsert to database
                match db::connect() {
                    Ok(con) => {
                        if let Err(e) = db::upsert_bar(&con, &bar, "5min") {
                            error!("Failed to upsert bar: {e}");
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect to database for bar upsert: {e}");
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
    // Skip signal processing after EOD flatten time to avoid reopening positions
    let now_et = chrono::Utc::now().with_timezone(&chrono_tz::America::New_York);
    let flatten_time = chrono::NaiveTime::from_hms_opt(15, 45, 0).unwrap();
    if now_et.time() >= flatten_time {
        return;
    }

    let http = reqwest::Client::new();

    // Fetch recent bars from DuckDB for this symbol to give strategy context
    let bars = match db::connect() {
        Ok(con) => match db::get_recent_bars(&con, &bar.symbol, "5min", 50) {
            Ok(b) => b,
            Err(e) => {
                error!("get_recent_bars failed for {}: {e}", bar.symbol);
                vec![bar.clone()]
            }
        },
        Err(e) => {
            error!("DuckDB connect failed in process_bar: {e}");
            vec![bar.clone()]
        }
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
        let (net_long, net_short) = positions.net_exposure();
        let ctx = RiskContext {
            trading_halted: *state.trading_halted.lock().await,
            account_equity: *state.account_equity.lock().await,
            daily_loss: *state.daily_pnl.lock().await,
            open_position_count: positions.count(),
            position_value_for_symbol: positions.position_value(&signal.symbol, bar.close),
            spy_day_change_pct: *state.spy_day_change_pct.lock().await,
            net_long_exposure: net_long,
            net_short_exposure: net_short,
            strategy_position_count: positions.count_by_strategy(&signal.strategy_name),
            profit_target_hit: *state.profit_target_hit.lock().await,
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

                // Calculate qty: use configured max_position_size_pct of equity
                let equity = *state.account_equity.lock().await;
                let position_size_pct = {
                    let engine = state.risk_engine.lock().await;
                    engine.config.max_position_size_pct
                };
                let max_value = equity * position_size_pct;
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
                            trade_type: signal.trade_type.clone(),
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

                        // Update position tracker with day stop/take levels and strategy name
                        if let Some(fp) = fill_price {
                            let (stop_loss, take_profit) = {
                                let engine = state.risk_engine.lock().await;
                                engine.day_stop_take(fp, &signal.direction)
                            };
                            let mut positions = state.positions.lock().await;
                            let pos = positions.update_on_fill_with_strategy(&signal.symbol, side, qty, fp, signal.trade_type.clone(), Some(stop_loss), Some(take_profit), &signal.strategy_name);
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

// --- Manual position management ---

/// POST /flatten — close all day-trading positions immediately.
async fn flatten_day_positions(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let positions = {
        let tracker = state.positions.lock().await;
        tracker.day_positions()
    };

    if positions.is_empty() {
        return Json(serde_json::json!({"status": "ok", "closed": 0, "message": "No day positions to flatten"}));
    }

    info!(count = positions.len(), "Manual flatten triggered");

    let mut closed = 0u32;
    let mut failed = Vec::new();

    for pos in &positions {
        let close_side = match pos.side {
            PositionSide::Long => "sell",
            PositionSide::Short => "buy",
        };

        match state.alpaca.submit_market_order(&pos.symbol, pos.qty, close_side).await {
            Ok(alpaca_order) => {
                let now = chrono::Utc::now().to_rfc3339();
                let order = Order {
                    order_id: uuid::Uuid::new_v4().to_string(),
                    alpaca_id: Some(alpaca_order.id.clone()),
                    symbol: pos.symbol.clone(),
                    side: close_side.to_string(),
                    qty: pos.qty,
                    filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                    status: alpaca_order.status.clone(),
                    strategy_name: "MANUAL_FLATTEN".to_string(),
                    created_at: now,
                    filled_at: alpaca_order.filled_at.clone(),
                    trade_type: models::TradeType::Day,
                };

                if let Ok(con) = db::connect() {
                    let _ = db::insert_order(&con, &order);
                }
                state.risk_engine.lock().await.record_order(&pos.symbol);

                let fill_price = poll_for_fill(
                    &state, &alpaca_order.id, &order.order_id, close_side, pos.qty, &pos.symbol,
                ).await;

                {
                    let mut tracker = state.positions.lock().await;
                    let updated = tracker.update_on_fill(
                        &pos.symbol, close_side, pos.qty, fill_price.unwrap_or(0.0),
                        models::TradeType::Day, None, None,
                    );
                    if let Ok(con) = db::connect() {
                        match updated {
                            Some(ref p) => { let _ = db::upsert_position(&con, p); }
                            None => { let _ = db::delete_position(&con, &pos.symbol); }
                        }
                    }
                }

                state.broadcaster.send(SseEvent {
                    event_type: SseEventType::PositionUpdate,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    payload: serde_json::json!({
                        "symbol": pos.symbol,
                        "action": "MANUAL_FLATTEN",
                        "qty_sold": pos.qty,
                        "fill_price": fill_price,
                    }),
                });

                info!(symbol = %pos.symbol, fill_price, "Manually flattened");
                closed += 1;
            }
            Err(e) => {
                error!(symbol = %pos.symbol, "Manual flatten failed: {e}");
                failed.push(pos.symbol.clone());
            }
        }
    }

    info!(closed, failed = ?failed, "Manual flatten complete");
    Json(serde_json::json!({
        "status": if failed.is_empty() { "ok" } else { "partial" },
        "closed": closed,
        "failed": failed,
    }))
}

/// POST /positions/:symbol/close — close a single position.
async fn close_position(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(symbol): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let symbol = symbol.to_uppercase();

    let pos = {
        let tracker = state.positions.lock().await;
        tracker.get(&symbol).cloned()
    };

    let pos = match pos {
        Some(p) => p,
        None => return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("No open position for {symbol}")})),
        )),
    };

    let close_side = match pos.side {
        PositionSide::Long => "sell",
        PositionSide::Short => "buy",
    };

    info!(symbol = %symbol, qty = pos.qty, side = close_side, "Manual position close");

    match state.alpaca.submit_market_order(&symbol, pos.qty, close_side).await {
        Ok(alpaca_order) => {
            let now = chrono::Utc::now().to_rfc3339();
            let order = Order {
                order_id: uuid::Uuid::new_v4().to_string(),
                alpaca_id: Some(alpaca_order.id.clone()),
                symbol: symbol.clone(),
                side: close_side.to_string(),
                qty: pos.qty,
                filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                status: alpaca_order.status.clone(),
                strategy_name: "MANUAL_CLOSE".to_string(),
                created_at: now,
                filled_at: alpaca_order.filled_at.clone(),
                trade_type: pos.trade_type.clone(),
            };

            if let Ok(con) = db::connect() {
                let _ = db::insert_order(&con, &order);
            }
            state.risk_engine.lock().await.record_order(&symbol);

            let fill_price = poll_for_fill(
                &state, &alpaca_order.id, &order.order_id, close_side, pos.qty, &symbol,
            ).await;

            {
                let mut tracker = state.positions.lock().await;
                let updated = tracker.update_on_fill(
                    &symbol, close_side, pos.qty, fill_price.unwrap_or(0.0),
                    pos.trade_type.clone(), None, None,
                );
                if let Ok(con) = db::connect() {
                    match updated {
                        Some(ref p) => { let _ = db::upsert_position(&con, p); }
                        None => { let _ = db::delete_position(&con, &symbol); }
                    }
                }
            }

            state.broadcaster.send(SseEvent {
                event_type: SseEventType::PositionUpdate,
                timestamp: chrono::Utc::now().to_rfc3339(),
                payload: serde_json::json!({
                    "symbol": symbol,
                    "action": "MANUAL_CLOSE",
                    "qty_sold": pos.qty,
                    "fill_price": fill_price,
                }),
            });

            info!(symbol = %symbol, fill_price, "Position manually closed");
            Ok(Json(serde_json::json!({
                "status": "ok",
                "symbol": symbol,
                "fill_price": fill_price,
            })))
        }
        Err(e) => {
            error!(symbol = %symbol, "Manual close failed: {e}");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Order submission failed: {e}")})),
            ))
        }
    }
}

// --- DB proxy endpoints (strategy engine writes through these) ---

#[derive(Debug, Deserialize)]
struct SignalInsert {
    strategy_name: String,
    symbol: String,
    timestamp: String,
    direction: String,
    confidence: f64,
    reason: String,
    #[serde(default = "default_trade_type_str")]
    trade_type: String,
}

fn default_trade_type_str() -> String {
    "day".to_string()
}

#[derive(Debug, Deserialize)]
struct SignalBatch {
    signals: Vec<SignalInsert>,
}

async fn post_db_signals(
    State(_state): State<Arc<AppState>>,
    Json(batch): Json<SignalBatch>,
) -> Json<serde_json::Value> {
    let con = match db::connect() {
        Ok(c) => c,
        Err(e) => {
            error!("DB connect failed for signal insert: {e}");
            return Json(serde_json::json!({"error": format!("{e}"), "inserted": 0}));
        }
    };
    let mut inserted = 0;
    for sig in &batch.signals {
        if let Err(e) = db::insert_signal(
            &con,
            &sig.strategy_name,
            &sig.symbol,
            &sig.timestamp,
            &sig.direction,
            sig.confidence,
            &sig.reason,
            &sig.trade_type,
        ) {
            error!(strategy = %sig.strategy_name, symbol = %sig.symbol, "Failed to insert signal: {e}");
        } else {
            inserted += 1;
        }
    }
    Json(serde_json::json!({"inserted": inserted}))
}

#[derive(Debug, Deserialize)]
struct WatchedSymbolReq {
    symbol: String,
}

async fn post_db_watched_symbol(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<WatchedSymbolReq>,
) -> Json<serde_json::Value> {
    let symbol = req.symbol.trim().to_uppercase();
    let con = match db::connect() {
        Ok(c) => c,
        Err(e) => {
            error!("DB connect failed for watched_symbols insert: {e}");
            return Json(serde_json::json!({"error": format!("{e}")}));
        }
    };
    if let Err(e) = db::add_watched_symbol(&con, &symbol) {
        error!("Failed to add watched symbol {symbol}: {e}");
        return Json(serde_json::json!({"error": format!("{e}")}));
    }
    info!("Watched symbol added via proxy: {symbol}");
    Json(serde_json::json!({"status": "ok", "symbol": symbol}))
}

async fn delete_db_watched_symbol(
    State(_state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Json<serde_json::Value> {
    let symbol = symbol.trim().to_uppercase();
    let con = match db::connect() {
        Ok(c) => c,
        Err(e) => {
            error!("DB connect failed for watched_symbols delete: {e}");
            return Json(serde_json::json!({"error": format!("{e}")}));
        }
    };
    match db::remove_watched_symbol(&con, &symbol) {
        Ok(n) => {
            info!("Watched symbol removed via proxy: {symbol} (rows: {n})");
            Json(serde_json::json!({"status": "ok", "symbol": symbol, "deleted": n}))
        }
        Err(e) => {
            error!("Failed to remove watched symbol {symbol}: {e}");
            Json(serde_json::json!({"error": format!("{e}")}))
        }
    }
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

async fn get_risk_config(
    State(state): State<Arc<AppState>>,
) -> Json<RiskConfigResponse> {
    let engine = state.risk_engine.lock().await;
    Json(RiskConfigResponse {
        max_daily_loss_pct: engine.config.max_daily_loss_pct,
        max_position_size_pct: engine.config.max_position_size_pct,
        max_open_positions: engine.config.max_open_positions,
        min_signal_confidence: engine.config.min_signal_confidence,
        order_throttle_secs: engine.config.order_throttle_secs,
        eod_flatten_time_et: "15:45".to_string(),
        day_stop_loss_pct: engine.config.day_stop_loss_pct,
        day_take_profit_pct: engine.config.day_take_profit_pct,
        regime_filter_enabled: engine.config.regime_filter_enabled,
        regime_filter_threshold_pct: engine.config.regime_filter_threshold_pct,
        max_net_exposure_pct: engine.config.max_net_exposure_pct,
        max_positions_per_strategy: engine.config.max_positions_per_strategy,
        daily_loss_tier1_pct: engine.config.daily_loss_tier1_pct,
        daily_loss_tier2_pct: engine.config.daily_loss_tier2_pct,
        daily_profit_target_pct: engine.config.daily_profit_target_pct,
        regime_boosted_exposure_pct: engine.config.regime_boosted_exposure_pct,
    })
}

async fn patch_risk_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<RiskConfigUpdate>,
) -> Result<Json<RiskConfigResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // Reject attempts to change eod_flatten_time_et
    if update.eod_flatten_time_et.is_some() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "eod_flatten_time_et is not editable via API in v1"
            })),
        ));
    }

    // Validate pct fields: must be >= 0.0 and <= 1.0
    for (name, val) in [
        ("max_daily_loss_pct", update.max_daily_loss_pct),
        ("max_position_size_pct", update.max_position_size_pct),
        ("min_signal_confidence", update.min_signal_confidence),
        ("day_stop_loss_pct", update.day_stop_loss_pct),
        ("day_take_profit_pct", update.day_take_profit_pct),
        ("regime_filter_threshold_pct", update.regime_filter_threshold_pct),
        ("max_net_exposure_pct", update.max_net_exposure_pct),
        ("daily_loss_tier1_pct", update.daily_loss_tier1_pct),
        ("daily_loss_tier2_pct", update.daily_loss_tier2_pct),
        ("daily_profit_target_pct", update.daily_profit_target_pct),
        ("regime_boosted_exposure_pct", update.regime_boosted_exposure_pct),
    ] {
        if let Some(v) = val {
            if v < 0.0 || v > 1.0 {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("{name} must be between 0.0 and 1.0, got {v}")
                    })),
                ));
            }
        }
    }

    // Validate max_open_positions: must be 1..=10
    if let Some(v) = update.max_open_positions {
        if v == 0 || v > 10 {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("max_open_positions must be between 1 and 10, got {v}")
                })),
            ));
        }
    }

    // Apply updates
    let response = {
        let mut engine = state.risk_engine.lock().await;
        if let Some(v) = update.max_daily_loss_pct {
            engine.config.max_daily_loss_pct = v;
        }
        if let Some(v) = update.max_position_size_pct {
            engine.config.max_position_size_pct = v;
        }
        if let Some(v) = update.max_open_positions {
            engine.config.max_open_positions = v;
        }
        if let Some(v) = update.min_signal_confidence {
            engine.config.min_signal_confidence = v;
        }
        if let Some(v) = update.order_throttle_secs {
            engine.config.order_throttle_secs = v;
        }
        if let Some(v) = update.day_stop_loss_pct {
            engine.config.day_stop_loss_pct = v;
        }
        if let Some(v) = update.day_take_profit_pct {
            engine.config.day_take_profit_pct = v;
        }
        if let Some(v) = update.regime_filter_enabled {
            engine.config.regime_filter_enabled = v;
        }
        if let Some(v) = update.regime_filter_threshold_pct {
            engine.config.regime_filter_threshold_pct = v;
        }
        if let Some(v) = update.max_net_exposure_pct {
            engine.config.max_net_exposure_pct = v;
        }
        if let Some(v) = update.max_positions_per_strategy {
            engine.config.max_positions_per_strategy = v;
        }
        if let Some(v) = update.daily_loss_tier1_pct {
            engine.config.daily_loss_tier1_pct = v;
        }
        if let Some(v) = update.daily_loss_tier2_pct {
            engine.config.daily_loss_tier2_pct = v;
        }
        if let Some(v) = update.daily_profit_target_pct {
            engine.config.daily_profit_target_pct = v;
        }
        if let Some(v) = update.regime_boosted_exposure_pct {
            engine.config.regime_boosted_exposure_pct = v;
        }

        RiskConfigResponse {
            max_daily_loss_pct: engine.config.max_daily_loss_pct,
            max_position_size_pct: engine.config.max_position_size_pct,
            max_open_positions: engine.config.max_open_positions,
            min_signal_confidence: engine.config.min_signal_confidence,
            order_throttle_secs: engine.config.order_throttle_secs,
            eod_flatten_time_et: "15:45".to_string(),
            day_stop_loss_pct: engine.config.day_stop_loss_pct,
            day_take_profit_pct: engine.config.day_take_profit_pct,
            regime_filter_enabled: engine.config.regime_filter_enabled,
            regime_filter_threshold_pct: engine.config.regime_filter_threshold_pct,
            max_net_exposure_pct: engine.config.max_net_exposure_pct,
            max_positions_per_strategy: engine.config.max_positions_per_strategy,
            daily_loss_tier1_pct: engine.config.daily_loss_tier1_pct,
            daily_loss_tier2_pct: engine.config.daily_loss_tier2_pct,
            daily_profit_target_pct: engine.config.daily_profit_target_pct,
            regime_boosted_exposure_pct: engine.config.regime_boosted_exposure_pct,
        }
    };

    info!(
        max_daily_loss_pct = response.max_daily_loss_pct,
        max_position_size_pct = response.max_position_size_pct,
        max_open_positions = response.max_open_positions,
        min_signal_confidence = response.min_signal_confidence,
        order_throttle_secs = response.order_throttle_secs,
        day_stop_loss_pct = response.day_stop_loss_pct,
        day_take_profit_pct = response.day_take_profit_pct,
        regime_filter_enabled = response.regime_filter_enabled,
        regime_filter_threshold_pct = response.regime_filter_threshold_pct,
        max_net_exposure_pct = response.max_net_exposure_pct,
        max_positions_per_strategy = response.max_positions_per_strategy,
        "Risk config updated"
    );

    // Broadcast SSE event with full new config
    state.broadcaster.send(SseEvent {
        event_type: SseEventType::RiskConfigUpdated,
        timestamp: chrono::Utc::now().to_rfc3339(),
        payload: serde_json::json!({
            "max_daily_loss_pct": response.max_daily_loss_pct,
            "max_position_size_pct": response.max_position_size_pct,
            "max_open_positions": response.max_open_positions,
            "min_signal_confidence": response.min_signal_confidence,
            "order_throttle_secs": response.order_throttle_secs,
            "eod_flatten_time_et": "15:45",
            "day_stop_loss_pct": response.day_stop_loss_pct,
            "day_take_profit_pct": response.day_take_profit_pct,
            "regime_filter_enabled": response.regime_filter_enabled,
            "regime_filter_threshold_pct": response.regime_filter_threshold_pct,
            "max_net_exposure_pct": response.max_net_exposure_pct,
            "max_positions_per_strategy": response.max_positions_per_strategy,
            "daily_loss_tier1_pct": response.daily_loss_tier1_pct,
            "daily_loss_tier2_pct": response.daily_loss_tier2_pct,
            "daily_profit_target_pct": response.daily_profit_target_pct,
            "regime_boosted_exposure_pct": response.regime_boosted_exposure_pct,
        }),
    });

    Ok(Json(response))
}

async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> axum::response::sse::Sse<impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>
{
    state.broadcaster.subscribe()
}

async fn get_day_positions(State(state): State<Arc<AppState>>) -> Json<Vec<Position>> {
    let positions = state.positions.lock().await;
    Json(positions.day_positions())
}

async fn get_swing_positions(State(state): State<Arc<AppState>>) -> Json<Vec<Position>> {
    let positions = state.positions.lock().await;
    Json(positions.swing_positions())
}

async fn get_swing_risk_config(
    State(state): State<Arc<AppState>>,
) -> Json<SwingRiskConfigResponse> {
    let engine = state.risk_engine.lock().await;
    Json(SwingRiskConfigResponse {
        max_swing_positions: engine.swing_config.max_swing_positions,
        max_portfolio_heat_pct: engine.swing_config.max_portfolio_heat_pct,
        per_position_stop_loss_pct: engine.swing_config.per_position_stop_loss_pct,
        per_position_take_profit_pct: engine.swing_config.per_position_take_profit_pct,
        min_composite_confidence: engine.swing_config.min_composite_confidence,
    })
}

async fn patch_swing_risk_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<SwingRiskConfigUpdate>,
) -> Result<Json<SwingRiskConfigResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // Validate pct fields
    for (name, val) in [
        ("max_portfolio_heat_pct", update.max_portfolio_heat_pct),
        ("per_position_stop_loss_pct", update.per_position_stop_loss_pct),
        ("per_position_take_profit_pct", update.per_position_take_profit_pct),
        ("min_composite_confidence", update.min_composite_confidence),
    ] {
        if let Some(v) = val {
            if v < 0.0 || v > 1.0 {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("{name} must be between 0.0 and 1.0, got {v}")
                    })),
                ));
            }
        }
    }

    if let Some(v) = update.max_swing_positions {
        if v == 0 || v > 20 {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("max_swing_positions must be between 1 and 20, got {v}")
                })),
            ));
        }
    }

    let response = {
        let mut engine = state.risk_engine.lock().await;
        if let Some(v) = update.max_swing_positions {
            engine.swing_config.max_swing_positions = v;
        }
        if let Some(v) = update.max_portfolio_heat_pct {
            engine.swing_config.max_portfolio_heat_pct = v;
        }
        if let Some(v) = update.per_position_stop_loss_pct {
            engine.swing_config.per_position_stop_loss_pct = v;
        }
        if let Some(v) = update.per_position_take_profit_pct {
            engine.swing_config.per_position_take_profit_pct = v;
        }
        if let Some(v) = update.min_composite_confidence {
            engine.swing_config.min_composite_confidence = v;
        }

        SwingRiskConfigResponse {
            max_swing_positions: engine.swing_config.max_swing_positions,
            max_portfolio_heat_pct: engine.swing_config.max_portfolio_heat_pct,
            per_position_stop_loss_pct: engine.swing_config.per_position_stop_loss_pct,
            per_position_take_profit_pct: engine.swing_config.per_position_take_profit_pct,
            min_composite_confidence: engine.swing_config.min_composite_confidence,
        }
    };

    info!(
        max_swing_positions = response.max_swing_positions,
        max_portfolio_heat_pct = response.max_portfolio_heat_pct,
        per_position_stop_loss_pct = response.per_position_stop_loss_pct,
        per_position_take_profit_pct = response.per_position_take_profit_pct,
        min_composite_confidence = response.min_composite_confidence,
        "Swing risk config updated"
    );

    Ok(Json(response))
}

#[cfg(test)]
mod risk_config_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        let config = alpaca::AlpacaConfig {
            api_key: "test".into(),
            secret_key: "test".into(),
            mode: alpaca::AlpacaMode::Paper,
        };
        Arc::new(AppState {
            alpaca: AlpacaClient::new(config),
            broadcaster: SseBroadcaster::new(100),
            positions: Mutex::new(PositionTracker::new()),
            risk_engine: Mutex::new(RiskEngine::new(RiskConfig::default())),
            trading_halted: Mutex::new(false),
            daily_pnl: Mutex::new(0.0),
            account_equity: Mutex::new(100_000.0),
            strategy_engine_url: "http://localhost:9100".into(),
            symbols: Mutex::new(vec!["SPY".into(), "QQQ".into(), "AAPL".into(), "MSFT".into(), "NVDA".into(), "GOOGL".into()]),
            spy_day_open: Mutex::new(None),
            spy_day_change_pct: Mutex::new(0.0),
            profit_target_hit: Mutex::new(false),
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/risk/config", get(get_risk_config).patch(patch_risk_config))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_get_risk_config_returns_defaults() {
        let app = test_app(test_state());
        let resp = app
            .oneshot(Request::get("/risk/config").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();

        assert_eq!(body["max_daily_loss_pct"], 0.02);
        assert_eq!(body["max_position_size_pct"], 0.10);
        assert_eq!(body["max_open_positions"], 4);
        assert_eq!(body["min_signal_confidence"], 0.60);
        assert_eq!(body["order_throttle_secs"], 300);
        assert_eq!(body["eod_flatten_time_et"], "15:45");
    }

    #[tokio::test]
    async fn test_patch_risk_config_happy_path() {
        let state = test_state();
        let app = test_app(state.clone());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"max_daily_loss_pct": 0.03, "max_open_positions": 6}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();

        assert_eq!(body["max_daily_loss_pct"], 0.03);
        assert_eq!(body["max_open_positions"], 6);
        // Unchanged fields remain at defaults
        assert_eq!(body["max_position_size_pct"], 0.10);
        assert_eq!(body["min_signal_confidence"], 0.60);
        assert_eq!(body["order_throttle_secs"], 300);

        // Verify state was actually mutated
        let engine = state.risk_engine.lock().await;
        assert_eq!(engine.config.max_daily_loss_pct, 0.03);
        assert_eq!(engine.config.max_open_positions, 6);
    }

    #[tokio::test]
    async fn test_patch_risk_config_rejects_eod_flatten_time() {
        let app = test_app(test_state());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"eod_flatten_time_et": "15:30"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(body["error"].as_str().unwrap().contains("not editable"));
    }

    #[tokio::test]
    async fn test_patch_risk_config_rejects_negative_pct() {
        let app = test_app(test_state());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"max_daily_loss_pct": -0.01}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(body["error"].as_str().unwrap().contains("max_daily_loss_pct"));
    }

    #[tokio::test]
    async fn test_patch_risk_config_rejects_pct_above_one() {
        let app = test_app(test_state());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"min_signal_confidence": 1.5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(body["error"].as_str().unwrap().contains("min_signal_confidence"));
    }

    #[tokio::test]
    async fn test_patch_risk_config_rejects_max_positions_above_10() {
        let app = test_app(test_state());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"max_open_positions": 11}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(body["error"].as_str().unwrap().contains("max_open_positions"));
    }

    #[tokio::test]
    async fn test_patch_risk_config_rejects_zero_positions() {
        let app = test_app(test_state());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"max_open_positions": 0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_patch_risk_config_empty_body_is_noop() {
        let state = test_state();
        let app = test_app(state.clone());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/risk/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        // All values should remain at defaults
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(body["max_daily_loss_pct"], 0.02);
        assert_eq!(body["max_open_positions"], 4);
    }
}
