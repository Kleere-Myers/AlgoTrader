"use client";

import { useEffect, useState, useCallback } from "react";
import type { BacktestResult, Strategy } from "@/types";
import type { BacktestEquityPoint } from "@/types";
import { strategyApi } from "@/lib/api";
import EquityCurveChart from "@/components/EquityCurveChart";
import Tip from "@/components/Tip";
import { useSymbols } from "@/hooks/useSymbols";


export default function BacktestPage() {
  const { symbols: SYMBOLS } = useSymbols();
  const [strategies, setStrategies] = useState<Strategy[]>([]);
  const [results, setResults] = useState<BacktestResult[]>([]);
  const [equityData, setEquityData] = useState<Record<string, BacktestEquityPoint[]>>({});
  const [selectedStrategy, setSelectedStrategy] = useState<string>("all");
  const [selectedSymbol, setSelectedSymbol] = useState<string>("all");
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const strats = await strategyApi.getStrategies();
      setStrategies(strats);

      const allResults: BacktestResult[] = [];
      const allEquity: Record<string, BacktestEquityPoint[]> = {};

      const promises = strats.flatMap((s) =>
        SYMBOLS.map(async (sym) => {
          try {
            const result = await strategyApi.getBacktestResult(s.name, sym);
            allResults.push(result);
            try {
              const equity = await strategyApi.getBacktestEquity(s.name, sym);
              allEquity[`${s.name}:${sym}`] = equity;
            } catch {
              // equity endpoint may not exist for all combos
            }
          } catch {
            // no result for this combo — skip
          }
        })
      );

      await Promise.all(promises);
      setResults(allResults);
      setEquityData(allEquity);
    } catch (e) {
      console.error("Failed to fetch backtest data:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const strategyNames = [...new Set(results.map((r) => r.strategy_name))];

  const filtered = results.filter((r) => {
    if (selectedStrategy !== "all" && r.strategy_name !== selectedStrategy) return false;
    if (selectedSymbol !== "all" && r.symbol !== selectedSymbol) return false;
    return true;
  });

  const filteredEquityKeys = Object.keys(equityData).filter((key) => {
    const [strat, sym] = key.split(":");
    if (selectedStrategy !== "all" && strat !== selectedStrategy) return false;
    if (selectedSymbol !== "all" && sym !== selectedSymbol) return false;
    return true;
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-2xl font-bold text-text-primary">Backtest Results</h2>
          <p className="text-text-secondary text-sm mt-1">
            Equity curves, metrics, and performance comparison per strategy and symbol.
          </p>
        </div>
        <button
          onClick={fetchData}
          disabled={loading}
          className="text-xs px-3 py-1.5 rounded border border-navy-600 hover:bg-navy-700 text-text-secondary disabled:opacity-50"
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      {/* Filters */}
      <div className="flex gap-3 mb-6">
        <div>
          <label className="text-xs text-text-secondary block mb-1">Strategy</label>
          <select
            value={selectedStrategy}
            onChange={(e) => setSelectedStrategy(e.target.value)}
            className="text-sm border border-navy-600 bg-navy-900 text-text-primary rounded px-2 py-1.5 focus:outline-none focus:ring-1 focus:ring-accent-blue"
          >
            <option value="all">All Strategies</option>
            {strategyNames.map((name) => (
              <option key={name} value={name}>{name}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="text-xs text-text-secondary block mb-1">Symbol</label>
          <select
            value={selectedSymbol}
            onChange={(e) => setSelectedSymbol(e.target.value)}
            className="text-sm border border-navy-600 bg-navy-900 text-text-primary rounded px-2 py-1.5 focus:outline-none focus:ring-1 focus:ring-accent-blue"
          >
            <option value="all">All Symbols</option>
            {SYMBOLS.map((sym) => (
              <option key={sym} value={sym}>{sym}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Equity curves */}
      {filteredEquityKeys.length > 0 && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
          {filteredEquityKeys.map((key) => (
            <div key={key} className="bg-navy-900 rounded-lg border border-navy-600 p-4">
              <EquityCurveChart
                data={equityData[key]}
                label={key.replace(":", " \u2014 ")}
              />
            </div>
          ))}
        </div>
      )}

      {/* Metrics table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-navy-600 bg-navy-900 rounded-lg">
          <thead className="bg-navy-800 text-text-secondary uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Strategy</th>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3 text-right">Return % <Tip text="Total percentage gain or loss over the backtest period. Green is profit, red is loss." inline /></th>
              <th className="px-4 py-3 text-right">Sharpe <Tip text="Measures return vs risk. Above 1.0 is decent, above 2.0 is very good. Higher means better risk-adjusted performance." inline /></th>
              <th className="px-4 py-3 text-right">Max DD % <Tip text="Maximum drawdown \u2014 the biggest peak-to-valley drop. Shows the worst-case scenario you would have experienced." inline /></th>
              <th className="px-4 py-3 text-right">Win Rate <Tip text="Percentage of trades that made money. Even 50-55% can be profitable if winners are bigger than losers." inline /></th>
              <th className="px-4 py-3 text-right">Trades <Tip text="Total number of buy+sell trades executed during the backtest period." inline /></th>
              <th className="px-4 py-3 text-right">Avg Duration <Tip text="How long the average trade was held before selling." inline /></th>
              <th className="px-4 py-3 text-right">Profit Factor <Tip text="Total profits divided by total losses. Above 1.0 means profitable overall. Above 1.5 is good." inline /></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-navy-600">
            {loading ? (
              <tr>
                <td className="px-4 py-8 text-center text-text-secondary" colSpan={9}>
                  Loading backtest results...
                </td>
              </tr>
            ) : filtered.length === 0 ? (
              <tr>
                <td className="px-4 py-8 text-center text-text-secondary" colSpan={9}>
                  No backtest results yet — run a backtest from the Strategies page
                </td>
              </tr>
            ) : (
              filtered.map((r) => (
                <tr key={`${r.strategy_name}-${r.symbol}`} className="hover:bg-navy-800">
                  <td className="px-4 py-2.5 font-medium text-text-primary">{r.strategy_name}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{r.symbol}</td>
                  <td className={`px-4 py-2.5 text-right ${r.total_return_pct >= 0 ? "text-gain" : "text-loss"}`}>
                    {r.total_return_pct >= 0 ? "+" : ""}{r.total_return_pct.toFixed(2)}%
                  </td>
                  <td className={`px-4 py-2.5 text-right ${r.sharpe_ratio >= 1 ? "text-gain" : r.sharpe_ratio >= 0 ? "text-text-secondary" : "text-loss"}`}>
                    {r.sharpe_ratio.toFixed(2)}
                  </td>
                  <td className="px-4 py-2.5 text-right text-loss">
                    {r.max_drawdown_pct.toFixed(2)}%
                  </td>
                  <td className={`px-4 py-2.5 text-right ${r.win_rate > 0.55 ? "text-gain" : r.win_rate >= 0.45 ? "text-yellow-500" : "text-loss"}`}>
                    {(r.win_rate * 100).toFixed(1)}%
                  </td>
                  <td className="px-4 py-2.5 text-right text-text-secondary">{r.total_trades}</td>
                  <td className="px-4 py-2.5 text-right text-text-secondary">{formatDuration(r.avg_trade_duration_mins)}</td>
                  <td className={`px-4 py-2.5 text-right ${r.profit_factor >= 1.5 ? "text-gain" : r.profit_factor >= 1 ? "text-text-secondary" : "text-loss"}`}>
                    {r.profit_factor.toFixed(2)}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function formatDuration(mins: number): string {
  if (mins < 60) return `${Math.round(mins)}m`;
  const h = Math.floor(mins / 60);
  const m = Math.round(mins % 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}
