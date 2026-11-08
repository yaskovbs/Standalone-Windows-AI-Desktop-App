import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { toolsApi } from "../lib/toolsApi";
import type { RecapResult, RecapRow } from "../types";
import AssetDownloads from "./AssetDownloads";
import Modal from "./Modal";

const mmss = (s: number) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export default function RecapPanel({ onClose }: { onClose: () => void }) {
  const [path, setPath] = useState("");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [targetSecs, setTargetSecs] = useState(120);
  const [clipSecs, setClipSecs] = useState(8);
  const [maxFrames, setMaxFrames] = useState(240);
  const [narrate, setNarrate] = useState(true);
  const [language, setLanguage] = useState("English");
  const [sceneDetect, setSceneDetect] = useState(true);

  const [working, setWorking] = useState(false);
  const [stage, setStage] = useState("");
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [result, setResult] = useState<RecapResult | null>(null);
  const [history, setHistory] = useState<RecapRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  const loadHistory = () => toolsApi.listRecaps().then(setHistory).catch(() => undefined);
  useEffect(() => {
    loadHistory();
  }, []);

  const browse = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Video", extensions: ["mp4", "mkv", "avi", "mov", "webm", "m4v", "wmv"] }],
    });
    if (typeof picked === "string") {
      setPath(picked);
      if (!title) setTitle(picked.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "");
    }
  };

  const make = async () => {
    setError(null);
    setResult(null);
    setWorking(true);
    try {
      const r = await toolsApi.makeRecap(
        { path, title, description, target_secs: targetSecs, clip_secs: clipSecs, max_frames: maxFrames, narrate, language, scene_detect: sceneDetect },
        (e) => (e.event === "stage" ? setStage(e.data.label) : setProgress(e.data)),
      );
      setResult(r);
      loadHistory();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
      setStage("");
      setProgress(null);
    }
  };

  const clipsWanted = Math.max(1, Math.round(targetSecs / clipSecs));
  const field = "w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent";

  return (
    <Modal title="Recap maker (on-device AI)" onClose={onClose}>
      <p className="mb-3 text-xs text-slate-400">
        Your local models watch the film, rate the story moments, and ffmpeg cuts them into a recap. No cloud AI is used
        and the file never leaves this computer. No file-size limit.
      </p>
      <AssetDownloads />

      <div className="space-y-3 text-sm">
        <div className="flex gap-2">
          <input className={field} placeholder="C:\Movies\film.mkv" value={path} onChange={(e) => setPath(e.target.value)} />
          <button className="rounded-lg border border-line px-3" onClick={browse}>Browse</button>
        </div>
        <input className={field} placeholder="Title of the film / episode" value={title} onChange={(e) => setTitle(e.target.value)} />
        <textarea
          className={field}
          rows={3}
          placeholder="Describe the plot in a few lines (optional but it helps the model pick the right moments)"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />

        <div className="grid grid-cols-2 gap-3">
          <label>Recap length: {mmss(targetSecs)}
            <input type="range" min={30} max={900} step={15} className="w-full" value={targetSecs} onChange={(e) => setTargetSecs(Number(e.target.value))} />
          </label>
          <label>Seconds per clip: {clipSecs}
            <input type="range" min={3} max={20} className="w-full" value={clipSecs} onChange={(e) => setClipSecs(Number(e.target.value))} />
          </label>
        </div>
        <p className="text-xs text-slate-500">≈ {clipsWanted} clips of {clipSecs}s.</p>

        <div className="grid grid-cols-2 gap-3">
          <label>Frames to analyse (more = better, slower)
            <input className={field} type="number" min={20} max={2000} value={maxFrames} onChange={(e) => setMaxFrames(Number(e.target.value))} />
          </label>
          <label>Narration language
            <input className={field} value={language} onChange={(e) => setLanguage(e.target.value)} />
          </label>
        </div>
        <div className="flex gap-5 text-xs text-slate-300">
          <label className="flex items-center gap-1"><input type="checkbox" checked={narrate} onChange={(e) => setNarrate(e.target.checked)} /> Add spoken narration (Windows voice)</label>
          <label className="flex items-center gap-1"><input type="checkbox" checked={sceneDetect} onChange={(e) => setSceneDetect(e.target.checked)} /> Detect scene cuts (extra pass)</label>
        </div>

        <div className="flex items-center gap-2">
          <button className="rounded-lg bg-accent px-4 py-2 font-medium text-black disabled:opacity-40" disabled={!path.trim() || working} onClick={make}>
            {working ? "Working..." : "Create recap"}
          </button>
          {working && <button className="rounded-lg border border-line px-3 py-2" onClick={() => toolsApi.cancelMedia()}>Cancel</button>}
          <span className="text-xs text-slate-400">
            {stage}{progress && ` ${progress.done}/${progress.total}`}
          </span>
        </div>
      </div>

      {error && <p className="mt-4 rounded-lg bg-red-500/10 p-2 text-sm text-red-300">{error}</p>}

      {result && (
        <section className="mt-5 rounded-xl border border-line p-3 text-sm">
          <div className="font-medium text-emerald-300">Recap ready ({mmss(result.total_secs)})</div>
          <div className="mt-1 break-all text-xs text-slate-300">{result.output_path}</div>
          <button className="mt-2 text-xs text-accent" onClick={() => toolsApi.revealFile(result.output_path)}>Show in folder</button>
          {!result.rated_by_model && (
            <p className="mt-2 text-xs text-amber-300">The language model could not rate the moments, so loudness, scene cuts and dialogue were used instead.</p>
          )}
          {result.narration && !result.voiceover && (
            <p className="mt-2 text-xs text-amber-300">Narration text was written but the voice-over could not be recorded (Windows speech is required).</p>
          )}
          <ul className="mt-3 max-h-40 space-y-1 overflow-y-auto text-xs text-slate-400">
            {result.clips.map((c) => (
              <li key={c.start}>[{mmss(c.start)}–{mmss(c.end)}] {c.summary}</li>
            ))}
          </ul>
          {result.narration && <p className="mt-3 whitespace-pre-wrap rounded-lg bg-bg p-3 text-xs">{result.narration}</p>}
        </section>
      )}

      {history.length > 0 && (
        <section className="mt-5">
          <h3 className="mb-1 text-sm font-medium">Saved recaps</h3>
          <ul className="space-y-1 text-xs">
            {history.map((r) => (
              <li key={r.id} className="flex items-center justify-between rounded bg-bg px-2 py-1">
                <span className="truncate">{r.title} · {mmss(r.duration)}</span>
                <span className="ml-2 flex shrink-0 gap-3">
                  <button className="text-accent" onClick={() => toolsApi.revealFile(r.path).catch((e) => setError(String(e)))}>Show</button>
                  <button className="text-slate-500 hover:text-red-300" onClick={() => toolsApi.deleteRecap(r.id).then(loadHistory)}>Remove</button>
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </Modal>
  );
}
