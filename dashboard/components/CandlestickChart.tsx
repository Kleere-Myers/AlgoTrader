"use client";

import {
  ComposedChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
  Cell,
  Scatter,
} from "recharts";
import type { OhlcvBar, Signal } from "@/types";

interface CandlestickChartProps {
  bars: OhlcvBar[];
  signals: Signal[];
  symbol: string;
}

// Custom candlestick shape for the Bar component
function CandlestickShape(props: any) {
  const { x, y, width, payload } = props;
  if (!payload) return null;

  const { open, close, high, low } = payload;
  const isGreen = close >= open;
  const fill = isGreen ? "#16a34a" : "#dc2626";
  const stroke = isGreen ? "#15803d" : "#b91c1c";

  // Scale: y is the top of the bar area, we need to compute pixel positions
  // The bar is rendered with y at the value position, so we use the yAxis scale
  const yScale = props.yAxis?.scale;
  if (!yScale) return null;

  const yHigh = yScale(high);
  const yLow = yScale(low);
  const yOpen = yScale(open);
  const yClose = yScale(close);

  const bodyTop = Math.min(yOpen, yClose);
  const bodyHeight = Math.max(Math.abs(yOpen - yClose), 1);
  const centerX = x + width / 2;

  return (
    <g>
      {/* Wick */}
      <line x1={centerX} y1={yHigh} x2={centerX} y2={yLow} stroke={stroke} strokeWidth={1} />
      {/* Body */}
      <rect
        x={x + 1}
        y={bodyTop}
        width={Math.max(width - 2, 2)}
        height={bodyHeight}
        fill={fill}
        stroke={stroke}
        strokeWidth={0.5}
      />
    </g>
  );
}

// Signal triangle marker
function SignalMarker(props: any) {
  const { cx, cy, payload } = props;
  if (!cx || !cy || !payload) return null;

  const isBuy = payload.direction === "BUY";
  const color = isBuy ? "#16a34a" : "#dc2626";
  const size = 8;

  // Upward triangle for BUY, downward for SELL
  const points = isBuy
    ? `${cx},${cy - size} ${cx - size},${cy + size} ${cx + size},${cy + size}`
    : `${cx},${cy + size} ${cx - size},${cy - size} ${cx + size},${cy - size}`;

  return <polygon points={points} fill={color} stroke={color} strokeWidth={0.5} />;
}

export default function CandlestickChart({ bars, signals, symbol }: CandlestickChartProps) {
  if (bars.length === 0) {
    return (
      <div className="rounded border border-dashed border-gray-300 p-12 text-center text-gray-400">
        No bar data available for {symbol}
      </div>
    );
  }

  // Merge signals into bar data for scatter overlay
  const signalMap = new Map<string, Signal>();
  for (const sig of signals) {
    if (sig.symbol === symbol && sig.direction !== "HOLD") {
      // Key by the bar timestamp closest to the signal
      signalMap.set(sig.timestamp, sig);
    }
  }

  // Build chart data with signal markers
  const chartData = bars.map((bar) => {
    const matchedSignal = signalMap.get(bar.timestamp);
    return {
      ...bar,
      // For candlestick bar rendering we need a dummy value
      range: bar.high - bar.low,
      // Signal scatter point
      signalPrice: matchedSignal ? (matchedSignal.direction === "BUY" ? bar.low * 0.998 : bar.high * 1.002) : null,
      direction: matchedSignal?.direction || null,
    };
  });

  const formatTime = (ts: string) => {
    const d = new Date(ts);
    return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
  };

  const allPrices = bars.flatMap((b) => [b.high, b.low]);
  const minPrice = Math.min(...allPrices) * 0.999;
  const maxPrice = Math.max(...allPrices) * 1.001;
  const maxVolume = Math.max(...bars.map((b) => b.volume));

  return (
    <ResponsiveContainer width="100%" height={400}>
      <ComposedChart data={chartData} margin={{ top: 10, right: 20, bottom: 5, left: 10 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
        <XAxis
          dataKey="timestamp"
          tickFormatter={formatTime}
          tick={{ fontSize: 10 }}
          interval="preserveStartEnd"
        />
        <YAxis
          yAxisId="price"
          domain={[minPrice, maxPrice]}
          tick={{ fontSize: 10 }}
          tickFormatter={(v: number) => `$${v.toFixed(0)}`}
          orientation="right"
        />
        <YAxis
          yAxisId="volume"
          domain={[0, maxVolume * 4]}
          hide
          orientation="left"
        />
        <Tooltip
          content={({ payload }) => {
            if (!payload || payload.length === 0) return null;
            const d = payload[0]?.payload;
            if (!d) return null;
            return (
              <div className="bg-white border border-gray-200 rounded shadow-sm p-2 text-xs">
                <p className="font-medium mb-1">{new Date(d.timestamp).toLocaleString()}</p>
                <p>O: ${d.open?.toFixed(2)} H: ${d.high?.toFixed(2)}</p>
                <p>L: ${d.low?.toFixed(2)} C: ${d.close?.toFixed(2)}</p>
                <p className="text-gray-400">Vol: {d.volume?.toLocaleString()}</p>
                {d.direction && (
                  <p className={d.direction === "BUY" ? "text-green-600 font-medium" : "text-red-600 font-medium"}>
                    Signal: {d.direction}
                  </p>
                )}
              </div>
            );
          }}
        />
        {/* Volume bars */}
        <Bar dataKey="volume" yAxisId="volume" fill="#e5e7eb" barSize={6} isAnimationActive={false} />
        {/* Candlestick body — uses range as dummy value, custom shape renders actual OHLC */}
        <Bar
          dataKey="range"
          yAxisId="price"
          shape={<CandlestickShape />}
          isAnimationActive={false}
        />
        {/* Signal markers */}
        <Scatter
          dataKey="signalPrice"
          yAxisId="price"
          shape={<SignalMarker />}
          isAnimationActive={false}
        />
      </ComposedChart>
    </ResponsiveContainer>
  );
}
