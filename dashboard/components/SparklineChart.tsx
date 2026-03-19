"use client";

import { LineChart, Line, ResponsiveContainer } from "recharts";

interface SparklineChartProps {
  data: { value: number }[];
  color?: string;
  width?: number;
  height?: number;
}

export default function SparklineChart({
  data,
  color = "#21d87d",
  width,
  height = 40,
}: SparklineChartProps) {
  if (!data || data.length === 0) return null;

  const chart = (
    <LineChart data={data}>
      <Line
        type="monotone"
        dataKey="value"
        stroke={color}
        strokeWidth={1.5}
        dot={false}
        isAnimationActive={false}
      />
    </LineChart>
  );

  if (width) {
    return (
      <div style={{ width, height }}>
        <ResponsiveContainer width="100%" height="100%">
          {chart}
        </ResponsiveContainer>
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      {chart}
    </ResponsiveContainer>
  );
}
