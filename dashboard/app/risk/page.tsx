export default function RiskPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Risk Settings</h2>
      <p className="text-gray-500 mb-6">
        View risk rule thresholds and control trading halt state.
      </p>

      {/* Risk rules table */}
      <div className="overflow-x-auto mb-8">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Rule</th>
              <th className="px-4 py-3">Current Value</th>
              <th className="px-4 py-3">Breach Behavior</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            <RiskRow rule="Max daily loss" value="2% of equity" breach="Halt all trading" />
            <RiskRow rule="Max position size" value="10% of equity" breach="Reject signal" />
            <RiskRow rule="Max open positions" value="4" breach="Reject signal" />
            <RiskRow rule="Min signal confidence" value="0.60" breach="Reject signal" />
            <RiskRow rule="Order throttle" value="5 min per symbol" breach="Throttle" />
            <RiskRow rule="EOD flatten" value="3:45 PM ET" breach="Market-sell all" />
          </tbody>
        </table>
      </div>

      {/* Daily loss progress */}
      <div className="mb-8 rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
        <p className="text-xs text-gray-400 uppercase tracking-wide mb-2">
          Daily Loss Used vs Limit
        </p>
        <div className="w-full h-4 bg-gray-100 rounded-full overflow-hidden">
          <div className="h-full bg-green-500 rounded-full" style={{ width: "0%" }} />
        </div>
        <p className="text-xs text-gray-400 mt-1">$0.00 / -- limit</p>
      </div>

      {/* Emergency halt */}
      <div className="rounded-lg border border-red-200 bg-red-50 p-5">
        <h3 className="text-sm font-semibold text-red-800 mb-2">Emergency Trading Halt</h3>
        <p className="text-xs text-red-600 mb-4">
          Immediately halt all order submission. Open positions will NOT be closed automatically.
        </p>
        <button
          disabled
          className="px-4 py-2 rounded bg-red-300 text-white font-semibold text-sm cursor-not-allowed"
        >
          Halt Trading
        </button>
      </div>
    </div>
  );
}

function RiskRow({
  rule,
  value,
  breach,
}: {
  rule: string;
  value: string;
  breach: string;
}) {
  return (
    <tr>
      <td className="px-4 py-3 font-medium">{rule}</td>
      <td className="px-4 py-3">{value}</td>
      <td className="px-4 py-3 text-gray-500">{breach}</td>
    </tr>
  );
}
