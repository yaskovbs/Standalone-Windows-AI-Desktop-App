import { useState } from "react";
import { parseReply } from "../lib/api";
import type { Message } from "../types";

export default function MessageBubble({ message }: { message: Message }) {
  const [showReasoning, setShowReasoning] = useState(false);
  const isUser = message.role === "user";
  const { reasoning, answer } = isUser
    ? { reasoning: "", answer: message.content }
    : parseReply(message.content);

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[80%] rounded-2xl px-4 py-3 text-sm leading-relaxed ${
          isUser ? "bg-accent/20 text-slate-100" : "border border-line bg-panel"
        }`}
      >
        {reasoning && (
          <div className="mb-2">
            <button
              className="text-xs text-slate-500 hover:text-slate-300"
              onClick={() => setShowReasoning((v) => !v)}
            >
              {showReasoning ? "Hide reasoning" : "Show reasoning"}
            </button>
            {showReasoning && <div className="reasoning mt-1">{reasoning}</div>}
          </div>
        )}
        <div className={`whitespace-pre-wrap ${message.streaming ? "cursor" : ""}`}>
          {answer || (message.streaming && !reasoning ? "" : answer)}
        </div>

        {message.sources && message.sources.length > 0 && (
          <ol className="mt-3 space-y-1 border-t border-line pt-2 text-xs text-slate-400">
            {message.sources.map((s, i) => (
              <li key={s.url} className="truncate">
                [{i + 1}]{" "}
                <a className="text-accent hover:underline" href={s.url} target="_blank" rel="noreferrer">
                  {s.title}
                </a>
              </li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
