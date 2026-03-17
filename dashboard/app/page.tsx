"use client";

import { useEffect, useState } from "react";
import { executionApi, strategyApi } from "@/lib/api";
import { useSseEvents } from "@/hooks/useSseEvents";
import type { AccountInfo, Position, OhlcvBar, Signal } from "@/types";
import CandlestickChart from "@/components/CandlestickChart";
import Tip from "@/components/Tip";
import { useSymbols } from "@/hooks/useSymbols";

export default function OverviewPage() {
  const { symbols: SYMBOLS, addSymbol, removeSymbol, error: symbolError } = useSymbols();
  const [newSymbol, setNewSymbol] = useState("");
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [positions, setPositions] = useState<Position[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [bars, setBars] = useState<OhlcvBar[]>([]);
  const [signals, setSignals] = useState<Signal[]>([]);
  const [chartSymbol, setChartSymbol] = useState("SPY");
  const { events, isConnected, isTradingHalted } = useSseEvents();

  useEffect(() => {
    const load = async () => {
      try {
        const [acct, pos] = await Promise.all([
          executionApi.getAccount(),
          executionApi.getPositions(),
        ]);
        setAccount(acct);
        setPositions(pos);
      } catch (e: any) {
        setError(e.message);
      }
    };
    load();
    const interval = setInterval(load, 10_000);
    return () => clearInterval(interval);
  }, []);

  // Fetch bars for selected chart symbol
  useEffect(() => {
    strategyApi.getBars(chartSymbol).then(setBars).catch(() => setBars([]));
  }, [chartSymbol]);

  // Fetch signals from strategies
  useEffect(() => {
    strategyApi.getStrategies().then((strats) => {
      const sigs = strats
        .map((s) => s.last_signal)
        .filter((s): s is Signal => s != null);
      setSignals(sigs);
    }).catch(() => {});
  }, []);

  // Refresh positions on OrderFill events
  useEffect(() => {
    const fills = events.filter(
      (e) => e.event_type === "ORDER_FILL"
    );
    if (fills.length > 0) {
      executionApi.getPositions().then(setPositions).catch(() => {});
      executionApi.getAccount().then(setAccount).catch(() => {});
    }
  }, [events]);

  return (
    <div>
      {isTradingHalted && (
        <div className="mb-4 rounded-lg bg-red-100 border border-red-500 px-4 py-2 text-red-800 text-sm font-semibold">
          TRADING HALTED
        </div>
      )}

      <h2 className="text-2xl font-bold mb-4">Overview</h2>

      {error && (
        <p className="text-red-500 text-sm mb-4">
          Failed to load: {error} — is the execution engine running?
        </p>
      )}

      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
        <StatCard label="Equity" tip="The total value of your account — cash plus all stocks you currently hold." value={account ? `$${account.equity.toLocaleString()}` : "--"} />
        <StatCard label="Buying Power" tip="How much money you have available to buy stocks right now." value={account ? `$${account.buying_power.toLocaleString()}` : "--"} />
        <StatCard label="Cash" tip="Money in your account not currently invested in any stock." value={account ? `$${account.cash.toLocaleString()}` : "--"} />
        <StatCard
          label="SSE Stream"
          tip="Real-time connection to the trading engine. When connected, the dashboard updates automatically as trades happen."
          value={isConnected ? "Connected" : "Disconnected"}
          color={isConnected ? "text-green-600" : "text-gray-400"}
        />
      </div>

      {/* Symbol management */}
      <div className="mb-6 rounded-lg border border-gray-200 bg-white p-4 shadow-sm">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-lg font-semibold">
            Symbols
            <Tip text="The stocks and ETFs that AlgoTrader monitors. Add or remove tickers here. Changes take effect immediately for the strategy engine." inline />
          </h3>
          <form
            className="flex gap-2"
            onSubmit={async (e) => {
              e.preventDefault();
              const sym = newSymbol.trim().toUpperCase();
              if (!sym) return;
              try {
                await addSymbol(sym);
                setNewSymbol("");
              } catch {}
            }}
          >
            <input
              type="text"
              value={newSymbol}
              onChange={(e) => setNewSymbol(e.target.value)}
              placeholder="e.g. AMZN"
              className="text-sm border border-gray-200 rounded px-2 py-1.5 w-28 focus:outline-none focus:ring-1 focus:ring-blue-400"
            />
            <button
              type="submit"
              className="text-xs px-3 py-1.5 rounded bg-blue-600 text-white hover:bg-blue-700"
            >
              Add
            </button>
          </form>
        </div>
        {symbolError && (
          <p className="text-red-500 text-xs mb-2">{symbolError}</p>
        )}
        <div className="flex flex-wrap gap-2">
          {SYMBOLS.map((sym) => (
            <span
              key={sym}
              className="inline-flex items-center gap-1 text-sm px-2.5 py-1 rounded-full bg-gray-100 text-gray-700"
            >
              {sym}
              <button
                onClick={async () => {
                  try { await removeSymbol(sym); } catch {}
                }}
                className="text-gray-400 hover:text-red-500 text-xs ml-0.5"
                title={`Remove ${sym}`}
              >
                &times;
              </button>
            </span>
          ))}
        </div>
      </div>

      {/* Candlestick chart */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-lg font-semibold">Price Chart</h3>
          <select
            value={chartSymbol}
            onChange={(e) => setChartSymbol(e.target.value)}
            className="text-sm border border-gray-200 rounded px-2 py-1.5 focus:outline-none focus:ring-1 focus:ring-blue-400"
          >
            {SYMBOLS.map((sym) => (
              <option key={sym} value={sym}>{sym}</option>
            ))}
          </select>
        </div>
        <div className="bg-white rounded-lg border border-gray-200 p-4">
          <CandlestickChart bars={bars} signals={signals} symbol={chartSymbol} />
        </div>
      </div>

      <h3 className="text-lg font-semibold mb-3">Open Positions</h3>
      {positions.length === 0 ? (
        <p className="text-gray-400 text-sm">No open positions</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
            <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
              <tr>
                <th className="px-4 py-3">Symbol</th>
                <th className="px-4 py-3">Qty</th>
                <th className="px-4 py-3">Avg Entry</th>
                <th className="px-4 py-3">Current</th>
                <th className="px-4 py-3">Unrealized P&amp;L</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {positions.map((p) => (
                <tr key={p.symbol}>
                  <td className="px-4 py-3 font-medium">{p.symbol}</td>
                  <td className="px-4 py-3">{p.qty}</td>
                  <td className="px-4 py-3">${p.avg_entry_price.toFixed(2)}</td>
                  <td className="px-4 py-3">${p.current_price.toFixed(2)}</td>
                  <td
                    className={`px-4 py-3 font-medium ${
                      p.unrealized_pnl >= 0 ? "text-green-600" : "text-red-600"
                    }`}
                  >
                    ${p.unrealized_pnl.toFixed(2)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h3 className="text-lg font-semibold mt-6 mb-3">Recent Events</h3>
      <div className="rounded-lg border border-gray-200 bg-white p-4 h-48 overflow-y-auto font-mono text-xs">
        {events.length === 0 ? (
          <span className="text-gray-400">Waiting for events...</span>
        ) : (
          events.slice(0, 50).map((e, i) => (
            <div key={i} className="mb-1">
              <span className="text-gray-400">{e.timestamp}</span>{" "}
              <span className="font-semibold">{e.event_type}</span>{" "}
              <span className="text-gray-500">{JSON.stringify(e.payload)}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  color,
  tip,
}: {
  label: string;
  value: string;
  color?: string;
  tip?: string;
}) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 shadow-sm">
      <p className="text-xs text-gray-400 uppercase tracking-wide">
        {label}
        {tip && <Tip text={tip} inline />}
      </p>
      <p className={`mt-1 text-2xl font-semibold ${color || "text-gray-900"}`}>
        {value}
      </p>
    </div>
  );
}
