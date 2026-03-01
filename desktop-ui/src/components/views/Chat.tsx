import { useState, useRef, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router';
import { Plus, Mic, Send, Search, Hash } from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { mockProjects, mockChatMessages } from '../../data/mockData';
import type { ChatMessage, SidebarItem } from '../../lib/types';

interface Thread {
  id: string;
  title: string;
  projectId: string;
  lastMessage: string;
  time: string;
  unread: boolean;
}

const mockThreads: Thread[] = [
  { id: '1', title: 'Landing page design', projectId: '1', lastMessage: 'Here are the mockups you requested...', time: '2m ago', unread: true },
  { id: '2', title: 'Payment integration', projectId: '1', lastMessage: 'Gateway setup is complete', time: '1h ago', unread: false },
  { id: '3', title: 'Mobile nav feedback', projectId: '2', lastMessage: 'The prototype looks great!', time: '3h ago', unread: true },
  { id: '4', title: 'API rate limits', projectId: '4', lastMessage: 'We should implement backoff...', time: '5h ago', unread: false },
  { id: '5', title: 'Wallet security audit', projectId: '6', lastMessage: 'Found two potential issues...', time: '1d ago', unread: false },
];

type ContextScope = 'Local' | 'Project' | 'Permissions';

export function Chat() {
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Chat');
  const [activeThread, setActiveThread] = useState<string>('1');
  const [threadSearch, setThreadSearch] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>(mockChatMessages);
  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [activeScope, setActiveScope] = useState<ContextScope>('Local');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    return () => clearTimeout(timerRef.current);
  }, []);

  const threadsByProject = useMemo(() => {
    const filtered = threadSearch
      ? mockThreads.filter(t => t.title.toLowerCase().includes(threadSearch.toLowerCase()))
      : mockThreads;

    const grouped = new Map<string, Thread[]>();
    for (const thread of filtered) {
      const existing = grouped.get(thread.projectId) ?? [];
      existing.push(thread);
      grouped.set(thread.projectId, existing);
    }
    return grouped;
  }, [threadSearch]);

  const handleSend = () => {
    if (!input.trim()) return;

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: input,
    };
    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setIsStreaming(true);

    timerRef.current = setTimeout(() => {
      const aiMessage: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: 'I can help you with that! Let me analyze your tasks and provide recommendations...',
      };
      setMessages(prev => [...prev, aiMessage]);
      setIsStreaming(false);
    }, 1000);
  };

  return (
    <div className="h-screen w-screen bg-[#0E0E0D] text-[#E6EDF3] flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Tasks') navigate('/');
        }}
      />

      {/* Thread Sidebar */}
      <div className="w-72 border-r border-[rgba(255,255,255,0.08)] flex flex-col">
        {/* Search */}
        <div className="h-14 flex items-center px-4 gap-2">
          <div className="flex-1 flex items-center bg-[rgba(255,255,255,0.04)] rounded-lg px-3 py-2 gap-2">
            <Search className="w-3.5 h-3.5 text-[#8B949E]" strokeWidth={1.5} />
            <input
              type="text"
              value={threadSearch}
              onChange={(e) => setThreadSearch(e.target.value)}
              placeholder="Search threads..."
              className="flex-1 bg-transparent text-[12px] text-[#E6EDF3] placeholder:text-[#8B949E] focus:outline-none font-light"
            />
          </div>
          <button className="w-8 h-8 rounded-md flex items-center justify-center bg-[#F97316] hover:bg-[#F97316]/90 transition-colors">
            <Plus className="w-4 h-4 text-white" strokeWidth={2} />
          </button>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-y-auto">
          {[...threadsByProject.entries()].map(([projectId, threads]) => {
            const project = mockProjects.find(p => p.id === projectId);
            if (!project) return null;
            return (
              <div key={projectId}>
                <div className="px-4 py-2 flex items-center gap-2">
                  <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: project.color }} />
                  <span className="text-[11px] font-light text-[#8B949E] uppercase tracking-wider">{project.name}</span>
                </div>
                {threads.map(thread => (
                  <button
                    key={thread.id}
                    onClick={() => setActiveThread(thread.id)}
                    className={`w-full text-left px-4 py-3 transition-colors ${
                      activeThread === thread.id
                        ? 'bg-[rgba(255,255,255,0.06)]'
                        : 'hover:bg-[rgba(255,255,255,0.03)]'
                    }`}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <span className={`text-[13px] font-light truncate ${thread.unread ? 'text-white' : 'text-[#C9D1D9]'}`}>
                        {thread.title}
                      </span>
                      <span className="text-[10px] text-[#8B949E] font-light ml-2 flex-shrink-0">{thread.time}</span>
                    </div>
                    <p className="text-[11px] text-[#8B949E] font-light truncate">{thread.lastMessage}</p>
                    {thread.unread && (
                      <div className="mt-1.5 w-1.5 h-1.5 rounded-full bg-[#F97316]" />
                    )}
                  </button>
                ))}
              </div>
            );
          })}
        </div>
      </div>

      {/* Message Area */}
      <div className="flex-1 flex flex-col">
        {/* Thread Header */}
        <div className="h-14 flex items-center justify-between px-6 border-b border-[rgba(255,255,255,0.08)]">
          <div className="flex items-center gap-2">
            <Hash className="w-4 h-4 text-[#8B949E]" strokeWidth={1.5} />
            <span className="text-[14px] font-light text-[#E6EDF3]">
              {mockThreads.find(t => t.id === activeThread)?.title ?? 'Chat'}
            </span>
          </div>
          <div className="flex items-center gap-1">
            {(['Local', 'Project', 'Permissions'] as ContextScope[]).map(scope => (
              <button
                key={scope}
                onClick={() => setActiveScope(scope)}
                className={`px-3 py-1.5 rounded-md text-[11px] font-light transition-colors ${
                  activeScope === scope
                    ? 'bg-[rgba(255,255,255,0.08)] text-white'
                    : 'text-[#8B949E] hover:text-[#C9D1D9]'
                }`}
              >
                {scope}
              </button>
            ))}
          </div>
        </div>

        {/* Messages */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          {messages.map(message => (
            <div
              key={message.id}
              className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
            >
              <div
                className={`max-w-[70%] rounded-xl px-4 py-3 ${
                  message.role === 'user'
                    ? 'bg-[#F97316] text-white'
                    : 'bg-[rgba(255,255,255,0.04)] text-[#C9D1D9]'
                }`}
              >
                <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed">{message.content}</p>
              </div>
            </div>
          ))}
          {isStreaming && (
            <div className="flex justify-start">
              <div className="bg-[rgba(255,255,255,0.04)] rounded-xl px-4 py-3">
                <div className="flex gap-1">
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
              </div>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* Input */}
        <div className="px-6 py-4 border-t border-[rgba(255,255,255,0.08)]">
          <div className="flex items-center gap-2 bg-[rgba(255,255,255,0.04)] rounded-xl px-4 py-2.5">
            <button className="text-[#8B949E] hover:text-[#C9D1D9] transition-colors">
              <Plus className="w-4 h-4" strokeWidth={2} />
            </button>
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSend()}
              placeholder="Ask me anything..."
              className="flex-1 bg-transparent text-[13px] text-[#E6EDF3] placeholder:text-[#8B949E] focus:outline-none font-light"
            />
            <button className="text-[#8B949E] hover:text-[#C9D1D9] transition-colors">
              <Mic className="w-4 h-4" strokeWidth={1.5} />
            </button>
            <button
              onClick={handleSend}
              disabled={!input.trim()}
              className="w-8 h-8 rounded-lg bg-[#F97316] hover:bg-[#F97316]/90 disabled:bg-[rgba(255,255,255,0.04)] disabled:text-[#8B949E] flex items-center justify-center transition-colors"
            >
              <Send className="w-3.5 h-3.5" strokeWidth={2} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
