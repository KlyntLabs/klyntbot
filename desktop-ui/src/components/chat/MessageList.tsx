import { useRef, useEffect } from 'react';
import type { ChatMessage } from '../../lib/types';

interface MessageListProps {
  messages: ChatMessage[];
  streamingContent: string;
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
}

export function MessageList({
  messages,
  streamingContent,
  isStreaming,
  activeTools,
  error,
}: MessageListProps) {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages.length, isStreaming]);

  return (
    <div className="space-y-6">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
        >
          {msg.role === 'user' ? (
            <div className="max-w-[85%] rounded-2xl px-5 py-3.5 bg-surface-raised backdrop-blur-sm">
              <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
                {msg.content}
              </p>
            </div>
          ) : (
            <div className="max-w-[85%]">
              <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-secondary">
                {msg.content}
              </p>
            </div>
          )}
        </div>
      ))}

      {/* Tool execution indicator */}
      {activeTools.length > 0 && (
        <div className="flex justify-start">
          <div className="flex items-center gap-2 text-[12px] text-muted font-light">
            <div className="w-3 h-3 border border-muted/50 rounded-full animate-spin border-t-transparent" />
            <span>Using {activeTools[activeTools.length - 1]}&hellip;</span>
          </div>
        </div>
      )}

      {/* Streaming partial response */}
      {isStreaming && streamingContent && (
        <div className="flex justify-start">
          <div className="max-w-[85%]">
            <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-secondary">
              {streamingContent}
              <span className="inline-block w-1.5 h-4 bg-muted/50 ml-0.5 animate-pulse align-text-bottom" />
            </p>
          </div>
        </div>
      )}

      {/* Thinking indicator (streaming but no content yet and no tools running) */}
      {isStreaming && !streamingContent && activeTools.length === 0 && (
        <div className="flex justify-start">
          <div className="flex gap-1">
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: '0ms' }}
            />
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: '150ms' }}
            />
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: '300ms' }}
            />
          </div>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="flex justify-start">
          <div className="rounded-xl px-4 py-3 bg-destructive/10 border border-destructive/20">
            <p className="text-[12px] font-light text-destructive">{error}</p>
          </div>
        </div>
      )}

      <div ref={endRef} />
    </div>
  );
}
