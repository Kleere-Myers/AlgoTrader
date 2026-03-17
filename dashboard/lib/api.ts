import type { AccountInfo, Position, Order, Strategy, BacktestResult, BacktestEquityPoint, OhlcvBar, RiskConfig, CompanyInfo, NewsArticle } from "@/types";

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
  getRiskConfig: () => fetchJson<RiskConfig>(`${EXECUTION_URL}/risk/config`),
  patchRiskConfig: async (patch: Partial<RiskConfig>): Promise<{ data?: RiskConfig; error?: string }> => {
    const res = await fetch(`${EXECUTION_URL}/risk/config`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
      cache: "no-store",
    });
    const body = await res.json();
    if (!res.ok) {
      return { error: body.error || `${res.status} ${res.statusText}` };
    }
    return { data: body };
  },
  sseUrl: `${EXECUTION_URL}/stream/events`,
};

export const strategyApi = {
  getSymbols: () => fetchJson<{ symbols: string[] }>(`${STRATEGY_URL}/symbols`),
  addSymbol: (symbol: string) =>
    fetchJson<{ symbols: string[] }>(`${STRATEGY_URL}/symbols`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ symbol }),
    }),
  removeSymbol: (symbol: string) =>
    fetchJson<{ symbols: string[] }>(`${STRATEGY_URL}/symbols/${symbol}`, {
      method: "DELETE",
    }),
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
  getBars: (symbol: string) =>
    fetchJson<OhlcvBar[]>(`${STRATEGY_URL}/bars/${symbol}`),
  getCompanyInfo: (symbol: string) =>
    fetchJson<CompanyInfo>(`${STRATEGY_URL}/company/${symbol}`),
  getNews: (symbol: string) =>
    fetchJson<{ symbol: string; articles: NewsArticle[] }>(`${STRATEGY_URL}/news/${symbol}`),
};
