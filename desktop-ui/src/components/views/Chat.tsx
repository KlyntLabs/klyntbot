import { useState, useRef, useEffect, useMemo, useCallback } from 'react';
import { useNavigate } from 'react-router';
import {
  Send, ChevronDown, Plus, Settings, FolderOpen, MessageSquare,
  RotateCcw, Mic, Shield, Server,
} from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { isTauri } from '../../lib/utils';
import type { ChatMessage, ChatThread, SidebarItem } from '../../lib/types';

// Local mock threads grouped by project (used as fallback in browser dev)
interface ChatProject {
  id: string;
  name: string;
  threads: { id: string; name: string; timestamp: string; projectId: string }[];
}

const mockChatProjects: ChatProject[] = [
  {
    id: '1',
    name: 'KlyntBot',
    threads: [
      { id: 't1', name: 'Task automation logic', timestamp: '1w', projectId: '1' },
      { id: 't2', name: 'Integration with calendar', timestamp: '2w', projectId: '1' },
    ],
  },
  {
    id: '2',
    name: 'klynt',
    threads: [
      { id: 't3', name: 'UI improvements', timestamp: '3w', projectId: '2' },
      { id: 't4', name: 'Dark theme refinements', timestamp: '1m', projectId: '2' },
    ],
  },
  {
    id: '3',
    name: 'CryptoGuard',
    threads: [
      { id: 't5', name: 'Security audit checklist', timestamp: '2m', projectId: '3' },
    ],
  },
];

const mockMessages: ChatMessage[] = [
  {
    id: '1',
    role: 'user',
    content: 'Can you help me prioritize my tasks for this week?',
  },
  {
    id: '2',
    role: 'assistant',
    content: "I'd be happy to help you prioritize your tasks! Based on your current task list, I recommend focusing on:\n\n1. P1 tasks first - these are your highest priority items\n2. Review your OKR progress to ensure alignment\n3. Block time for deep work on complex projects\n\nWould you like me to create a detailed schedule for the week?",
  },
  {
    id: '3',
    role: 'user',
    content: 'Yes, that would be great. Can you focus on the Work area tasks?',
  },
  {
    id: '4',
    role: 'assistant',
    content: "Perfect! I'll create a schedule focusing on your Work area tasks. Here's a suggested breakdown:\n\n**Monday-Tuesday:**\n- Morning: P1 tasks (high urgency items)\n- Afternoon: Team collaboration and meetings\n\n**Wednesday-Thursday:**\n- Deep work blocks for complex projects\n- Review and update OKRs\n\n**Friday:**\n- Wrap up pending items\n- Plan for next week\n\nWould you like me to add these to your calendar?",
  },
];

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const days = Math.floor(diff / 86400000);
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}m`;
}

export function Chat() {
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Chat');
  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [selectedThread, setSelectedThread] = useState<string>(isTauri ? '' : 't1');
  const [expandedProjects, toggleProject] = useSetToggle(['1', '2', '3']);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  // IPC: fetch threads from backend, fallback to mock
  const { data: threads } = useQuery<ChatThread[]>('chat_threads', undefined, []);

  // IPC: fetch messages for selected thread
  const { data: ipcMessages } = useQuery<ChatMessage[]>(
    'chat_messages',
    selectedThread && isTauri ? { session_key: selectedThread } : undefined,
    [],
  );

  // In browser dev mode, use local mock messages; in Tauri mode, use IPC messages
  const [localMessages, setLocalMessages] = useState<ChatMessage[]>(mockMessages);
  const messages = isTauri ? ipcMessages : localMessages;

  // Select first thread when threads load from IPC
  useEffect(() => {
    if (isTauri && threads.length > 0 && !selectedThread) {
      setSelectedThread(threads[0].sessionKey);
    }
  }, [threads, selectedThread]);

  const sendMessage = useMutation<ChatMessage, { content: string; session_key: string }>('chat_send');

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    return () => clearTimeout(timerRef.current);
  }, []);

  // Group threads by projectId for sidebar display
  const threadsByProject = useMemo(() => {
    const groups: Record<string, { name: string; threads: ChatThread[] }> = {};
    for (const t of threads) {
      const key = t.projectId ?? '_general';
      if (!groups[key]) groups[key] = { name: key === '_general' ? 'General' : key, threads: [] };
      groups[key].threads.push(t);
    }
    return groups;
  }, [threads]);

  const handleSend = useCallback(async () => {
    if (!input.trim()) return;

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: input,
    };

    if (isTauri) {
      const sessionKey = selectedThread || 'desktop-chat';
      await sendMessage.mutate({ content: input, session_key: sessionKey });
      if (!selectedThread) setSelectedThread(sessionKey);
    } else {
      setLocalMessages(prev => [...prev, userMessage]);
    }
    setInput('');
    setIsStreaming(true);

    // Simulate AI response in browser dev mode
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
      // In Tauri mode, streaming will be handled by useEvent('agent:content_chunk') later
      setIsStreaming(false);
    }
  }, [input, selectedThread, sendMessage]);

  // Render thread sidebar: use IPC threads in Tauri mode, mock projects in browser dev
  const renderThreadSidebar = () => {
    if (isTauri && threads.length > 0) {
      return Object.entries(threadsByProject).map(([projectId, group]) => (
        <div key={projectId}>
          <button
            onClick={() => toggleProject(projectId)}
            className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
          >
            <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
            <span className="flex-1 text-left">{group.name}</span>
            <ChevronDown
              className={`w-3.5 h-3.5 transition-transform ${
                expandedProjects.has(projectId) ? 'rotate-0' : '-rotate-90'
              }`}
              strokeWidth={1.5}
            />
          </button>
          {expandedProjects.has(projectId) && (
            <div className="mt-1 ml-3 space-y-1">
              {group.threads.map(thread => (
                <button
                  key={thread.sessionKey}
                  onClick={() => setSelectedThread(thread.sessionKey)}
                  className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
                    selectedThread === thread.sessionKey
                      ? 'bg-surface-highest text-primary'
                      : 'text-muted hover:bg-surface-base hover:text-secondary'
                  }`}
                >
                  <MessageSquare className="w-3 h-3" strokeWidth={1.5} />
                  <span className="flex-1 text-left truncate">{thread.title}</span>
                  <span className="text-[11px]">{formatRelativeTime(thread.updatedAt)}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      ));
    }

    // Browser dev fallback: mock projects
    return mockChatProjects.map(project => (
      <div key={project.id}>
        <button
          onClick={() => toggleProject(project.id)}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
        >
          <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
          <span className="flex-1 text-left">{project.name}</span>
          <ChevronDown
            className={`w-3.5 h-3.5 transition-transform ${
              expandedProjects.has(project.id) ? 'rotate-0' : '-rotate-90'
            }`}
            strokeWidth={1.5}
          />
        </button>
        {expandedProjects.has(project.id) && (
          <div className="mt-1 ml-3 space-y-1">
            {project.threads.map(thread => (
              <button
                key={thread.id}
                onClick={() => setSelectedThread(thread.id)}
                className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
                  selectedThread === thread.id
                    ? 'bg-surface-highest text-primary'
                    : 'text-muted hover:bg-surface-base hover:text-secondary'
                }`}
              >
                <MessageSquare className="w-3 h-3" strokeWidth={1.5} />
                <span className="flex-1 text-left truncate">{thread.name}</span>
                <span className="text-[11px]">{thread.timestamp}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    ));
  };

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Tasks') navigate('/');
        }}
      />

      {/* Left Sidebar — Thread List */}
      <div className="w-[250px] bg-background border-r border-border flex flex-col">
        {/* Quick Links */}
        <div className="px-4 py-3 space-y-1">
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary">
            <Plus className="w-[13px] h-[13px]" strokeWidth={1.5} />
            New thread
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary">
            <RotateCcw className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Automations
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary">
            <Settings className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Skills and Apps
          </button>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-y-auto px-3 pb-3">
          <div className="space-y-4">
            {renderThreadSidebar()}
          </div>
        </div>
      </div>

      {/* Right Panel — Conversation */}
      <div className="flex-1 flex flex-col">
        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-3xl mx-auto space-y-6">
            {messages.map(message => (
              <div
                key={message.id}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                {message.role === 'user' ? (
                  <div className="max-w-[85%] rounded-2xl px-5 py-3.5 bg-surface-raised backdrop-blur-sm">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
                      {message.content}
                    </p>
                  </div>
                ) : (
                  <div className="max-w-[85%]">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-secondary">
                      {message.content}
                    </p>
                  </div>
                )}
              </div>
            ))}
            {isStreaming && (
              <div className="flex justify-start">
                <div className="flex gap-1">
                  <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <div className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        </div>

        {/* Input Area */}
        <div className="p-6">
          <div className="max-w-3xl mx-auto">
            <div className="bg-surface-base rounded-2xl flex items-center px-2 gap-2">
              <button className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0">
                <Plus className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                  }
                }}
                placeholder="Ask Klynt anything, @ to add files, / for commands"
                rows={1}
                className="flex-1 bg-transparent py-3.5 text-[13px] text-primary placeholder:text-muted focus:outline-none font-light resize-none"
                style={{ maxHeight: '200px' }}
              />
              <button className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0">
                <Mic className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <button
                onClick={handleSend}
                disabled={!input.trim()}
                className="w-9 h-9 rounded-full bg-brand hover:bg-brand/90 disabled:bg-surface-base disabled:text-muted flex items-center justify-center transition-colors shrink-0"
              >
                <Send className="w-4 h-4" strokeWidth={2} />
              </button>
            </div>
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                  <Server className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Local</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                  <Shield className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Default permissions</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
              </div>
              <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
                <span>KlyntBot</span>
                <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
