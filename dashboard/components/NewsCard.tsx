"use client";

import type { NewsArticle } from "@/types";

interface NewsCardProps {
  article: NewsArticle;
}

function relativeTime(dateStr: string | null): string {
  if (!dateStr) return "";
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const diffMs = now - then;
  const diffMins = Math.floor(diffMs / 60000);
  if (diffMins < 1) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays}d ago`;
}

function sentimentColor(sentiment: string | null): string {
  if (!sentiment) return "bg-gray-500";
  const s = sentiment.toLowerCase();
  if (s === "positive" || s === "bullish") return "bg-gain";
  if (s === "negative" || s === "bearish") return "bg-loss";
  return "bg-gray-500";
}

export default function NewsCard({ article }: NewsCardProps) {
  const timeAgo = relativeTime(article.published_at);
  const sourceLine = [article.source, timeAgo].filter(Boolean).join(" · ");

  return (
    <a
      href={article.url || "#"}
      target="_blank"
      rel="noopener noreferrer"
      className="flex gap-3 bg-navy-800 hover:bg-navy-700 transition-colors rounded-lg p-3 border border-navy-600 cursor-pointer"
    >
      {/* Thumbnail */}
      <div className="shrink-0 w-20 h-20 rounded overflow-hidden bg-navy-600">
        {article.thumbnail_url ? (
          <img
            src={article.thumbnail_url}
            alt=""
            className="w-full h-full object-cover"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-text-secondary text-xs">
            No img
          </div>
        )}
      </div>

      {/* Text content */}
      <div className="flex-1 min-w-0">
        <p className="text-sm font-semibold text-text-primary line-clamp-2 leading-snug">
          {article.headline}
        </p>
        <p className="text-xs text-text-secondary mt-1">{sourceLine}</p>
        <div className="flex items-center gap-2 mt-1.5">
          {article.sentiment && (
            <span className="flex items-center gap-1 text-xs text-text-secondary">
              <span
                className={`inline-block w-2 h-2 rounded-full ${sentimentColor(article.sentiment)}`}
              />
              {article.sentiment}
            </span>
          )}
          {article.symbol && (
            <span className="text-xs px-1.5 py-0.5 rounded bg-accent-purple/20 text-accent-purple-light font-medium">
              {article.symbol}
            </span>
          )}
        </div>
      </div>
    </a>
  );
}
