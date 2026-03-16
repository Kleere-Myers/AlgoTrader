export default function StrategiesPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Strategies</h2>
      <p className="text-gray-500 mb-6">
        Enable/disable strategies, edit parameters, and trigger backtest runs.
      </p>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <StrategyPlaceholder name="MovingAverageCrossover" params="fast=10, slow=30" />
        <StrategyPlaceholder name="RSIMeanReversion" params="period=14, oversold=30, overbought=70" />
        <StrategyPlaceholder name="MomentumVolume" params="lookback=20, vol_mult=1.5" />
        <StrategyPlaceholder name="MLSignalGenerator" params="min_confidence=0.65" />
      </div>
    </div>
  );
}

function StrategyPlaceholder({ name, params }: { name: string; params: string }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <h3 className="font-semibold text-sm">{name}</h3>
        <span className="text-xs px-2 py-0.5 rounded bg-gray-100 text-gray-400">
          disabled
        </span>
      </div>
      <p className="text-xs text-gray-400 mb-3">{params}</p>
      <div className="flex gap-2">
        <button
          disabled
          className="text-xs px-3 py-1 rounded bg-blue-50 text-blue-400 cursor-not-allowed"
        >
          Run Backtest
        </button>
        <button
          disabled
          className="text-xs px-3 py-1 rounded bg-gray-50 text-gray-400 cursor-not-allowed"
        >
          Edit Params
        </button>
      </div>
    </div>
  );
}
