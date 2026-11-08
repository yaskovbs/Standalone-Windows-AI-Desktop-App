import { KeyboardEvent, useCallback, useState } from "react";
import { useDictation } from "../hooks/useVoice";
import { api } from "../lib/api";
import { toolsApi } from "../lib/toolsApi";

interface Props {
  generating: boolean;
  online: boolean;
  webDefault: boolean;
  speak: boolean;
  onSpeakChange: (v: boolean) => void;
  onSend: (text: string, web: boolean) => void;
  onStop: () => void;
}

export default function Composer({ generating, online, webDefault, speak, onSpeakChange, onSend, onStop }: Props) {
  const [text, setText] = useState("");
  const [web, setWeb] = useState(webDefault);
  const [autoSend, setAutoSend] = useState(true);
  const [saved, setSaved] = useState(false);

  const onVoiceText = useCallback(
    (t: string) => {
      if (autoSend) onSend(t, web && online);
      else setText((prev) => (prev ? `${prev} ${t}` : t));
    },
    [autoSend, onSend, web, online],
  );
  const voice = useDictation(onVoiceText);

  const submit = () => {
    if (!text.trim() || generating) return;
    toolsApi.stopSpeaking().catch(() => undefined);
    onSend(text, web && online);
    setText("");
  };

  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const bookmark = async () => {
    if (!text.trim()) return;
    await api.addBookmark(text);
    setSaved(true);
    setTimeout(() => setSaved(false), 1200);
  };

  const micLabel = voice.transcribing ? "..." : voice.recording ? "■ Stop" : "🎤";

  return (
    <div className="border-t border-line bg-panel p-3">
      <div className="mx-auto flex max-w-3xl items-end gap-2">
        <textarea
          className="max-h-40 min-h-[44px] flex-1 resize-none rounded-xl border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent"
          placeholder="Type or press the microphone... (Enter to send, Shift+Enter for newline)"
          value={text}
          rows={1}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKey}
        />
        <button
          title={voice.recording ? "Stop and transcribe" : "Speak"}
          disabled={voice.transcribing || generating}
          className={`h-[44px] rounded-xl border px-3 text-sm disabled:opacity-40 ${
            voice.recording ? "animate-pulse border-red-400 text-red-300" : "border-line text-slate-300"
          }`}
          onClick={voice.toggle}
        >
          {micLabel}
        </button>
        <button
          title="Bookmark this prompt"
          className="h-[44px] rounded-xl border border-line px-3 text-sm text-slate-400 hover:text-slate-100"
          onClick={bookmark}
        >
          {saved ? "Saved" : "★"}
        </button>
        {generating ? (
          <button className="h-[44px] rounded-xl bg-red-500/80 px-4 text-sm font-medium" onClick={onStop}>
            Stop
          </button>
        ) : (
          <button
            className="h-[44px] rounded-xl bg-accent px-4 text-sm font-medium text-black disabled:opacity-40"
            disabled={!text.trim()}
            onClick={submit}
          >
            Send
          </button>
        )}
      </div>

      <div className="mx-auto mt-2 flex max-w-3xl flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-400">
        <label className="flex items-center gap-1">
          <input type="checkbox" checked={web && online} disabled={!online} onChange={(e) => setWeb(e.target.checked)} />
          Web search {online ? "" : "(offline)"}
        </label>
        <label className="flex items-center gap-1">
          <input type="checkbox" checked={speak} onChange={(e) => onSpeakChange(e.target.checked)} />
          Speak replies
        </label>
        <label className="flex items-center gap-1">
          <input type="checkbox" checked={autoSend} onChange={(e) => setAutoSend(e.target.checked)} />
          Send voice automatically
        </label>
        <button className="hover:text-slate-100" onClick={() => toolsApi.stopSpeaking()}>
          Silence
        </button>
        {voice.error && <span className="text-red-300">{voice.error}</span>}
      </div>
    </div>
  );
}
