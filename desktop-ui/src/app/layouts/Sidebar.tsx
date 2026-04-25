import { useAmbientSignals } from "@shared/hooks/useAmbientSignals";
import { ipc } from "@shared/hooks/useIpc";
import type { SidebarItem } from "@shared/types";
import { KlyntLogo } from "@shared/ui/KlyntLogo";
import {
  Brain,
  CheckSquare,
  Code2,
  Cpu,
  FileText,
  GraduationCap,
  LayoutDashboard,
  MessageCircle,
  MessageSquare,
  Settings,
  Sparkles,
  Timer,
  Wallet,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";

interface SidebarProps {
  active: SidebarItem;
  onNavigate?: (item: SidebarItem) => void;
  isChatOpen?: boolean;
  onToggleChat?: () => void;
}

const items: { key: SidebarItem; icon: typeof MessageSquare; path?: string; bottom?: boolean }[] = [
  { key: "Chat", icon: MessageSquare, path: "/chat" },
  { key: "Dashboard", icon: LayoutDashboard, path: "/" },
  { key: "Tasks", icon: CheckSquare, path: "/tasks" },
  { key: "Notes", icon: FileText, path: "/notes" },
  { key: "Learn", icon: GraduationCap, path: "/learn" },
  { key: "Finance", icon: Wallet, path: "/finance" },
  { key: "Coaching", icon: Sparkles, path: "/coaching" },
  { key: "Brain", icon: Brain, path: "/brain" },
  { key: "Automations", icon: Timer, path: "/automations" },
  { key: "CodingMemory", icon: Code2, path: "/coding-memory" },
  { key: "System", icon: Cpu, path: "/system", bottom: true },
  { key: "Settings", icon: Settings, path: "/settings", bottom: true },
];

export function Sidebar({ active, onNavigate, isChatOpen, onToggleChat }: SidebarProps) {
  const navigate = useNavigate();
  const [dueCount, setDueCount] = useState(0);
  const dueCountRef = useRef(0);
  const { badgeCount, isPulsing } = useAmbientSignals();

  useEffect(() => {
    const fetchDue = () => {
      ipc<number>("flashcard_total_due", {})
        .then((count) => {
          if (count !== dueCountRef.current) {
            dueCountRef.current = count;
            setDueCount(count);
          }
        })
        .catch(() => {});
    };
    fetchDue();
    const interval = setInterval(fetchDue, 30000);
    return () => clearInterval(interval);
  }, []);

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
          className="w-8 h-8 rounded-xl bg-muted flex items-center justify-center p-0.5 hover:bg-muted transition-all"
        >
          <KlyntLogo className="w-full h-full" />
        </button>
      </div>

      {/* Nav Items */}
      {topItems.map((item) => {
        const Icon = item.icon;
        const isActive = active === item.key;
        const showBadge = item.key === "Learn" && dueCount > 0;
        const showBrainPulse = item.key === "Brain" && (badgeCount > 0 || isPulsing);
        return (
          <button
            type="button"
            key={item.key}
            onClick={() => handleClick(item)}
            aria-label={item.key}
            className={`relative w-8 h-8 rounded-xl flex items-center justify-center transition-all duration-200 ${
              isActive
                ? "glass-button-active text-brand"
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`}
          >
            <Icon className="w-[17px] h-[17px]" strokeWidth={1.5} aria-hidden="true" />
            {showBadge && (
              <span className="absolute -top-0.5 -right-0.5 min-w-[14px] h-[14px] rounded-full bg-brand text-[8px] text-white flex items-center justify-center font-medium px-0.5">
                {dueCount > 99 ? "99" : dueCount}
              </span>
            )}
            {showBrainPulse && (
              <div className="absolute top-1 right-1 size-1 rounded-full bg-amber-400" />
            )}
          </button>
        );
      })}

      <div className="flex-1" />

      {/* Quick chat panel toggle — slide-out contextual chat */}
      {onToggleChat && (
        <button
          type="button"
          onClick={onToggleChat}
          aria-label="Quick chat panel"
          title="Quick chat"
          className={`w-8 h-8 rounded-xl flex items-center justify-center transition-all duration-200 ${
            isChatOpen
              ? "glass-button-active text-brand"
              : "text-muted-foreground hover:text-foreground hover:bg-accent"
          }`}
        >
          <MessageCircle className="w-[17px] h-[17px]" strokeWidth={1.5} aria-hidden="true" />
        </button>
      )}

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
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`}
          >
            <Icon className="w-[17px] h-[17px]" strokeWidth={1.5} aria-hidden="true" />
          </button>
        );
      })}
    </div>
  );
}
