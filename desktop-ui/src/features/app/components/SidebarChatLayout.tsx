import Calendar from "lucide-react/dist/esm/icons/calendar";
import Clock from "lucide-react/dist/esm/icons/clock";
import FolderPlus from "lucide-react/dist/esm/icons/folder-plus";
import LayoutGrid from "lucide-react/dist/esm/icons/layout-grid";
import Search from "lucide-react/dist/esm/icons/search";
import Settings from "lucide-react/dist/esm/icons/settings";
import SquarePen from "lucide-react/dist/esm/icons/square-pen";
import Timer from "lucide-react/dist/esm/icons/timer";
import { memo } from "react";
import type { ChatThread } from "@/features/chat/types";

type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectPlugins: () => void;
  onSelectCalendar?: () => void;
  onSelectFocus?: () => void;
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
  onSelectFocus,
  threads,
  selectedSessionKey,
  onSelectThread,
  activeNavId,
}: SidebarChatLayoutProps) {
  const handleSelectCalendar = onSelectCalendar ?? (() => {});
  const handleSelectFocus = onSelectFocus ?? (() => {});
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
    {
      id: "focus",
      label: "Focus",
      icon: <Timer aria-hidden />,
      onClick: handleSelectFocus,
    },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    {
      id: "project",
      label: "Project",
      icon: <FolderPlus aria-hidden />,
    },
  ];

  return (
    <aside className="sidebar-chat">
      <div className="sidebar-chat__drag-strip" />
      <div className="sidebar-chat__topbar">
        <div className="sidebar-chat__topbar-traffic-reserve" aria-hidden />
        <div className="sidebar-chat__topbar-spacer" />
      </div>

      <nav className="sidebar-chat__nav" aria-label="Primary">
        {allNavItems.map((item) => {
          const isActive = activeNavId === item.id;
          const cls = `sidebar-chat__nav-item${isActive ? " sidebar-chat__nav-item--active" : ""}`;
          return (
            <button key={item.id} type="button" className={cls} onClick={item.onClick}>
              <span className="sidebar-chat__nav-icon">{item.icon}</span>
              <span className="sidebar-chat__nav-label">{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-chat__chats">
        <div className="sidebar-chat__section-title">Chats</div>
        {threads.length === 0 ? (
          <div className="sidebar-chat__chats-empty">No chats</div>
        ) : (
          <ul className="sidebar-chat__thread-list">
            {threads.map((t) => (
              <li key={t.sessionKey}>
                <button
                  type="button"
                  className={
                    "sidebar-chat__thread-item" +
                    (t.sessionKey === selectedSessionKey
                      ? " sidebar-chat__thread-item--active"
                      : "")
                  }
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

      <div className="sidebar-chat__spacer" />

      <div className="sidebar-chat__footer">
        <button type="button" className="sidebar-chat__settings" onClick={onOpenSettings}>
          <Settings aria-hidden />
          <span>Settings</span>
        </button>
        <button type="button" className="sidebar-chat__upgrade">
          Upgrade
        </button>
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
