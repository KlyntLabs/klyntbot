import { useState, useMemo, useCallback } from 'react';
import { useQuery } from './useQuery';
import { useMutation } from './useMutation';
import { useAgentStream } from './useAgentStream';
import type { ChatMessage } from '../lib/types';

interface ChatSession {
  messages: ChatMessage[];
  streamingContent: string;
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  input: string;
  setInput: (value: string) => void;
  send: (extraPayload?: Record<string, unknown>) => Promise<void>;
}

/**
 * Encapsulates chat session state: message fetching, optimistic pending message,
 * streaming, and send logic. Used by ChatPanel, SidebarChat, and Chat.
 */
export function useChatSession(sessionKey: string): ChatSession {
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    'chat_messages',
    sessionKey ? { session_key: sessionKey } : null,
    [],
  );
  const sendMessage = useMutation<ChatMessage, Record<string, unknown>>('chat_send');

  const [input, setInput] = useState('');
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  const stream = useAgentStream(sessionKey, () => {
    setPendingUserMsg(null);
    refetch();
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
    await sendMessage.mutate({
      content: text,
      session_key: sessionKey,
      ...extraPayload,
    });
  }, [input, sessionKey, stream, sendMessage]);

  return {
    messages: displayMessages,
    streamingContent: stream.streamingContent,
    isStreaming: stream.isStreaming,
    activeTools: stream.activeTools,
    error: stream.error,
    input,
    setInput,
    send,
  };
}
