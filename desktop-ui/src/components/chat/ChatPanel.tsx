import { Send } from 'lucide-react';
import { MessageList } from './MessageList';
import { useChatSession } from '../../hooks/useChatSession';

const SESSION_KEY = 'desktop-panel';

interface ChatPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ChatPanel({ isOpen, onClose }: ChatPanelProps) {
  const chat = useChatSession(SESSION_KEY);

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
          messages={chat.messages}
          streamingContent={chat.streamingContent}
          isStreaming={chat.isStreaming}
          activeTools={chat.activeTools}
          error={chat.error}
        />
      </div>

      {/* Input */}
      <div className="p-5 border-t border-border">
        <div className="flex gap-2">
          <input
            type="text"
            value={chat.input}
            onChange={(e) => chat.setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') chat.send();
            }}
            placeholder="Ask me anything..."
            className="flex-1 bg-surface-base rounded-xl px-4 py-2.5 text-[13px] text-primary placeholder:text-muted focus:outline-none focus:bg-surface-raised font-light"
          />
          <button
            onClick={() => chat.send()}
            disabled={!chat.input.trim() || chat.isStreaming}
            className="w-10 h-10 rounded-xl bg-brand hover:bg-brand/90 disabled:bg-surface-base disabled:text-muted flex items-center justify-center transition-colors"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>
  );
}
