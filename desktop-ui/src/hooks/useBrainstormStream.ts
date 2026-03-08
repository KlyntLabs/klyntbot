import { useCallback, useEffect, useRef, useState } from "react";
import type { BrainstormMessage } from "../lib/session-types";
import { useEvent } from "./useEvent";
import { ipc } from "./useIpc";
import { useQuery } from "./useQuery";

interface BrainstormSegment {
  type: "text";
  content: string;
}

interface BrainstormStream {
  messages: BrainstormMessage[];
  segments: BrainstormSegment[];
  isStreaming: boolean;
  send: (content: string) => Promise<void>;
}

/**
 * Manages brainstorm conversation state with streaming token support.
 * Listens to brainstorm:token and brainstorm:complete events.
 */
export function useBrainstormStream(conversationId: string | undefined): BrainstormStream {
  const [segments, setSegments] = useState<BrainstormSegment[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const streamTextRef = useRef("");
  const rafRef = useRef(0);

  const { data: messages, refetch } = useQuery<BrainstormMessage[]>(
    "get_brainstorm_messages",
    conversationId ? { conversationId } : null,
    [],
  );

  const flushText = useCallback(() => {
    if (rafRef.current) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = 0;
    }
    setSegments(() => {
      const text = streamTextRef.current;
      if (!text) return [];
      return [{ type: "text" as const, content: text }];
    });
  }, []);

  // Cleanup pending rAF on unmount
  useEffect(() => {
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  useEvent<{ conversationId: string; token: string }>("brainstorm:token", (payload) => {
    if (payload.conversationId !== conversationId) return;
    streamTextRef.current += payload.token;
    if (!rafRef.current) {
      rafRef.current = requestAnimationFrame(flushText);
    }
  });

  useEvent<{ conversationId: string }>("brainstorm:complete", (payload) => {
    if (payload.conversationId !== conversationId) return;
    // Clear streaming state before refetch to avoid showing both segments and the new message
    if (rafRef.current) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = 0;
    }
    streamTextRef.current = "";
    setSegments([]);
    setIsStreaming(false);
    refetch();
  });

  const send = useCallback(
    async (content: string) => {
      if (!conversationId) return;
      streamTextRef.current = "";
      setSegments([]);
      setIsStreaming(true);
      await ipc("send_brainstorm_message", { conversationId, content });
    },
    [conversationId],
  );

  return { messages, segments, isStreaming, send };
}
