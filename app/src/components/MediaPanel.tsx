import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { toolsApi } from "../lib/toolsApi";
import type { MediaMemory } from "../types";
import AssetDownloads from "./AssetDownloads";
import Modal from "./Modal";

interface Props {
  onClose: () => void;
  ensureConversation: () => Promise<number>;
}

export default function MediaPanel({ onClose, ensureConversation }: Props) {
  const [path, setPath] = useState("");
  const [stage, setStage] = useState("");
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [memory, setMemory] = useState<MediaMemory | null>(null);
  const [watching, setWatching] = useState(false);
  const [screenQ, setScreenQ] = useState("What is on the screen right now?");
  const [screenA, setScreenA] = useState("");
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);

  const browse = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Video", extensions: ["mp4", "mkv", "avi", "mov", "webm", "m4v", "wmv"] }],
    });
    if (typeof picked === "string") setPath(picked);
  };

  const watch = async () => {
    setError(null);
    setMemory(null);
    setNote("");
    setWatching(true);
    try {
      const m = await toolsApi.watchMedia(path, (e) => {
        if (e.event === "stage") setStage(e.data.label);
        else setProgress(e.data);
      });
      setMemory(m);
    } catch (e) {
      setError(String(e));
    } finally {
      setWatching(false);
      setStage("");
      setProgress(null);
    }
  };

  const attach = async () => {
    if (!memory) return;
    const id = await ensureConversation();
    await toolsApi.attachMedia(id, memory);
    setNote("Attached. Ask questions about the video in the chat.");
  };

  const lookAtScreen = async () => {
    setError(null);
    setScreenA("Looking...");
    try {
      setScreenA(await toolsApi.describeScreen(screenQ));
    } catch (e) {
      setScreenA("");
      setError(String(e));
    }
  };

  const field = "w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent";

  return (
    <Modal title="Eyes & ears" onClose={onClose}>
      <AssetDownloads />

      <section className="mb-5">
        <h3 className="mb-1 text-sm font-medium">Watch a movie or episode (local file)</h3>
        <div className="flex gap-2">
          <input className={field} placeholder="C:\\Videos\\episode.mkv" value={path} onChange={(e) => setPath(e.target.value)} />
          <button className="rounded-lg border border-line px-3 text-sm" onClick={browse}>
            Browse
          </button>
        </div>
        <div className="mt-2 flex items-center gap-2">
          <button
            className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black disabled:opacity-40"
            disabled={!path.trim() || watching}
            onClick={watch}
          >
            {watching ? "Watching..." : "Watch"}
          </button>
          {watching && (
            <button className="rounded-lg border border-line px-3 py-2 text-sm" onClick={() => toolsApi.cancelMedia()}>
              Cancel
            </button>
          )}
          <span className="text-xs text-slate-400">
            {stage}
            {progress && ` ${progress.done}/${progress.total} frames`}
          </span>
        </div>
        <p className="mt-2 text-xs text-slate-500">
          It samples frames and transcribes the audio, so it follows the story but does not see every moment.
          Streaming services (Netflix etc.) cannot be read - use files you own.
        </p>
      </section>

      {memory && (
        <section className="mb-5">
          <div className="mb-1 text-sm">
            {memory.title}: {memory.frames} frames, {memory.heard_audio ? "audio transcribed" : "no audio transcript"}
          </div>
          <textarea readOnly className={`${field} h-40 font-mono text-xs`} value={memory.timeline} />
          <div className="mt-2 flex items-center gap-3">
            <button className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black" onClick={attach}>
              Attach to chat
            </button>
            <span className="text-xs text-emerald-300">{note}</span>
          </div>
        </section>
      )}

      <section>
        <h3 className="mb-1 text-sm font-medium">Look at my screen</h3>
        <div className="flex gap-2">
          <input className={field} value={screenQ} onChange={(e) => setScreenQ(e.target.value)} />
          <button className="rounded-lg border border-line px-3 text-sm" onClick={lookAtScreen}>
            Look
          </button>
        </div>
        {screenA && <p className="mt-2 whitespace-pre-wrap rounded-lg bg-bg p-3 text-sm">{screenA}</p>}
      </section>

      {error && <p className="mt-4 rounded-lg bg-red-500/10 p-2 text-sm text-red-300">{error}</p>}
    </Modal>
  );
}
