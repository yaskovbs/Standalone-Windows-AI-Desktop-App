import { KeyboardEvent, useState } from "react";
import { api } from "../lib/api";

interface Props {
  generating: boolean;
  online: boolean;
  webDefault: boolean;
  onSend: (text: string, web: boolean) => void;
  onStop: () => void;
}

export default function Composer({ generating, online, webDefault, onSend, onStop }: Props) {
  const [text, setText] = useState("");
  const [web, setWeb] = useState(webDefault);
  const [saved, setSaved] = useState(false);

  const submit = () => {
    if (!text.trim() || generating) return;
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

  return (
    <div className="border-t border-line bg-panel p-3">
      <div className="mx-auto flex max-w-3xl items-end gap-2">
        <textarea
          className="max-h-40 min-h-[44px] flex-1 resize-none rounded-xl border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent"
          placeholder="Message the assistant... (Enter to send, Shift+Enter for newline)"
          value={text}
          rows={1}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKey}
        />
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
      <label className="mx-auto mt-2 flex max-w-3xl items-center gap-2 text-xs text-slate-400">
        <input type="checkbox" checked={web && online} disabled={!online} onChange={(e) => setWeb(e.target.checked)} />
        Search the web for fresh context {online ? "" : "(offline - local model only)"}
      </label>
    </div>
  );
}
