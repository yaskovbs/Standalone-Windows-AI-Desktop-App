import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  DownloadEvent,
  GithubStatus,
  MediaEvent,
  MediaMemory,
  Proposal,
  ToolsStatus,
} from "../types";

/** Wrappers for voice, vision, video and GitHub self-update commands. */
export const toolsApi = {
  status: () => invoke<ToolsStatus>("tools_status"),

  downloadAsset(kind: "whisper" | "vision" | "mmproj", onEvent: (e: DownloadEvent) => void) {
    const channel = new Channel<DownloadEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("download_asset", { kind, onEvent: channel });
  },

  voiceStart: () => invoke<void>("voice_start"),
  voiceStop: () => invoke<string>("voice_stop"),
  speak: (text: string) => invoke<void>("speak", { text }),
  stopSpeaking: () => invoke<void>("stop_speaking"),

  describeImage: (path: string, question: string) => invoke<string>("describe_image", { path, question }),
  describeScreen: (question: string) => invoke<string>("describe_screen", { question }),

  watchMedia(path: string, onEvent: (e: MediaEvent) => void) {
    const channel = new Channel<MediaEvent>();
    channel.onmessage = onEvent;
    return invoke<MediaMemory>("watch_media", { path, onEvent: channel });
  },
  cancelMedia: () => invoke<void>("cancel_media"),
  attachMedia: (conversationId: number, memory: MediaMemory) =>
    invoke<void>("attach_media", { conversationId, memory }),
  clearMedia: (conversationId: number) => invoke<void>("clear_media", { conversationId }),

  githubStatus: () => invoke<GithubStatus>("github_status"),
  githubSaveToken: (token: string) => invoke<string>("github_save_token", { token }),
  githubClearToken: () => invoke<void>("github_clear_token"),
  githubPropose: (instruction: string, paths: string[]) => invoke<Proposal>("github_propose", { instruction, paths }),
  githubReject: (id: string) => invoke<void>("github_reject", { id }),
  githubApprove: (id: string) => invoke<string>("github_approve", { id }),
};
