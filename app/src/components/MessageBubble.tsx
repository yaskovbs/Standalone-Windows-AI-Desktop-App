import { useState } from "react";
import { parseReply } from "../lib/api";
import type { Message } from "../types";

interface Props {
  message: Message;
  onRate?: (messageId: number, rating: number) => Promise<string>;
}

export default function MessageBubble({ message, onRate }: Props) {
  const [info, setInfo] = useState("");
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

        {!isUser && !message.streaming && message.id > 0 && onRate && (
          <div className="mt-2 flex items-center gap-2 text-xs text-slate-500">
            {[1, -1].map((value) => (
              <button
                key={value}
                title={value === 1 ? "Helpful" : "Not helpful"}
                className={`rounded px-1.5 py-0.5 hover:bg-white/10 ${message.rating === value ? "bg-white/10 text-slate-100" : ""}`}
                onClick={() => onRate(message.id, message.rating === value ? 0 : value).then(setInfo).catch((e) => setInfo(String(e)))}
              >
                {value === 1 ? "👍" : "👎"}
              </button>
            ))}
            {info && <span>{info}</span>}
          </div>
        )}

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
