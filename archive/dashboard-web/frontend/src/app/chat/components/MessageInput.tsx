import { useState, useCallback, useRef } from 'react';
import { Send, X, Plus } from 'lucide-react';

interface MessageInputProps {
  onSend: (text: string) => void;
  onCancel: () => void;
  onNewSession: () => void;
  isStreaming: boolean;
}

export function MessageInput({ onSend, onCancel, onNewSession, isStreaming }: MessageInputProps) {
  const [message, setMessage] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = useCallback(() => {
    const text = message.trim();
    if (!text) return;
    setMessage('');
    onSend(text);
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [message, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleTextareaChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setMessage(e.target.value);
      e.target.style.height = 'auto';
      e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
    },
    [],
  );

  return (
    <div
      className="p-4 border-t"
      style={{
        borderColor: 'var(--codex-border-subtle)',
        backgroundColor: 'var(--codex-bg)',
      }}
    >
      <div className="px-4">
        {/* Cancel button when streaming */}
        {isStreaming && (
          <div className="flex justify-center mb-2">
            <button
              onClick={onCancel}
              className="flex items-center gap-1.5 px-3 py-1 rounded-md text-[12px] border transition-colors"
              style={{
                borderColor: 'var(--codex-border)',
                color: 'var(--codex-fg-subtle)',
                backgroundColor: 'var(--codex-bg-secondary)',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = '#ef4444';
                e.currentTarget.style.color = '#ef4444';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = 'var(--codex-border)';
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
              }}
            >
              <X className="w-3 h-3" strokeWidth={1.5} />
              Cancel
            </button>
          </div>
        )}

        <div
          className="flex gap-2 items-end px-3 py-2.5 rounded-lg border"
          style={{
            backgroundColor: 'var(--codex-bg-tertiary)',
            borderColor: 'var(--codex-border)',
          }}
        >
          <button
            onClick={onNewSession}
            className="flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors"
            style={{ color: 'var(--codex-fg-subtle)' }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
              e.currentTarget.style.color = 'var(--codex-fg)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent';
              e.currentTarget.style.color = 'var(--codex-fg-subtle)';
            }}
            title="New conversation"
          >
            <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
            <span>New</span>
          </button>

          <div className="w-px h-5" style={{ backgroundColor: 'var(--codex-border)' }} />

          <textarea
            ref={textareaRef}
            value={message}
            onChange={handleTextareaChange}
            onKeyDown={handleKeyDown}
            placeholder="Message klyntbot..."
            rows={1}
            disabled={isStreaming}
            className="flex-1 bg-transparent outline-none resize-none text-[14px]"
            style={{
              color: 'var(--codex-fg)',
              fontFamily: 'var(--font-ui)',
              maxHeight: '200px',
              opacity: isStreaming ? 0.5 : 1,
            }}
          />

          <button
            onClick={handleSend}
            disabled={!message.trim() || isStreaming}
            className="p-1.5 rounded transition-colors"
            style={{
              color: message.trim() && !isStreaming ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
              opacity: isStreaming ? 0.5 : 1,
            }}
            onMouseEnter={(e) => {
              if (message.trim() && !isStreaming)
                e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent';
            }}
          >
            <Send className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </div>
  );
}
