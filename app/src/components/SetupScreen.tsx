import { formatBytes } from "../lib/api";
import type { SetupState } from "../hooks/useSetup";

export default function SetupScreen({ setup }: { setup: SetupState }) {
  const { status, hardware, online, progress, busy, error, start } = setup;
  const pct = progress && progress.total > 0 ? Math.round((progress.downloaded / progress.total) * 100) : 0;
  const needsDownload = status && !status.present;

  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-full max-w-md space-y-4 rounded-2xl border border-line bg-panel p-6">
        <h1 className="text-xl font-semibold">Setting up LocalAutonomousAI</h1>

        <ul className="space-y-1 text-sm text-slate-400">
          <li>Internet: {online ? "connected" : "not connected"}</li>
          <li>
            Compute:{" "}
            {hardware?.gpu_name
              ? `${hardware.gpu_name} (${hardware.vram_mb} MB VRAM)`
              : `CPU, ${hardware?.cpu_threads ?? "?"} threads${hardware?.avx2 ? ", AVX2" : ""}`}
          </li>
          <li>Model: {status?.name ?? "..."}</li>
        </ul>

        {progress && (
          <div>
            <div className="h-2 overflow-hidden rounded bg-bg">
              <div className="h-full bg-accent transition-all" style={{ width: `${pct}%` }} />
            </div>
            <p className="mt-1 text-xs text-slate-400">
              {formatBytes(progress.downloaded)} / {progress.total ? formatBytes(progress.total) : "?"} ({pct}%)
            </p>
          </div>
        )}

        {error && <p className="rounded-lg bg-red-500/10 p-2 text-sm text-red-300">{error}</p>}

        {needsDownload && !online && (
          <p className="text-sm text-amber-300">An internet connection is required to download the model.</p>
        )}

        <button
          className="w-full rounded-lg bg-accent py-2 text-sm font-medium text-black disabled:opacity-40"
          disabled={busy || (needsDownload === true && !online)}
          onClick={start}
        >
          {busy ? "Working..." : needsDownload ? "Download & start" : "Load model"}
        </button>
      </div>
    </div>
  );
}
