"use client";

import type { PortfolioPnlHistory } from "@/types";

interface PortfolioSummaryProps {
  summary: PortfolioPnlHistory["summary"];
}

function formatCurrency(value: number): string {
  return value.toLocaleString("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function pnlColor(value: number): string {
  if (value > 0) return "text-gain";
  if (value < 0) return "text-loss";
  return "text-text-primary";
}

export default function PortfolioSummary({ summary }: PortfolioSummaryProps) {
  const metrics = [
    {
      label: "Total Equity",
      value: formatCurrency(summary.total_equity),
      color: "text-text-primary",
    },
    {
      label: "Period P&L",
      value: `${formatCurrency(summary.period_pnl)}  (${summary.period_pnl_pct >= 0 ? "+" : ""}${summary.period_pnl_pct.toFixed(2)}%)`,
      color: pnlColor(summary.period_pnl),
    },
    {
      label: "Realized P&L",
      value: formatCurrency(summary.realized_pnl),
      color: pnlColor(summary.realized_pnl),
    },
    {
      label: "Buying Power",
      value: formatCurrency(summary.buying_power),
      color: "text-text-primary",
    },
    {
      label: "Cash",
      value: formatCurrency(summary.cash),
      color: "text-text-primary",
    },
    {
      label: "Open Positions",
      value: `${summary.day_positions} day · ${summary.swing_positions} swing`,
      color: "text-text-primary",
    },
    {
      label: "Win Rate",
      value: `${(summary.win_rate * 100).toFixed(1)}%`,
      color: "text-text-primary",
    },
  ];

  return (
    <div className="bg-navy-800 rounded-lg p-4 border border-navy-600 space-y-4">
      {metrics.map((m) => (
        <div key={m.label}>
          <p className="text-text-secondary text-xs uppercase tracking-wider">
            {m.label}
          </p>
          <p className={`text-lg font-semibold ${m.color}`}>{m.value}</p>
        </div>
      ))}
    </div>
  );
}
