import { useCallback, useEffect, useMemo, useState } from "react";
import type { SessionMessage } from "../lib/session-types";
import { useEvent } from "./useEvent";
import { useMutation } from "./useMutation";
import { useQuery } from "./useQuery";

interface SessionStream {
  messages: SessionMessage[];
  isLive: boolean;
  pinMessage: (messageUuid: string, messageContent: string, messageRole: string) => Promise<void>;
}

/**
 * Fetches session messages and subscribes to real-time message events.
 * Returns a live-updating message list for the session mirror view.
 */
export function useSessionStream(sessionId: string | undefined): SessionStream {
  const [liveMessages, setLiveMessages] = useState<SessionMessage[]>([]);

  const { data: initialMessages, loading } = useQuery<SessionMessage[]>(
    "get_session_messages",
    sessionId ? { sessionId } : null,
    [],
  );

  // Reset live buffer when session changes or when initial messages are refetched
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset on session or data change
  useEffect(() => {
    setLiveMessages([]);
  }, [sessionId, initialMessages]);

  const pin = useMutation<
    void,
    { sessionId: string; messageUuid: string; messageContent: string; messageRole: string }
  >("pin_session_message");

  useEvent<{ sessionId: string; message: SessionMessage }>("session:message", (payload) => {
    if (payload.sessionId === sessionId) {
      setLiveMessages((prev) => {
        const next = [...prev, payload.message];
        // Cap live buffer to avoid unbounded growth; older messages are in initialMessages
        return next.length > 500 ? next.slice(-500) : next;
      });
    }
  });

  const pinMessage = useCallback(
    async (messageUuid: string, messageContent: string, messageRole: string) => {
      if (!sessionId) return;
      await pin.mutate({ sessionId, messageUuid, messageContent, messageRole });
    },
    [sessionId, pin.mutate],
  );

  const messages = useMemo(
    () => (liveMessages.length > 0 ? [...initialMessages, ...liveMessages] : initialMessages),
    [initialMessages, liveMessages],
  );

  return { messages, isLive: !loading, pinMessage };
}
