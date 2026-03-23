use duckdb::{params, Connection};

use crate::models::{Bar, Order, Position};

/// Open a DuckDB connection for read-write. Path comes from DUCKDB_PATH env var.
pub fn connect() -> Result<Connection, duckdb::Error> {
    let path = std::env::var("DUCKDB_PATH").unwrap_or_else(|_| "../data/algotrader.duckdb".into());
    Connection::open(&path)
}

/// Open a DuckDB connection for read-only access.
pub fn connect_readonly() -> Result<Connection, duckdb::Error> {
    let path = std::env::var("DUCKDB_PATH").unwrap_or_else(|_| "../data/algotrader.duckdb".into());
    Connection::open_with_flags(&path, duckdb::Config::default().access_mode(duckdb::AccessMode::ReadOnly)?)
}

/// Ensure all required tables exist. Called on startup so the engine
/// self-heals if the DB file is new or was recreated after corruption.
pub fn ensure_schema(con: &Connection) -> Result<(), duckdb::Error> {
    // Sequences (IF NOT EXISTS is idempotent)
    con.execute_batch(
        "CREATE SEQUENCE IF NOT EXISTS signals_id_seq START 1;
         CREATE SEQUENCE IF NOT EXISTS orders_id_seq START 1;
         CREATE SEQUENCE IF NOT EXISTS strategy_config_id_seq START 1;",
    )?;

    con.execute_batch(
        "CREATE TABLE IF NOT EXISTS ohlcv_bars (
            symbol    VARCHAR NOT NULL,
            timestamp TIMESTAMP NOT NULL,
            open      DOUBLE NOT NULL,
            high      DOUBLE NOT NULL,
            low       DOUBLE NOT NULL,
            close     DOUBLE NOT NULL,
            volume    BIGINT NOT NULL,
            bar_size  VARCHAR NOT NULL DEFAULT '5min',
            PRIMARY KEY (symbol, timestamp, bar_size)
        );
        CREATE TABLE IF NOT EXISTS signals (
            id            INTEGER PRIMARY KEY DEFAULT nextval('signals_id_seq'),
            strategy_name VARCHAR NOT NULL,
            symbol        VARCHAR NOT NULL,
            timestamp     TIMESTAMP NOT NULL,
            direction     VARCHAR NOT NULL CHECK (direction IN ('BUY','SELL','HOLD')),
            confidence    DOUBLE NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
            reason        VARCHAR,
            metadata      JSON,
            trade_type    VARCHAR NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing'))
        );
        CREATE TABLE IF NOT EXISTS orders (
            id            INTEGER PRIMARY KEY DEFAULT nextval('orders_id_seq'),
            order_id      VARCHAR NOT NULL UNIQUE,
            alpaca_id     VARCHAR,
            symbol        VARCHAR NOT NULL,
            side          VARCHAR NOT NULL CHECK (side IN ('buy','sell')),
            qty           DOUBLE NOT NULL,
            order_type    VARCHAR NOT NULL DEFAULT 'market',
            limit_price   DOUBLE,
            filled_price  DOUBLE,
            status        VARCHAR NOT NULL DEFAULT 'pending',
            strategy_name VARCHAR,
            created_at    TIMESTAMP NOT NULL DEFAULT current_timestamp,
            filled_at     TIMESTAMP,
            trade_type    VARCHAR NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing'))
        );
        CREATE TABLE IF NOT EXISTS positions (
            symbol          VARCHAR PRIMARY KEY,
            qty             DOUBLE NOT NULL,
            avg_entry_price DOUBLE NOT NULL,
            current_price   DOUBLE,
            unrealized_pnl  DOUBLE,
            updated_at      TIMESTAMP NOT NULL DEFAULT current_timestamp,
            trade_type      VARCHAR NOT NULL DEFAULT 'day' CHECK (trade_type IN ('day','swing')),
            stop_loss_price DOUBLE,
            take_profit_price DOUBLE
        );
        CREATE TABLE IF NOT EXISTS daily_pnl (
            date           DATE PRIMARY KEY,
            realized_pnl   DOUBLE NOT NULL DEFAULT 0.0,
            unrealized_pnl DOUBLE NOT NULL DEFAULT 0.0,
            total_trades   INTEGER NOT NULL DEFAULT 0,
            win_rate       DOUBLE,
            account_equity DOUBLE
        );
        CREATE TABLE IF NOT EXISTS strategy_config (
            id            INTEGER PRIMARY KEY DEFAULT nextval('strategy_config_id_seq'),
            strategy_name VARCHAR NOT NULL UNIQUE,
            params        JSON,
            enabled       BOOLEAN NOT NULL DEFAULT true,
            created_at    TIMESTAMP NOT NULL DEFAULT current_timestamp,
            updated_at    TIMESTAMP NOT NULL DEFAULT current_timestamp
        );",
    )?;
    Ok(())
}

/// Upsert an OHLCV bar into the ohlcv_bars table.
pub fn upsert_bar(con: &Connection, bar: &Bar, bar_size: &str) -> Result<(), duckdb::Error> {
    con.execute(
        "INSERT OR REPLACE INTO ohlcv_bars (symbol, timestamp, open, high, low, close, volume, bar_size) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![bar.symbol, bar.timestamp, bar.open, bar.high, bar.low, bar.close, bar.volume, bar_size],
    )?;
    Ok(())
}

/// Insert an order into the orders table.
pub fn insert_order(con: &Connection, order: &Order) -> Result<(), duckdb::Error> {
    con.execute(
        "INSERT INTO orders (order_id, alpaca_id, symbol, side, qty, filled_price, status, strategy_name, created_at, filled_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            order.order_id,
            order.alpaca_id,
            order.symbol,
            order.side,
            order.qty,
            order.filled_price,
            order.status,
            order.strategy_name,
            order.created_at,
            order.filled_at,
        ],
    )?;
    Ok(())
}

/// Update an order's status, fill price, and fill time.
pub fn update_order_fill(
    con: &Connection,
    order_id: &str,
    status: &str,
    filled_price: Option<f64>,
    filled_at: Option<&str>,
) -> Result<(), duckdb::Error> {
    con.execute(
        "UPDATE orders SET status = ?, filled_price = ?, filled_at = ? WHERE order_id = ?",
        params![status, filled_price, filled_at, order_id],
    )?;
    Ok(())
}

/// Upsert a position in the positions table.
pub fn upsert_position(con: &Connection, pos: &Position) -> Result<(), duckdb::Error> {
    let trade_type_str = match pos.trade_type {
        crate::models::TradeType::Day => "day",
        crate::models::TradeType::Swing => "swing",
    };
    con.execute(
        "INSERT OR REPLACE INTO positions (symbol, qty, avg_entry_price, current_price, unrealized_pnl, updated_at, trade_type, stop_loss_price, take_profit_price) \
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?)",
        params![pos.symbol, pos.qty, pos.avg_entry_price, pos.current_price, pos.unrealized_pnl, trade_type_str, pos.stop_loss_price, pos.take_profit_price],
    )?;
    Ok(())
}

/// Remove a position (when fully closed).
pub fn delete_position(con: &Connection, symbol: &str) -> Result<(), duckdb::Error> {
    con.execute("DELETE FROM positions WHERE symbol = ?", params![symbol])?;
    Ok(())
}

/// Load all positions from DuckDB.
pub fn load_positions(con: &Connection) -> Result<Vec<Position>, duckdb::Error> {
    let mut stmt = con.prepare(
        "SELECT symbol, qty, avg_entry_price, current_price, unrealized_pnl, \
         COALESCE(trade_type, 'day'), stop_loss_price, take_profit_price FROM positions",
    )?;
    let rows = stmt.query_map([], |row| {
        let tt_str: String = row.get(5)?;
        let trade_type = if tt_str == "swing" {
            crate::models::TradeType::Swing
        } else {
            crate::models::TradeType::Day
        };
        Ok(Position {
            symbol: row.get(0)?,
            qty: row.get(1)?,
            avg_entry_price: row.get(2)?,
            current_price: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            unrealized_pnl: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
            trade_type,
            stop_loss_price: row.get(6)?,
            take_profit_price: row.get(7)?,
        })
    })?;
    let mut positions = Vec::new();
    for row in rows {
        positions.push(row?);
    }
    Ok(positions)
}

/// Load recent orders from DuckDB.
pub fn load_orders(con: &Connection, limit: usize) -> Result<Vec<Order>, duckdb::Error> {
    let sql = format!(
        "SELECT order_id, COALESCE(alpaca_id, ''), symbol, side, qty, filled_price, status, \
         COALESCE(strategy_name, ''), CAST(created_at AS VARCHAR), CAST(filled_at AS VARCHAR), \
         COALESCE(trade_type, 'day') \
         FROM orders ORDER BY created_at DESC LIMIT {}",
        limit
    );
    let mut stmt = con.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let alpaca_id: String = row.get(1)?;
        let strategy_name: String = row.get(7)?;
        let tt_str: String = row.get(10)?;
        let trade_type = if tt_str == "swing" {
            crate::models::TradeType::Swing
        } else {
            crate::models::TradeType::Day
        };
        Ok(Order {
            order_id: row.get(0)?,
            alpaca_id: if alpaca_id.is_empty() { None } else { Some(alpaca_id) },
            symbol: row.get(2)?,
            side: row.get(3)?,
            qty: row.get(4)?,
            filled_price: row.get(5)?,
            status: row.get(6)?,
            strategy_name: if strategy_name.is_empty() { String::new() } else { strategy_name },
            created_at: row.get(8)?,
            filled_at: row.get(9)?,
            trade_type,
        })
    })?;
    let mut orders = Vec::new();
    for row in rows {
        orders.push(row?);
    }
    Ok(orders)
}

/// Fetch recent bars for a symbol from DuckDB (for sending to strategy engine).
pub fn get_recent_bars(
    con: &Connection,
    symbol: &str,
    bar_size: &str,
    limit: usize,
) -> Result<Vec<Bar>, duckdb::Error> {
    let mut stmt = con.prepare(
        "SELECT symbol, CAST(timestamp AS VARCHAR), open, high, low, close, volume \
         FROM ohlcv_bars WHERE symbol = ? AND bar_size = ? \
         ORDER BY timestamp DESC LIMIT ?",
    )?;
    let rows = stmt.query_map(params![symbol, bar_size, limit as i64], |row| {
        Ok(Bar {
            symbol: row.get(0)?,
            timestamp: row.get(1)?,
            open: row.get(2)?,
            high: row.get(3)?,
            low: row.get(4)?,
            close: row.get(5)?,
            volume: row.get(6)?,
        })
    })?;
    let mut bars: Vec<Bar> = Vec::new();
    for row in rows {
        bars.push(row?);
    }
    // Reverse so oldest is first (ascending order)
    bars.reverse();
    Ok(bars)
}
