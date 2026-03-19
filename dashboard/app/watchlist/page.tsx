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
          <h2 className="text-2xl font-bold text-text-primary">Watchlist</h2>
          <p className="text-text-secondary text-sm">
            {symbols.length} symbols tracked — auto-refreshes every 60s
          </p>
        </div>
        <button
          onClick={() => fetchAll(symbols)}
          disabled={loading}
          className="text-sm px-4 py-2 rounded bg-accent-purple text-white hover:bg-accent-purple-dark disabled:opacity-50"
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      {error && (
        <div className="bg-loss/10 border border-loss/30 rounded-lg p-3 text-sm text-loss mb-4">
          {error}
        </div>
      )}

      {loading && Object.keys(companies).length === 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {symbols.map((s) => (
            <div key={s} className="rounded-lg border border-navy-600 bg-navy-900 p-4 h-48 animate-pulse">
              <div className="h-4 w-16 bg-navy-600 rounded mb-3" />
              <div className="h-3 w-32 bg-navy-800 rounded mb-2" />
              <div className="h-3 w-24 bg-navy-800 rounded mb-2" />
              <div className="h-3 w-40 bg-navy-800 rounded" />
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
