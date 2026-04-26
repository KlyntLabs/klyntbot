import { memo } from "react";
import {
  Clock,
  FolderPlus,
  LayoutGrid,
  Search,
  Settings,
  SquarePen,
} from "lucide-react";
import type { ChatThread } from "@/features/chat/types";

type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
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
  threads,
  selectedSessionKey,
  onSelectThread,
}: SidebarChatLayoutProps) {
  const navItems: NavItem[] = [
    { id: "new-chat", label: "New chat", icon: <SquarePen aria-hidden />, onClick: onNewChat },
    { id: "search", label: "Search", icon: <Search aria-hidden /> },
    { id: "plugins", label: "Plugins", icon: <LayoutGrid aria-hidden /> },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    { id: "project", label: "Project", icon: <FolderPlus aria-hidden /> },
  ];

  return (
    <aside className="sidebar-chat">
      <div className="sidebar-chat__drag-strip" />
      <div className="sidebar-chat__topbar" aria-hidden />

      <nav className="sidebar-chat__nav" aria-label="Primary">
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className="sidebar-chat__nav-item"
            onClick={item.onClick}
          >
            <span className="sidebar-chat__nav-icon">{item.icon}</span>
            <span className="sidebar-chat__nav-label">{item.label}</span>
          </button>
        ))}
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
        <button
          type="button"
          className="sidebar-chat__settings"
          onClick={onOpenSettings}
        >
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
