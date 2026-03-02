import { useState, useMemo, useCallback } from 'react';
import { Send } from 'lucide-react';
import { MessageList } from './MessageList';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useAgentStream } from '../../hooks/useAgentStream';
import type { ChatMessage } from '../../lib/types';

const SESSION_KEY = 'desktop-panel';

interface ChatPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ChatPanel({ isOpen, onClose }: ChatPanelProps) {
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    'chat_messages',
    { session_key: SESSION_KEY },
    [],
  );
  const sendMessage = useMutation<ChatMessage, Record<string, unknown>>('chat_send');

  const [input, setInput] = useState('');
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  const stream = useAgentStream(SESSION_KEY, () => {
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

  const handleSend = useCallback(async () => {
    if (!input.trim() || stream.isStreaming) return;
    const text = input;
    setInput('');

    setPendingUserMsg(text);
    stream.startStreaming();
    await sendMessage.mutate({ content: text, session_key: SESSION_KEY });
  }, [input, stream, sendMessage]);

  if (!isOpen) return null;

  return (
    <div className="w-96 bg-background border-l border-border flex flex-col">
      {/* Header */}
      <div className="h-14 flex items-center justify-between px-5 border-b border-border">
        <span className="text-[13px] font-light text-secondary">Chat</span>
        <button
          onClick={onClose}
          className="text-muted hover:text-secondary transition-colors text-[18px] leading-none"
        >
          &times;
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-5">
        <MessageList
          messages={displayMessages}
          streamingContent={stream.streamingContent}
          isStreaming={stream.isStreaming}
          activeTools={stream.activeTools}
          error={stream.error}
        />
      </div>

      {/* Input */}
      <div className="p-5 border-t border-border">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleSend();
            }}
            placeholder="Ask me anything..."
            className="flex-1 bg-surface-base rounded-xl px-4 py-2.5 text-[13px] text-primary placeholder:text-muted focus:outline-none focus:bg-surface-raised font-light"
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || stream.isStreaming}
            className="w-10 h-10 rounded-xl bg-brand hover:bg-brand/90 disabled:bg-surface-base disabled:text-muted flex items-center justify-center transition-colors"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>
  );
}
