import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ChatEvent,
  Conversation,
  DownloadEvent,
  HardwareInfo,
  ModelStatus,
  Settings,
  StoredMessage,
  UpdateInfo,
  WebSource,
} from "../types";

/** Typed wrappers around the Rust #[tauri::command] functions. */
export const api = {
  hardware: () => invoke<HardwareInfo>("hardware_info"),
  modelStatus: () => invoke<ModelStatus>("model_status"),
  loadModel: () => invoke<ModelStatus>("load_model"),
  isOnline: () => invoke<boolean>("is_online"),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),

  downloadModel(onEvent: (e: DownloadEvent) => void) {
    const channel = new Channel<DownloadEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("download_model", { onEvent: channel });
  },

  conversations: () => invoke<Conversation[]>("list_conversations"),
  createConversation: () => invoke<number>("create_conversation"),
  deleteConversation: (id: number) => invoke<void>("delete_conversation", { id }),
  renameConversation: (id: number, title: string) =>
    invoke<void>("rename_conversation", { id, title }),
  messages: (id: number) => invoke<StoredMessage[]>("get_messages", { id }),

  chat(conversationId: number, content: string, web: boolean, onEvent: (e: ChatEvent) => void) {
    const channel = new Channel<ChatEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("chat", { conversationId, content, web, onEvent: channel });
  },
  stop: () => invoke<void>("stop_generation"),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),

  bookmarks: () => invoke<string[]>("list_bookmarks"),
  addBookmark: (prompt: string) => invoke<void>("add_bookmark", { prompt }),
  removeBookmark: (prompt: string) => invoke<void>("remove_bookmark", { prompt }),

  webSearch: (query: string) => invoke<WebSource[]>("web_search", { query }),
};

export function formatBytes(n: number): string {
  if (n >= 1 << 30) return `${(n / (1 << 30)).toFixed(2)} GB`;
  if (n >= 1 << 20) return `${(n / (1 << 20)).toFixed(1)} MB`;
  return `${(n / 1024).toFixed(0)} KB`;
}

/** Split an assistant reply into its <reasoning> and <answer> parts (tolerates partial streams). */
export function parseReply(text: string): { reasoning: string; answer: string } {
  const r = text.match(/<reasoning>([\s\S]*?)(?:<\/reasoning>|$)/);
  const a = text.match(/<answer>([\s\S]*?)(?:<\/answer>|$)/);
  if (!r && !a) return { reasoning: "", answer: text };
  return { reasoning: r?.[1].trim() ?? "", answer: a?.[1].trim() ?? "" };
}
