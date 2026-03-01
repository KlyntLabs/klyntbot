import { useState, useMemo } from 'react';
import { useNavigate } from 'react-router';
import {
  Search, Plus, MessageSquare, Calendar, Target, TrendingUp, Settings,
  Sparkles, Command, ArrowLeft,
} from 'lucide-react';
import { KlyntLogo } from '../ui/KlyntLogo';
import { mockLauncherItems } from '../../data/mockData';

const iconMap: Record<string, typeof Search> = {
  Plus, Search, MessageSquare, Calendar, Target, TrendingUp, Settings, Sparkles,
};

export function Launcher() {
  const navigate = useNavigate();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filteredItems = useMemo(() =>
    mockLauncherItems.filter(item =>
      item.title.toLowerCase().includes(query.toLowerCase()) ||
      item.subtitle.toLowerCase().includes(query.toLowerCase()),
    ),
  [query]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(i => Math.min(i + 1, filteredItems.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(i => Math.max(i - 1, 0));
    }
  };

  return (
    <div className="min-h-screen bg-[#0E0E0D] flex items-start justify-center pt-32 p-4">
      {/* Back Button */}
      <button
        onClick={() => navigate('/')}
        className="fixed top-4 left-4 flex items-center gap-2 px-3 py-2 bg-[rgba(255,255,255,0.04)] hover:bg-[rgba(255,255,255,0.06)] rounded-md text-[#8B949E] hover:text-[#C9D1D9] transition-colors"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px] font-light">Back to Main</span>
      </button>

      {/* Launcher Window */}
      <div className="w-full max-w-[700px] bg-[#0E0E0D] backdrop-blur-2xl rounded-xl border border-[rgba(255,255,255,0.08)] shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="px-5 pt-5 pb-3">
          <div className="flex items-center gap-2 mb-1">
            <div className="w-7 h-7 rounded-lg bg-white flex items-center justify-center p-1">
              <KlyntLogo className="w-full h-full" />
            </div>
            <h1 className="text-[15px] font-normal text-[#E6EDF3]">Klynt Launcher</h1>
          </div>
          <p className="text-[11px] text-[#8B949E] font-light">AI assistant & quick actions</p>
        </div>

        {/* Search Bar */}
        <div className="px-5 pb-4">
          <div className="flex items-center gap-3 bg-[rgba(255,255,255,0.04)] rounded-xl px-4 py-3">
            <Sparkles className="w-[18px] h-[18px] text-[#F97316]" strokeWidth={1.5} />
            <input
              type="text"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSelectedIndex(0);
              }}
              onKeyDown={handleKeyDown}
              placeholder="Ask Klynt anything or type a command..."
              autoFocus
              className="flex-1 bg-transparent text-[#E6EDF3] text-[13px] placeholder:text-[#8B949E] outline-none font-light"
            />
            <div className="flex items-center gap-1.5">
              <span className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[11px] text-[#8B949E] font-light">AI</span>
              <span className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[11px] text-[#8B949E] font-light">Tab</span>
            </div>
          </div>
        </div>

        {/* Results */}
        <div className="max-h-[500px] overflow-y-auto px-5 pb-4">
          {query.trim() || filteredItems.length > 0 ? (
            <div className="space-y-1">
              {/* AI Query Option */}
              {query.trim() && (
                <button
                  className={`w-full flex items-center gap-4 px-4 py-3.5 rounded-xl transition-all ${
                    0 === selectedIndex ? 'bg-[rgba(255,255,255,0.08)]' : 'hover:bg-[rgba(255,255,255,0.04)]'
                  }`}
                  onMouseEnter={() => setSelectedIndex(0)}
                >
                  <div className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
                    0 === selectedIndex ? 'bg-[#F97316] text-white' : 'bg-[rgba(255,255,255,0.04)] text-[#F97316]'
                  }`}>
                    <Sparkles className="w-[18px] h-[18px]" strokeWidth={1.5} />
                  </div>
                  <div className="flex-1 text-left">
                    <h3 className="text-[#E6EDF3] text-[13px] font-light">Ask Klynt AI: {query}</h3>
                    <p className="text-[11px] text-[#8B949E] font-light">Get AI-powered response</p>
                  </div>
                </button>
              )}

              {/* Commands */}
              {filteredItems.map((item, index) => {
                const Icon = iconMap[item.icon] ?? Search;
                const actualIndex = query.trim() ? index + 1 : index;
                const isSelected = actualIndex === selectedIndex;
                return (
                  <button
                    key={item.id}
                    className={`w-full flex items-center gap-4 px-4 py-3.5 rounded-xl transition-all ${
                      isSelected ? 'bg-[rgba(255,255,255,0.08)]' : 'hover:bg-[rgba(255,255,255,0.04)]'
                    }`}
                    onMouseEnter={() => setSelectedIndex(actualIndex)}
                  >
                    <div className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
                      isSelected ? 'bg-[#F97316] text-white' : 'bg-[rgba(255,255,255,0.04)] text-[#8B949E]'
                    }`}>
                      <Icon className="w-[18px] h-[18px]" strokeWidth={1.5} />
                    </div>
                    <div className="flex-1 text-left">
                      <h3 className="text-[#E6EDF3] text-[13px] font-light">{item.title}</h3>
                      <p className="text-[11px] text-[#8B949E] font-light">{item.subtitle}</p>
                    </div>
                    <div className="flex items-center gap-1 text-[11px] text-[#8B949E] font-light">
                      {item.shortcut}
                    </div>
                  </button>
                );
              })}
            </div>
          ) : (
            <div className="px-5 py-10 text-center text-[#8B949E] text-[13px] font-light">
              No results found
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-5 py-3 bg-[rgba(0,0,0,0.2)]">
          <div className="flex items-center justify-between text-[11px] text-[#8B949E]">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1.5 font-light">
                <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded">↵</kbd>
                Open
              </span>
              <span className="flex items-center gap-1.5 font-light">
                <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded">&#8984;</kbd>
                <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded">↵</kbd>
                Open in Background
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="flex items-center gap-1.5 font-light">
                <Command className="w-3 h-3" strokeWidth={1.5} />
                Commands
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
