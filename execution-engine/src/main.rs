mod alpaca;
mod models;

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use tracing::{error, info};

use alpaca::{AlpacaClient, AlpacaConfig};
use models::AccountSummary;

/// Shared application state available to all Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub alpaca: AlpacaClient,
}

#[tokio::main]
async fn main() {
    // 1. Load .env (walk up to project root if running from execution-engine/)
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

    // --check-auth: print summary and exit without starting the server
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

    // 5. Build shared state
    let state = Arc::new(AppState {
        alpaca: alpaca.clone(),
    });

    // 6. Build Axum router
    let app = Router::new()
        .route("/health", get(health))
        .route("/account", get(get_account))
        .with_state(state);

    // 7. Start server
    let addr = "0.0.0.0:8080";
    info!("execution-engine listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

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
