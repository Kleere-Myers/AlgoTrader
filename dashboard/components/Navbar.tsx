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
    <nav className="w-full bg-navy-900 border-b border-navy-600">
      <div className="max-w-[1600px] mx-auto px-4 flex items-center justify-between h-14">
        {/* Left: Logo */}
        <Link href="/" className="flex items-center gap-1 shrink-0">
          <span className="text-white font-bold text-lg tracking-tight">
            Algo
          </span>
          <span className="text-white font-bold text-lg tracking-tight">
            Trader
          </span>
          <span className="w-1.5 h-1.5 rounded-full bg-accent-purple inline-block mb-2" />
        </Link>

        {/* Center: Nav links */}
        <div className="flex items-center gap-1 overflow-x-auto hide-scrollbar">
          {NAV_LINKS.map((link) => {
            const isActive =
              link.href === "/"
                ? pathname === "/"
                : pathname.startsWith(link.href);

            return (
              <Link
                key={link.href}
                href={link.href}
                className={`px-3 py-4 text-sm whitespace-nowrap transition-colors border-b-2 ${
                  isActive
                    ? "text-white border-accent-purple"
                    : "text-text-secondary border-transparent hover:text-text-primary"
                }`}
              >
                {link.label}
              </Link>
            );
          })}
        </div>

        {/* Right: Paper mode badge */}
        <span className="shrink-0 text-xs font-semibold px-2.5 py-1 rounded bg-yellow-500/20 text-yellow-400 border border-yellow-500/30 tracking-wide">
          PAPER MODE
        </span>
      </div>
    </nav>
  );
}
