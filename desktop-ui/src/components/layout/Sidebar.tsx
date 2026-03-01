import { useNavigate } from 'react-router';
import { MessageSquare, CheckSquare, Target, Calendar, Settings } from 'lucide-react';
import { KlyntLogo } from '../ui/KlyntLogo';
import type { SidebarItem } from '../../lib/types';

interface SidebarProps {
  active: SidebarItem;
  onNavigate?: (item: SidebarItem) => void;
}

const items: { key: SidebarItem; icon: typeof MessageSquare; path?: string }[] = [
  { key: 'Chat', icon: MessageSquare, path: '/chat' },
  { key: 'Tasks', icon: CheckSquare, path: '/' },
  { key: 'OKR', icon: Target },
  { key: 'Calendar', icon: Calendar },
];

export function Sidebar({ active, onNavigate }: SidebarProps) {
  const navigate = useNavigate();

  const handleClick = (item: typeof items[number]) => {
    if (item.path) {
      navigate(item.path);
    }
    onNavigate?.(item.key);
  };

  return (
    <div className="w-14 bg-[#0E0E0D] backdrop-blur-xl border-r border-[rgba(255,255,255,0.08)] flex flex-col items-center gap-1 pb-3">
      {/* Logo */}
      <div className="h-14 flex items-center px-2">
        <button
          onClick={() => navigate('/')}
          className="w-9 h-9 rounded-lg bg-white flex items-center justify-center p-0.5 hover:opacity-80 transition-opacity"
        >
          <KlyntLogo className="w-full h-full" />
        </button>
      </div>

      {/* Nav Items */}
      {items.map(item => {
        const Icon = item.icon;
        const isActive = active === item.key;
        return (
          <button
            key={item.key}
            onClick={() => handleClick(item)}
            className={`w-9 h-9 rounded-md flex items-center justify-center transition-colors ${
              isActive
                ? 'bg-[rgba(255,255,255,0.08)] text-[#F97316]'
                : 'text-[#8B949E] hover:bg-[rgba(255,255,255,0.04)] hover:text-[#C9D1D9]'
            }`}
          >
            <Icon className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        );
      })}

      <div className="flex-1" />

      {/* Settings */}
      <button
        onClick={() => onNavigate?.('Settings')}
        className={`w-9 h-9 rounded-md flex items-center justify-center transition-colors ${
          active === 'Settings'
            ? 'bg-[rgba(255,255,255,0.08)] text-[#F97316]'
            : 'text-[#8B949E] hover:bg-[rgba(255,255,255,0.04)] hover:text-[#C9D1D9]'
        }`}
      >
        <Settings className="w-[18px] h-[18px]" strokeWidth={1.5} />
      </button>
    </div>
  );
}
