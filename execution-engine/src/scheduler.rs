use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveTime, Utc, Weekday};
use chrono_tz::America::New_York;
use tracing::{error, info, warn};

use crate::db;
use crate::models::{Order, PositionSide, SseEvent, SseEventType, SwingSignalRequest, SwingSignalResponse, TradeType};
use crate::risk::SwingRiskContext;
use crate::AppState;

/// Hardcoded NYSE holidays for 2026 (v1 — no external calendar API).
/// Dates when the market is fully closed.
const NYSE_HOLIDAYS_2026: &[(u32, u32)] = &[
    (1, 1),   // New Year's Day
    (1, 19),  // MLK Day
    (2, 16),  // Presidents' Day
    (4, 3),   // Good Friday
    (5, 25),  // Memorial Day
    (7, 3),   // Independence Day (observed)
    (9, 7),   // Labor Day
    (11, 26), // Thanksgiving
    (12, 25), // Christmas
];

/// Returns true if the given date is a trading day (not weekend, not NYSE holiday).
pub fn is_trading_day(date: NaiveDate) -> bool {
    let weekday = date.weekday();
    if weekday == Weekday::Sat || weekday == Weekday::Sun {
        return false;
    }
    let month = date.month();
    let day = date.day();
    !NYSE_HOLIDAYS_2026.iter().any(|&(m, d)| m == month && d == day)
}

/// Returns true if the current ET time has reached or passed 15:45.
pub fn should_flatten(date: NaiveDate, time: NaiveTime) -> bool {
    if !is_trading_day(date) {
        return false;
    }
    let flatten_time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
    time >= flatten_time
}

/// Background task: checks every 30 seconds and flattens all positions at 15:45 ET.
pub async fn eod_flatten_loop(state: Arc<AppState>) {
    let mut flattened_today: Option<NaiveDate> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        // Already flattened today — skip until next trading day
        if flattened_today == Some(today) {
            continue;
        }

        if !should_flatten(today, current_time) {
            continue;
        }

        info!("EOD auto-flatten triggered at {now_et}");
        flattened_today = Some(today);

        // Get only day-trading positions (swing positions survive overnight)
        let positions = {
            let tracker = state.positions.lock().await;
            tracker.day_positions()
        };

        if positions.is_empty() {
            info!("No open positions to flatten");
            continue;
        }

        for pos in &positions {
            // Close direction: sell longs, buy to cover shorts
            let close_side = match pos.side {
                PositionSide::Long => "sell",
                PositionSide::Short => "buy",
            };

            info!(
                symbol = %pos.symbol,
                qty = pos.qty,
                side = close_side,
                reason = "EOD_AUTO_FLATTEN",
                "Flattening position"
            );

            let qty = pos.qty;
            let symbol = &pos.symbol;

            match state.alpaca.submit_market_order(symbol, qty, close_side).await {
                Ok(alpaca_order) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let order = Order {
                        order_id: uuid::Uuid::new_v4().to_string(),
                        alpaca_id: Some(alpaca_order.id.clone()),
                        symbol: symbol.clone(),
                        side: close_side.to_string(),
                        qty,
                        filled_price: alpaca_order
                            .filled_avg_price
                            .as_ref()
                            .and_then(|p| p.parse::<f64>().ok()),
                        status: alpaca_order.status.clone(),
                        strategy_name: "EOD_AUTO_FLATTEN".to_string(),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
                        trade_type: crate::models::TradeType::Day,
                    };

                    if let Ok(con) = db::connect() {
                        if let Err(e) = db::insert_order(&con, &order) {
                            error!("Failed to insert flatten order: {e}");
                        }
                    }

                    state.risk_engine.lock().await.record_order(symbol);

                    // Poll for fill
                    let fill_price = crate::poll_for_fill(
                        &state,
                        &alpaca_order.id,
                        &order.order_id,
                        close_side,
                        qty,
                        symbol,
                    )
                    .await;

                    // Update position tracker
                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(symbol, close_side, qty, fill_price.unwrap_or(0.0), crate::models::TradeType::Day, None, None);
                        if let Ok(con) = db::connect() {
                            match updated {
                                Some(ref p) => {
                                    let _ = db::upsert_position(&con, p);
                                }
                                None => {
                                    let _ = db::delete_position(&con, symbol);
                                }
                            }
                        }
                    }

                    // Broadcast POSITION_UPDATE for each closed position
                    state.broadcaster.send(SseEvent {
                        event_type: SseEventType::PositionUpdate,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        payload: serde_json::json!({
                            "symbol": symbol,
                            "action": "EOD_AUTO_FLATTEN",
                            "qty_sold": qty,
                            "fill_price": fill_price,
                        }),
                    });

                    info!(
                        symbol,
                        qty,
                        fill_price,
                        reason = "EOD_AUTO_FLATTEN",
                        "Position flattened"
                    );
                }
                Err(e) => {
                    warn!(
                        symbol,
                        local_qty = qty,
                        reason = "EOD_AUTO_FLATTEN",
                        "Flatten order failed: {e} — syncing with Alpaca and retrying"
                    );

                    // Sync with Alpaca to get correct qty, then retry
                    let retry_qty = match state.alpaca.get_positions().await {
                        Ok(alpaca_positions) => {
                            let mut tracker = state.positions.lock().await;
                            tracker.sync_with_alpaca(&alpaca_positions);
                            tracker.get(symbol).map(|p| p.qty).filter(|&q| q > 0.001)
                        }
                        Err(sync_err) => {
                            error!(symbol, "Alpaca position sync failed during flatten retry: {sync_err}");
                            None
                        }
                    };

                    if let Some(actual_qty) = retry_qty {
                        info!(symbol, actual_qty, side = close_side, "Retrying flatten with synced qty");
                        match state.alpaca.submit_market_order(symbol, actual_qty, close_side).await {
                            Ok(alpaca_order) => {
                                let now = chrono::Utc::now().to_rfc3339();
                                let order = Order {
                                    order_id: uuid::Uuid::new_v4().to_string(),
                                    alpaca_id: Some(alpaca_order.id.clone()),
                                    symbol: symbol.clone(),
                                    side: close_side.to_string(),
                                    qty: actual_qty,
                                    filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                                    status: alpaca_order.status.clone(),
                                    strategy_name: "EOD_AUTO_FLATTEN".to_string(),
                                    created_at: now,
                                    filled_at: alpaca_order.filled_at.clone(),
                                    trade_type: crate::models::TradeType::Day,
                                };
                                if let Ok(con) = db::connect() {
                                    let _ = db::insert_order(&con, &order);
                                }
                                state.risk_engine.lock().await.record_order(symbol);

                                let fill_price = crate::poll_for_fill(
                                    &state, &alpaca_order.id, &order.order_id, close_side, actual_qty, symbol,
                                ).await;

                                {
                                    let mut tracker = state.positions.lock().await;
                                    let updated = tracker.update_on_fill(symbol, close_side, actual_qty, fill_price.unwrap_or(0.0), crate::models::TradeType::Day, None, None);
                                    if let Ok(con) = db::connect() {
                                        match updated {
                                            Some(ref p) => { let _ = db::upsert_position(&con, p); }
                                            None => { let _ = db::delete_position(&con, symbol); }
                                        }
                                    }
                                }

                                state.broadcaster.send(SseEvent {
                                    event_type: SseEventType::PositionUpdate,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    payload: serde_json::json!({
                                        "symbol": symbol,
                                        "action": "EOD_AUTO_FLATTEN",
                                        "qty_sold": actual_qty,
                                        "fill_price": fill_price,
                                        "retry": true,
                                    }),
                                });

                                info!(symbol, actual_qty, fill_price, "Position flattened on retry");
                            }
                            Err(retry_err) => {
                                error!(symbol, "Flatten retry also failed: {retry_err}");
                            }
                        }
                    } else {
                        info!(symbol, "Position no longer exists on Alpaca — removing locally");
                        let mut tracker = state.positions.lock().await;
                        tracker.sync_with_alpaca(&[]);  // Will remove if not in empty list
                        if let Ok(con) = db::connect() {
                            let _ = db::delete_position(&con, symbol);
                        }
                    }
                }
            }
        }

        info!(
            count = positions.len(),
            "EOD auto-flatten complete"
        );
    }
}

/// Returns true if current ET time is within market hours (9:30-16:00).
fn is_market_hours(time: NaiveTime) -> bool {
    let open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
    time >= open && time <= close
}

/// Background task: after market close (4:05 PM ET), fetch daily bars and generate swing signals.
pub async fn daily_swing_signal_loop(state: Arc<AppState>) {
    let mut triggered_today: Option<NaiveDate> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        // Already triggered today
        if triggered_today == Some(today) {
            continue;
        }

        if !is_trading_day(today) {
            continue;
        }

        // Trigger at 16:05 ET (5 minutes after close to ensure daily bars are finalized)
        let trigger_time = NaiveTime::from_hms_opt(16, 5, 0).unwrap();
        if current_time < trigger_time {
            continue;
        }

        info!("Daily swing signal scan triggered at {now_et}");
        triggered_today = Some(today);

        let http = reqwest::Client::new();
        let symbols = state.symbols.lock().await.clone();

        for symbol in &symbols {
            // Fetch daily bars from Alpaca (last 150 for weekly EMA lookback)
            let bars = match state.alpaca.get_daily_bars(symbol, 150).await {
                Ok(b) => b,
                Err(e) => {
                    error!(symbol, "Failed to fetch daily bars from Alpaca: {e}");
                    continue;
                }
            };

            if bars.is_empty() {
                warn!(symbol, "No daily bars returned from Alpaca");
                continue;
            }

            // Store daily bars in DuckDB
            if let Ok(con) = db::connect() {
                for bar in &bars {
                    if let Err(e) = db::upsert_bar(&con, bar, "1d") {
                        error!(symbol, "Failed to upsert daily bar: {e}");
                    }
                }
            }

            // Call strategy engine POST /signal/swing
            let req = SwingSignalRequest {
                symbol: symbol.clone(),
                bars_daily: bars,
            };

            let url = format!("{}/signal/swing", state.strategy_engine_url);
            let resp = match http.post(&url).json(&req).send().await {
                Ok(r) => r,
                Err(e) => {
                    error!(symbol, "Failed to call swing signal endpoint: {e}");
                    continue;
                }
            };

            let swing_resp: SwingSignalResponse = match resp.json().await {
                Ok(r) => r,
                Err(e) => {
                    error!(symbol, "Failed to parse swing signal response: {e}");
                    continue;
                }
            };

            let composite = swing_resp.composite;
            info!(
                symbol,
                direction = ?composite.direction,
                confidence = composite.confidence,
                reason = %composite.reason,
                "Swing composite signal"
            );

            // Skip HOLD signals
            if composite.direction == crate::models::Direction::Hold {
                continue;
            }

            // Evaluate through swing risk engine
            let risk_decision = {
                let engine = state.risk_engine.lock().await;
                let positions = state.positions.lock().await;
                let equity = *state.account_equity.lock().await;
                let daily_pnl = *state.daily_pnl.lock().await;
                let halted = *state.trading_halted.lock().await;

                let swing_positions = positions.swing_positions();
                let current_heat: f64 = swing_positions.iter().map(|p| {
                    if let (Some(sl), _) = (p.stop_loss_price, p.take_profit_price) {
                        let distance = (p.avg_entry_price - sl).abs() / p.avg_entry_price;
                        (p.qty * p.avg_entry_price * distance) / equity
                    } else {
                        engine.swing_config.per_position_stop_loss_pct
                            * (p.qty * p.avg_entry_price) / equity
                    }
                }).sum();

                let ctx = SwingRiskContext {
                    trading_halted: halted,
                    account_equity: equity,
                    daily_loss: daily_pnl,
                    swing_position_count: swing_positions.len(),
                    current_portfolio_heat: current_heat,
                    position_value_for_symbol: positions.position_value(symbol, composite.confidence * equity * engine.swing_config.per_position_stop_loss_pct / engine.swing_config.per_position_stop_loss_pct),
                };

                engine.evaluate_swing(&composite, &ctx)
            };

            match risk_decision {
                crate::risk::RiskDecision::Approved => {
                    let side = match composite.direction {
                        crate::models::Direction::Buy => "buy",
                        crate::models::Direction::Sell => "sell",
                        _ => continue,
                    };

                    // Calculate position size: use max_position_size_pct of equity
                    let equity = *state.account_equity.lock().await;
                    let engine = state.risk_engine.lock().await;
                    let position_value = equity * engine.config.max_position_size_pct;
                    // Get last close price from the signal context
                    drop(engine);

                    // Fetch current price for qty calculation
                    let current_price = match state.alpaca.get_daily_bars(symbol, 1).await {
                        Ok(bars) if !bars.is_empty() => bars.last().unwrap().close,
                        _ => {
                            warn!(symbol, "Cannot determine price for swing order sizing");
                            continue;
                        }
                    };

                    let qty = (position_value / current_price).floor();
                    if qty < 1.0 {
                        warn!(symbol, "Calculated qty < 1, skipping");
                        continue;
                    }

                    // Calculate stop/take prices
                    let (stop_loss, take_profit) = {
                        let engine = state.risk_engine.lock().await;
                        engine.swing_stop_take(current_price, &composite.direction)
                    };

                    info!(
                        symbol,
                        side,
                        qty,
                        stop_loss,
                        take_profit,
                        "Submitting swing order"
                    );

                    match state.alpaca.submit_market_order(symbol, qty, side).await {
                        Ok(alpaca_order) => {
                            let now = chrono::Utc::now().to_rfc3339();
                            let order = Order {
                                order_id: uuid::Uuid::new_v4().to_string(),
                                alpaca_id: Some(alpaca_order.id.clone()),
                                symbol: symbol.clone(),
                                side: side.to_string(),
                                qty,
                                filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                                status: alpaca_order.status.clone(),
                                strategy_name: composite.strategy_name.clone(),
                                created_at: now,
                                filled_at: alpaca_order.filled_at.clone(),
                                trade_type: TradeType::Swing,
                            };

                            if let Ok(con) = db::connect() {
                                if let Err(e) = db::insert_order(&con, &order) {
                                    error!("Failed to insert swing order: {e}");
                                }
                            }

                            state.risk_engine.lock().await.record_order(symbol);

                            let fill_price = crate::poll_for_fill(
                                &state, &alpaca_order.id, &order.order_id, side, qty, symbol,
                            ).await;

                            if let Some(fp) = fill_price {
                                let mut tracker = state.positions.lock().await;
                                let pos = tracker.update_on_fill(
                                    symbol, side, qty, fp,
                                    TradeType::Swing,
                                    Some(stop_loss),
                                    Some(take_profit),
                                );
                                if let Ok(con) = db::connect() {
                                    match pos {
                                        Some(ref p) => { let _ = db::upsert_position(&con, p); }
                                        None => { let _ = db::delete_position(&con, symbol); }
                                    }
                                }
                            }

                            state.broadcaster.send(SseEvent {
                                event_type: SseEventType::OrderFill,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                payload: serde_json::json!({
                                    "symbol": symbol,
                                    "side": side,
                                    "qty": qty,
                                    "trade_type": "swing",
                                    "stop_loss": stop_loss,
                                    "take_profit": take_profit,
                                }),
                            });
                        }
                        Err(e) => {
                            error!(symbol, "Swing order submission failed: {e}");
                        }
                    }
                }
                crate::risk::RiskDecision::Rejected(reason) => {
                    warn!(symbol, reason, "Swing signal rejected by risk engine");
                }
                crate::risk::RiskDecision::HaltAll(reason) => {
                    error!(reason, "Daily loss limit breached during swing evaluation");
                    *state.trading_halted.lock().await = true;
                }
            }
        }

        info!("Daily swing signal scan complete");
    }
}

/// Background task: every 60 seconds during market hours, check swing positions
/// against their stop-loss and take-profit levels.
pub async fn swing_stop_check_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        if !is_trading_day(today) || !is_market_hours(current_time) {
            continue;
        }

        // Get swing positions with stop/take levels
        let swing_positions = {
            let tracker = state.positions.lock().await;
            tracker.swing_positions()
        };

        if swing_positions.is_empty() {
            continue;
        }

        for pos in &swing_positions {
            let stop_loss = match pos.stop_loss_price {
                Some(sl) => sl,
                None => continue,
            };
            let take_profit = match pos.take_profit_price {
                Some(tp) => tp,
                None => continue,
            };

            // Get current price from Alpaca
            let current_price = match state.alpaca.get_daily_bars(&pos.symbol, 1).await {
                Ok(bars) if !bars.is_empty() => bars.last().unwrap().close,
                _ => continue,
            };

            // Stop/take logic depends on position side
            let (hit_stop, hit_take) = match pos.side {
                PositionSide::Long => (current_price <= stop_loss, current_price >= take_profit),
                PositionSide::Short => (current_price >= stop_loss, current_price <= take_profit),
            };

            if !hit_stop && !hit_take {
                continue;
            }

            let reason = if hit_stop { "STOP_LOSS" } else { "TAKE_PROFIT" };
            let close_side = match pos.side {
                PositionSide::Long => "sell",
                PositionSide::Short => "buy",
            };
            info!(
                symbol = %pos.symbol,
                current_price,
                stop_loss,
                take_profit,
                reason,
                side = close_side,
                "Swing position exit triggered"
            );

            // Close position
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
                        strategy_name: format!("SWING_{reason}"),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
                        trade_type: TradeType::Swing,
                    };

                    if let Ok(con) = db::connect() {
                        if let Err(e) = db::insert_order(&con, &order) {
                            error!("Failed to insert swing exit order: {e}");
                        }
                    }

                    let fill_price = crate::poll_for_fill(
                        &state, &alpaca_order.id, &order.order_id, close_side, pos.qty, &pos.symbol,
                    ).await;

                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(
                            &pos.symbol, close_side, pos.qty, fill_price.unwrap_or(0.0),
                            TradeType::Swing, None, None,
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
                            "action": reason,
                            "trade_type": "swing",
                            "qty_sold": pos.qty,
                            "fill_price": fill_price,
                        }),
                    });

                    info!(
                        symbol = %pos.symbol,
                        reason,
                        fill_price,
                        "Swing position closed"
                    );
                }
                Err(e) => {
                    error!(symbol = %pos.symbol, "Failed to submit swing exit order: {e}");
                }
            }
        }
    }
}

/// Background task: refreshes position prices from Alpaca latest trades every 15 seconds.
/// Every 5 minutes, also syncs position quantities with Alpaca's actual holdings.
/// Also tracks SPY intraday change for the market regime filter and checks day trade
/// positions against their stop-loss / take-profit levels.
/// Runs during extended hours (4 AM – 8 PM ET) to capture pre-market and after-hours moves.
pub async fn quote_refresh_loop(state: Arc<AppState>) {
    let mut tick_count: u32 = 0;
    let mut last_spy_open_date: Option<NaiveDate> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        tick_count += 1;

        let now_et = Utc::now().with_timezone(&New_York);
        let today = now_et.date_naive();
        let current_time = now_et.time();

        // Skip on non-trading days
        if !is_trading_day(today) {
            continue;
        }

        // Extended hours window: 4 AM – 8 PM ET
        let extended_open = NaiveTime::from_hms_opt(4, 0, 0).unwrap();
        let extended_close = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
        if current_time < extended_open || current_time > extended_close {
            continue;
        }

        // --- SPY open price: fetch once per trading day ---
        if last_spy_open_date != Some(today) {
            match state.alpaca.get_daily_bars("SPY", 1).await {
                Ok(bars) if !bars.is_empty() => {
                    let open = bars.last().unwrap().open;
                    *state.spy_day_open.lock().await = Some(open);
                    last_spy_open_date = Some(today);
                    // Reset profit target flag for the new trading day
                    *state.profit_target_hit.lock().await = false;
                    info!(spy_open = open, "SPY daily open price set, profit target reset");
                }
                Ok(_) => warn!("No daily bars returned for SPY"),
                Err(e) => warn!("Failed to fetch SPY daily bars: {e}"),
            }
        }

        // Every 20 ticks (~5 min), do a full position sync with Alpaca
        if tick_count % 20 == 0 {
            match state.alpaca.get_positions().await {
                Ok(alpaca_positions) => {
                    let mut tracker = state.positions.lock().await;
                    let changed = tracker.sync_with_alpaca(&alpaca_positions);
                    // Warn if synced position count exceeds max_open_positions
                    let pos_count = tracker.count();
                    let max_positions = state.risk_engine.lock().await.config.max_open_positions;
                    if pos_count > max_positions {
                        warn!(
                            current = pos_count,
                            max = max_positions,
                            "Alpaca position count ({}) exceeds max_open_positions ({}). \
                             New signal-driven orders will be blocked until positions close.",
                            pos_count, max_positions,
                        );
                    }

                    if !changed.is_empty() {
                        info!(changed = ?changed, "Position sync: updated from Alpaca");
                        if let Ok(con) = db::connect() {
                            for sym in &changed {
                                if let Some(pos) = tracker.get(sym) {
                                    let _ = db::upsert_position(&con, pos);
                                } else {
                                    let _ = db::delete_position(&con, sym);
                                }
                            }
                        }
                        // Broadcast so dashboard refreshes
                        state.broadcaster.send(SseEvent {
                            event_type: SseEventType::PositionUpdate,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            payload: serde_json::json!({
                                "action": "SYNC",
                                "changed": changed,
                            }),
                        });
                    }
                }
                Err(e) => warn!("Position sync failed: {e}"),
            }
            continue; // Skip the price-only update on sync ticks
        }

        // Build symbols list: positions + always include SPY for regime tracking
        let position_symbols: Vec<String> = {
            let tracker = state.positions.lock().await;
            tracker.all().iter().map(|p| p.symbol.clone()).collect()
        };

        let mut fetch_symbols = position_symbols.clone();
        if !fetch_symbols.iter().any(|s| s == "SPY") {
            fetch_symbols.push("SPY".to_string());
        }

        // Fetch latest trade prices in a single API call
        let prices = match state.alpaca.get_latest_trades(&fetch_symbols).await {
            Ok(p) => p,
            Err(e) => {
                warn!("Quote refresh failed: {e}");
                continue;
            }
        };

        // --- Update SPY intraday change ---
        if let Some(spy_price) = prices.get("SPY") {
            let spy_open = state.spy_day_open.lock().await;
            if let Some(open) = *spy_open {
                if open > 0.0 {
                    let change_pct = (spy_price - open) / open;
                    *state.spy_day_change_pct.lock().await = change_pct;
                }
            }
        }

        if position_symbols.is_empty() {
            continue;
        }

        // Update each position's price
        let mut tracker = state.positions.lock().await;
        for (symbol, price) in &prices {
            if let Some(updated) = tracker.update_price(symbol, *price) {
                if let Ok(con) = db::connect() {
                    let _ = db::upsert_position(&con, &updated);
                }

                state.broadcaster.send(SseEvent {
                    event_type: SseEventType::PositionUpdate,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    payload: serde_json::json!({
                        "symbol": symbol,
                        "current_price": updated.current_price,
                        "unrealized_pnl": updated.unrealized_pnl,
                    }),
                });
            }
        }

        // --- Portfolio-level daily profit target check ---
        // If unrealized day P&L exceeds the configured target, flatten all day positions.
        if !*state.profit_target_hit.lock().await {
            let profit_target_pct = {
                let engine = state.risk_engine.lock().await;
                engine.config.daily_profit_target_pct
            };
            if profit_target_pct > 0.0 {
                let equity = *state.account_equity.lock().await;
                let day_pnl = tracker.day_unrealized_pnl();
                let target_dollars = equity * profit_target_pct;
                if day_pnl >= target_dollars {
                    info!(
                        day_pnl,
                        target_dollars,
                        target_pct = profit_target_pct * 100.0,
                        "Daily profit target hit — flattening all day positions"
                    );

                    // Set the flag to block new entries for the rest of the day
                    *state.profit_target_hit.lock().await = true;

                    let day_positions = tracker.day_positions();
                    drop(tracker);

                    // Broadcast event so dashboard knows
                    state.broadcaster.send(SseEvent {
                        event_type: SseEventType::TradingHalted,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        payload: serde_json::json!({
                            "reason": "DAILY_PROFIT_TARGET",
                            "day_pnl": day_pnl,
                            "target_dollars": target_dollars,
                        }),
                    });

                    for pos in &day_positions {
                        let close_side = match pos.side {
                            PositionSide::Long => "sell",
                            PositionSide::Short => "buy",
                        };

                        info!(
                            symbol = %pos.symbol,
                            qty = pos.qty,
                            side = close_side,
                            reason = "DAILY_PROFIT_TARGET",
                            "Flattening position"
                        );

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
                                    strategy_name: "DAILY_PROFIT_TARGET".to_string(),
                                    created_at: now,
                                    filled_at: alpaca_order.filled_at.clone(),
                                    trade_type: crate::models::TradeType::Day,
                                };

                                if let Ok(con) = db::connect() {
                                    if let Err(e) = db::insert_order(&con, &order) {
                                        error!("Failed to insert profit target order: {e}");
                                    }
                                }

                                state.risk_engine.lock().await.record_order(&pos.symbol);

                                let fill_price = crate::poll_for_fill(
                                    &state, &alpaca_order.id, &order.order_id, close_side, pos.qty, &pos.symbol,
                                ).await;

                                {
                                    let mut tracker = state.positions.lock().await;
                                    let updated = tracker.update_on_fill(
                                        &pos.symbol, close_side, pos.qty,
                                        fill_price.unwrap_or(0.0),
                                        crate::models::TradeType::Day, None, None,
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
                                        "action": "DAILY_PROFIT_TARGET",
                                        "qty_sold": pos.qty,
                                        "fill_price": fill_price,
                                    }),
                                });

                                info!(
                                    symbol = %pos.symbol,
                                    fill_price,
                                    reason = "DAILY_PROFIT_TARGET",
                                    "Position flattened"
                                );
                            }
                            Err(e) => {
                                error!(symbol = %pos.symbol, "Profit target flatten failed: {e}");
                            }
                        }
                    }

                    info!(count = day_positions.len(), "Daily profit target flatten complete");
                    continue;
                }
            }
        }

        // --- Day trade stop-loss / take-profit check ---
        // Skip if within 1 minute of EOD flatten to avoid race
        let flatten_time = NaiveTime::from_hms_opt(15, 44, 0).unwrap();
        if !is_market_hours(current_time) || current_time >= flatten_time {
            continue;
        }

        let day_positions = tracker.day_positions();
        // Collect exits needed, then release the lock before submitting orders
        let mut exits: Vec<(String, f64, PositionSide, String)> = Vec::new();
        for pos in &day_positions {
            let (stop, take) = match (pos.stop_loss_price, pos.take_profit_price) {
                (Some(sl), Some(tp)) => (sl, tp),
                _ => continue,
            };

            let (hit_stop, hit_take) = match pos.side {
                PositionSide::Long => (pos.current_price <= stop, pos.current_price >= take),
                PositionSide::Short => (pos.current_price >= stop, pos.current_price <= take),
            };

            if hit_stop {
                exits.push((pos.symbol.clone(), pos.qty, pos.side.clone(), "DAY_STOP_LOSS".to_string()));
            } else if hit_take {
                exits.push((pos.symbol.clone(), pos.qty, pos.side.clone(), "DAY_TAKE_PROFIT".to_string()));
            }
        }
        drop(tracker);

        // Submit exit orders
        for (symbol, qty, side, reason) in exits {
            let close_side = match side {
                PositionSide::Long => "sell",
                PositionSide::Short => "buy",
            };

            info!(
                symbol = %symbol,
                qty,
                side = close_side,
                reason = %reason,
                "Day trade exit triggered"
            );

            match state.alpaca.submit_market_order(&symbol, qty, close_side).await {
                Ok(alpaca_order) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let order = crate::models::Order {
                        order_id: uuid::Uuid::new_v4().to_string(),
                        alpaca_id: Some(alpaca_order.id.clone()),
                        symbol: symbol.clone(),
                        side: close_side.to_string(),
                        qty,
                        filled_price: alpaca_order.filled_avg_price.as_ref().and_then(|p| p.parse::<f64>().ok()),
                        status: alpaca_order.status.clone(),
                        strategy_name: reason.clone(),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
                        trade_type: crate::models::TradeType::Day,
                    };

                    if let Ok(con) = db::connect() {
                        if let Err(e) = db::insert_order(&con, &order) {
                            error!("Failed to insert day exit order: {e}");
                        }
                    }

                    state.risk_engine.lock().await.record_order(&symbol);

                    let fill_price = crate::poll_for_fill(
                        &state, &alpaca_order.id, &order.order_id, close_side, qty, &symbol,
                    ).await;

                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(&symbol, close_side, qty, fill_price.unwrap_or(0.0), crate::models::TradeType::Day, None, None);
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
                            "action": reason,
                            "trade_type": "day",
                            "qty_closed": qty,
                            "fill_price": fill_price,
                        }),
                    });

                    info!(symbol = %symbol, fill_price, reason = %reason, "Day trade exit completed");
                }
                Err(e) => {
                    error!(symbol = %symbol, "Day trade exit order failed: {e}");
                }
            }
        }
    }
}

/// Background task: syncs symbol list from strategy engine every 5 minutes.
/// New symbols are automatically picked up without restarting.
pub async fn symbol_sync_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;

        let url = format!("{}/symbols", state.strategy_engine_url);
        let client = reqwest::Client::new();
        let resp = match client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("Symbol sync failed: {e}");
                continue;
            }
        };

        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                warn!("Symbol sync parse failed: {e}");
                continue;
            }
        };

        if let Some(arr) = data.get("symbols").and_then(|v| v.as_array()) {
            let new_syms: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s: &str| s.to_uppercase()))
                .collect();

            if new_syms.is_empty() {
                continue;
            }

            let mut current = state.symbols.lock().await;
            if *current != new_syms {
                let added: Vec<&String> = new_syms.iter().filter(|s| !current.contains(s)).collect();
                let removed: Vec<&String> = current.iter().filter(|s| !new_syms.contains(s)).collect();
                info!(
                    added = ?added,
                    removed = ?removed,
                    "Symbol list updated from strategy engine"
                );
                *current = new_syms;
            }
        }
    }
}

/// Background task: every 30 seconds, reconcile pending orders with Alpaca.
/// Orders that get stuck at pending_new after the 5-second poll timeout are
/// checked here and updated to their actual status (filled/canceled/expired).
pub async fn order_reconciliation_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let pending = match db::connect() {
            Ok(con) => match db::get_pending_orders(&con) {
                Ok(orders) => orders,
                Err(e) => {
                    warn!("Failed to query pending orders: {e}");
                    continue;
                }
            },
            Err(e) => {
                warn!("DB connect failed in order reconciliation: {e}");
                continue;
            }
        };

        if pending.is_empty() {
            continue;
        }

        info!(count = pending.len(), "Reconciling pending orders with Alpaca");

        for (order_id, alpaca_id) in &pending {
            match state.alpaca.get_order(alpaca_id).await {
                Ok(order) => {
                    let terminal = matches!(
                        order.status.as_str(),
                        "filled" | "canceled" | "expired" | "rejected"
                    );
                    if !terminal {
                        continue;
                    }

                    let fill_price = order
                        .filled_avg_price
                        .as_ref()
                        .and_then(|p| p.parse::<f64>().ok());

                    if let Ok(con) = db::connect() {
                        let _ = db::update_order_fill(
                            &con,
                            order_id,
                            &order.status,
                            fill_price,
                            order.filled_at.as_deref(),
                        );
                    }

                    if order.status == "filled" {
                        info!(
                            symbol = %order.symbol,
                            fill_price,
                            "Reconciled stale order as filled"
                        );
                    } else {
                        info!(
                            symbol = %order.symbol,
                            status = %order.status,
                            "Reconciled stale order as {}", order.status
                        );
                    }
                }
                Err(e) => {
                    warn!(alpaca_id, "Failed to check order status: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_flatten_fires_at_345pm_et_on_trading_day() {
        // Monday March 16, 2026 is a trading day
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        assert!(is_trading_day(date));

        // Before 15:45 — should not flatten
        let before = NaiveTime::from_hms_opt(15, 44, 59).unwrap();
        assert!(!should_flatten(date, before));

        // Exactly 15:45 — should flatten
        let at = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(should_flatten(date, at));

        // After 15:45 — should flatten
        let after = NaiveTime::from_hms_opt(15, 50, 0).unwrap();
        assert!(should_flatten(date, after));
    }

    #[test]
    fn test_flatten_skips_weekends() {
        // Saturday March 14, 2026
        let saturday = NaiveDate::from_ymd_opt(2026, 3, 14).unwrap();
        assert_eq!(saturday.weekday(), Weekday::Sat);
        assert!(!is_trading_day(saturday));
        let time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(!should_flatten(saturday, time));

        // Sunday March 15, 2026
        let sunday = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        assert_eq!(sunday.weekday(), Weekday::Sun);
        assert!(!is_trading_day(sunday));
        assert!(!should_flatten(sunday, time));
    }

    #[test]
    fn test_flatten_skips_nyse_holidays() {
        // Christmas 2026 is a Friday
        let christmas = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap();
        assert_eq!(christmas.weekday(), Weekday::Fri);
        assert!(!is_trading_day(christmas));
        let time = NaiveTime::from_hms_opt(15, 45, 0).unwrap();
        assert!(!should_flatten(christmas, time));
    }

    #[test]
    fn test_regular_weekday_is_trading_day() {
        // Tuesday March 17, 2026
        let tuesday = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        assert_eq!(tuesday.weekday(), Weekday::Tue);
        assert!(is_trading_day(tuesday));
    }

    #[test]
    fn test_early_morning_does_not_trigger() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let morning = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        assert!(!should_flatten(date, morning));
    }
}
