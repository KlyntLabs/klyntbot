import { useState, useMemo, useEffect } from 'react';
import {
  Search, Plus, MessageSquare, Calendar, Target, TrendingUp, Settings,
  Sparkles, Command,
} from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { KlyntLogo } from '../ui/KlyntLogo';
import { isTauri } from '../../lib/utils';
import { useTransparentBackground } from '../../hooks/useTransparentBackground';
import { mockLauncherItems } from '../../data/mockData';

const iconMap: Record<string, typeof Search> = {
  Plus, Search, MessageSquare, Calendar, Target, TrendingUp, Settings, Sparkles,
};

export function Launcher() {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  useTransparentBackground();

  const filteredItems = useMemo(() => {
    const q = query.toLowerCase();
    return mockLauncherItems.filter(item =>
      item.title.toLowerCase().includes(q) ||
      item.subtitle.toLowerCase().includes(q),
    );
  }, [query]);

  // Escape key hides the launcher window
  useEffect(() => {
    if (!isTauri) return;
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        getCurrentWindow().hide();
      }
    };
    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, []);

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
    <div className="w-screen text-primary flex justify-center pt-4 px-4">
      <div className="w-full max-w-[660px] rounded-2xl overflow-hidden bg-surface-floating shadow-2xl shadow-black/50 border border-border-subtle">
        {/* Header */}
        <div className="px-5 pt-5 pb-3">
          <div className="flex items-center gap-2 mb-1">
            <div className="w-7 h-7 rounded-lg bg-white flex items-center justify-center p-1">
              <KlyntLogo className="w-full h-full" />
            </div>
            <h1 className="text-[15px] font-normal text-primary">Klynt Launcher</h1>
          </div>
          <p className="text-[11px] text-muted font-light">AI assistant & quick actions</p>
        </div>

        {/* Search Bar */}
        <div className="px-5 pb-4">
          <div className="flex items-center gap-3 bg-surface-base rounded-xl px-4 py-3">
            <Sparkles className="w-[18px] h-[18px] text-brand" strokeWidth={1.5} />
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
              className="flex-1 bg-transparent text-primary text-[13px] placeholder:text-muted outline-none font-light"
            />
            <div className="flex items-center gap-1.5">
              <span className="px-1.5 py-0.5 bg-surface-highest rounded text-[11px] text-muted font-light">AI</span>
              <span className="px-1.5 py-0.5 bg-surface-highest rounded text-[11px] text-muted font-light">Tab</span>
            </div>
          </div>
        </div>

        {/* Results */}
        <div className="max-h-[320px] overflow-y-auto px-5 pb-4">
          {query.trim() || filteredItems.length > 0 ? (
            <div className="space-y-1">
              {/* AI Query Option */}
              {query.trim() && (
                <button
                  className={`w-full flex items-center gap-4 px-4 py-3.5 rounded-xl transition-all ${
                    0 === selectedIndex ? 'bg-surface-highest' : 'hover:bg-surface-base'
                  }`}
                  onMouseEnter={() => setSelectedIndex(0)}
                >
                  <div className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
                    0 === selectedIndex ? 'bg-brand text-white' : 'bg-surface-base text-brand'
                  }`}>
                    <Sparkles className="w-[18px] h-[18px]" strokeWidth={1.5} />
                  </div>
                  <div className="flex-1 text-left">
                    <h3 className="text-primary text-[13px] font-light">Ask Klynt AI: {query}</h3>
                    <p className="text-[11px] text-muted font-light">Get AI-powered response</p>
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
                      isSelected ? 'bg-surface-highest' : 'hover:bg-surface-base'
                    }`}
                    onMouseEnter={() => setSelectedIndex(actualIndex)}
                  >
                    <div className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
                      isSelected ? 'bg-brand text-white' : 'bg-surface-base text-muted'
                    }`}>
                      <Icon className="w-[18px] h-[18px]" strokeWidth={1.5} />
                    </div>
                    <div className="flex-1 text-left">
                      <h3 className="text-primary text-[13px] font-light">{item.title}</h3>
                      <p className="text-[11px] text-muted font-light">{item.subtitle}</p>
                    </div>
                    <div className="flex items-center gap-1 text-[11px] text-muted font-light">
                      {item.shortcut}
                    </div>
                  </button>
                );
              })}
            </div>
          ) : (
            <div className="px-5 py-10 text-center text-muted text-[13px] font-light">
              No results found
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-5 py-3 border-t border-border-subtle">
          <div className="flex items-center justify-between text-[11px] text-muted">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1.5 font-light">
                <kbd className="px-1.5 py-0.5 bg-surface-highest rounded">↵</kbd>
                Open
              </span>
              <span className="flex items-center gap-1.5 font-light">
                <kbd className="px-1.5 py-0.5 bg-surface-highest rounded">Esc</kbd>
                Close
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
