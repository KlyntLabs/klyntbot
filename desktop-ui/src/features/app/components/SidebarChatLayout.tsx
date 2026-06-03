import Calendar from "lucide-react/dist/esm/icons/calendar";
import Clock from "lucide-react/dist/esm/icons/clock";
import FolderPlus from "lucide-react/dist/esm/icons/folder-plus";
import LayoutGrid from "lucide-react/dist/esm/icons/layout-grid";
import Search from "lucide-react/dist/esm/icons/search";
import Settings from "lucide-react/dist/esm/icons/settings";
import SquarePen from "lucide-react/dist/esm/icons/square-pen";
import { memo } from "react";
import { cn } from "@/utils/cn";
import type { ChatThread } from "@/features/chat/types";
type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectPlugins: () => void;
  onSelectCalendar?: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
  activeNavId?: string | null;
};

type NavItem = {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
};

export const SidebarChatLayout = memo(function SidebarChatLayout({
  onOpenSettings,
  onNewChat,
  onSelectPlugins,
  onSelectCalendar,
  threads,
  selectedSessionKey,
  onSelectThread,
  activeNavId,
}: SidebarChatLayoutProps) {
  const handleSelectCalendar = onSelectCalendar ?? (() => {});
  const allNavItems: NavItem[] = [
    {
      id: "new-chat",
      label: "New chat",
      icon: <SquarePen aria-hidden />,
      onClick: onNewChat,
    },
    { id: "search", label: "Search", icon: <Search aria-hidden /> },
    {
      id: "calendar",
      label: "Calendar",
      icon: <Calendar aria-hidden />,
      onClick: handleSelectCalendar,
    },
    {
      id: "plugins",
      label: "Plugins",
      icon: <LayoutGrid aria-hidden />,
      onClick: onSelectPlugins,
    },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    {
      id: "project",
      label: "Project",
      icon: <FolderPlus aria-hidden />,
    },
  ];

  return (
    <aside className="flex flex-col h-full min-h-0 py-2 px-3 bg-transparent text-text-strong relative">
      <div
        className="absolute top-0 left-0 right-0 z-[1] pointer-events-auto"
        style={{
          height: "var(--side-panel-drag-strip-height, 28px)",
          WebkitAppRegion: "drag",
        } as React.CSSProperties}
      />
      <div className="h-8 shrink-0 flex items-center justify-end gap-2 pr-1 relative z-[2]">
        <div className="w-[72px] h-[22px] shrink-0" aria-hidden />
        <div className="flex-1" />
      </div>

      <nav className="flex flex-col gap-0.5 py-0.5 pb-3" aria-label="Primary">
        {allNavItems.map((item) => {
          const isActive = activeNavId === item.id;
          return (
            <button
              key={item.id}
              type="button"
              className={cn(
                "flex items-center gap-2.5 w-full px-2 py-1.5 bg-transparent border-0 rounded-md text-text-strong cursor-pointer text-ui-sm font-medium text-left transition-colors duration-100",
                "hover:bg-surface-hover",
                isActive && "bg-surface-active text-text-stronger",
              )}
              onClick={item.onClick}
            >
              <span className="inline-flex items-center justify-center w-3.5 h-3.5 text-text-muted shrink-0">
                {item.icon}
              </span>
              <span className="flex-1 min-w-0 whitespace-nowrap overflow-hidden text-ellipsis">
                {item.label}
              </span>
            </button>
          );
        })}
      </nav>

      <div className="flex flex-col gap-2 pt-2.5 px-2.5">
        <div className="px-3 py-1.5 text-ui-2xs uppercase tracking-[0.08em] text-text-faint">
          Chats
        </div>
        {threads.length === 0 ? (
          <div className="text-ui-xs text-text-faint opacity-70">No chats</div>
        ) : (
          <ul className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-0.5 list-none m-0 p-0">
            {threads.map((t) => (
              <li key={t.sessionKey}>
                <button
                  type="button"
                  className={cn(
                    "block w-full text-left bg-transparent border-0 text-inherit text-ui-xs px-2.5 py-1.5 rounded-md cursor-pointer whitespace-nowrap overflow-hidden text-ellipsis transition-colors duration-100",
                    "hover:bg-surface-hover",
                    t.sessionKey === selectedSessionKey &&
                      "bg-surface-hover font-medium",
                  )}
                  onClick={() => onSelectThread(t.sessionKey)}
                  title={t.title}
                >
                  {t.title || "Untitled"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex-1 min-h-0" />

      <div className="flex items-center justify-between gap-2 px-1 pt-2 pb-1">
        <button
          type="button"
          className="inline-flex items-center gap-2 bg-transparent border-0 text-text-strong text-ui-sm font-medium px-2 py-1 rounded-md cursor-pointer transition-colors duration-100 hover:bg-surface-hover"
          onClick={onOpenSettings}
        >
          <span className="inline-flex items-center justify-center w-3.5 h-3.5 text-text-muted">
            <Settings aria-hidden />
          </span>
          <span>Settings</span>
        </button>
        <button
          type="button"
          className="bg-transparent border border-border-subtle text-text-strong text-ui-xs font-medium px-3 py-1 rounded-full cursor-pointer transition-colors duration-100 hover:bg-surface-hover hover:border-border-strong"
        >
          Upgrade
        </button>
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
