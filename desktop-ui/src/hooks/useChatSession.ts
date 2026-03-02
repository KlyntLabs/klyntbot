import { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { useQuery } from './useQuery';
import { useAgentStream } from './useAgentStream';
import { ipc } from './useIpc';
import { isTauri } from '../lib/utils';
import type { ActiveInteraction, ChatMessage, MessageSegment } from '../lib/types';

interface ChatSession {
  messages: ChatMessage[];
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  input: string;
  setInput: (value: string) => void;
  send: (extraPayload?: Record<string, unknown>) => Promise<void>;
  clearInteraction: () => void;
}

/**
 * Encapsulates chat session state: message fetching, optimistic pending message,
 * streaming, and send logic. Used by SidebarChat and Chat.
 *
 * @param onDone Optional callback fired when the agent finishes — use for
 *               refreshing thread lists or other side-effects.
 */
export function useChatSession(sessionKey: string, onDone?: () => void): ChatSession {
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    'chat_messages',
    sessionKey ? { sessionKey } : null,
    [],
  );
  const [input, setInput] = useState('');
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  const stream = useAgentStream(sessionKey, () => {
    setPendingUserMsg(null);
    refetch();
    onDone?.();
  });

  // Clear streaming segments only when a NEW assistant message arrives from refetch.
  // Tracks assistant count to avoid clearing before the refetch completes — the old
  // approach fired on isStreaming→false before the new message existed, causing a flash.
  const { isStreaming, clearSegments, segments } = stream;
  const hasSegmentsRef = useRef(false);
  hasSegmentsRef.current = segments.length > 0;
  const assistantCountRef = useRef(0);

  useEffect(() => {
    const count = messages.filter(m => m.role === 'assistant').length;
    if (!isStreaming && hasSegmentsRef.current && count > assistantCountRef.current) {
      clearSegments();
    }
    assistantCountRef.current = count;
  }, [messages, isStreaming, clearSegments]);

  const displayMessages = useMemo(() => {
    if (!pendingUserMsg) return messages;
    return [...messages, { id: 'pending', role: 'user' as const, content: pendingUserMsg }];
  }, [messages, pendingUserMsg]);

  const send = useCallback(async (extraPayload?: Record<string, unknown>) => {
    if (!input.trim() || stream.isStreaming) return;
    const text = input;
    setInput('');

    setPendingUserMsg(text);
    stream.startStreaming();

    if (!isTauri) return;

    try {
      await ipc<ChatMessage>('chat_send', {
        content: text,
        sessionKey,
        ...extraPayload,
      });
    } catch (e) {
      stream.failStreaming(String(e));
    }
  }, [input, sessionKey, stream]);

  return {
    messages: displayMessages,
    segments: stream.segments,
    isStreaming: stream.isStreaming,
    activeTools: stream.activeTools,
    error: stream.error,
    activeInteraction: stream.activeInteraction,
    input,
    setInput,
    send,
    clearInteraction: stream.clearInteraction,
  };
}
