import "./globals.css";
import Link from "next/link";

export const metadata = {
  title: "AlgoTrader Dashboard",
};

const navItems = [
  { href: "/", label: "Overview" },
  { href: "/positions", label: "Positions" },
  { href: "/orders", label: "Orders" },
  { href: "/strategies", label: "Strategies" },
  { href: "/backtest", label: "Backtest" },
  { href: "/risk", label: "Risk" },
  { href: "/logs", label: "Logs" },
  { href: "/guide", label: "Guide" },
];

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="bg-gray-50 text-gray-900">
        {/* Top bar */}
        <header className="bg-slate-900 text-white px-6 py-3 flex items-center justify-between">
          <h1 className="text-lg font-semibold tracking-tight">AlgoTrader</h1>
          <span className="text-xs font-medium px-2 py-1 rounded bg-yellow-500 text-yellow-950">
            PAPER MODE
          </span>
        </header>

        <div className="flex min-h-[calc(100vh-52px)]">
          {/* Sidebar nav */}
          <nav className="w-48 bg-slate-800 text-slate-300 flex-shrink-0">
            <ul className="py-4 space-y-1">
              {navItems.map((item) => (
                <li key={item.href}>
                  <Link
                    href={item.href}
                    className="block px-6 py-2 text-sm hover:bg-slate-700 hover:text-white transition-colors"
                  >
                    {item.label}
                  </Link>
                </li>
              ))}
            </ul>
          </nav>

          {/* Main content */}
          <main className="flex-1 p-6">{children}</main>
        </div>
      </body>
    </html>
  );
}
