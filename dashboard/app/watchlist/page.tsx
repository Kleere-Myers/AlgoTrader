"use client";

import { useEffect, useState, useCallback } from "react";
import { useSymbols } from "@/hooks/useSymbols";
import { strategyApi } from "@/lib/api";
import type { CompanyInfo, NewsArticle } from "@/types";
import WatchlistCard from "@/components/WatchlistCard";

const REFRESH_INTERVAL = 60_000; // 60 seconds

export default function WatchlistPage() {
  const { symbols } = useSymbols();
  const [companies, setCompanies] = useState<Record<string, CompanyInfo | null>>({});
  const [news, setNews] = useState<Record<string, NewsArticle[]>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAll = useCallback(async (syms: string[]) => {
    setError(null);
    try {
      const results = await Promise.allSettled(
        syms.flatMap((s) => [
          strategyApi.getCompanyInfo(s).then((c) => ({ type: "company" as const, symbol: s, data: c })),
          strategyApi.getNews(s).then((n) => ({ type: "news" as const, symbol: s, data: n })),
        ])
      );

      const newCompanies: Record<string, CompanyInfo | null> = {};
      const newNews: Record<string, NewsArticle[]> = {};

      for (const result of results) {
        if (result.status === "fulfilled") {
          const { type, symbol, data } = result.value;
          if (type === "company") {
            newCompanies[symbol] = data as CompanyInfo;
          } else {
            newNews[symbol] = (data as { symbol: string; articles: NewsArticle[] }).articles;
          }
        }
      }

      setCompanies(newCompanies);
      setNews(newNews);
    } catch {
      setError("Failed to fetch watchlist data");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (symbols.length === 0) return;
    setLoading(true);
    fetchAll(symbols);

    const interval = setInterval(() => fetchAll(symbols), REFRESH_INTERVAL);
    return () => clearInterval(interval);
  }, [symbols, fetchAll]);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold">Watchlist</h2>
          <p className="text-gray-500 text-sm">
            {symbols.length} symbols tracked — auto-refreshes every 60s
          </p>
        </div>
        <button
          onClick={() => fetchAll(symbols)}
          disabled={loading}
          className="text-sm px-4 py-2 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      {error && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700 mb-4">
          {error}
        </div>
      )}

      {loading && Object.keys(companies).length === 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {symbols.map((s) => (
            <div key={s} className="rounded-lg border border-gray-200 bg-white p-4 h-48 animate-pulse">
              <div className="h-4 w-16 bg-gray-200 rounded mb-3" />
              <div className="h-3 w-32 bg-gray-100 rounded mb-2" />
              <div className="h-3 w-24 bg-gray-100 rounded mb-2" />
              <div className="h-3 w-40 bg-gray-100 rounded" />
            </div>
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {symbols.map((s) => (
            <WatchlistCard
              key={s}
              symbol={s}
              company={companies[s] ?? null}
              news={news[s] ?? []}
            />
          ))}
        </div>
      )}
    </div>
  );
}
