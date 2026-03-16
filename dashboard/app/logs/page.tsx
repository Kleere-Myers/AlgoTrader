export default function LogsPage() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Logs</h2>
      <p className="text-gray-500 mb-6">
        Real-time signal and order event stream from the execution engine.
      </p>
      <div className="rounded-lg border border-gray-200 bg-white shadow-sm overflow-hidden">
        <div className="px-4 py-3 bg-gray-50 border-b border-gray-200 flex items-center justify-between">
          <span className="text-xs text-gray-500 uppercase tracking-wide">
            SSE Event Stream
          </span>
          <span className="inline-flex items-center gap-1.5 text-xs text-gray-400">
            <span className="w-2 h-2 rounded-full bg-gray-300" />
            Disconnected
          </span>
        </div>
        <div className="p-4 h-96 overflow-y-auto font-mono text-xs text-gray-400">
          Waiting for events...
        </div>
      </div>
    </div>
  );
}
