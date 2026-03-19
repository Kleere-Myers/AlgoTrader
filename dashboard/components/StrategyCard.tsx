"use client";

import { useState } from "react";
import type { Strategy } from "@/types";
import { strategyApi } from "@/lib/api";
import Tip from "@/components/Tip";

const STRATEGY_TOOLTIPS: Record<string, string> = {
  MovingAverageCrossover: "Follows the trend by comparing a fast moving average to a slow one. Buys when the short-term average crosses above the long-term average.",
  RSIMeanReversion: "Looks for stocks pushed too far in one direction using the Relative Strength Index. Buys oversold stocks, sells overbought ones.",
  MomentumVolume: "Watches for price breakouts above recent highs confirmed by a spike in trading volume. High volume breakouts tend to continue.",
  MLSignalGenerator: "Uses a machine learning model trained on dozens of indicators to predict price direction. Only acts when confidence is high.",
  VWAPStrategy: "Compares price to the Volume Weighted Average Price. Buys when price dips below VWAP (cheap vs average), sells when above.",
  OpeningRangeBreakout: "Tracks the high and low of the first N bars as an opening range. Buys on breakout above the range, sells on breakdown below.",
  NewsSentimentStrategy: "Analyzes recent news headlines using FinBERT AI to detect bullish or bearish sentiment. Buys on strongly positive news, sells on strongly negative.",
};

function confidenceBadge(confidence: number) {
  const pct = (confidence * 100).toFixed(0);
  if (confidence >= 0.7) return <span className="text-xs px-2 py-0.5 rounded bg-gain/15 text-gain">{pct}%</span>;
  if (confidence >= 0.4) return <span className="text-xs px-2 py-0.5 rounded bg-yellow-500/15 text-yellow-500">{pct}%</span>;
  return <span className="text-xs px-2 py-0.5 rounded bg-loss/15 text-loss">{pct}%</span>;
}

function winRateBadge(winRate: number | null) {
  if (winRate === null) return <span className="text-xs px-2 py-0.5 rounded bg-navy-600 text-text-secondary">N/A</span>;
  const pct = (winRate * 100).toFixed(1);
  if (winRate > 0.55) return <span className="text-xs px-2 py-0.5 rounded bg-gain/15 text-gain">{pct}%</span>;
  if (winRate >= 0.45) return <span className="text-xs px-2 py-0.5 rounded bg-yellow-500/15 text-yellow-500">{pct}%</span>;
  return <span className="text-xs px-2 py-0.5 rounded bg-loss/15 text-loss">{pct}%</span>;
}

function directionBadge(direction: string) {
  const colors: Record<string, string> = {
    BUY: "bg-gain/15 text-gain",
    SELL: "bg-loss/15 text-loss",
    HOLD: "bg-navy-600 text-text-secondary",
  };
  return (
    <span className={`text-xs px-2 py-0.5 rounded font-medium ${colors[direction] || "bg-navy-600 text-text-secondary"}`}>
      {direction}
    </span>
  );
}

interface StrategyCardProps {
  strategy: Strategy;
  symbols: string[];
  onUpdate: () => void;
  onBacktestComplete: () => void;
}

export default function StrategyCard({ strategy, symbols, onUpdate, onBacktestComplete }: StrategyCardProps) {
  const [paramsExpanded, setParamsExpanded] = useState(false);
  const [editedParams, setEditedParams] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [backtesting, setBacktesting] = useState(false);
  const [backtestProgress, setBacktestProgress] = useState(0);

  const handleToggle = async () => {
    setSaving(true);
    try {
      await strategyApi.updateStrategy(strategy.id, { enabled: !strategy.enabled });
      onUpdate();
    } catch (e) {
      console.error("Failed to toggle strategy:", e);
    } finally {
      setSaving(false);
    }
  };

  const handleParamSave = async () => {
    if (Object.keys(editedParams).length === 0) return;
    setSaving(true);
    try {
      const newParams = { ...strategy.params };
      for (const [key, val] of Object.entries(editedParams)) {
        const num = Number(val);
        newParams[key] = isNaN(num) ? val : num;
      }
      await strategyApi.updateStrategy(strategy.id, { params: newParams });
      setEditedParams({});
      onUpdate();
    } catch (e) {
      console.error("Failed to save params:", e);
    } finally {
      setSaving(false);
    }
  };

  const handleBacktest = async () => {
    setBacktesting(true);
    setBacktestProgress(0);
    try {
      for (let i = 0; i < symbols.length; i++) {
        await strategyApi.triggerBacktest(strategy.name, symbols[i]);
        setBacktestProgress(i + 1);
      }
      onBacktestComplete();
    } catch (e) {
      console.error("Backtest failed:", e);
    } finally {
      setBacktesting(false);
    }
  };

  const hasEdits = Object.keys(editedParams).length > 0;

  return (
    <div className="rounded-lg border border-navy-600 bg-navy-900 p-5">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <h3 className="font-semibold text-sm text-text-primary">
          {strategy.name}
          {STRATEGY_TOOLTIPS[strategy.name] && (
            <Tip text={STRATEGY_TOOLTIPS[strategy.name]} inline />
          )}
        </h3>
        <button
          onClick={handleToggle}
          disabled={saving}
          className={`relative w-10 h-5 rounded-full transition-colors ${
            strategy.enabled ? "bg-accent-purple" : "bg-navy-600"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform ${
              strategy.enabled ? "translate-x-5" : ""
            }`}
          />
        </button>
      </div>

      {/* Last signal + win rate */}
      <div className="flex items-center gap-2 mb-3 flex-wrap">
        {strategy.last_signal ? (
          <>
            {directionBadge(strategy.last_signal.direction)}
            {confidenceBadge(strategy.last_signal.confidence)}
            <span className="text-xs text-text-secondary">{strategy.last_signal.symbol}</span>
          </>
        ) : (
          <span className="text-xs text-text-secondary">No signals yet</span>
        )}
        <span className="text-xs text-text-secondary ml-auto">Win rate:</span>
        {winRateBadge(strategy.win_rate)}
      </div>

      {/* Params */}
      <div className="mb-3">
        <button
          onClick={() => setParamsExpanded(!paramsExpanded)}
          className="text-xs text-accent-purple hover:text-accent-purple-light"
        >
          {paramsExpanded ? "Hide Params" : "Edit Params"}
        </button>
        {paramsExpanded && (
          <div className="mt-2 space-y-1.5">
            {Object.entries(strategy.params).map(([key, value]) => (
              <div key={key} className="flex items-center gap-2">
                <label className="text-xs text-text-secondary w-32 text-right">{key}</label>
                <input
                  type="text"
                  defaultValue={String(value)}
                  onChange={(e) =>
                    setEditedParams((prev) => ({ ...prev, [key]: e.target.value }))
                  }
                  className="text-xs border border-navy-600 bg-navy-800 text-text-primary rounded px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-accent-blue"
                />
              </div>
            ))}
            {hasEdits && (
              <button
                onClick={handleParamSave}
                disabled={saving}
                className="text-xs px-3 py-1 mt-1 rounded bg-accent-purple text-white hover:bg-accent-purple-dark disabled:opacity-50"
              >
                {saving ? "Saving..." : "Save Changes"}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Backtest button */}
      <button
        onClick={handleBacktest}
        disabled={backtesting}
        className="text-xs px-3 py-1.5 rounded bg-accent-purple/15 text-accent-purple-light hover:bg-accent-purple/25 disabled:opacity-50 w-full"
      >
        {backtesting
          ? `Running backtests... (${backtestProgress}/${symbols.length})`
          : "Run Backtest"}
      </button>
    </div>
  );
}
