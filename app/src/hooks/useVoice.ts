import { useCallback, useEffect, useRef, useState } from "react";
import { parseReply } from "../lib/api";
import { toolsApi } from "../lib/toolsApi";
import type { Message } from "../types";

/** Push-to-talk style dictation: click once to start recording, again to transcribe. */
export function useDictation(onText: (text: string) => void) {
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggle = useCallback(async () => {
    setError(null);
    try {
      if (!recording) {
        await toolsApi.voiceStart();
        setRecording(true);
        return;
      }
      setRecording(false);
      setTranscribing(true);
      const text = await toolsApi.voiceStop();
      if (text.trim()) onText(text.trim());
      else setError("I did not hear anything.");
    } catch (e) {
      setRecording(false);
      setError(String(e));
    } finally {
      setTranscribing(false);
    }
  }, [recording, onText]);

  return { recording, transcribing, error, toggle };
}

/** Read each finished assistant answer aloud (answer only, never the reasoning). */
export function useSpeakReplies(messages: Message[], generating: boolean, enabled: boolean) {
  const wasGenerating = useRef(false);

  useEffect(() => {
    if (wasGenerating.current && !generating && enabled) {
      const last = messages[messages.length - 1];
      if (last?.role === "assistant" && last.content) {
        const { answer } = parseReply(last.content);
        if (answer.trim()) toolsApi.speak(answer).catch(() => undefined);
      }
    }
    wasGenerating.current = generating;
  }, [generating, enabled, messages]);
}
