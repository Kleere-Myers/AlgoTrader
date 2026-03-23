"use client";

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";

const RANGES = [
  { label: "1D", value: "1d" },
  { label: "1W", value: "1w" },
  { label: "1M", value: "1m" },
  { label: "3M", value: "3m" },
  { label: "YTD", value: "ytd" },
];

interface PnlChartProps {
  range: string;
  onRangeChange: (range: string) => void;
  data: { timestamp: string; equity: number }[];
}

function formatCurrency(value: number): string {
  return value.toLocaleString("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  });
}

function formatDate(timestamp: string, range: string): string {
  if (timestamp === "Now" || timestamp === "now") return "Now";
  const d = new Date(timestamp);
  if (isNaN(d.getTime())) return timestamp;
  if (range === "1d") {
    return d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  }
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

function CustomTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: { value: number }[];
  label?: string;
}) {
  if (!active || !payload?.length || !label) return null;
  return (
    <div className="bg-surface-900 border border-surface-600 rounded px-3 py-2 text-xs shadow-lg">
      <p className="text-text-secondary">
        {label === "Now" || label === "now" ? "Now" : (() => {
          const d = new Date(label);
          return isNaN(d.getTime()) ? label : d.toLocaleString();
        })()}
      </p>
      <p className="text-text-primary font-semibold">
        {formatCurrency(payload[0].value)}
      </p>
    </div>
  );
}

export default function PnlChart({ range, onRangeChange, data }: PnlChartProps) {
  // Determine if overall P&L is positive
  const isPositive =
    data.length >= 2 ? data[data.length - 1].equity >= data[0].equity : true;
  const strokeColor = isPositive ? "#34d399" : "#f87171";
  const gradientId = "pnl-gradient";

  return (
    <div>
      {/* Range tabs */}
      <div className="flex gap-1 mb-4">
        {RANGES.map((r) => (
          <button
            key={r.value}
            onClick={() => onRangeChange(r.value)}
            className={`px-3 py-1.5 text-xs font-medium rounded transition-colors ${
              range === r.value
                ? "bg-accent text-white"
                : "text-text-secondary hover:text-text-primary bg-surface-800"
            }`}
          >
            {r.label}
          </button>
        ))}
      </div>

      {/* Chart */}
      <ResponsiveContainer width="100%" height={300}>
        <AreaChart data={data}>
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={strokeColor} stopOpacity={0.3} />
              <stop offset="100%" stopColor={strokeColor} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="#2e2f38" vertical={false} />
          <XAxis
            dataKey="timestamp"
            tickFormatter={(t) => formatDate(t, range)}
            stroke="#2e2f38"
            tick={{ fill: "#8b8d98", fontSize: 11 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            tickFormatter={formatCurrency}
            stroke="#2e2f38"
            tick={{ fill: "#8b8d98", fontSize: 11 }}
            axisLine={false}
            tickLine={false}
            width={80}
          />
          <Tooltip
            content={<CustomTooltip />}
            cursor={{ stroke: "#2e2f38", strokeDasharray: "3 3" }}
          />
          <Area
            type="monotone"
            dataKey="equity"
            stroke={strokeColor}
            strokeWidth={2}
            fill={`url(#${gradientId})`}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
