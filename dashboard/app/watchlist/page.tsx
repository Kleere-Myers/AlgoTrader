"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import { useSymbols } from "@/hooks/useSymbols";
import { strategyApi } from "@/lib/api";
import type { CompanyInfo, NewsArticle } from "@/types";
import WatchlistCard from "@/components/WatchlistCard";

const REFRESH_INTERVAL = 60_000; // 60 seconds

export default function WatchlistPage() {
  const { symbols, addSymbol, removeSymbol, error: symbolError } = useSymbols();
  const [companies, setCompanies] = useState<Record<string, CompanyInfo | null>>({});
  const [news, setNews] = useState<Record<string, NewsArticle[]>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newSymbol, setNewSymbol] = useState("");
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

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

  const handleAdd = async () => {
    const sym = newSymbol.trim().toUpperCase();
    if (!sym || symbols.includes(sym)) return;
    setAdding(true);
    try {
      await addSymbol(sym);
      setNewSymbol("");
      inputRef.current?.focus();
    } catch {}
    setAdding(false);
  };

  const handleRemove = async (sym: string) => {
    setRemoving(sym);
    try {
      await removeSymbol(sym);
    } catch {}
    setRemoving(null);
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-lg font-semibold text-text-primary">Watchlist</h2>
          <p className="text-text-secondary text-sm">
            {symbols.length} symbols tracked — auto-refreshes every 60s
          </p>
        </div>
        <button
          onClick={() => fetchAll(symbols)}
          disabled={loading}
          className="text-sm px-4 py-2 rounded bg-accent text-white hover:bg-accent-dark disabled:opacity-50"
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      {/* Add symbol bar */}
      <div className="flex items-center gap-2 mb-4">
        <div className="relative flex-1 max-w-xs">
          <input
            ref={inputRef}
            type="text"
            value={newSymbol}
            onChange={(e) => setNewSymbol(e.target.value.toUpperCase())}
            onKeyDown={(e) => e.key === "Enter" && handleAdd()}
            placeholder="Add symbol (e.g. TSLA)"
            className="w-full text-sm pl-3 pr-3 py-1.5 rounded-lg bg-surface-800 border border-surface-600 text-text-primary placeholder:text-text-secondary/50 focus:border-accent focus:outline-none"
          />
        </div>
        <button
          onClick={handleAdd}
          disabled={adding || !newSymbol.trim()}
          className="bg-accent text-white text-xs font-medium px-3 py-1.5 rounded-lg hover:bg-accent-dark transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {adding ? "Adding..." : "Add"}
        </button>
      </div>

      {/* Symbol pills with remove buttons */}
      <div className="flex flex-wrap gap-1.5 mb-5">
        {symbols.map((s) => (
          <span
            key={s}
            className="inline-flex items-center gap-1 bg-surface-800 text-text-primary text-xs font-mono px-2.5 py-1 rounded-lg border border-surface-600"
          >
            {s}
            <button
              onClick={() => handleRemove(s)}
              disabled={removing === s}
              className="text-text-secondary hover:text-loss transition-colors ml-0.5 disabled:opacity-50"
              title={`Remove ${s}`}
            >
              {removing === s ? (
                <span className="text-[10px]">...</span>
              ) : (
                <svg viewBox="0 0 16 16" fill="currentColor" className="w-3 h-3">
                  <path d="M4.646 4.646a.5.5 0 0 1 .708 0L8 7.293l2.646-2.647a.5.5 0 0 1 .708.708L8.707 8l2.647 2.646a.5.5 0 0 1-.708.708L8 8.707l-2.646 2.647a.5.5 0 0 1-.708-.708L7.293 8 4.646 5.354a.5.5 0 0 1 0-.708z" />
                </svg>
              )}
            </button>
          </span>
        ))}
      </div>

      {(error || symbolError) && (
        <div className="bg-loss/10 border border-loss/30 rounded-lg p-3 text-sm text-loss mb-4">
          {error || symbolError}
        </div>
      )}

      {loading && Object.keys(companies).length === 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {symbols.map((s) => (
            <div key={s} className="rounded-lg border border-surface-600 bg-surface-900 p-4 h-48 animate-pulse">
              <div className="h-4 w-16 bg-surface-600 rounded mb-3" />
              <div className="h-3 w-32 bg-surface-800 rounded mb-2" />
              <div className="h-3 w-24 bg-surface-800 rounded mb-2" />
              <div className="h-3 w-40 bg-surface-800 rounded" />
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
