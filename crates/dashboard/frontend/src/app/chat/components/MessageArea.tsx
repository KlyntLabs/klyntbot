import { useRef, useEffect, useCallback } from 'react';
import { Code, FileCode, Lightbulb, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { ChatMessage } from '../../../lib/types';
import type { ThinkingState, PendingInteraction } from '../../../lib/hooks/useAgent';
import { MessageBubble } from './MessageBubble';
import { ThinkingIndicator } from './ThinkingIndicator';
import { InteractionPanel } from './InteractionPanel';
import type { ConnectionStatus } from '../../../lib/ws';
import { StatusDot } from './MessageBubble';

const suggestions = [
  { id: '1', icon: Code, title: 'Build a classic Snake game', description: 'Create a retro snake game with canvas rendering' },
  { id: '2', icon: FileCode, title: 'Refactor legacy code', description: 'Improve code quality and add type safety' },
  { id: '3', icon: Lightbulb, title: 'Optimize performance', description: 'Analyze and improve application speed' },
  { id: '4', icon: Sparkles, title: 'Add new feature', description: 'Implement a feature with best practices' },
];

interface MessageAreaProps {
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  pendingInteraction: PendingInteraction | null;
  onSendSuggestion: (text: string) => void;
  onRespondToInteraction: (requestId: string, response: Record<string, unknown>) => void;
}

export function MessageArea({
  messages,
  thinking,
  isStreaming,
  status,
  pendingInteraction,
  onSendSuggestion,
  onRespondToInteraction,
}: MessageAreaProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const hasMessages = messages.length > 0;

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, thinking]);

  const handleSuggestionClick = useCallback(
    (description: string) => {
      onSendSuggestion(description);
    },
    [onSendSuggestion],
  );

  return (
    <div className="flex-1 flex flex-col">
      {/* Connection status bar */}
      {status !== 'connected' && (
        <div
          className="px-4 py-1.5 flex items-center justify-center gap-2 border-b"
          style={{
            backgroundColor: 'var(--codex-bg-secondary)',
            borderColor: 'var(--codex-border-subtle)',
          }}
        >
          <StatusDot status={status} />
          {status === 'reconnecting' && (
            <span className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              Reconnecting...
            </span>
          )}
        </div>
      )}

      {/* Chat Messages */}
      <div className="flex-1 overflow-y-auto px-6 py-8">
        {!hasMessages ? (
          <div className="h-full flex flex-col items-center justify-center max-w-3xl mx-auto">
            <div className="mb-12 text-center">
              <h1 className="text-2xl mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
                How can I help you today?
              </h1>
              <p className="text-sm" style={{ color: 'var(--codex-fg-subtle)' }}>
                Choose a suggestion below or describe your task
              </p>
            </div>

            <div className="grid grid-cols-2 gap-3 w-full max-w-2xl">
              {suggestions.map((suggestion) => (
                <button
                  key={suggestion.id}
                  className="p-4 rounded-lg border text-left transition-all group"
                  style={{
                    backgroundColor: 'var(--codex-bg-tertiary)',
                    borderColor: 'var(--codex-border)',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.borderColor = 'var(--codex-accent)';
                    e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.borderColor = 'var(--codex-border)';
                    e.currentTarget.style.backgroundColor = 'var(--codex-bg-tertiary)';
                  }}
                  onClick={() => handleSuggestionClick(suggestion.description)}
                >
                  <suggestion.icon
                    className="w-5 h-5 mb-3"
                    strokeWidth={1.5}
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  />
                  <div className="text-sm mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
                    {suggestion.title}
                  </div>
                  <div className="text-xs" style={{ color: 'var(--codex-fg-subtle)' }}>
                    {suggestion.description}
                  </div>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div className="max-w-3xl mx-auto space-y-6">
            {messages.map((msg) => (
              <MessageBubble key={msg.id} msg={msg} />
            ))}

            <AnimatePresence>
              {thinking && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -5 }}
                  className="flex gap-3"
                >
                  <div className="flex-1">
                    <ThinkingIndicator thinking={thinking} />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <AnimatePresence>
              {pendingInteraction && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -5 }}
                >
                  <InteractionPanel
                    interaction={pendingInteraction}
                    onRespond={onRespondToInteraction}
                  />
                </motion.div>
              )}
            </AnimatePresence>

            <div ref={messagesEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
