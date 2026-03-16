export default function BacktestPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Backtest Results</h2>
      <p className="text-gray-500 mb-6">
        Equity curves, metrics, and performance comparison per strategy and symbol.
      </p>
      <div className="rounded border border-dashed border-gray-300 p-12 text-center text-gray-400">
        Backtest equity curve chart will render here
      </div>
      <div className="mt-6 overflow-x-auto">
        <table className="w-full text-sm text-left border border-gray-200 bg-white rounded-lg">
          <thead className="bg-gray-50 text-gray-500 uppercase text-xs">
            <tr>
              <th className="px-4 py-3">Strategy</th>
              <th className="px-4 py-3">Symbol</th>
              <th className="px-4 py-3">Return %</th>
              <th className="px-4 py-3">Sharpe</th>
              <th className="px-4 py-3">Max DD %</th>
              <th className="px-4 py-3">Win Rate</th>
              <th className="px-4 py-3">Trades</th>
              <th className="px-4 py-3">Profit Factor</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="px-4 py-8 text-center text-gray-400" colSpan={8}>
                No backtest results yet — run a backtest from the Strategies page
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
