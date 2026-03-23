"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const NAV_LINKS = [
  { label: "Overview", href: "/" },
  { label: "Watchlist", href: "/watchlist" },
  { label: "Positions", href: "/positions" },
  { label: "Orders", href: "/orders" },
  { label: "Strategies", href: "/strategies" },
  { label: "Backtest", href: "/backtest" },
  { label: "Risk", href: "/risk" },
  { label: "Logs", href: "/logs" },
  { label: "Guide", href: "/guide" },
];

export default function Navbar() {
  const pathname = usePathname();

  return (
    <nav className="sticky top-0 z-50 w-full border-b border-white/[0.06] bg-surface-950/80 backdrop-blur-xl">
      <div className="max-w-[1440px] mx-auto px-6 flex items-center justify-between h-14">
        {/* Logo */}
        <Link href="/" className="flex items-center gap-2 shrink-0 group">
          <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-accent to-accent-dark flex items-center justify-center shadow-lg shadow-accent/20 group-hover:shadow-accent/30 transition-shadow">
            <span className="text-surface-950 font-bold text-[11px] tracking-tight">AT</span>
          </div>
          <span className="text-text-primary font-semibold text-sm tracking-tight">
            AlgoTrader
          </span>
        </Link>

        {/* Nav links */}
        <div className="flex items-center gap-0.5 overflow-x-auto hide-scrollbar">
          {NAV_LINKS.map((link) => {
            const isActive =
              link.href === "/"
                ? pathname === "/"
                : pathname.startsWith(link.href);

            return (
              <Link
                key={link.href}
                href={link.href}
                className={`px-3 py-1.5 text-[13px] font-medium rounded-md whitespace-nowrap transition-all duration-150 ${
                  isActive
                    ? "text-text-primary bg-white/[0.08]"
                    : "text-text-secondary hover:text-text-primary hover:bg-white/[0.04]"
                }`}
              >
                {link.label}
              </Link>
            );
          })}
        </div>

        {/* Paper mode */}
        <span className="shrink-0 text-[11px] font-mono font-medium px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 tracking-wider uppercase">
          Paper
        </span>
      </div>
    </nav>
  );
}
