export type Direction = "BUY" | "SELL" | "HOLD";

export interface Signal {
  symbol: string;
  direction: Direction;
  confidence: number;
  reason: string;
  strategy_name: string;
  timestamp: string;
}

export interface Position {
  symbol: string;
  qty: number;
  avg_entry_price: number;
  current_price: number;
  unrealized_pnl: number;
  opened_at: string;
}

export interface Order {
  order_id: string;
  alpaca_id: string | null;
  symbol: string;
  side: "buy" | "sell";
  qty: number;
  filled_price: number | null;
  status: string;
  strategy_name: string;
  submitted_at: string;
  filled_at: string | null;
}

export type SseEventType =
  | "PositionUpdate"
  | "OrderFill"
  | "TradingHalted"
  | "TradingResumed"
  | "DailyPnl"
  | "RiskBreach";

export interface SseEvent {
  event_type: SseEventType;
  timestamp: string;
  payload: Record<string, unknown>;
}

export interface AccountInfo {
  equity: number;
  buying_power: number;
  daily_pnl: number;
  trading_halted: boolean;
  mode: "paper" | "live";
}

export interface Strategy {
  id: string;
  name: string;
  enabled: boolean;
  params: Record<string, unknown>;
  last_signal: Signal | null;
  win_rate: number | null;
}

export interface BacktestResult {
  strategy_name: string;
  symbol: string;
  total_return_pct: number;
  sharpe_ratio: number;
  max_drawdown_pct: number;
  win_rate: number;
  total_trades: number;
  avg_trade_duration_mins: number;
  profit_factor: number;
  period_start: string;
  period_end: string;
}
