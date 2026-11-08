import { useEffect, useState } from "react";
import ChatView from "./components/ChatView";
import Composer from "./components/Composer";
import SettingsPanel from "./components/SettingsPanel";
import SetupScreen from "./components/SetupScreen";
import Sidebar from "./components/Sidebar";
import { useChat } from "./hooks/useChat";
import { useSetup } from "./hooks/useSetup";
import { api } from "./lib/api";

export default function App() {
  const setup = useSetup();
  const chat = useChat();
  const [showSettings, setShowSettings] = useState(false);
  const [webDefault, setWebDefault] = useState(true);

  useEffect(() => {
    api.getSettings().then((s) => setWebDefault(s.web_search_default));
  }, []);

  if (!setup.status?.loaded) return <SetupScreen setup={setup} />;

  return (
    <div className="flex h-full flex-col">
      {setup.update?.update_available && (
        <a
          href={setup.update.url}
          target="_blank"
          rel="noreferrer"
          className="bg-accent/20 px-4 py-1 text-center text-xs text-accent"
        >
          Update available: {setup.update.latest} (you have {setup.update.current})
        </a>
      )}

      <div className="flex min-h-0 flex-1">
        <Sidebar
          conversations={chat.conversations}
          activeId={chat.activeId}
          onOpen={chat.open}
          onNew={chat.newChat}
          onDelete={chat.remove}
          onSettings={() => setShowSettings(true)}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          <ChatView messages={chat.messages} error={chat.error} stats={chat.stats} />
          <Composer
            key={webDefault ? "web" : "noweb"}
            generating={chat.generating}
            online={setup.online}
            webDefault={webDefault}
            onSend={chat.send}
            onStop={chat.stop}
          />
        </main>
      </div>

      <footer className="border-t border-line bg-panel px-4 py-1 text-xs text-slate-500">
        {setup.status.name} · {setup.status.gpu_layers > 0 ? `GPU (${setup.status.gpu_layers} layers)` : "CPU"} ·{" "}
        {setup.online ? "online" : "offline"}
      </footer>

      {showSettings && (
        <SettingsPanel onClose={() => setShowSettings(false)} onSaved={(s) => setWebDefault(s.web_search_default)} />
      )}
    </div>
  );
}
