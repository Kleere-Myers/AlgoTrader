use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveTime, Utc, Weekday};
use chrono_tz::America::New_York;
use tracing::{error, info};

use crate::db;
use crate::models::{Order, SseEvent, SseEventType};
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

        // Get all open positions
        let positions = {
            let tracker = state.positions.lock().await;
            tracker.all()
        };

        if positions.is_empty() {
            info!("No open positions to flatten");
            continue;
        }

        for pos in &positions {
            info!(
                symbol = %pos.symbol,
                qty = pos.qty,
                reason = "EOD_AUTO_FLATTEN",
                "Flattening position"
            );

            let qty = pos.qty;
            let symbol = &pos.symbol;

            match state.alpaca.submit_market_order(symbol, qty, "sell").await {
                Ok(alpaca_order) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let order = Order {
                        order_id: uuid::Uuid::new_v4().to_string(),
                        alpaca_id: Some(alpaca_order.id.clone()),
                        symbol: symbol.clone(),
                        side: "sell".to_string(),
                        qty,
                        filled_price: alpaca_order
                            .filled_avg_price
                            .as_ref()
                            .and_then(|p| p.parse::<f64>().ok()),
                        status: alpaca_order.status.clone(),
                        strategy_name: "EOD_AUTO_FLATTEN".to_string(),
                        created_at: now,
                        filled_at: alpaca_order.filled_at.clone(),
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
                        "sell",
                        qty,
                        symbol,
                    )
                    .await;

                    // Update position tracker
                    {
                        let mut tracker = state.positions.lock().await;
                        let updated = tracker.update_on_fill(symbol, "sell", qty, fill_price.unwrap_or(0.0));
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
                    error!(
                        symbol,
                        reason = "EOD_AUTO_FLATTEN",
                        "Failed to submit flatten order: {e}"
                    );
                }
            }
        }

        info!(
            count = positions.len(),
            "EOD auto-flatten complete"
        );
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
