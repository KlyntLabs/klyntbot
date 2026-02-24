import { useState } from 'react';
import {
  Send,
  ChevronDown,
  ChevronRight,
  Terminal,
  ArrowRight,
  Sparkles,
  Code,
  Lightbulb,
  FileCode,
  Loader2,
  Check,
  Slash,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';

type Message = {
  id: string;
  type:
    | 'agent'
    | 'user'
    | 'tool'
    | 'delegation'
    | 'ask-form'
    | 'parallel-tools';
  content: string;
  timestamp: string;
  toolName?: string;
  delegationFrom?: string;
  delegationTo?: string;
  strategy?: {
    type: string;
    confidence: number;
  };
  toolData?: any;
};

type SuggestionCard = {
  id: string;
  icon: typeof Code;
  title: string;
  description: string;
};

export default function Chat() {
  const [sessionOpen, setSessionOpen] = useState(true);
  const [tokenOpen, setTokenOpen] = useState(true);
  const [tokenBreakdownOpen, setTokenBreakdownOpen] = useState(false);
  const [costBreakdownOpen, setCostBreakdownOpen] = useState(false);
  const [planOpen, setPlanOpen] = useState(true);
  const [memoryOpen, setMemoryOpen] = useState(true);
  const [tasksOpen, setTasksOpen] = useState(true);
  const [calendarOpen, setCalendarOpen] = useState(true);
  const [subagentsOpen, setSubagentsOpen] = useState(false);
  const [message, setMessage] = useState('');
  const [hasMessages, setHasMessages] = useState(true);
  const [expandedOutput, setExpandedOutput] = useState<string | null>(null);
  const [selectedModel] = useState('GPT-4');

  const messages: Message[] = [
    {
      id: '1',
      type: 'user',
      content:
        'Can you help me refactor the authentication module and add unit tests?',
      timestamp: '10:32 AM',
    },
    {
      id: '2',
      type: 'agent',
      content:
        "I'll help you refactor the authentication module and add comprehensive unit tests. Let me break this down into a plan and delegate to the coding agent.",
      timestamp: '10:32 AM',
      strategy: { type: 'ToolAssisted', confidence: 87 },
    },
    {
      id: '3',
      type: 'delegation',
      content: 'Delegating task to coding specialist',
      timestamp: '10:32 AM',
      delegationFrom: 'root',
      delegationTo: 'coding-agent',
    },
    {
      id: '4',
      type: 'parallel-tools',
      content: 'Running parallel analysis',
      timestamp: '10:33 AM',
      toolData: {
        tools: [
          {
            name: 'web_search',
            status: 'running',
            args: { query: 'Rust async patterns' },
          },
          {
            name: 'read_file',
            status: 'success',
            args: { path: 'src/main.rs' },
            duration: '342ms',
          },
        ],
      },
    },
    {
      id: '5',
      type: 'agent',
      content: 'I need some details before creating this task:',
      timestamp: '10:33 AM',
      strategy: { type: 'Direct', confidence: 95 },
    },
    {
      id: '6',
      type: 'ask-form',
      content: 'Task details form',
      timestamp: '10:33 AM',
    },
    {
      id: '7',
      type: 'tool',
      content:
        'cargo test --workspace\n\nrunning 42 tests\ntest auth::test_login ... ok\ntest auth::test_logout ... ok\ntest utils::test_hash ... ok\n\ntest result: ok. 42 passed; 0 failed; 0 ignored\n\nFinished in 2.4s',
      timestamp: '10:33 AM',
      toolName: 'shell',
      toolData: {
        command: 'cargo test --workspace',
        status: 'success',
        duration: '2.4s',
      },
    },
    {
      id: '8',
      type: 'agent',
      content:
        "I've successfully refactored the authentication module with improved error handling and added 20 unit tests. Code coverage is now at 94.2%.",
      timestamp: '10:34 AM',
      strategy: { type: 'Autonomous', confidence: 72 },
    },
  ];

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
                <motion.div
                  key={msg.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="flex flex-col"
                >
                  {msg.type === 'user' && (
                    <div className="flex justify-end">
                      <div
                        className="max-w-[85%] px-4 py-3 rounded-lg"
                        style={{
                          backgroundColor: 'var(--codex-bg-user)',
                          color: 'var(--codex-fg)',
                        }}
                      >
                        <div
                          className="text-[14px] leading-relaxed"
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
                          {msg.timestamp}
                        </div>
                      </div>
                    </div>
                  )}

                  {msg.type === 'agent' && (
                    <div className="flex gap-3">
                      <div className="flex-1">
                        <div
                          className="text-[14px] leading-relaxed"
                          style={{ color: 'var(--codex-fg)' }}
                        >
                          {msg.content}
                        </div>
                        <div className="flex items-center gap-2 mt-2">
                          <div
                            className="text-[11px]"
                            style={{
                              color: 'var(--codex-fg-subtle)',
                              fontFamily: 'var(--font-mono)',
                            }}
                          >
                            {msg.timestamp}
                          </div>
                          {msg.strategy && (
                            <div
                              className="flex items-center gap-1 px-2 py-0.5 rounded"
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
                                {msg.strategy.type}
                              </span>
                              <span
                                className="text-[10px]"
                                style={{ color: '#666' }}
                              >
                                &middot;
                              </span>
                              <span
                                className="text-[10px]"
                                style={{
                                  color: getStrategyColor(
                                    msg.strategy.confidence,
                                  ),
                                  fontFamily: 'var(--font-mono)',
                                  fontWeight: 500,
                                }}
                              >
                                {msg.strategy.confidence}%
                              </span>
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                  )}

                  {msg.type === 'parallel-tools' && msg.toolData && (
                    <div className="flex gap-3">
                      {msg.toolData.tools.map((tool: any, idx: number) => (
                        <div
                          key={idx}
                          className="flex-1 rounded-lg overflow-hidden"
                          style={{
                            backgroundColor: '#141414',
                            border: '1px solid var(--codex-border)',
                          }}
                        >
                          <div
                            className="px-3 py-2 flex items-center gap-2"
                            style={{
                              backgroundColor: 'var(--codex-bg-secondary)',
                              borderBottom: '1px solid var(--codex-border)',
                            }}
                          >
                            {tool.status === 'running' ? (
                              <Loader2
                                className="w-3.5 h-3.5 animate-spin"
                                strokeWidth={1.5}
                                style={{ color: 'var(--codex-accent)' }}
                              />
                            ) : (
                              <Check
                                className="w-3.5 h-3.5"
                                strokeWidth={1.5}
                                style={{ color: '#10b981' }}
                              />
                            )}
                            <span
                              className="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wide"
                              style={{
                                backgroundColor:
                                  tool.status === 'running'
                                    ? 'var(--codex-accent-dim)'
                                    : 'rgba(16, 185, 129, 0.1)',
                                color:
                                  tool.status === 'running'
                                    ? 'var(--codex-accent)'
                                    : '#10b981',
                                fontFamily: 'var(--font-mono)',
                                fontWeight: 500,
                              }}
                            >
                              {tool.name}
                            </span>
                          </div>
                          <div
                            className="px-3 py-3 text-[12px]"
                            style={{
                              fontFamily: 'var(--font-mono)',
                              color: 'var(--codex-fg-muted)',
                            }}
                          >
                            <div className="mb-1" style={{ color: '#888' }}>
                              {Object.keys(tool.args)[0]}:
                            </div>
                            <div>
                              {Object.values(tool.args)[0] as string}
                            </div>
                            {tool.duration && (
                              <div
                                className="mt-2 text-[11px]"
                                style={{ color: '#888' }}
                              >
                                {tool.duration}
                              </div>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                  {msg.type === 'ask-form' && (
                    <div
                      className="rounded-lg overflow-hidden"
                      style={{
                        backgroundColor: '#141414',
                        borderLeft: '2px solid var(--codex-accent)',
                      }}
                    >
                      <div className="p-4">
                        <h3
                          className="text-[14px] mb-4"
                          style={{
                            color: 'var(--codex-fg)',
                            fontWeight: 500,
                          }}
                        >
                          Task Details
                        </h3>

                        {/* Priority */}
                        <div className="mb-4">
                          <label
                            className="block text-[12px] mb-2"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          >
                            Priority
                          </label>
                          <div className="flex gap-2">
                            {['Critical', 'High', 'Medium', 'Low'].map(
                              (priority) => (
                                <button
                                  key={priority}
                                  className="flex-1 px-3 py-2 rounded text-[12px] border transition-all"
                                  style={{
                                    backgroundColor:
                                      priority === 'Medium'
                                        ? 'var(--codex-accent-dim)'
                                        : 'transparent',
                                    borderColor:
                                      priority === 'Medium'
                                        ? 'var(--codex-accent)'
                                        : 'var(--codex-border)',
                                    color:
                                      priority === 'Medium'
                                        ? 'var(--codex-accent)'
                                        : 'var(--codex-fg-subtle)',
                                  }}
                                >
                                  {priority}
                                </button>
                              ),
                            )}
                          </div>
                        </div>

                        {/* Category */}
                        <div className="mb-4">
                          <label
                            className="block text-[12px] mb-2"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          >
                            Category
                          </label>
                          <div className="relative">
                            <select
                              className="w-full px-3 py-2 rounded border text-[13px] appearance-none"
                              style={{
                                backgroundColor: 'var(--codex-bg)',
                                borderColor: 'var(--codex-border)',
                                color: 'var(--codex-fg)',
                              }}
                            >
                              <option>Development</option>
                              <option>Design</option>
                              <option>Research</option>
                              <option>Documentation</option>
                            </select>
                            <ChevronDown
                              className="w-4 h-4 absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none"
                              strokeWidth={1.5}
                              style={{ color: 'var(--codex-fg-subtle)' }}
                            />
                          </div>
                        </div>

                        {/* Due date */}
                        <div className="mb-4">
                          <label
                            className="block text-[12px] mb-2"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          >
                            Due date
                          </label>
                          <input
                            type="text"
                            placeholder="e.g., next Friday"
                            className="w-full px-3 py-2 rounded border text-[13px] outline-none"
                            style={{
                              backgroundColor: 'var(--codex-bg)',
                              borderColor: 'var(--codex-border)',
                              color: 'var(--codex-fg)',
                            }}
                          />
                        </div>

                        {/* Actions */}
                        <div className="flex items-center justify-end gap-3">
                          <button
                            className="text-[12px]"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          >
                            Skip
                          </button>
                          <button
                            className="px-4 py-2 rounded text-[13px]"
                            style={{
                              backgroundColor: 'var(--codex-accent)',
                              color: 'white',
                            }}
                          >
                            Submit
                          </button>
                        </div>
                      </div>
                    </div>
                  )}

                  {msg.type === 'tool' && msg.toolData && (
                    <div className="flex">
                      <div
                        className="flex-1 max-w-[85%] rounded-lg overflow-hidden"
                        style={{
                          backgroundColor: 'var(--codex-bg-tertiary)',
                          border: '1px solid var(--codex-border)',
                        }}
                      >
                        <div
                          className="px-3 py-2 flex items-center justify-between"
                          style={{
                            backgroundColor: 'var(--codex-bg-secondary)',
                            borderBottom: '1px solid var(--codex-border)',
                          }}
                        >
                          <div className="flex items-center gap-2">
                            <Terminal
                              className="w-3.5 h-3.5"
                              strokeWidth={1.5}
                              style={{ color: 'var(--codex-fg-subtle)' }}
                            />
                            <span
                              className="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wide"
                              style={{
                                backgroundColor: 'var(--codex-accent-dim)',
                                color: 'var(--codex-accent)',
                                fontFamily: 'var(--font-mono)',
                                fontWeight: 500,
                              }}
                            >
                              {msg.toolName}
                            </span>
                          </div>
                          <div className="flex items-center gap-2">
                            {msg.toolData.status === 'success' && (
                              <div
                                className="w-1.5 h-1.5 rounded-full"
                                style={{ backgroundColor: '#10b981' }}
                              />
                            )}
                            <span
                              className="text-[11px]"
                              style={{
                                color: '#888',
                                fontFamily: 'var(--font-mono)',
                              }}
                            >
                              {msg.toolData.duration}
                            </span>
                          </div>
                        </div>
                        <div className="px-3 py-2.5">
                          <div
                            className="text-[12px] mb-2"
                            style={{
                              fontFamily: 'var(--font-mono)',
                              color: 'var(--codex-fg-muted)',
                            }}
                          >
                            {msg.toolData.command}
                          </div>
                          <button
                            onClick={() =>
                              setExpandedOutput(
                                expandedOutput === msg.id ? null : msg.id,
                              )
                            }
                            className="flex items-center gap-1 text-[11px] transition-colors"
                            style={{ color: 'var(--codex-accent)' }}
                          >
                            {expandedOutput === msg.id ? '\u25BC' : '\u25B6'}{' '}
                            {expandedOutput === msg.id ? 'Hide' : 'Show'} output
                          </button>
                          <AnimatePresence>
                            {expandedOutput === msg.id && (
                              <motion.div
                                initial={{ height: 0, opacity: 0 }}
                                animate={{ height: 'auto', opacity: 1 }}
                                exit={{ height: 0, opacity: 0 }}
                                className="mt-2 pt-2 border-t text-[12px]"
                                style={{
                                  borderColor: 'var(--codex-border)',
                                  fontFamily: 'var(--font-mono)',
                                  color: 'var(--codex-fg-muted)',
                                }}
                              >
                                <pre className="whitespace-pre-wrap">
                                  {msg.content}
                                </pre>
                              </motion.div>
                            )}
                          </AnimatePresence>
                        </div>
                      </div>
                    </div>
                  )}

                  {msg.type === 'delegation' && (
                    <div className="flex justify-center py-2">
                      <div
                        className="flex items-center gap-2.5 px-3 py-1.5 rounded-md"
                        style={{
                          backgroundColor: 'var(--codex-bg-secondary)',
                          border: '1px solid var(--codex-border)',
                        }}
                      >
                        <span
                          className="px-2 py-0.5 rounded text-[11px]"
                          style={{
                            backgroundColor: 'var(--codex-bg-tertiary)',
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)',
                            border: '1px solid var(--codex-border)',
                          }}
                        >
                          {msg.delegationFrom}
                        </span>
                        <ArrowRight
                          className="w-3 h-3"
                          strokeWidth={1.5}
                          style={{ color: 'var(--codex-fg-subtle)' }}
                        />
                        <span
                          className="px-2 py-0.5 rounded text-[11px]"
                          style={{
                            backgroundColor: 'var(--codex-bg-tertiary)',
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)',
                            border: '1px solid var(--codex-border)',
                          }}
                        >
                          {msg.delegationTo}
                        </span>
                        <button
                          onClick={() => setSubagentsOpen(!subagentsOpen)}
                          className="ml-2 flex items-center gap-1.5 px-2 py-0.5 rounded text-[10px] transition-colors"
                          style={{
                            backgroundColor: 'var(--codex-bg-tertiary)',
                            color: 'var(--codex-fg-subtle)',
                            border: '1px solid var(--codex-border)',
                          }}
                        >
                          <div className="flex gap-0.5">
                            <div
                              className="w-1 h-1 rounded-full animate-pulse"
                              style={{ backgroundColor: '#10b981' }}
                            />
                            <div
                              className="w-1 h-1 rounded-full animate-pulse"
                              style={{
                                backgroundColor: '#10b981',
                                animationDelay: '0.2s',
                              }}
                            />
                          </div>
                          2 active
                        </button>
                      </div>
                    </div>
                  )}
                </motion.div>
              ))}
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
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                placeholder="Message klyntbot..."
                rows={1}
                className="flex-1 bg-transparent outline-none resize-none text-[14px]"
                style={{
                  color: 'var(--codex-fg)',
                  fontFamily: 'var(--font-ui)',
                  maxHeight: '200px',
                }}
              />

              <button
                className="p-1.5 rounded transition-colors"
                style={{
                  color: message
                    ? 'var(--codex-fg)'
                    : 'var(--codex-fg-subtle)',
                }}
                onMouseEnter={(e) => {
                  if (message)
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
        {/* Session Info */}
        <SidebarSection
          title="Session Info"
          open={sessionOpen}
          onToggle={() => setSessionOpen(!sessionOpen)}
        >
          <div className="px-4 pb-4 space-y-3 text-[13px]">
            <SidebarRow label="Session ID">
              <span
                style={{
                  color: 'var(--codex-fg)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '12px',
                }}
              >
                #a8f32e
              </span>
            </SidebarRow>
            <SidebarRow label="Duration">1h 24m</SidebarRow>
            <SidebarRow label="Messages">47</SidebarRow>
          </div>
        </SidebarSection>

        {/* Token Usage */}
        <SidebarSection
          title="Token Usage"
          open={tokenOpen}
          onToggle={() => setTokenOpen(!tokenOpen)}
        >
          <div className="px-4 pb-4 space-y-3">
            <div className="space-y-2">
              <div className="flex justify-between text-[13px] items-center">
                <span style={{ color: 'var(--codex-fg)' }}>12.4K tokens</span>
                <span style={{ color: 'var(--codex-fg-subtle)' }}>8%</span>
              </div>
              <div
                className="h-1 rounded-full overflow-hidden"
                style={{ backgroundColor: 'var(--codex-bg)' }}
              >
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: '8%',
                    backgroundColor: 'var(--codex-accent)',
                  }}
                />
              </div>
            </div>

            {/* Token Breakdown */}
            <button
              onClick={() => setTokenBreakdownOpen(!tokenBreakdownOpen)}
              className="w-full text-left text-[11px]"
              style={{ color: '#888' }}
            >
              {tokenBreakdownOpen ? '\u25BC' : '\u25B6'} Breakdown
            </button>
            <AnimatePresence>
              {tokenBreakdownOpen && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  className="text-[11px] space-y-1"
                  style={{
                    color: '#888',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  <div>Prompt: 8.2K</div>
                  <div>Completion: 4.2K</div>
                  <div>Cache: 2.1K</div>
                </motion.div>
              )}
            </AnimatePresence>

            <div className="text-[13px] flex justify-between items-center pt-1">
              <span style={{ color: 'var(--codex-fg-subtle)' }}>Cost</span>
              <span
                style={{
                  color: 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)',
                }}
              >
                $0.05
              </span>
            </div>

            {/* Cost Breakdown */}
            <button
              onClick={() => setCostBreakdownOpen(!costBreakdownOpen)}
              className="w-full text-left text-[11px]"
              style={{ color: '#888' }}
            >
              {costBreakdownOpen ? '\u25BC' : '\u25B6'} Cost breakdown
            </button>
            <AnimatePresence>
              {costBreakdownOpen && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  className="text-[11px]"
                  style={{
                    color: '#888',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  $0.02 prompt + $0.03 completion = $0.05
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </SidebarSection>

        {/* Active Plan */}
        <SidebarSection
          title="Active Plan"
          open={planOpen}
          onToggle={() => setPlanOpen(!planOpen)}
        >
          <div className="px-4 pb-4 space-y-3">
            <div
              className="text-[13px]"
              style={{ color: 'var(--codex-fg)' }}
            >
              Refactor authentication module
            </div>
            <div className="space-y-2">
              <div className="flex justify-between text-[13px] items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>
                  Step 3/7
                </span>
                <span style={{ color: 'var(--codex-fg)' }}>43%</span>
              </div>
              <div
                className="h-1 rounded-full overflow-hidden"
                style={{ backgroundColor: 'var(--codex-bg)' }}
              >
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: '43%',
                    backgroundColor: 'var(--codex-accent)',
                  }}
                />
              </div>
            </div>
            {/* Step list */}
            <div className="space-y-1.5 text-[11px] pt-2">
              <div
                className="flex items-center gap-2"
                style={{ color: '#10b981' }}
              >
                <Check className="w-3 h-3" strokeWidth={2} />
                <span>Step 1</span>
              </div>
              <div
                className="flex items-center gap-2"
                style={{ color: '#10b981' }}
              >
                <Check className="w-3 h-3" strokeWidth={2} />
                <span>Step 2</span>
              </div>
              <div
                className="flex items-center gap-2"
                style={{ color: 'var(--codex-accent)' }}
              >
                <ArrowRight className="w-3 h-3" strokeWidth={2} />
                <span>Step 3</span>
              </div>
              <div
                className="flex items-center gap-2"
                style={{ color: '#666' }}
              >
                <div
                  className="w-3 h-3 rounded-full border"
                  style={{ borderColor: '#666' }}
                />
                <span>Steps 4-7</span>
              </div>
            </div>
          </div>
        </SidebarSection>

        {/* Memory Context */}
        <SidebarSection
          title="Memory Context"
          open={memoryOpen}
          onToggle={() => setMemoryOpen(!memoryOpen)}
        >
          <div className="px-4 pb-4 space-y-2">
            <div
              className="text-[12px] mb-2"
              style={{ color: 'var(--codex-fg)' }}
            >
              3 memories recalled
            </div>
            <div
              className="space-y-2 text-[11px]"
              style={{ color: 'var(--codex-fg-muted)' }}
            >
              {[
                { name: 'Auth refactor discussion', relevance: 0.82 },
                { name: 'Testing strategy notes', relevance: 0.74 },
                { name: 'Code review patterns', relevance: 0.68 },
              ].map((memory) => (
                <div key={memory.name} className="flex gap-2">
                  <span>&middot;</span>
                  <div>
                    <div>{memory.name}</div>
                    <div style={{ color: '#888' }}>
                      ({memory.relevance} relevance)
                    </div>
                  </div>
                </div>
              ))}
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
            <div className="flex items-center gap-2 text-[13px]">
              <div
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: 'var(--codex-accent)' }}
              />
              <span style={{ color: 'var(--codex-fg)' }}>4 active</span>
            </div>
            <div className="flex items-center gap-2 text-[13px]">
              <div
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: '#ef4444' }}
              />
              <span style={{ color: 'var(--codex-fg)' }}>1 overdue</span>
            </div>
            <button
              className="w-full mt-2 px-3 py-1.5 rounded text-[12px] border transition-all"
              style={{
                borderColor: 'var(--codex-border)',
                color: 'var(--codex-fg-subtle)',
                backgroundColor: 'transparent',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                e.currentTarget.style.color = 'var(--codex-fg)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
              }}
            >
              View all tasks
            </button>
          </div>
        </SidebarSection>

        {/* Calendar */}
        <SidebarSection
          title="Upcoming"
          open={calendarOpen}
          onToggle={() => setCalendarOpen(!calendarOpen)}
          noBorder
        >
          <div className="px-4 pb-4 space-y-4">
            <div className="space-y-1">
              <div
                className="text-[11px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                Today, 2:00 PM
              </div>
              <div
                className="text-[13px]"
                style={{ color: 'var(--codex-fg)' }}
              >
                Team standup
              </div>
            </div>
            <div className="space-y-1">
              <div
                className="text-[11px]"
                style={{ color: 'var(--codex-fg-subtle)' }}
              >
                Tomorrow, 10:00 AM
              </div>
              <div
                className="text-[13px]"
                style={{ color: 'var(--codex-fg)' }}
              >
                Code review session
              </div>
            </div>
          </div>
        </SidebarSection>
      </aside>
    </>
  );
}

/* Reusable sidebar section */
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
