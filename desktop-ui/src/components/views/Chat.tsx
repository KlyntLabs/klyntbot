import { useState, useEffect, useMemo, useCallback } from 'react';
import { useNavigate } from 'react-router';
import {
  Send, ChevronDown, Plus, Settings, FolderOpen, MessageSquare,
  RotateCcw, Mic, Shield, Server,
} from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { MessageList } from '../chat/MessageList';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useAgentStream } from '../../hooks/useAgentStream';
import type { ChatMessage, ChatThread, SidebarItem } from '../../lib/types';

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const days = Math.floor(diff / 86400000);
  if (days < 1) return 'now';
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}m`;
}

export function Chat() {
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Chat');
  const [input, setInput] = useState('');
  const [selectedThread, setSelectedThread] = useState('');
  const [expandedProjects, toggleProject] = useSetToggle();
  const [pendingUserMsg, setPendingUserMsg] = useState<string | null>(null);

  // IPC data
  const { data: threads } = useQuery<ChatThread[]>('chat_threads', undefined, []);
  const { data: messages, refetch: refetchMessages } = useQuery<ChatMessage[]>(
    'chat_messages',
    selectedThread ? { session_key: selectedThread } : null,
    [],
  );

  const sendMessage = useMutation<ChatMessage, Record<string, unknown>>('chat_send');

  // Streaming
  const stream = useAgentStream(selectedThread, () => {
    setPendingUserMsg(null);
    refetchMessages();
  });

  // Auto-select first thread on load
  useEffect(() => {
    if (threads.length > 0 && !selectedThread) {
      setSelectedThread(threads[0].sessionKey);
    }
  }, [threads, selectedThread]);

  // Display: persisted messages + optimistic pending user message
  const displayMessages = useMemo(() => {
    const list = [...messages];
    if (pendingUserMsg) {
      list.push({ id: 'pending', role: 'user', content: pendingUserMsg });
    }
    return list;
  }, [messages, pendingUserMsg]);

  // Group threads by projectId for sidebar
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
    if (!input.trim() || stream.isStreaming) return;
    const text = input;
    setInput('');

    const sessionKey = selectedThread || `chat:${crypto.randomUUID()}`;
    if (!selectedThread) setSelectedThread(sessionKey);

    setPendingUserMsg(text);
    stream.startStreaming();
    await sendMessage.mutate({ content: text, session_key: sessionKey });
  }, [input, selectedThread, stream, sendMessage]);

  const handleNewThread = useCallback(() => {
    setSelectedThread(`chat:${crypto.randomUUID()}`);
    setPendingUserMsg(null);
  }, []);

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
          <button
            onClick={handleNewThread}
            className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
          >
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
            {Object.entries(threadsByProject).map(([projectId, group]) => (
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
                    {group.threads.map((thread) => (
                      <button
                        key={thread.sessionKey}
                        onClick={() => {
                          setSelectedThread(thread.sessionKey);
                          setPendingUserMsg(null);
                        }}
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
            ))}
            {threads.length === 0 && (
              <div className="text-center py-8 text-muted text-[12px] font-light">
                No conversations yet
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right Panel — Conversation */}
      <div className="flex-1 flex flex-col">
        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-3xl mx-auto">
            {displayMessages.length === 0 && !stream.isStreaming ? (
              <div className="flex flex-col items-center justify-center py-20">
                <p className="text-muted text-sm font-light">Start a conversation</p>
                <p className="text-dim text-xs font-light mt-1">
                  Ask Klynt anything about your tasks, projects, or schedule
                </p>
              </div>
            ) : (
              <MessageList
                messages={displayMessages}
                streamingContent={stream.streamingContent}
                isStreaming={stream.isStreaming}
                activeTools={stream.activeTools}
                error={stream.error}
              />
            )}
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
                disabled={!input.trim() || stream.isStreaming}
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
