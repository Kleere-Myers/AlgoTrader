"use client";

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import type { BacktestEquityPoint } from "@/types";

interface EquityCurveChartProps {
  data: BacktestEquityPoint[];
  label: string;
}

export default function EquityCurveChart({ data, label }: EquityCurveChartProps) {
  if (data.length === 0) {
    return (
      <div className="rounded border border-dashed border-surface-600 p-12 text-center text-text-secondary">
        No equity data available
      </div>
    );
  }

  const startEquity = data[0].equity;

  const formatTime = (ts: string) => {
    const d = new Date(ts);
    return `${(d.getMonth() + 1).toString().padStart(2, "0")}/${d.getDate().toString().padStart(2, "0")} ${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
  };

  return (
    <div>
      <h4 className="text-sm font-medium text-text-secondary mb-2">{label}</h4>
      <ResponsiveContainer width="100%" height={300}>
        <LineChart data={data} margin={{ top: 5, right: 20, bottom: 5, left: 10 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#2e2f38" />
          <XAxis
            dataKey="timestamp"
            tickFormatter={formatTime}
            tick={{ fontSize: 10, fill: "#8b8d98" }}
            interval="preserveStartEnd"
          />
          <YAxis
            domain={["auto", "auto"]}
            tick={{ fontSize: 10, fill: "#8b8d98" }}
            tickFormatter={(v: number) => `$${v.toLocaleString()}`}
          />
          <Tooltip
            formatter={(value: number) => {
              const pctChange = ((value - startEquity) / startEquity * 100).toFixed(2);
              return [`$${value.toLocaleString()} (${pctChange}%)`, "Equity"];
            }}
            labelFormatter={formatTime}
            contentStyle={{ backgroundColor: "#1a1b22", border: "1px solid #2e2f38", borderRadius: "6px", color: "#e4e4e7" }}
            itemStyle={{ color: "#e4e4e7" }}
            labelStyle={{ color: "#8b8d98" }}
          />
          <ReferenceLine
            y={startEquity}
            stroke="#8b8d98"
            strokeDasharray="4 4"
            label={{ value: `Start: $${startEquity.toLocaleString()}`, position: "right", fontSize: 10, fill: "#8b8d98" }}
          />
          <Line
            type="monotone"
            dataKey="equity"
            stroke="#06b6d4"
            dot={false}
            strokeWidth={2}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
