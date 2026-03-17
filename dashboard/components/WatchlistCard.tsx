"use client";

import { useState } from "react";
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
  if (sentiment === "positive") return "bg-green-400";
  if (sentiment === "negative") return "bg-red-400";
  return "bg-gray-300";
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
    <div className="rounded-lg border border-gray-200 bg-white p-4 shadow-sm flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <div>
          <span className="font-bold text-base">{symbol}</span>
          {company?.name && company.name !== symbol && (
            <span className="text-xs text-gray-500 ml-2">{company.name}</span>
          )}
        </div>
        <div className="text-right">
          {price != null && (
            <span className="font-semibold text-sm">${price.toFixed(2)}</span>
          )}
          {changePct != null && (
            <span
              className={`ml-2 text-xs px-1.5 py-0.5 rounded font-medium ${
                changePct >= 0
                  ? "bg-green-100 text-green-700"
                  : "bg-red-100 text-red-700"
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
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 text-blue-600">{company.sector}</span>
        )}
        {company?.industry && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-50 text-purple-600">{company.industry}</span>
        )}
        {company?.market_cap != null && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 text-gray-600">
            {formatMarketCap(company.market_cap)}
          </span>
        )}
      </div>

      {/* 52-week range */}
      {low52 != null && high52 != null && (
        <div className="mb-3">
          <div className="flex justify-between text-[10px] text-gray-400 mb-0.5">
            <span>${low52.toFixed(2)}</span>
            <span className="text-gray-500 text-[10px]">52W Range</span>
            <span>${high52.toFixed(2)}</span>
          </div>
          <div className="h-1.5 bg-gray-100 rounded-full relative">
            <div
              className="absolute top-0 left-0 h-full bg-blue-400 rounded-full"
              style={{ width: `${rangePosition}%` }}
            />
            <div
              className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-blue-600 rounded-full border-2 border-white shadow"
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
                      className="text-xs text-gray-800 hover:text-blue-600 line-clamp-2 leading-snug"
                    >
                      {article.headline}
                    </a>
                  ) : (
                    <span className="text-xs text-gray-800 line-clamp-2 leading-snug">{article.headline}</span>
                  )}
                  <div className="flex gap-2 text-[10px] text-gray-400">
                    {article.source && <span>{article.source}</span>}
                    {article.published_at && <span>{formatRelativeTime(article.published_at)}</span>}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-gray-400">No news available</p>
        )}
      </div>

      {/* Summary */}
      {company?.summary && (
        <div className="mt-3 pt-2 border-t border-gray-100">
          <p className="text-[11px] text-gray-500 leading-relaxed">
            {expanded ? company.summary : company.summary.slice(0, 150)}
            {company.summary.length > 150 && (
              <button
                onClick={() => setExpanded(!expanded)}
                className="text-blue-500 hover:text-blue-700 ml-1"
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
