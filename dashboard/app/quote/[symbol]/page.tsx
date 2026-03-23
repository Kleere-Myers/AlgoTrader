"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { strategyApi } from "@/lib/api";
import type { CompanyInfo, OhlcvBar, NewsArticle, HistoryRange } from "@/types";
import CandlestickChart from "@/components/CandlestickChart";
import NewsCard from "@/components/NewsCard";

const RANGES: { label: string; value: HistoryRange }[] = [
  { label: "1D", value: "1d" },
  { label: "5D", value: "5d" },
  { label: "1M", value: "1m" },
  { label: "6M", value: "6m" },
  { label: "1Y", value: "1y" },
  { label: "5Y", value: "5y" },
];

function formatLargeNumber(n: number | null | undefined): string {
  if (n == null) return "N/A";
  if (n >= 1e12) return `${(n / 1e12).toFixed(2)}T`;
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  return n.toLocaleString();
}

function formatVolume(n: number | null | undefined): string {
  if (n == null) return "N/A";
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return n.toLocaleString();
}

function fmt(n: number | null | undefined, decimals = 2): string {
  if (n == null) return "N/A";
  return n.toFixed(decimals);
}

function fmtPct(n: number | null | undefined): string {
  if (n == null) return "N/A";
  return `${(n * 100).toFixed(2)}%`;
}

function formatChartDate(timestamp: string, range: HistoryRange): string {
  const d = new Date(timestamp);
  if (isNaN(d.getTime())) return timestamp;
  if (range === "1d" || range === "5d") {
    return d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  }
  if (range === "1m" || range === "6m") {
    return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
  }
  return d.toLocaleDateString("en-US", { month: "short", year: "2-digit" });
}

function TriangleUp({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M8 4l5 8H3z" />
    </svg>
  );
}

function TriangleDown({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" className={className}>
      <path d="M8 12L3 4h10z" />
    </svg>
  );
}

function ChartTooltip({
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
        {(() => {
          const d = new Date(label);
          return isNaN(d.getTime()) ? label : d.toLocaleString();
        })()}
      </p>
      <p className="text-text-primary font-semibold">
        ${payload[0].value.toFixed(2)}
      </p>
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between py-1.5 border-b border-surface-600 last:border-b-0">
      <span className="text-xs text-text-secondary">{label}</span>
      <span className="text-xs font-medium text-text-primary font-mono tabular-nums">{value}</span>
    </div>
  );
}

export default function QuotePage() {
  const params = useParams();
  const symbol = (params.symbol as string).toUpperCase();

  const [company, setCompany] = useState<CompanyInfo | null>(null);
  const [news, setNews] = useState<NewsArticle[]>([]);
  const [bars, setBars] = useState<OhlcvBar[]>([]);
  const [range, setRange] = useState<HistoryRange>("1d");
  const [chartType, setChartType] = useState<"line" | "candle">("line");
  const [loading, setLoading] = useState(true);
  const [barsLoading, setBarsLoading] = useState(true);
  const [expanded, setExpanded] = useState(false);

  // Fetch company info + news
  useEffect(() => {
    const load = async () => {
      setLoading(true);
      const [c, n] = await Promise.allSettled([
        strategyApi.getCompanyInfo(symbol),
        strategyApi.getNews(symbol),
      ]);
      if (c.status === "fulfilled") setCompany(c.value);
      if (n.status === "fulfilled") setNews(n.value.articles);
      setLoading(false);
    };
    load();
    const interval = setInterval(load, 60_000);
    return () => clearInterval(interval);
  }, [symbol]);

  // Fetch bars when range changes
  useEffect(() => {
    setBarsLoading(true);
    strategyApi
      .getHistoricalBars(symbol, range)
      .then(setBars)
      .catch(() => setBars([]))
      .finally(() => setBarsLoading(false));
  }, [symbol, range]);

  const changeAbs =
    company?.current_price != null && company?.previous_close != null
      ? company.current_price - company.previous_close
      : null;
  const isPositive = (company?.change_pct ?? 0) >= 0;
  const changeColor = isPositive ? "text-gain" : "text-loss";

  // Chart data
  const lineData = bars.map((b) => ({ timestamp: b.timestamp, close: b.close }));
  const chartIsPositive =
    bars.length >= 2 ? bars[bars.length - 1].close >= bars[0].close : true;
  const strokeColor = chartIsPositive ? "#34d399" : "#f87171";

  if (loading && !company) {
    return (
      <div className="animate-pulse space-y-6">
        <div className="h-10 bg-surface-800 rounded w-48" />
        <div className="h-6 bg-surface-800 rounded w-32" />
        <div className="h-[400px] bg-surface-800 rounded" />
      </div>
    );
  }

  return (
    <div>
      {/* Breadcrumb */}
      <div className="mb-4">
        <Link
          href="/"
          className="text-xs text-text-secondary hover:text-accent-light transition-colors"
        >
          Overview
        </Link>
        <span className="text-xs text-text-secondary mx-1.5">/</span>
        <span className="text-xs text-text-primary">{symbol}</span>
      </div>

      {/* Price Header */}
      <section className="mb-6">
        <div className="flex items-baseline gap-3 mb-1">
          <h1 className="text-2xl font-bold text-text-primary">{symbol}</h1>
          {company?.name && company.name !== symbol && (
            <span className="text-sm text-text-secondary">{company.name}</span>
          )}
          {company?.exchange && (
            <span className="text-xs text-text-secondary">
              {company.exchange}
            </span>
          )}
        </div>
        <div className="flex items-baseline gap-3">
          <span className="text-3xl font-bold text-text-primary font-mono tabular-nums">
            {company?.current_price != null
              ? `$${company.current_price.toFixed(2)}`
              : "—"}
          </span>
          {changeAbs != null && company?.change_pct != null && (
            <div className={`flex items-center gap-1 ${changeColor}`}>
              {isPositive ? (
                <TriangleUp className="w-4 h-4" />
              ) : (
                <TriangleDown className="w-4 h-4" />
              )}
              <span className="text-lg font-semibold">
                {isPositive ? "+" : ""}
                {changeAbs.toFixed(2)} ({isPositive ? "+" : ""}
                {company.change_pct.toFixed(2)}%)
              </span>
            </div>
          )}
        </div>
        {company?.currency && (
          <p className="text-xs text-text-secondary mt-1">
            {company.currency} &middot; As of market close
          </p>
        )}
      </section>

      {/* Chart */}
      <section className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <div className="flex gap-1">
            {RANGES.map((r) => (
              <button
                key={r.value}
                onClick={() => setRange(r.value)}
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
          <div className="flex gap-1">
            <button
              onClick={() => setChartType("line")}
              className={`px-2.5 py-1.5 text-xs font-medium rounded transition-colors ${
                chartType === "line"
                  ? "bg-accent text-white"
                  : "text-text-secondary hover:text-text-primary bg-surface-800"
              }`}
            >
              Line
            </button>
            <button
              onClick={() => setChartType("candle")}
              className={`px-2.5 py-1.5 text-xs font-medium rounded transition-colors ${
                chartType === "candle"
                  ? "bg-accent text-white"
                  : "text-text-secondary hover:text-text-primary bg-surface-800"
              }`}
            >
              Candle
            </button>
          </div>
        </div>

        {barsLoading ? (
          <div className="h-[400px] bg-surface-800 rounded-lg animate-pulse" />
        ) : chartType === "candle" ? (
          <CandlestickChart bars={bars} signals={[]} symbol={symbol} />
        ) : (
          <ResponsiveContainer width="100%" height={400}>
            <AreaChart data={lineData}>
              <defs>
                <linearGradient id="quote-gradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={strokeColor} stopOpacity={0.3} />
                  <stop offset="100%" stopColor={strokeColor} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#2e2f38" vertical={false} />
              <XAxis
                dataKey="timestamp"
                tickFormatter={(t) => formatChartDate(t, range)}
                stroke="#2e2f38"
                tick={{ fill: "#8b8d98", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                interval="preserveStartEnd"
              />
              <YAxis
                domain={["auto", "auto"]}
                tickFormatter={(v: number) => `$${v.toFixed(2)}`}
                stroke="#2e2f38"
                tick={{ fill: "#8b8d98", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                width={75}
              />
              <Tooltip
                content={<ChartTooltip />}
                cursor={{ stroke: "#2e2f38", strokeDasharray: "3 3" }}
              />
              <Area
                type="monotone"
                dataKey="close"
                stroke={strokeColor}
                strokeWidth={2}
                fill="url(#quote-gradient)"
                isAnimationActive={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        )}
      </section>

      {/* Stats + Profile + News */}
      <section className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        {/* Key Statistics */}
        <div className="lg:col-span-1">
          <h2 className="text-[11px] font-mono font-medium text-text-secondary mb-2 uppercase tracking-widest">Key Statistics</h2>
          <div className="bg-surface-900 rounded-lg border border-surface-600 p-4">
            <StatRow label="Previous Close" value={fmt(company?.previous_close)} />
            <StatRow label="Open" value={fmt(company?.open)} />
            <StatRow
              label="Bid"
              value={
                company?.bid != null
                  ? `${fmt(company.bid)} x ${company.bid_size ?? "—"}`
                  : "N/A"
              }
            />
            <StatRow
              label="Ask"
              value={
                company?.ask != null
                  ? `${fmt(company.ask)} x ${company.ask_size ?? "—"}`
                  : "N/A"
              }
            />
            <StatRow
              label="Day's Range"
              value={
                company?.day_low != null && company?.day_high != null
                  ? `${fmt(company.day_low)} - ${fmt(company.day_high)}`
                  : "N/A"
              }
            />
            <StatRow
              label="52 Week Range"
              value={
                company?.fifty_two_week_low != null && company?.fifty_two_week_high != null
                  ? `${fmt(company.fifty_two_week_low)} - ${fmt(company.fifty_two_week_high)}`
                  : "N/A"
              }
            />
            <StatRow label="Volume" value={formatVolume(company?.volume)} />
            <StatRow label="Avg. Volume" value={formatVolume(company?.average_volume)} />
            <StatRow label="Market Cap" value={formatLargeNumber(company?.market_cap)} />
            <StatRow label="Beta" value={fmt(company?.beta)} />
            <StatRow label="PE Ratio (TTM)" value={fmt(company?.trailing_pe)} />
            <StatRow label="EPS (TTM)" value={fmt(company?.eps)} />
            <StatRow
              label="Dividend & Yield"
              value={
                company?.dividend_rate != null
                  ? `${fmt(company.dividend_rate)} (${fmtPct(company.dividend_yield)})`
                  : "N/A"
              }
            />
            <StatRow label="1y Target Est" value={fmt(company?.target_mean_price)} />
          </div>
        </div>

        {/* Profile + News */}
        <div className="lg:col-span-2 space-y-4">
          {/* Company Profile */}
          {company && (
            <div>
              <h2 className="text-[11px] font-mono font-medium text-text-secondary mb-2 uppercase tracking-widest">Profile</h2>
              <div className="bg-surface-900 rounded-lg border border-surface-600 p-4">
                <div className="flex flex-wrap gap-1.5 mb-3">
                  {company.sector && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/15 text-accent-light">
                      {company.sector}
                    </span>
                  )}
                  {company.industry && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/10 text-accent-light">
                      {company.industry}
                    </span>
                  )}
                </div>
                {company.summary && (
                  <p className="text-xs text-text-secondary leading-relaxed">
                    {expanded ? company.summary : company.summary.slice(0, 300)}
                    {company.summary.length > 300 && (
                      <button
                        onClick={() => setExpanded(!expanded)}
                        className="text-accent hover:text-accent-light ml-1"
                      >
                        {expanded ? "less" : "...more"}
                      </button>
                    )}
                  </p>
                )}
              </div>
            </div>
          )}

          {/* News */}
          <div>
            <h2 className="text-[11px] font-mono font-medium text-text-secondary mb-2 uppercase tracking-widest">News</h2>
            <div className="space-y-2 max-h-[500px] overflow-y-auto pr-1">
              {news.length === 0 ? (
                <p className="text-text-secondary text-sm">No news available</p>
              ) : (
                news.slice(0, 10).map((article, i) => (
                  <NewsCard key={i} article={article} />
                ))
              )}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
