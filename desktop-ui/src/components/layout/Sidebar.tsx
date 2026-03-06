import {
  Activity,
  Calendar,
  CheckSquare,
  FileText,
  MessageSquare,
  Settings,
  Target,
  Wallet,
} from "lucide-react";
import { useNavigate } from "react-router";
import type { SidebarItem } from "../../lib/types";
import { KlyntLogo } from "../ui/KlyntLogo";

interface SidebarProps {
  active: SidebarItem;
  onNavigate?: (item: SidebarItem) => void;
}

const items: { key: SidebarItem; icon: typeof MessageSquare; path?: string; bottom?: boolean }[] = [
  { key: "Chat", icon: MessageSquare, path: "/chat" },
  { key: "Tasks", icon: CheckSquare, path: "/" },
  { key: "OKR", icon: Target },
  { key: "Calendar", icon: Calendar },
  { key: "Notes", icon: FileText, path: "/notes" },
  { key: "Finance", icon: Wallet, path: "/finance" },
  { key: "Productivity", icon: Activity, path: "/productivity" },
  { key: "Settings", icon: Settings, path: "/settings", bottom: true },
];

export function Sidebar({ active, onNavigate }: SidebarProps) {
  const navigate = useNavigate();

  const handleClick = (item: (typeof items)[number]) => {
    if (item.path) {
      navigate(item.path);
    }
    onNavigate?.(item.key);
  };

  const topItems = items.filter((i) => !i.bottom);
  const bottomItems = items.filter((i) => i.bottom);

  return (
    <div className="w-[52px] glass-sidebar flex flex-col items-center gap-1 py-2">
      {/* Logo */}
      <div className="flex items-center mb-1">
        <button
          type="button"
          onClick={() => navigate("/")}
          aria-label="Home"
          className="w-8 h-8 rounded-xl bg-white/90 flex items-center justify-center p-0.5 hover:bg-white transition-all"
        >
          <KlyntLogo className="w-full h-full" />
        </button>
      </div>

      {/* Nav Items */}
      {topItems.map((item) => {
        const Icon = item.icon;
        const isActive = active === item.key;
        return (
          <button
            type="button"
            key={item.key}
            onClick={() => handleClick(item)}
            aria-label={item.key}
            className={`w-8 h-8 rounded-xl flex items-center justify-center transition-all duration-200 ${
              isActive
                ? "glass-button-active text-brand"
                : "text-muted hover:text-secondary hover:bg-white/[0.05]"
            }`}
          >
            <Icon className="w-[17px] h-[17px]" strokeWidth={1.5} aria-hidden="true" />
          </button>
        );
      })}

      <div className="flex-1" />

      {/* Bottom Items */}
      {bottomItems.map((item) => {
        const Icon = item.icon;
        const isActive = active === item.key;
        return (
          <button
            type="button"
            key={item.key}
            onClick={() => handleClick(item)}
            aria-label={item.key}
            className={`w-8 h-8 rounded-xl flex items-center justify-center transition-all duration-200 ${
              isActive
                ? "glass-button-active text-brand"
                : "text-muted hover:text-secondary hover:bg-white/[0.05]"
            }`}
          >
            <Icon className="w-[17px] h-[17px]" strokeWidth={1.5} aria-hidden="true" />
          </button>
        );
      })}
    </div>
  );
}
