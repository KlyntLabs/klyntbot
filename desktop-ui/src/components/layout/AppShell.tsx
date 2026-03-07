import { useCallback, useMemo, useState } from "react";
import { Outlet, useLocation } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import type { SidebarItem } from "../../lib/types";
import { SidebarChat } from "../chat/SidebarChat";
import { Sidebar } from "./Sidebar";

export function AppShell() {
  const location = useLocation();
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [openSessionKey, setOpenSessionKey] = useState<string | null>(null);

  // Derive active sidebar item from current route
  const activeSidebarItem = useMemo((): SidebarItem => {
    const path = location.pathname;
    if (path.startsWith("/chat")) return "Chat";
    if (path.startsWith("/notes")) return "Notes";
    if (path.startsWith("/finance")) return "Finance";
    if (path.startsWith("/productivity")) return "Productivity";
    if (path.startsWith("/settings")) return "Settings";
    return "Tasks";
  }, [location.pathname]);

  // Derive chat context from current route (detail pages handled by usePageContext inside SidebarChat)
  const viewContext = useMemo(() => {
    const path = location.pathname;
    const search = new URLSearchParams(location.search);

    if (path === "/") {
      const tab = search.get("tab");
      if (tab) return { entityKind: `tasks.${tab.toLowerCase()}` };
      return { entityKind: "tasks" };
    }
    if (path.startsWith("/chat")) return { entityKind: "chat" };
    if (path.startsWith("/notes")) return { entityKind: "notes" };
    if (path.startsWith("/productivity")) return { entityKind: "productivity" };
    return undefined;
  }, [location.pathname, location.search]);

  const toggleChat = useCallback(() => setIsChatOpen((p) => !p), []);
  const closeChat = useCallback(() => setIsChatOpen(false), []);
  const clearSessionKey = useCallback(() => setOpenSessionKey(null), []);

  // Listen for open-chat events from tray / launcher
  useEvent<{ text?: string; sessionKey?: string }>("open-chat", (payload) => {
    setIsChatOpen(true);
    setOpenSessionKey(payload?.sessionKey ?? null);
  });

  return (
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar active={activeSidebarItem} isChatOpen={isChatOpen} onToggleChat={toggleChat} />
      <Outlet />
      <SidebarChat
        isOpen={isChatOpen}
        onClose={closeChat}
        viewContext={viewContext}
        openSessionKey={openSessionKey}
        onSessionKeyUsed={clearSessionKey}
      />
    </div>
  );
}
