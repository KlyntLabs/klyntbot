import Calendar from "lucide-react/dist/esm/icons/calendar";
import Search from "lucide-react/dist/esm/icons/search";
import Settings from "lucide-react/dist/esm/icons/settings";
import SquarePen from "lucide-react/dist/esm/icons/square-pen";
import { memo } from "react";
import type { ChatThread } from "@/features/chat/types";

type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
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
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
