export default function OrdersPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Orders</h2>
      <p className="text-gray-500 mb-6">
        Recent order history with fill prices, status, and strategy attribution.
      </p>
      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Time</th>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Side</th>
              <th className="px-4 py-3">Qty</th>
              <th className="px-4 py-3">Fill Price</th>
              <th className="px-4 py-3">Status</th>
              <th className="px-4 py-3">Strategy</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="px-4 py-8 text-center text-gray-400" colSpan={7}>
                No orders yet
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
