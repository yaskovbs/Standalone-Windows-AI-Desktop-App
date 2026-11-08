// Types mirroring the Rust structs (serde keeps snake_case field names).

export interface Conversation {
  id: number;
  title: string;
  updated_at: number;
}

export interface WebSource {
  title: string;
  url: string;
  snippet: string;
}

export interface Message {
  id: number;
  role: "user" | "assistant";
  content: string;
  sources?: WebSource[];
  streaming?: boolean;
}

export interface StoredMessage {
  id: number;
  role: string;
  content: string;
  sources: string | null;
  created_at: number;
}

export interface Settings {
  model_url: string;
  model_file: string;
  system_prompt: string;
  temperature: number;
  max_tokens: number;
  context_size: number;
  web_search_default: boolean;
  search_results: number;
  check_updates: boolean;
  update_repo: string;
}

export interface HardwareInfo {
  gpu_name: string | null;
  vram_mb: number;
  cpu_threads: number;
  avx2: boolean;
  avx512: boolean;
}

export interface ModelStatus {
  present: boolean;
  loaded: boolean;
  name: string;
  gpu_layers: number;
}

export interface GenStats {
  prompt_tokens: number;
  generated_tokens: number;
  tokens_per_second: number;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  update_available: boolean;
  url: string;
}

export type ChatEvent =
  | { event: "sources"; data: { items: WebSource[] } }
  | { event: "token"; data: { text: string } }
  | { event: "done"; data: { stats: GenStats; message_id: number } }
  | { event: "error"; data: { message: string } };

export type DownloadEvent =
  | { event: "progress"; data: { downloaded: number; total: number } }
  | { event: "done"; data: { path: string } }
  | { event: "error"; data: { message: string } };
