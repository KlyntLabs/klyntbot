import { ipc } from "@shared/hooks/useIpc";
import { Send } from "lucide-react";
import { useCallback, useRef, useState } from "react";

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
}

interface PersonaChatProps {
  noteId: string;
  personaId: string;
  personaName: string;
  personaRole: string;
  personaTone: string;
}

export function PersonaChat({
  noteId,
  personaId,
  personaName,
  personaRole,
  personaTone,
}: PersonaChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const nextId = useRef(0);

  const send = useCallback(async () => {
    const text = input.trim();
    if (!text || loading) return;

    nextId.current += 1;
    const userId = `msg-${nextId.current}`;
    const userMsg: ChatMessage = { id: userId, role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const result = await ipc<{ reply: string }>("note_insight_persona_chat", {
        params: {
          noteId,
          personaId,
          personaName,
          personaRole,
          personaTone,
          userMessage: text,
          history: messages.map((m) => ({ role: m.role, content: m.content })),
        },
      });
      nextId.current += 1;
      setMessages((prev) => [
        ...prev,
        { id: `msg-${nextId.current}`, role: "assistant", content: result.reply },
      ]);
    } catch {
      nextId.current += 1;
      setMessages((prev) => [
        ...prev,
        {
          id: `msg-${nextId.current}`,
          role: "assistant",
          content: "Failed to get response. Try again.",
        },
      ]);
    } finally {
      setLoading(false);
    }
  }, [input, loading, messages, noteId, personaId, personaName, personaRole, personaTone]);

  return (
    <div className="mt-2 space-y-2 border-t border-border pt-2">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={`text-[11px] leading-relaxed ${
            msg.role === "user" ? "text-foreground" : "text-muted-foreground italic"
          }`}
        >
          <span className="text-[9px] text-dim mr-1">
            {msg.role === "user" ? "You:" : `${personaName}:`}
          </span>
          {msg.content}
        </div>
      ))}
      {loading && (
        <div className="text-[10px] text-dim italic animate-pulse">
          {personaName} is thinking...
        </div>
      )}
      <div className="flex gap-1">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && send()}
          placeholder={`Ask ${personaName}...`}
          className="flex-1 text-[11px] px-2 py-1 rounded-md bg-input border border-border text-foreground placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-purple/30"
          disabled={loading}
        />
        <button
          type="button"
          onClick={send}
          disabled={loading || !input.trim()}
          className="p-1 rounded-md text-purple hover:bg-purple/10 transition-colors disabled:text-dim"
        >
          <Send size={12} />
        </button>
      </div>
    </div>
  );
}
