import { useState, useCallback, useRef, useEffect } from 'react';
import {
  Send,
  ChevronDown,
  ChevronRight,
  Terminal,
  Sparkles,
  Code,
  Lightbulb,
  FileCode,
  Loader2,
  Check,
  Slash,
  X,
  Wifi,
  WifiOff,
  Clock,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useAgent } from '../../lib/hooks/useAgent';
import type { ThinkingState, ToolCallState } from '../../lib/hooks/useAgent';
import { useApi } from '../../lib/hooks/useApi';
import type { SessionListItem } from '../../lib/types';
import type { ChatMessage } from '../../lib/types';
import type { ConnectionStatus } from '../../lib/ws';

type SuggestionCard = {
  id: string;
  icon: typeof Code;
  title: string;
  description: string;
};

/** Map thinking phase to a human-readable label */
function phaseLabel(phase: ThinkingState['phase']): string {
  switch (phase) {
    case 'classifying':
      return 'Classifying';
    case 'buildingContext':
      return 'Building context';
    case 'thinking':
      return 'Thinking';
    case 'idle':
      return 'Idle';
  }
}

/** Format a Date to a short time string like "10:32 AM" */
function formatTime(d: Date): string {
  return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

/** Compute a human-readable duration between two ISO date strings */
function formatDuration(createdAt: string, updatedAt: string): string {
  const start = new Date(createdAt).getTime();
  const end = new Date(updatedAt).getTime();
  const diffMs = Math.max(0, end - start);
  const totalMinutes = Math.floor(diffMs / 60000);
  if (totalMinutes < 1) return '<1m';
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes}m`;
}

/** Connection status indicator component */
function StatusDot({ status }: { status: ConnectionStatus }) {
  const color =
    status === 'connected'
      ? '#10b981'
      : status === 'connecting' || status === 'reconnecting'
        ? '#f59e0b'
        : '#ef4444';

  const Icon =
    status === 'connected' || status === 'connecting' || status === 'reconnecting'
      ? Wifi
      : WifiOff;

  return (
    <div className="flex items-center gap-1.5" title={`WebSocket: ${status}`}>
      <Icon className="w-3 h-3" strokeWidth={1.5} style={{ color }} />
      <span
        className="text-[10px] uppercase tracking-wide"
        style={{ color, fontFamily: 'var(--font-mono)' }}
      >
        {status}
      </span>
    </div>
  );
}

export default function Chat() {
  const [sessionOpen, setSessionOpen] = useState(true);
  const [sessionsListOpen, setSessionsListOpen] = useState(false);
  const [memoryOpen, setMemoryOpen] = useState(true);
  const [tasksOpen, setTasksOpen] = useState(true);
  const [calendarOpen, setCalendarOpen] = useState(true);
  const [message, setMessage] = useState('');
  const [selectedModel] = useState('GPT-4');

  // Real agent hook
  const { messages, thinking, isStreaming, status, sendMessage, cancel } =
    useAgent();

  // Session list from API
  const {
    data: sessions,
    loading: sessionsLoading,
  } = useApi<SessionListItem[]>('/api/sessions');

  // Sorted sessions (most recent first)
  const sortedSessions = (sessions ?? [])
    .slice()
    .sort(
      (a, b) =>
        new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
    );

  // Ref for auto-scrolling chat
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, thinking]);

  const hasMessages = messages.length > 0;

  const handleSend = useCallback(() => {
    const text = message.trim();
    if (!text) return;
    setMessage('');
    sendMessage(text);
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [message, sendMessage]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleSuggestionClick = useCallback(
    (description: string) => {
      sendMessage(description);
    },
    [sendMessage],
  );

  // Auto-resize textarea
  const handleTextareaChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setMessage(e.target.value);
      // Auto-resize
      e.target.style.height = 'auto';
      e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
    },
    [],
  );

  const suggestions: SuggestionCard[] = [
    {
      id: '1',
      icon: Code,
      title: 'Build a classic Snake game',
      description: 'Create a retro snake game with canvas rendering',
    },
    {
      id: '2',
      icon: FileCode,
      title: 'Refactor legacy code',
      description: 'Improve code quality and add type safety',
    },
    {
      id: '3',
      icon: Lightbulb,
      title: 'Optimize performance',
      description: 'Analyze and improve application speed',
    },
    {
      id: '4',
      icon: Sparkles,
      title: 'Add new feature',
      description: 'Implement a feature with best practices',
    },
  ];

  const getStrategyColor = (confidence: number) => {
    if (confidence > 80) return 'var(--codex-accent)';
    if (confidence > 50) return '#e5a00d';
    return '#ef4444';
  };

  return (
    <>
      {/* Center Chat Area */}
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
              <span
                className="text-[11px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                Reconnecting...
              </span>
            )}
          </div>
        )}

        {/* Chat Messages */}
        <div className="flex-1 overflow-y-auto px-6 py-8">
          {!hasMessages ? (
            /* Empty state with suggestions */
            <div className="h-full flex flex-col items-center justify-center max-w-3xl mx-auto">
              <div className="mb-12 text-center">
                <h1
                  className="text-2xl mb-2"
                  style={{ color: 'var(--codex-fg)', fontWeight: 400 }}
                >
                  How can I help you today?
                </h1>
                <p
                  className="text-sm"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                >
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
                      e.currentTarget.style.borderColor =
                        'var(--codex-accent)';
                      e.currentTarget.style.backgroundColor =
                        'var(--codex-bg-secondary)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.borderColor =
                        'var(--codex-border)';
                      e.currentTarget.style.backgroundColor =
                        'var(--codex-bg-tertiary)';
                    }}
                    onClick={() =>
                      handleSuggestionClick(suggestion.description)
                    }
                  >
                    <suggestion.icon
                      className="w-5 h-5 mb-3"
                      strokeWidth={1.5}
                      style={{ color: 'var(--codex-fg-subtle)' }}
                    />
                    <div
                      className="text-sm mb-1"
                      style={{ color: 'var(--codex-fg)', fontWeight: 400 }}
                    >
                      {suggestion.title}
                    </div>
                    <div
                      className="text-xs"
                      style={{ color: 'var(--codex-fg-subtle)' }}
                    >
                      {suggestion.description}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            /* Messages */
            <div className="max-w-3xl mx-auto space-y-6">
              {messages.map((msg) => (
                <MessageBubble
                  key={msg.id}
                  msg={msg}
                />
              ))}

              {/* Thinking indicator */}
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

              <div ref={messagesEndRef} />
            </div>
          )}
        </div>

        {/* Message Input */}
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
                  onClick={cancel}
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
              <div className="flex items-center gap-2">
                <div
                  className="w-px h-5"
                  style={{ backgroundColor: 'var(--codex-border)' }}
                />
                <Slash
                  className="w-3.5 h-3.5"
                  strokeWidth={1.5}
                  style={{ color: 'var(--codex-fg-subtle)', opacity: 0.5 }}
                />
              </div>

              <button
                className="flex items-center gap-1.5 px-2 py-1 rounded text-xs hover:bg-[var(--codex-bg-secondary)] transition-colors"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                <span>{selectedModel}</span>
                <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
              </button>

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
                  color:
                    message.trim() && !isStreaming
                      ? 'var(--codex-fg)'
                      : 'var(--codex-fg-subtle)',
                  opacity: isStreaming ? 0.5 : 1,
                }}
                onMouseEnter={(e) => {
                  if (message.trim() && !isStreaming)
                    e.currentTarget.style.backgroundColor =
                      'var(--codex-bg-secondary)';
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
      </div>

      {/* Right Context Sidebar */}
      <aside
        className="w-[260px] border-l overflow-y-auto"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderColor: 'var(--codex-border-subtle)',
        }}
      >
        {/* Connection Status */}
        <div
          className="px-4 py-2.5 border-b flex items-center justify-between"
          style={{ borderColor: 'var(--codex-border-subtle)' }}
        >
          <StatusDot status={status} />
          {isStreaming && (
            <Loader2
              className="w-3 h-3 animate-spin"
              strokeWidth={1.5}
              style={{ color: 'var(--codex-accent)' }}
            />
          )}
        </div>

        {/* Session Info */}
        <SidebarSection
          title="Session Info"
          open={sessionOpen}
          onToggle={() => setSessionOpen(!sessionOpen)}
        >
          <div className="px-4 pb-4 space-y-3 text-[13px]">
            <SidebarRow label="Messages">
              <span style={{ color: 'var(--codex-fg)' }}>
                {messages.length}
              </span>
            </SidebarRow>
            <SidebarRow label="Streaming">
              <span
                style={{
                  color: isStreaming
                    ? 'var(--codex-accent)'
                    : 'var(--codex-fg-subtle)',
                }}
              >
                {isStreaming ? 'Active' : '--'}
              </span>
            </SidebarRow>
            {thinking && (
              <SidebarRow label="Phase">
                <span
                  style={{
                    color: 'var(--codex-accent)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: '12px',
                  }}
                >
                  {phaseLabel(thinking.phase)}
                </span>
              </SidebarRow>
            )}
            {thinking?.strategy && (
              <SidebarRow label="Strategy">
                <div className="flex items-center gap-1">
                  <span
                    style={{
                      color: 'var(--codex-fg)',
                      fontFamily: 'var(--font-mono)',
                      fontSize: '12px',
                    }}
                  >
                    {thinking.strategy}
                  </span>
                  {thinking.confidence != null && (
                    <span
                      className="text-[10px]"
                      style={{
                        color: getStrategyColor(thinking.confidence * 100),
                        fontFamily: 'var(--font-mono)',
                      }}
                    >
                      {Math.round(thinking.confidence * 100)}%
                    </span>
                  )}
                </div>
              </SidebarRow>
            )}
            {thinking?.engine && (
              <SidebarRow label="Engine">
                <span
                  style={{
                    color: 'var(--codex-fg)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: '12px',
                  }}
                >
                  {thinking.engine}
                </span>
              </SidebarRow>
            )}
            {thinking?.iteration != null && thinking?.maxIterations != null && (
              <SidebarRow label="Iteration">
                <span
                  style={{
                    color: 'var(--codex-fg)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: '12px',
                  }}
                >
                  {thinking.iteration}/{thinking.maxIterations}
                </span>
              </SidebarRow>
            )}
          </div>
        </SidebarSection>

        {/* Active Tool Calls (shown during streaming) */}
        {thinking && thinking.toolCalls.length > 0 && (
          <SidebarSection title="Tool Calls" open={true} onToggle={() => {}}>
            <div className="px-4 pb-4 space-y-2">
              {thinking.toolCalls.map((tc, idx) => (
                <ToolCallItem key={`${tc.name}-${idx}`} toolCall={tc} />
              ))}
            </div>
          </SidebarSection>
        )}

        {/* Recent Sessions */}
        <SidebarSection
          title="Recent Sessions"
          open={sessionsListOpen}
          onToggle={() => setSessionsListOpen(!sessionsListOpen)}
        >
          <div className="px-4 pb-4 space-y-2">
            {sessionsLoading && (
              <div className="flex items-center gap-2 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                <Loader2 className="w-3 h-3 animate-spin" strokeWidth={1.5} />
                Loading...
              </div>
            )}
            {!sessionsLoading && sortedSessions.length === 0 && (
              <div
                className="text-[12px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                No sessions yet
              </div>
            )}
            {sortedSessions.slice(0, 10).map((session) => (
              <button
                key={session.key}
                // TODO: Load session messages when clicked
                // onClick={() => loadSession(session.key)}
                className="w-full text-left p-2 rounded transition-colors"
                style={{
                  backgroundColor: 'transparent',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'transparent';
                }}
              >
                <div className="flex items-center justify-between mb-1">
                  <span
                    className="text-[11px]"
                    style={{
                      color: 'var(--codex-fg)',
                      fontFamily: 'var(--font-mono)',
                    }}
                  >
                    #{session.key.slice(0, 8)}
                  </span>
                  <span
                    className="text-[10px]"
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    {session.messageCount} msgs
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-[10px]" style={{ color: '#888' }}>
                  <Clock className="w-2.5 h-2.5" strokeWidth={1.5} />
                  {formatDuration(session.createdAt, session.updatedAt)}
                </div>
              </button>
            ))}
          </div>
        </SidebarSection>

        {/* Memory Context */}
        <SidebarSection
          title="Memory Context"
          open={memoryOpen}
          onToggle={() => setMemoryOpen(!memoryOpen)}
        >
          <div className="px-4 pb-4">
            <div
              className="text-[12px]"
              style={{ color: 'var(--codex-fg-subtle)' }}
            >
              --
            </div>
          </div>
        </SidebarSection>

        {/* Quick Tasks */}
        <SidebarSection
          title="Quick Tasks"
          open={tasksOpen}
          onToggle={() => setTasksOpen(!tasksOpen)}
        >
          <div className="px-4 pb-4 space-y-3">
            <div
              className="text-[12px]"
              style={{ color: 'var(--codex-fg-subtle)' }}
            >
              --
            </div>
          </div>
        </SidebarSection>

        {/* Calendar */}
        <SidebarSection
          title="Upcoming"
          open={calendarOpen}
          onToggle={() => setCalendarOpen(!calendarOpen)}
          noBorder
        >
          <div className="px-4 pb-4">
            <div
              className="text-[12px]"
              style={{ color: 'var(--codex-fg-subtle)' }}
            >
              --
            </div>
          </div>
        </SidebarSection>
      </aside>
    </>
  );
}

/* ── Message rendering ───────────────────────────────────────────────────── */

function MessageBubble({
  msg,
}: {
  msg: ChatMessage;
}) {
  return (
    <motion.div
      key={msg.id}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex flex-col"
    >
      {msg.role === 'user' && (
        <div className="flex justify-end">
          <div
            className="max-w-[85%] px-4 py-3 rounded-lg"
            style={{
              backgroundColor: 'var(--codex-bg-user)',
              color: 'var(--codex-fg)',
            }}
          >
            <div
              className="text-[14px] leading-relaxed whitespace-pre-wrap"
              style={{ color: 'var(--codex-fg)' }}
            >
              {msg.content}
            </div>
            <div
              className="text-[11px] mt-2"
              style={{
                color: 'var(--codex-fg-subtle)',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {formatTime(msg.timestamp)}
            </div>
          </div>
        </div>
      )}

      {msg.role === 'assistant' && (
        <div className="flex gap-3">
          <div className="flex-1">
            <div
              className="text-[14px] leading-relaxed whitespace-pre-wrap"
              style={{ color: 'var(--codex-fg)' }}
            >
              {msg.content}
              {msg.isStreaming && (
                <span
                  className="inline-block w-[2px] h-[14px] ml-0.5 align-text-bottom animate-pulse"
                  style={{ backgroundColor: 'var(--codex-accent)' }}
                />
              )}
            </div>
            <div className="flex items-center gap-2 mt-2">
              <div
                className="text-[11px]"
                style={{
                  color: 'var(--codex-fg-subtle)',
                  fontFamily: 'var(--font-mono)',
                }}
              >
                {formatTime(msg.timestamp)}
              </div>
            </div>
          </div>
        </div>
      )}

      {msg.role === 'system' && (
        <div className="flex justify-center py-2">
          <div
            className="flex items-center gap-2 px-3 py-1.5 rounded-md text-[12px]"
            style={{
              backgroundColor: 'var(--codex-bg-secondary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            <Terminal className="w-3 h-3" strokeWidth={1.5} />
            {msg.content}
          </div>
        </div>
      )}
    </motion.div>
  );
}

/* ── Thinking / streaming indicator ──────────────────────────────────────── */

function ThinkingIndicator({ thinking }: { thinking: ThinkingState }) {
  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-border)',
      }}
    >
      {/* Phase header */}
      <div
        className="px-3 py-2 flex items-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderBottom: '1px solid var(--codex-border)',
        }}
      >
        <Loader2
          className="w-3.5 h-3.5 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[12px]"
          style={{
            color: 'var(--codex-fg)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {phaseLabel(thinking.phase)}
        </span>

        {/* Strategy badge */}
        {thinking.strategy && (
          <div
            className="flex items-center gap-1 px-2 py-0.5 rounded ml-auto"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
            }}
          >
            <span
              className="text-[10px]"
              style={{
                color: '#888',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {thinking.strategy}
            </span>
            {thinking.confidence != null && (
              <>
                <span className="text-[10px]" style={{ color: '#666' }}>
                  &middot;
                </span>
                <span
                  className="text-[10px]"
                  style={{
                    color:
                      thinking.confidence > 0.8
                        ? 'var(--codex-accent)'
                        : thinking.confidence > 0.5
                          ? '#e5a00d'
                          : '#ef4444',
                    fontFamily: 'var(--font-mono)',
                    fontWeight: 500,
                  }}
                >
                  {Math.round(thinking.confidence * 100)}%
                </span>
              </>
            )}
          </div>
        )}

        {/* Iteration counter */}
        {thinking.iteration != null && thinking.maxIterations != null && (
          <span
            className="text-[10px] ml-2"
            style={{
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            {thinking.iteration}/{thinking.maxIterations}
          </span>
        )}
      </div>

      {/* Tool calls */}
      {thinking.toolCalls.length > 0 && (
        <div className="px-3 py-2 space-y-1.5">
          {thinking.toolCalls.map((tc, idx) => (
            <div
              key={`${tc.name}-${idx}`}
              className="flex items-center gap-2"
            >
              {tc.completed ? (
                tc.success ? (
                  <Check
                    className="w-3 h-3"
                    strokeWidth={2}
                    style={{ color: '#10b981' }}
                  />
                ) : (
                  <X
                    className="w-3 h-3"
                    strokeWidth={2}
                    style={{ color: '#ef4444' }}
                  />
                )
              ) : (
                <Loader2
                  className="w-3 h-3 animate-spin"
                  strokeWidth={1.5}
                  style={{ color: 'var(--codex-accent)' }}
                />
              )}
              <span
                className="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wide"
                style={{
                  backgroundColor: tc.completed
                    ? tc.success
                      ? 'rgba(16, 185, 129, 0.1)'
                      : 'rgba(239, 68, 68, 0.1)'
                    : 'var(--codex-accent-dim)',
                  color: tc.completed
                    ? tc.success
                      ? '#10b981'
                      : '#ef4444'
                    : 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)',
                  fontWeight: 500,
                }}
              >
                {tc.name}
              </span>
              {tc.durationMs != null && (
                <span
                  className="text-[10px]"
                  style={{
                    color: '#888',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  {tc.durationMs}ms
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Pulsing dots when no tool calls yet */}
      {thinking.toolCalls.length === 0 && (
        <div className="px-3 py-3 flex gap-1.5">
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{ backgroundColor: 'var(--codex-accent)' }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.2s',
            }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.4s',
            }}
          />
        </div>
      )}
    </div>
  );
}

/* ── Tool call item for sidebar ──────────────────────────────────────────── */

function ToolCallItem({ toolCall }: { toolCall: ToolCallState }) {
  return (
    <div
      className="flex items-center gap-2 p-1.5 rounded"
      style={{ backgroundColor: 'var(--codex-bg)' }}
    >
      {toolCall.completed ? (
        toolCall.success ? (
          <Check
            className="w-3 h-3 flex-shrink-0"
            strokeWidth={2}
            style={{ color: '#10b981' }}
          />
        ) : (
          <X
            className="w-3 h-3 flex-shrink-0"
            strokeWidth={2}
            style={{ color: '#ef4444' }}
          />
        )
      ) : (
        <Loader2
          className="w-3 h-3 flex-shrink-0 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
      )}
      <span
        className="text-[11px] truncate"
        style={{
          color: toolCall.completed
            ? toolCall.success
              ? '#10b981'
              : '#ef4444'
            : 'var(--codex-accent)',
          fontFamily: 'var(--font-mono)',
        }}
      >
        {toolCall.name}
      </span>
      {toolCall.durationMs != null && (
        <span
          className="text-[10px] ml-auto flex-shrink-0"
          style={{ color: '#888', fontFamily: 'var(--font-mono)' }}
        >
          {toolCall.durationMs}ms
        </span>
      )}
    </div>
  );
}

/* ── Reusable sidebar section ────────────────────────────────────────────── */

function SidebarSection({
  title,
  open,
  onToggle,
  noBorder,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  noBorder?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      className={noBorder ? '' : 'border-b'}
      style={{ borderColor: 'var(--codex-border-subtle)' }}
    >
      <button
        onClick={onToggle}
        className="w-full px-4 py-3 flex items-center justify-between transition-colors"
        style={{
          backgroundColor: 'transparent',
          color: 'var(--codex-fg-subtle)',
        }}
        onMouseEnter={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-muted)')
        }
        onMouseLeave={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-subtle)')
        }
      >
        <span
          className="text-[10px] uppercase tracking-wider"
          style={{ fontWeight: 500 }}
        >
          {title}
        </span>
        {open ? (
          <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} />
        ) : (
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        )}
      </button>
      {open && children}
    </div>
  );
}

function SidebarRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex justify-between items-center">
      <span style={{ color: 'var(--codex-fg-subtle)' }}>{label}</span>
      <span style={{ color: 'var(--codex-fg)' }}>{children}</span>
    </div>
  );
}
