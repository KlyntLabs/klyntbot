import { useState, useRef, useEffect, useCallback } from 'react';
import { Send } from 'lucide-react';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { isTauri } from '../../lib/utils';
import type { ChatMessage } from '../../lib/types';

const SESSION_KEY = 'desktop-panel';

interface ChatPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ChatPanel({ isOpen, onClose }: ChatPanelProps) {
  const { data: ipcMessages } = useQuery<ChatMessage[]>(
    'chat_messages',
    isTauri ? { session_key: SESSION_KEY } : undefined,
    [],
  );
  const [localMessages, setLocalMessages] = useState<ChatMessage[]>([]);
  const messages = isTauri ? ipcMessages : localMessages;

  const sendMessage = useMutation<ChatMessage, { content: string; session_key: string }>('chat_send');

  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    return () => clearTimeout(timerRef.current);
  }, []);

  const handleSend = useCallback(async () => {
    if (!input.trim()) return;

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: input,
    };

    if (isTauri) {
      await sendMessage.mutate({ content: input, session_key: SESSION_KEY });
    } else {
      setLocalMessages(prev => [...prev, userMessage]);
    }
    setInput('');
    setIsStreaming(true);

    if (!isTauri) {
      timerRef.current = setTimeout(() => {
        const aiMessage: ChatMessage = {
          id: (Date.now() + 1).toString(),
          role: 'assistant',
          content: 'I can help you with that! Let me analyze your tasks and provide recommendations...',
        };
        setLocalMessages(prev => [...prev, aiMessage]);
        setIsStreaming(false);
      }, 1000);
    } else {
      setIsStreaming(false);
    }
  }, [input, sendMessage]);

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
      <div className="flex-1 overflow-y-auto p-5 space-y-4">
        {messages.map(message => (
          <div
            key={message.id}
            className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
          >
            <div
              className={`max-w-[85%] rounded-xl px-4 py-3 ${
                message.role === 'user'
                  ? 'bg-brand text-white'
                  : 'bg-surface-base text-secondary'
              }`}
            >
              <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed">{message.content}</p>
            </div>
          </div>
        ))}
        {isStreaming && (
          <div className="flex justify-start">
            <div className="bg-surface-base rounded-xl px-4 py-3">
              <div className="flex gap-1">
                <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
              </div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="p-5 border-t border-border">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder="Ask me anything..."
            className="flex-1 bg-surface-base rounded-xl px-4 py-2.5 text-[13px] text-primary placeholder:text-muted focus:outline-none focus:bg-surface-raised font-light"
          />
          <button
            onClick={handleSend}
            disabled={!input.trim()}
            className="w-10 h-10 rounded-xl bg-brand hover:bg-brand/90 disabled:bg-surface-base disabled:text-muted flex items-center justify-center transition-colors"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>
  );
}
