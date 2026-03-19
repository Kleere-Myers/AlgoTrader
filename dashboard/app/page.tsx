"use client";

import { useEffect, useState } from "react";
import { executionApi, strategyApi } from "@/lib/api";
import { useSseEvents } from "@/hooks/useSseEvents";
import type {
  AccountInfo,
  MarketIndex,
  SectorPerformance,
  MarketMover,
  PortfolioPnlHistory,
  NewsArticle,
} from "@/types";
import MarketIndexCard from "@/components/MarketIndexCard";
import SectorPerformanceBar from "@/components/SectorPerformanceBar";
import PnlChart from "@/components/PnlChart";
import PortfolioSummary from "@/components/PortfolioSummary";
import MoversList from "@/components/MoversList";
import NewsCard from "@/components/NewsCard";

export default function OverviewPage() {
  const [indices, setIndices] = useState<MarketIndex[]>([]);
  const [sectors, setSectors] = useState<SectorPerformance[]>([]);
  const [movers, setMovers] = useState<{ gainers: MarketMover[]; losers: MarketMover[] }>({ gainers: [], losers: [] });
  const [pnlRange, setPnlRange] = useState("1d");
  const [pnlData, setPnlData] = useState<PortfolioPnlHistory | null>(null);
  const [news, setNews] = useState<NewsArticle[]>([]);
  const [loading, setLoading] = useState(true);
  const { events, isTradingHalted } = useSseEvents();

  // Initial data load
  useEffect(() => {
    const load = async () => {
      try {
        const [idx, sec, mov, feed] = await Promise.allSettled([
          strategyApi.getMarketIndices(),
          strategyApi.getSectorPerformance(),
          strategyApi.getMarketMovers(),
          strategyApi.getNewsFeed(15),
        ]);
        if (idx.status === "fulfilled") setIndices(idx.value);
        if (sec.status === "fulfilled") setSectors(sec.value);
        if (mov.status === "fulfilled") setMovers(mov.value);
        if (feed.status === "fulfilled") setNews(feed.value.articles);
      } catch {}
      setLoading(false);
    };
    load();
    const interval = setInterval(load, 60_000);
    return () => clearInterval(interval);
  }, []);

  // Fetch P&L data when range changes
  useEffect(() => {
    strategyApi.getPnlHistory(pnlRange).then(setPnlData).catch(() => {});
  }, [pnlRange]);

  // Refresh P&L on order fills
  useEffect(() => {
    const fills = events.filter((e) => e.event_type === "ORDER_FILL");
    if (fills.length > 0) {
      strategyApi.getPnlHistory(pnlRange).then(setPnlData).catch(() => {});
    }
  }, [events, pnlRange]);

  const chartData = pnlData
    ? pnlData.timestamps.map((t, i) => ({
        timestamp: t === "now" ? "Now" : t,
        equity: pnlData.equity[i] || 0,
      }))
    : [];

  return (
    <div>
      {isTradingHalted && (
        <div className="mb-4 rounded-lg bg-loss/10 border border-loss/30 px-4 py-2 text-loss text-sm font-semibold">
          TRADING HALTED
        </div>
      )}

      {/* Markets Carousel */}
      <section className="mb-6">
        <h2 className="text-lg font-semibold text-text-secondary mb-3">Markets</h2>
        <div className="flex gap-3 overflow-x-auto pb-2 scrollbar-thin">
          {loading
            ? Array.from({ length: 6 }).map((_, i) => (
                <div key={i} className="bg-navy-800 rounded-lg min-w-[160px] h-[72px] animate-pulse flex-shrink-0" />
              ))
            : indices.map((idx) => (
                <MarketIndexCard key={idx.symbol} index={idx} />
              ))}
        </div>
      </section>

      {/* Sector Performance */}
      {sectors.length > 0 && (
        <section className="mb-6">
          <h2 className="text-lg font-semibold text-text-secondary mb-3">Sector Performance</h2>
          <SectorPerformanceBar sectors={sectors} />
        </section>
      )}

      {/* Portfolio P&L + Summary */}
      <section className="mb-6">
        <h2 className="text-lg font-semibold text-text-secondary mb-3">Portfolio</h2>
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
          <div className="lg:col-span-3">
            <PnlChart
              range={pnlRange}
              onRangeChange={setPnlRange}
              data={chartData}
            />
          </div>
          <div>
            {pnlData?.summary && (
              <PortfolioSummary summary={pnlData.summary} />
            )}
          </div>
        </div>
      </section>

      {/* Movers + News */}
      <section className="mb-6">
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div>
            <h2 className="text-lg font-semibold text-text-secondary mb-3">Movers</h2>
            <MoversList gainers={movers.gainers} losers={movers.losers} />
          </div>
          <div className="lg:col-span-2">
            <h2 className="text-lg font-semibold text-text-secondary mb-3">News</h2>
            <div className="space-y-2 max-h-[500px] overflow-y-auto pr-1">
              {news.length === 0 && !loading && (
                <p className="text-text-secondary text-sm">No news available</p>
              )}
              {news.map((article, i) => (
                <NewsCard key={i} article={article} />
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
