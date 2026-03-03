import { useCallback } from 'react';
import { Send, Pin, X } from 'lucide-react';
import { MessageList } from './MessageList';
import { usePageContext } from '../../hooks/usePageContext';
import { useMutation } from '../../hooks/useMutation';
import { useChatSession } from '../../hooks/useChatSession';

interface SidebarChatProps {
  isOpen: boolean;
  onClose: () => void;
}

/** Derive a stable session key from page context. */
function sessionKeyFor(entityKind?: string, entityId?: string): string {
  if (!entityKind) return 'desktop-panel';
  return `sidebar:${entityKind}:${entityId || 'all'}`;
}

/** Human-readable label for the sidebar header. */
function contextLabel(entityKind?: string): string {
  if (!entityKind) return 'Chat';
  const parts = entityKind.split('.');
  return parts.map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join(' › ');
}

export function SidebarChat({ isOpen, onClose }: SidebarChatProps) {
  const pageContext = usePageContext();
  const sessionKey = sessionKeyFor(pageContext?.entityKind, pageContext?.entityId);

  const chat = useChatSession(sessionKey);
  const pinThread = useMutation<void, Record<string, unknown>>('chat_pin_thread');

  const handleSend = useCallback(() => {
    chat.send(
      pageContext ? { context: { ...pageContext, isEphemeral: true } } : undefined,
    );
  }, [chat, pageContext]);

  const handlePin = useCallback(async () => {
    await pinThread.mutate({ sessionKey });
  }, [sessionKey, pinThread]);

  if (!isOpen) return null;

  const label = contextLabel(pageContext?.entityKind);

  return (
    <div className="w-96 bg-background border-l border-border flex flex-col">
      {/* Header */}
      <div className="h-14 flex items-center justify-between px-5 border-b border-border">
        <span className="text-[13px] font-light text-secondary">{label}</span>
        <div className="flex items-center gap-1">
          {pageContext && (
            <button
              onClick={handlePin}
              title="Pin to Chat threads"
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
            >
              <Pin className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          )}
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
          >
            <X className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-5">
        {chat.messages.length === 0 && !chat.isStreaming ? (
          <div className="flex flex-col items-center justify-center py-12">
            <p className="text-muted text-[12px] font-light text-center">
              {pageContext
                ? `Ask about this ${pageContext.entityKind?.split('.')[0] || 'item'}`
                : 'Ask me anything'}
            </p>
          </div>
        ) : (
          <MessageList
            messages={chat.messages}
            segments={chat.segments}
            isStreaming={chat.isStreaming}
            activeTools={chat.activeTools}
            error={chat.error}
            activeInteraction={chat.activeInteraction}
            sessionKey={sessionKey}
            onInteractionSubmitted={() => chat.clearInteraction()}
            showTransparency={false}
            liveTransparency={null}
          />
        )}
      </div>

      {/* Input */}
      <div className="p-4 border-t border-border">
        <div className="flex gap-2">
          <input
            type="text"
            value={chat.input}
            onChange={(e) => chat.setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleSend();
            }}
            placeholder="Ask about this page..."
            className="flex-1 bg-surface-base rounded-xl px-4 py-2.5 text-[13px] text-primary placeholder:text-muted focus:outline-none focus:bg-surface-raised font-light"
          />
          <button
            onClick={handleSend}
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
