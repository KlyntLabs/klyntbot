import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router';
import {
  Send, ChevronDown, Plus, Settings, FolderOpen, MessageSquare,
  RotateCcw, Mic, Shield, Server,
} from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { useSetToggle } from '../../hooks/useSetToggle';
import type { ChatMessage, SidebarItem } from '../../lib/types';

interface Thread {
  id: string;
  name: string;
  timestamp: string;
  projectId: string;
}

interface ChatProject {
  id: string;
  name: string;
  threads: Thread[];
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

export function Chat() {
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Chat');
  const [messages, setMessages] = useState<ChatMessage[]>(mockMessages);
  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [selectedThread, setSelectedThread] = useState<string>('t1');
  const [expandedProjects, toggleProject] = useSetToggle(['1', '2', '3']);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    return () => clearTimeout(timerRef.current);
  }, []);

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

      {/* Left Sidebar — Thread List */}
      <div className="w-[250px] bg-[#0E0E0D] border-r border-[rgba(255,255,255,0.08)] flex flex-col">
        {/* Quick Links */}
        <div className="px-4 py-3 space-y-1">
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-[rgba(255,255,255,0.04)] transition-colors text-[12px] font-light text-[#8B949E] hover:text-[#C9D1D9]">
            <Plus className="w-[13px] h-[13px]" strokeWidth={1.5} />
            New thread
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-[rgba(255,255,255,0.04)] transition-colors text-[12px] font-light text-[#8B949E] hover:text-[#C9D1D9]">
            <RotateCcw className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Automations
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-[rgba(255,255,255,0.04)] transition-colors text-[12px] font-light text-[#8B949E] hover:text-[#C9D1D9]">
            <Settings className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Skills and Apps
          </button>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-y-auto px-3 pb-3">
          <div className="space-y-4">
            {mockChatProjects.map(project => (
              <div key={project.id}>
                <button
                  onClick={() => toggleProject(project.id)}
                  className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-[rgba(255,255,255,0.04)] transition-colors text-[12px] font-light text-[#8B949E] hover:text-[#C9D1D9]"
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
                            ? 'bg-[rgba(255,255,255,0.08)] text-[#E6EDF3]'
                            : 'text-[#8B949E] hover:bg-[rgba(255,255,255,0.04)] hover:text-[#C9D1D9]'
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
            ))}
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
                  <div className="max-w-[85%] rounded-2xl px-5 py-3.5 bg-[rgba(255,255,255,0.06)] backdrop-blur-sm">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-[#E6EDF3]">
                      {message.content}
                    </p>
                  </div>
                ) : (
                  <div className="max-w-[85%]">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-[#C9D1D9]">
                      {message.content}
                    </p>
                  </div>
                )}
              </div>
            ))}
            {isStreaming && (
              <div className="flex justify-start">
                <div className="flex gap-1">
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <div className="w-1.5 h-1.5 bg-[#8B949E] rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>
        </div>

        {/* Input Area */}
        <div className="p-6">
          <div className="max-w-3xl mx-auto">
            <div className="bg-[rgba(255,255,255,0.04)] rounded-2xl flex items-center px-2 gap-2">
              <button className="w-8 h-8 flex items-center justify-center text-[#8B949E] hover:text-[#C9D1D9] transition-colors shrink-0">
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
                className="flex-1 bg-transparent py-3.5 text-[13px] text-[#E6EDF3] placeholder:text-[#8B949E] focus:outline-none font-light resize-none"
                style={{ maxHeight: '200px' }}
              />
              <button className="w-8 h-8 flex items-center justify-center text-[#8B949E] hover:text-[#C9D1D9] transition-colors shrink-0">
                <Mic className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <button
                onClick={handleSend}
                disabled={!input.trim()}
                className="w-9 h-9 rounded-full bg-[#F97316] hover:bg-[#F97316]/90 disabled:bg-[rgba(255,255,255,0.04)] disabled:text-[#8B949E] flex items-center justify-center transition-colors shrink-0"
              >
                <Send className="w-4 h-4" strokeWidth={2} />
              </button>
            </div>
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgba(255,255,255,0.04)] hover:bg-[rgba(255,255,255,0.06)] transition-colors text-[11px] font-light text-[#8B949E]">
                  <Server className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Local</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgba(255,255,255,0.04)] hover:bg-[rgba(255,255,255,0.06)] transition-colors text-[11px] font-light text-[#8B949E]">
                  <Shield className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Default permissions</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
              </div>
              <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[rgba(255,255,255,0.04)] hover:bg-[rgba(255,255,255,0.06)] transition-colors text-[11px] font-light text-[#8B949E]">
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
