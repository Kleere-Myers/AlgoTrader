use tracing::{error, info};

use crate::alpaca::{AlpacaClient, AlpacaError};
use crate::models::AlpacaOrder;

impl AlpacaClient {
    /// Submit a market order to Alpaca. Returns the Alpaca order response.
    pub async fn submit_market_order(
        &self,
        symbol: &str,
        qty: f64,
        side: &str,
    ) -> Result<AlpacaOrder, AlpacaError> {
        let url = format!("{}/v2/orders", self.config.base_url());

        let body = serde_json::json!({
            "symbol": symbol,
            "qty": qty.to_string(),
            "side": side,
            "type": "market",
            "time_in_force": "day",
        });

        info!(symbol, qty, side, "Submitting market order to Alpaca");

        let resp = self
            .http
            .post(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .json(&body)
            .send()
            .await
            .map_err(AlpacaError::Network)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            error!(status = status.as_u16(), body, "Order submission failed");
            return Err(AlpacaError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let order = resp
            .json::<AlpacaOrder>()
            .await
            .map_err(AlpacaError::Deserialize)?;

        info!(
            alpaca_id = %order.id,
            status = %order.status,
            "Order submitted successfully"
        );

        Ok(order)
    }

    /// Fetch recent daily bars for a symbol from Alpaca market data API.
    pub async fn get_daily_bars(
        &self,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<crate::models::Bar>, AlpacaError> {
        let url = format!(
            "https://data.alpaca.markets/v2/stocks/{}/bars?timeframe=1Day&limit={}",
            symbol, limit
        );

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
            return Err(AlpacaError::Api { status: status.as_u16(), body });
        }

        let data: serde_json::Value = resp.json().await.map_err(AlpacaError::Deserialize)?;
        let bars_json = data.get("bars").and_then(|b| b.as_array());
        let mut bars = Vec::new();
        if let Some(arr) = bars_json {
            for b in arr {
                bars.push(crate::models::Bar {
                    symbol: symbol.to_string(),
                    timestamp: b.get("t").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    open: b.get("o").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    high: b.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    low: b.get("l").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    close: b.get("c").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    volume: b.get("v").and_then(|v| v.as_i64()).unwrap_or(0),
                });
            }
        }
        Ok(bars)
    }

    /// Fetch a single order by Alpaca ID to check fill status.
    pub async fn get_order(&self, alpaca_id: &str) -> Result<AlpacaOrder, AlpacaError> {
        let url = format!("{}/v2/orders/{}", self.config.base_url(), alpaca_id);

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

        resp.json::<AlpacaOrder>()
            .await
            .map_err(AlpacaError::Deserialize)
    }
}
