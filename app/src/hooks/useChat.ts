import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { toolsApi } from "../lib/toolsApi";
import type { Conversation, GenStats, Message } from "../types";

/** Conversation list + active chat state + token streaming. */
export function useChat() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<GenStats | null>(null);
  const activeRef = useRef<number | null>(null);

  const refreshList = useCallback(async () => {
    setConversations(await api.conversations());
  }, []);

  const open = useCallback(async (id: number) => {
    activeRef.current = id;
    setActiveId(id);
    setError(null);
    setStats(null);
    const rows = await api.messages(id);
    setMessages(
      rows.map((m) => ({
        id: m.id,
        role: m.role as Message["role"],
        content: m.content,
        sources: m.sources ? JSON.parse(m.sources) : undefined,
        rating: m.rating,
      })),
    );
  }, []);

  const newChat = useCallback(async () => {
    const id = await api.createConversation();
    await refreshList();
    await open(id);
    return id;
  }, [open, refreshList]);

  const remove = useCallback(
    async (id: number) => {
      await api.deleteConversation(id);
      await refreshList();
      if (activeRef.current === id) {
        activeRef.current = null;
        setActiveId(null);
        setMessages([]);
      }
    },
    [refreshList],
  );

  const send = useCallback(
    async (text: string, web: boolean) => {
      const content = text.trim();
      if (!content || generating) return;
      const id = activeRef.current ?? (await newChat());
      setError(null);
      setGenerating(true);
      setMessages((prev) => [
        ...prev,
        { id: -Date.now(), role: "user", content },
        { id: -Date.now() - 1, role: "assistant", content: "", streaming: true },
      ]);

      const patchLast = (fn: (m: Message) => Message) =>
        setMessages((prev) => prev.map((m, i) => (i === prev.length - 1 ? fn(m) : m)));

      try {
        await api.chat(id, content, web, (e) => {
          if (e.event === "sources") patchLast((m) => ({ ...m, sources: e.data.items }));
          else if (e.event === "token") patchLast((m) => ({ ...m, content: m.content + e.data.text }));
          else if (e.event === "done") {
            setStats(e.data.stats);
            patchLast((m) => ({ ...m, id: e.data.message_id, streaming: false }));
          } else if (e.event === "error") setError(e.data.message);
        });
      } catch (err) {
        setError(String(err));
        patchLast((m) => ({ ...m, streaming: false }));
      } finally {
        setGenerating(false);
        refreshList();
      }
    },
    [generating, newChat, refreshList],
  );

  const rate = useCallback(async (messageId: number, rating: number): Promise<string> => {
    const conv = activeRef.current;
    if (conv === null) return "";
    const result = await toolsApi.rateMessage(conv, messageId, rating);
    setMessages((prev) => prev.map((m) => (m.id === messageId ? { ...m, rating } : m)));
    return result;
  }, []);

  useEffect(() => {
    refreshList();
  }, [refreshList]);

  return { conversations, activeId, messages, generating, error, stats, open, newChat, remove, send, rate, stop: api.stop };
}
