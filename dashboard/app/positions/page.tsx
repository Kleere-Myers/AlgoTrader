export default function PositionsPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Positions</h2>
      <p className="text-gray-500 mb-6">
        Open positions with entry price, current price, unrealized P&amp;L, and duration.
      </p>
      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Qty</th>
              <th className="px-4 py-3">Entry Price</th>
              <th className="px-4 py-3">Current Price</th>
              <th className="px-4 py-3">Unrealized P&amp;L</th>
              <th className="px-4 py-3">Duration</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="px-4 py-8 text-center text-gray-400" colSpan={6}>
                No open positions
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
