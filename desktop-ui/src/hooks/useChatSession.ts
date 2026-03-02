import { useState, useMemo, useCallback } from 'react';
import { useQuery } from './useQuery';
import { useAgentStream } from './useAgentStream';
import { ipc } from './useIpc';
import { isTauri } from '../lib/utils';
import type { ChatMessage, InteractionRequest } from '../lib/types';

interface ChatSession {
  messages: ChatMessage[];
  streamingContent: string;
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: { requestId: string; request: InteractionRequest } | null;
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

  const displayMessages = useMemo(() => {
    const list = [...messages];
    if (pendingUserMsg) {
      list.push({ id: 'pending', role: 'user', content: pendingUserMsg });
    }
    return list;
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
    streamingContent: stream.streamingContent,
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
