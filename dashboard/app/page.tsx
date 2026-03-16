export default function OverviewPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Overview</h2>
      <p className="text-gray-500">
        Account equity, daily P&amp;L, open positions summary, and trading status.
      </p>
      <div className="mt-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <Placeholder label="Equity" />
        <Placeholder label="Daily P&L" />
        <Placeholder label="Buying Power" />
      </div>
      <div className="mt-6 rounded border border-dashed border-gray-300 p-12 text-center text-gray-400">
        Equity curve chart will render here
      </div>
      <div className="mt-6 rounded border border-dashed border-gray-300 p-8 text-center text-gray-400">
        Open positions summary will render here
      </div>
    </div>
  );
}

function Placeholder({ label }: { label: string }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 shadow-sm">
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="mt-1 text-2xl font-semibold text-gray-300">--</p>
    </div>
  );
}
