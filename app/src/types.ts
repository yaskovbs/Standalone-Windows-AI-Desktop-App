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
  rating?: number;
}

export interface StoredMessage {
  id: number;
  role: string;
  content: string;
  sources: string | null;
  created_at: number;
  rating: number;
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
  whisper_model_url: string;
  whisper_model_file: string;
  whisper_language: string;
  speech_rate: number;
  speak_replies: boolean;
  vision_model_url: string;
  vision_model_file: string;
  mmproj_url: string;
  mmproj_file: string;
  frame_interval_secs: number;
  max_frames: number;
  contribute_url: string;
  share_asked: boolean;
  share_enabled: boolean;
  install_id: string;
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

export interface ToolsStatus {
  tools_dir: string;
  ffmpeg: boolean;
  whisper_cli: boolean;
  vision_cli: boolean;
  whisper_model: boolean;
  vision_model: boolean;
  mmproj: boolean;
}

export interface MediaMemory {
  title: string;
  duration_secs: number;
  frames: number;
  heard_audio: boolean;
  timeline: string;
}

export type MediaEvent =
  | { event: "stage"; data: { label: string } }
  | { event: "progress"; data: { done: number; total: number } };

export interface DiffView {
  before: string[];
  removed: string[];
  added: string[];
  after: string[];
}

export interface FileChange {
  path: string;
  is_new: boolean;
  sha: string | null;
  new_content: string;
  diff: DiffView;
}

export interface Proposal {
  id: string;
  title: string;
  description: string;
  changes: FileChange[];
}

export interface GithubStatus {
  has_token: boolean;
  repo: string;
}

export interface ShareStatus {
  available: boolean;
  asked: boolean;
  enabled: boolean;
  server: string;
}

export interface RecapRequest {
  path: string;
  title: string;
  description: string;
  target_secs: number;
  clip_secs: number;
  max_frames: number;
  narrate: boolean;
  language: string;
  scene_detect: boolean;
}

export interface ClipInfo {
  start: number;
  end: number;
  summary: string;
}

export interface RecapResult {
  output_path: string;
  clips: ClipInfo[];
  narration: string | null;
  voiceover: boolean;
  total_secs: number;
  rated_by_model: boolean;
}

export interface RecapRow {
  id: number;
  title: string;
  path: string;
  narration: string | null;
  duration: number;
  created_at: number;
}
