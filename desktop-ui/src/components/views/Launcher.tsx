import { useState, useMemo } from 'react';
import {
  Search, Plus, MessageSquare, Calendar, Target, TrendingUp, Settings, Sparkles,
} from 'lucide-react';
import { KlyntLogo } from '../ui/KlyntLogo';
import { mockLauncherItems } from '../../data/mockData';

const iconMap: Record<string, typeof Search> = {
  Plus, Search, MessageSquare, Calendar, Target, TrendingUp, Settings,
};

export function Launcher() {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filteredItems = useMemo(() => {
    if (!query) return mockLauncherItems;
    const q = query.toLowerCase();
    return mockLauncherItems.filter(
      item => item.title.toLowerCase().includes(q) || item.subtitle.toLowerCase().includes(q),
    );
  }, [query]);

  const isAiQuery = query.startsWith('/') || query.startsWith('?');

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
    <div className="h-screen w-screen bg-[rgba(0,0,0,0.85)] flex items-start justify-center pt-[20vh]">
      <div className="w-[560px] bg-[#1A1A19] rounded-2xl border border-[rgba(255,255,255,0.08)] shadow-2xl overflow-hidden">
        {/* Search Input */}
        <div className="flex items-center gap-3 px-5 py-4 border-b border-[rgba(255,255,255,0.08)]">
          <div className="w-6 h-6 rounded-md bg-white flex items-center justify-center p-0.5">
            <KlyntLogo className="w-full h-full" />
          </div>
          <input
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder="Search commands, tasks, or ask AI..."
            className="flex-1 bg-transparent text-[14px] text-[#E6EDF3] placeholder:text-[#8B949E] focus:outline-none font-light"
            autoFocus
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="text-[#8B949E] hover:text-[#C9D1D9] text-[12px] font-light transition-colors"
            >
              Clear
            </button>
          )}
        </div>

        {/* AI Query Banner */}
        {isAiQuery && (
          <div className="flex items-center gap-2 px-5 py-3 bg-[rgba(249,115,22,0.06)] border-b border-[rgba(249,115,22,0.1)]">
            <Sparkles className="w-3.5 h-3.5 text-[#F97316]" strokeWidth={1.5} />
            <span className="text-[12px] text-[#F97316] font-light">AI will process your query</span>
            <span className="text-[11px] text-[#8B949E] font-light ml-auto">Enter to send</span>
          </div>
        )}

        {/* Results */}
        <div className="max-h-[360px] overflow-y-auto py-2">
          {filteredItems.length === 0 ? (
            <div className="px-5 py-8 text-center">
              <p className="text-[13px] text-[#8B949E] font-light">No results found</p>
              <p className="text-[11px] text-[#6E7681] font-light mt-1">Try a different search or ask AI with /</p>
            </div>
          ) : (
            filteredItems.map((item, idx) => {
              const Icon = iconMap[item.icon] ?? Search;
              const isSelected = idx === selectedIndex;
              return (
                <button
                  key={item.id}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  className={`w-full flex items-center gap-3 px-5 py-2.5 text-left transition-colors ${
                    isSelected ? 'bg-[rgba(255,255,255,0.06)]' : 'hover:bg-[rgba(255,255,255,0.03)]'
                  }`}
                >
                  <div className="w-8 h-8 rounded-lg bg-[rgba(255,255,255,0.04)] flex items-center justify-center">
                    <Icon className="w-4 h-4 text-[#8B949E]" strokeWidth={1.5} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-[13px] font-light text-[#C9D1D9]">{item.title}</p>
                    <p className="text-[11px] font-light text-[#8B949E] truncate">{item.subtitle}</p>
                  </div>
                  <span className="text-[11px] text-[#6E7681] font-light font-mono">{item.shortcut}</span>
                </button>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-3 border-t border-[rgba(255,255,255,0.08)]">
          <div className="flex items-center gap-4">
            <span className="text-[10px] text-[#6E7681] font-light">
              <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[10px]">↑↓</kbd> navigate
            </span>
            <span className="text-[10px] text-[#6E7681] font-light">
              <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[10px]">↵</kbd> select
            </span>
            <span className="text-[10px] text-[#6E7681] font-light">
              <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[10px]">esc</kbd> close
            </span>
          </div>
          <span className="text-[10px] text-[#6E7681] font-light">
            <kbd className="px-1.5 py-0.5 bg-[rgba(255,255,255,0.06)] rounded text-[10px]">/</kbd> AI mode
          </span>
        </div>
      </div>
    </div>
  );
}
