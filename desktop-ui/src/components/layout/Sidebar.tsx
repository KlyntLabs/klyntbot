import {
  Activity,
  Calendar,
  CheckSquare,
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
    <div className="w-14 bg-background backdrop-blur-xl border-r border-border flex flex-col items-center gap-1 pb-3">
      {/* Logo */}
      <div className="h-14 flex items-center px-2">
        <button
          type="button"
          onClick={() => navigate("/")}
          aria-label="Home"
          className="w-9 h-9 rounded-lg bg-white flex items-center justify-center p-0.5 hover:opacity-80 transition-opacity"
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
            className={`w-9 h-9 rounded-md flex items-center justify-center transition-colors ${
              isActive
                ? "bg-surface-highest text-brand"
                : "text-muted hover:bg-surface-base hover:text-secondary"
            }`}
          >
            <Icon className="w-[18px] h-[18px]" strokeWidth={1.5} />
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
            className={`w-9 h-9 rounded-md flex items-center justify-center transition-colors ${
              isActive
                ? "bg-surface-highest text-brand"
                : "text-muted hover:bg-surface-base hover:text-secondary"
            }`}
          >
            <Icon className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        );
      })}
    </div>
  );
}
