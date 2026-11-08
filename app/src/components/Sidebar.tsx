import type { Conversation } from "../types";

interface Props {
  conversations: Conversation[];
  activeId: number | null;
  onOpen: (id: number) => void;
  onNew: () => void;
  onDelete: (id: number) => void;
  onSettings: () => void;
  onMedia: () => void;
  onGithub: () => void;
  onRecap: () => void;
}

export default function Sidebar({ conversations, activeId, onOpen, onNew, onDelete, onSettings, onMedia, onGithub, onRecap }: Props) {
  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-line bg-panel">
      <button
        className="m-3 rounded-lg bg-accent px-3 py-2 text-sm font-medium text-black"
        onClick={onNew}
      >
        + New chat
      </button>

      <nav className="flex-1 space-y-1 overflow-y-auto px-2">
        {conversations.map((c) => (
          <div
            key={c.id}
            className={`group flex items-center justify-between rounded-lg px-3 py-2 text-sm ${
              c.id === activeId ? "bg-white/10" : "hover:bg-white/5"
            }`}
          >
            <button className="flex-1 truncate text-left" onClick={() => onOpen(c.id)}>
              {c.title}
            </button>
            <button
              aria-label="Delete conversation"
              className="ml-2 hidden text-slate-500 hover:text-red-400 group-hover:block"
              onClick={() => onDelete(c.id)}
            >
              ✕
            </button>
          </div>
        ))}
        {conversations.length === 0 && <p className="px-3 py-2 text-xs text-slate-500">No conversations yet.</p>}
      </nav>

      <div className="border-t border-line text-sm text-slate-400">
        <button className="block w-full px-4 py-2 text-left hover:text-slate-100" onClick={onRecap}>
          🎬 Recap maker
        </button>
        <button className="block w-full px-4 py-2 text-left hover:text-slate-100" onClick={onMedia}>
          👁 Eyes &amp; ears
        </button>
        <button className="block w-full px-4 py-2 text-left hover:text-slate-100" onClick={onGithub}>
          ⬆ Self-update (GitHub)
        </button>
        <button className="block w-full px-4 py-2 text-left hover:text-slate-100" onClick={onSettings}>
          ⚙ Settings
        </button>
      </div>
    </aside>
  );
}
