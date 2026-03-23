"use client";

import { useState } from "react";
import Link from "next/link";
import type { CompanyInfo, NewsArticle } from "@/types";

function formatMarketCap(cap: number | null): string {
  if (cap === null || cap === undefined) return "N/A";
  if (cap >= 1e12) return `$${(cap / 1e12).toFixed(1)}T`;
  if (cap >= 1e9) return `$${(cap / 1e9).toFixed(1)}B`;
  if (cap >= 1e6) return `$${(cap / 1e6).toFixed(0)}M`;
  return `$${cap.toLocaleString()}`;
}

function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return "";
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}

function sentimentColor(sentiment: string | null): string {
  if (sentiment === "positive") return "bg-gain";
  if (sentiment === "negative") return "bg-loss";
  return "bg-gray-500";
}

interface WatchlistCardProps {
  symbol: string;
  company: CompanyInfo | null;
  news: NewsArticle[];
}

export default function WatchlistCard({ symbol, company, news }: WatchlistCardProps) {
  const [expanded, setExpanded] = useState(false);

  const price = company?.current_price;
  const changePct = company?.change_pct;
  const low52 = company?.fifty_two_week_low;
  const high52 = company?.fifty_two_week_high;

  // 52-week range position (0-100%)
  let rangePosition = 50;
  if (price != null && low52 != null && high52 != null && high52 > low52) {
    rangePosition = Math.max(0, Math.min(100, ((price - low52) / (high52 - low52)) * 100));
  }

  return (
    <div className="rounded-lg border border-surface-600 bg-surface-900 p-4 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <div>
          <Link href={`/quote/${symbol}`} className="font-bold text-base text-text-primary hover:text-accent-light transition-colors">{symbol}</Link>
          {company?.name && company.name !== symbol && (
            <span className="text-xs text-text-secondary ml-2">{company.name}</span>
          )}
        </div>
        <div className="text-right">
          {price != null && (
            <span className="font-semibold text-sm text-text-primary">${price.toFixed(2)}</span>
          )}
          {changePct != null && (
            <span
              className={`ml-2 text-xs px-1.5 py-0.5 rounded font-medium ${
                changePct >= 0
                  ? "bg-gain/15 text-gain"
                  : "bg-loss/15 text-loss"
              }`}
            >
              {changePct >= 0 ? "+" : ""}{changePct.toFixed(2)}%
            </span>
          )}
        </div>
      </div>

      {/* Tags */}
      <div className="flex flex-wrap gap-1.5 mb-3">
        {company?.sector && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/15 text-accent-light">{company.sector}</span>
        )}
        {company?.industry && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/10 text-accent-light">{company.industry}</span>
        )}
        {company?.market_cap != null && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-surface-600 text-text-secondary">
            {formatMarketCap(company.market_cap)}
          </span>
        )}
      </div>

      {/* 52-week range */}
      {low52 != null && high52 != null && (
        <div className="mb-3">
          <div className="flex justify-between text-[10px] text-text-secondary mb-0.5">
            <span>${low52.toFixed(2)}</span>
            <span className="text-text-secondary text-[10px]">52W Range</span>
            <span>${high52.toFixed(2)}</span>
          </div>
          <div className="h-1.5 bg-surface-600 rounded-full relative">
            <div
              className="absolute top-0 left-0 h-full bg-accent rounded-full"
              style={{ width: `${rangePosition}%` }}
            />
            <div
              className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-accent rounded-full border-2 border-surface-900 shadow"
              style={{ left: `${rangePosition}%`, marginLeft: "-5px" }}
            />
          </div>
        </div>
      )}

      {/* News */}
      <div className="flex-1">
        {news.length > 0 ? (
          <ul className="space-y-1.5">
            {news.slice(0, 5).map((article, i) => (
              <li key={i} className="flex items-start gap-1.5">
                <span className={`mt-1.5 w-2 h-2 rounded-full flex-shrink-0 ${sentimentColor(article.sentiment)}`} />
                <div className="min-w-0">
                  {article.url ? (
                    <a
                      href={article.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-text-secondary hover:text-accent-light line-clamp-2 leading-snug"
                    >
                      {article.headline}
                    </a>
                  ) : (
                    <span className="text-xs text-text-secondary line-clamp-2 leading-snug">{article.headline}</span>
                  )}
                  <div className="flex gap-2 text-[10px] text-text-secondary">
                    {article.source && <span>{article.source}</span>}
                    {article.published_at && <span>{formatRelativeTime(article.published_at)}</span>}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-text-secondary">No news available</p>
        )}
      </div>

      {/* Summary */}
      {company?.summary && (
        <div className="mt-3 pt-2 border-t border-surface-600">
          <p className="text-[11px] text-text-secondary leading-relaxed">
            {expanded ? company.summary : company.summary.slice(0, 150)}
            {company.summary.length > 150 && (
              <button
                onClick={() => setExpanded(!expanded)}
                className="text-accent hover:text-accent-light ml-1"
              >
                {expanded ? "less" : "...more"}
              </button>
            )}
          </p>
        </div>
      )}
    </div>
  );
}
