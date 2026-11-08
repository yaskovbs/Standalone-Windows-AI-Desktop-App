import { useEffect, useRef } from "react";
import type { GenStats, Message } from "../types";
import MessageBubble from "./MessageBubble";

interface Props {
  messages: Message[];
  error: string | null;
  stats: GenStats | null;
}

export default function ChatView({ messages, error, stats }: Props) {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-slate-500">
        <div className="text-center">
          <div className="text-2xl font-semibold text-slate-300">LocalAutonomousAI</div>
          <p className="mt-2 text-sm">A local model on your GPU/CPU, with live web context.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6">
      <div className="mx-auto max-w-3xl space-y-4">
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}
        {error && (
          <div className="rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-300">{error}</div>
        )}
        {stats && (
          <div className="text-center text-xs text-slate-500">
            {stats.generated_tokens} tokens · {stats.tokens_per_second.toFixed(1)} tok/s
          </div>
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
}
