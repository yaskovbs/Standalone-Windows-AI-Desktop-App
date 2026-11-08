import { useCallback, useEffect, useState } from "react";
import { formatBytes } from "../lib/api";
import { toolsApi } from "../lib/toolsApi";
import type { ToolsStatus } from "../types";

type Kind = "whisper" | "vision" | "mmproj";

/** Shows which ears/eyes components are ready and downloads the missing models. */
export default function AssetDownloads({ onChange }: { onChange?: (s: ToolsStatus) => void }) {
  const [status, setStatus] = useState<ToolsStatus | null>(null);
  const [active, setActive] = useState<Kind | null>(null);
  const [pct, setPct] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const s = await toolsApi.status();
    setStatus(s);
    onChange?.(s);
  }, [onChange]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const download = async (kind: Kind) => {
    setActive(kind);
    setError(null);
    try {
      await toolsApi.downloadAsset(kind, (e) => {
        if (e.event === "progress") setPct({ done: e.data.downloaded, total: e.data.total });
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setActive(null);
      setPct(null);
      refresh();
    }
  };

  if (!status) return null;

  const rows: { label: string; ok: boolean; kind?: Kind; hint?: string }[] = [
    { label: "ffmpeg (video frames and audio)", ok: status.ffmpeg, hint: "ffmpeg.exe" },
    { label: "whisper-cli (hearing)", ok: status.whisper_cli, hint: "whisper-cli.exe" },
    { label: "llama-mtmd-cli (seeing)", ok: status.vision_cli, hint: "llama-mtmd-cli.exe" },
    { label: "Speech model", ok: status.whisper_model, kind: "whisper" },
    { label: "Vision model", ok: status.vision_model, kind: "vision" },
    { label: "Vision projector", ok: status.mmproj, kind: "mmproj" },
  ];

  return (
    <div className="mb-4 rounded-xl border border-line p-3 text-sm">
      <ul className="space-y-1">
        {rows.map((r) => (
          <li key={r.label} className="flex items-center justify-between">
            <span className={r.ok ? "text-slate-300" : "text-amber-300"}>
              {r.ok ? "✓" : "•"} {r.label}
            </span>
            {!r.ok && r.kind && (
              <button
                className="rounded bg-accent px-2 py-0.5 text-xs font-medium text-black disabled:opacity-40"
                disabled={active !== null}
                onClick={() => download(r.kind as Kind)}
              >
                {active === r.kind && pct ? `${formatBytes(pct.done)} / ${pct.total ? formatBytes(pct.total) : "?"}` : "Download"}
              </button>
            )}
          </li>
        ))}
      </ul>
      {(!status.ffmpeg || !status.whisper_cli || !status.vision_cli) && (
        <p className="mt-2 text-xs text-slate-400">
          Put the missing .exe files (and their DLLs) in <code>{status.tools_dir}</code>, or add them to PATH.
        </p>
      )}
      {error && <p className="mt-2 text-xs text-red-300">{error}</p>}
    </div>
  );
}
