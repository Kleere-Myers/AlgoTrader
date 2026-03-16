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
