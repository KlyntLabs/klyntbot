import { useState, useRef, useEffect } from 'react';
import { Send } from 'lucide-react';
import { mockChatMessages } from '../../data/mockData';
import type { ChatMessage } from '../../lib/types';

interface ChatPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ChatPanel({ isOpen, onClose: _onClose }: ChatPanelProps) {
  const [messages, setMessages] = useState<ChatMessage[]>(mockChatMessages);
  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

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

    // Simulate streaming response
    setTimeout(() => {
      const aiMessage: ChatMessage = {
        id: (Date.now() + 1).toString(),
        role: 'assistant',
        content: 'I can help you with that! Let me analyze your tasks and provide recommendations...',
      };
      setMessages(prev => [...prev, aiMessage]);
      setIsStreaming(false);
    }, 1000);
  };

  if (!isOpen) return null;

  return (
    <div className="w-96 bg-[#0E0E0D] border-l border-[rgba(255,255,255,0.08)] flex flex-col">
      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-5 space-y-4">
        {messages.map(message => (
          <div
            key={message.id}
            className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
          >
            <div
              className={`max-w-[85%] rounded-xl px-4 py-3 ${
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
      <div className="p-5 border-t border-[rgba(255,255,255,0.08)]">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder="Ask me anything..."
            className="flex-1 bg-[rgba(255,255,255,0.04)] rounded-xl px-4 py-2.5 text-[13px] text-[#E6EDF3] placeholder:text-[#8B949E] focus:outline-none focus:bg-[rgba(255,255,255,0.06)] font-light"
          />
          <button
            onClick={handleSend}
            disabled={!input.trim()}
            className="w-10 h-10 rounded-xl bg-[#F97316] hover:bg-[#F97316]/90 disabled:bg-[rgba(255,255,255,0.04)] disabled:text-[#8B949E] flex items-center justify-center transition-colors"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>
  );
}
