import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { Settings } from "../types";

interface Props {
  onClose: () => void;
  onSaved: (s: Settings) => void;
}

export default function SettingsPanel({ onClose, onSaved }: Props) {
  const [s, setS] = useState<Settings | null>(null);
  const [bookmarks, setBookmarks] = useState<string[]>([]);
  const [note, setNote] = useState("");

  useEffect(() => {
    api.getSettings().then(setS);
    api.bookmarks().then(setBookmarks);
  }, []);

  if (!s) return null;
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => setS({ ...s, [k]: v });

  const save = async () => {
    const saved = await api.saveSettings(s);
    setS(saved);
    onSaved(saved);
    setNote("Saved. Reload the model for context/model changes to apply.");
  };

  const field = "w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent";

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-4">
      <div className="max-h-full w-full max-w-xl overflow-y-auto rounded-2xl border border-line bg-panel p-5">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">Settings</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-100">✕</button>
        </div>

        <div className="space-y-3 text-sm">
          <label className="block">Model download URL (https)
            <input className={field} value={s.model_url} onChange={(e) => set("model_url", e.target.value)} />
          </label>
          <label className="block">Model file name
            <input className={field} value={s.model_file} onChange={(e) => set("model_file", e.target.value)} />
          </label>
          <label className="block">System prompt
            <textarea className={field} rows={4} value={s.system_prompt} onChange={(e) => set("system_prompt", e.target.value)} />
          </label>

          <div className="grid grid-cols-3 gap-3">
            <label>Temperature
              <input className={field} type="number" step="0.1" value={s.temperature} onChange={(e) => set("temperature", Number(e.target.value))} />
            </label>
            <label>Max tokens
              <input className={field} type="number" value={s.max_tokens} onChange={(e) => set("max_tokens", Number(e.target.value))} />
            </label>
            <label>Context size
              <input className={field} type="number" value={s.context_size} onChange={(e) => set("context_size", Number(e.target.value))} />
            </label>
          </div>

          <label className="flex items-center gap-2">
            <input type="checkbox" checked={s.web_search_default} onChange={(e) => set("web_search_default", e.target.checked)} />
            Use web search by default
          </label>
          <label className="block">Web results per query
            <input className={field} type="number" min={1} max={8} value={s.search_results} onChange={(e) => set("search_results", Number(e.target.value))} />
          </label>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={s.check_updates} onChange={(e) => set("check_updates", e.target.checked)} />
            Check GitHub releases for updates
          </label>
          <label className="block">Update repository (owner/repo)
            <input className={field} value={s.update_repo} onChange={(e) => set("update_repo", e.target.value)} />
          </label>

          <div className="grid grid-cols-2 gap-3">
            <label>Speech language (auto, en, he...)
              <input className={field} value={s.whisper_language} onChange={(e) => set("whisper_language", e.target.value)} />
            </label>
            <label>Speaking speed (-10..10)
              <input className={field} type="number" value={s.speech_rate} onChange={(e) => set("speech_rate", Number(e.target.value))} />
            </label>
            <label>Video: seconds between frames
              <input className={field} type="number" value={s.frame_interval_secs} onChange={(e) => set("frame_interval_secs", Number(e.target.value))} />
            </label>
            <label>Video: max frames
              <input className={field} type="number" value={s.max_frames} onChange={(e) => set("max_frames", Number(e.target.value))} />
            </label>
          </div>
          <label className="block">Vision model URL
            <input className={field} value={s.vision_model_url} onChange={(e) => set("vision_model_url", e.target.value)} />
          </label>
          <label className="block">Vision projector (mmproj) URL
            <input className={field} value={s.mmproj_url} onChange={(e) => set("mmproj_url", e.target.value)} />
          </label>
          <label className="block">Speech model URL
            <input className={field} value={s.whisper_model_url} onChange={(e) => set("whisper_model_url", e.target.value)} />
          </label>

          {bookmarks.length > 0 && (
            <div>
              <div className="mb-1">Bookmarked prompts</div>
              <ul className="space-y-1">
                {bookmarks.map((b) => (
                  <li key={b} className="flex items-center justify-between rounded bg-bg px-2 py-1 text-xs">
                    <span className="truncate">{b}</span>
                    <button
                      className="ml-2 text-slate-500 hover:text-red-400"
                      onClick={async () => {
                        await api.removeBookmark(b);
                        setBookmarks(await api.bookmarks());
                      }}
                    >
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>

        <div className="mt-5 flex items-center justify-between">
          <span className="text-xs text-slate-400">{note}</span>
          <button className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black" onClick={save}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
