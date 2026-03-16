import type { AccountInfo, Position, Order, Strategy, BacktestResult, BacktestEquityPoint } from "@/types";

const EXECUTION_URL =
  process.env.NEXT_PUBLIC_EXECUTION_URL || "http://localhost:8080";
const STRATEGY_URL =
  process.env.NEXT_PUBLIC_STRATEGY_URL || "http://localhost:8000";

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, { cache: "no-store", ...init });
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText}`);
  }
  return res.json();
}

export const executionApi = {
  getAccount: () => fetchJson<AccountInfo>(`${EXECUTION_URL}/account`),
  getPositions: () => fetchJson<Position[]>(`${EXECUTION_URL}/positions`),
  getOrders: () => fetchJson<Order[]>(`${EXECUTION_URL}/orders`),
  haltTrading: () =>
    fetchJson<{ status: string }>(`${EXECUTION_URL}/trading/halt`, {
      method: "POST",
    }),
  resumeTrading: () =>
    fetchJson<{ status: string }>(`${EXECUTION_URL}/trading/resume`, {
      method: "POST",
    }),
  sseUrl: `${EXECUTION_URL}/stream/events`,
};

export const strategyApi = {
  getStrategies: () => fetchJson<Strategy[]>(`${STRATEGY_URL}/strategies`),
  updateStrategy: (id: string, patch: Partial<Strategy>) =>
    fetchJson<Strategy>(`${STRATEGY_URL}/strategies/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    }),
  triggerBacktest: (strategy: string, symbol: string) =>
    fetchJson<BacktestResult>(`${STRATEGY_URL}/backtest/${strategy}/${symbol}`, {
      method: "POST",
    }),
  getBacktestResult: (strategy: string, symbol: string) =>
    fetchJson<BacktestResult>(`${STRATEGY_URL}/backtest/${strategy}/${symbol}`),
  getBacktestEquity: (strategy: string, symbol: string) =>
    fetchJson<BacktestEquityPoint[]>(`${STRATEGY_URL}/backtest/${strategy}/${symbol}/equity`),
};
