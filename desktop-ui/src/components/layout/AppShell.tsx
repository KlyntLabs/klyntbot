import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, Outlet, useLocation, useNavigate } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { ipc } from "../../hooks/useIpc";
import type { AppInfoResponse, SidebarItem } from "../../lib/types";
import { SidebarChat } from "../chat/SidebarChat";
import { Sidebar } from "./Sidebar";

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [openSessionKey, setOpenSessionKey] = useState<string | null>(null);
  const [setupState, setSetupState] = useState<"loading" | "needed" | "ready">("loading");

  const isOnChatPage = location.pathname.startsWith("/chat");

  useEffect(() => {
    let cancelled = false;
    ipc<AppInfoResponse>("app_info")
      .then((info) => {
        if (!cancelled) setSetupState(info.setupCompleted ? "ready" : "needed");
      })
      .catch(() => {
        if (!cancelled) setSetupState("ready");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Auto-close sidebar chat when navigating to the full /chat page
  useEffect(() => {
    if (isOnChatPage) setIsChatOpen(false);
  }, [isOnChatPage]);

  // Derive active sidebar item from current route
  const activeSidebarItem = useMemo((): SidebarItem => {
    const path = location.pathname;
    if (path.startsWith("/chat")) return "Chat";
    if (path.startsWith("/tasks") || path.startsWith("/task/")) return "Tasks";
    if (path.startsWith("/notes")) return "Notes";
    if (path.startsWith("/finance")) return "Finance";
    if (path.startsWith("/productivity")) return "Productivity";
    if (path.startsWith("/automations")) return "Automations";
    if (path.startsWith("/settings")) return "Settings";
    if (path.startsWith("/debug")) return "Debug";
    return "Dashboard";
  }, [location.pathname]);

  // Derive chat context from current route (detail pages handled by usePageContext inside SidebarChat)
  const viewContext = useMemo(() => {
    const path = location.pathname;
    const search = new URLSearchParams(location.search);

    if (path.startsWith("/tasks")) {
      const tab = search.get("tab");
      if (tab) return { entityKind: `tasks.${tab.toLowerCase()}` };
      return { entityKind: "tasks" };
    }
    if (
      path.startsWith("/day/") ||
      path.startsWith("/week/") ||
      path.startsWith("/month/") ||
      path.startsWith("/year/")
    ) {
      return { entityKind: "dashboard" };
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

  // Listen for navigate events from tray / launcher
  useEvent<{ path: string }>("navigate", (payload) => {
    if (payload?.path) navigate(payload.path);
  });

  // Gate: redirect to setup wizard if setup not completed
  if (setupState === "loading") return null;
  if (setupState === "needed") return <Navigate to="/setup/welcome" replace />;

  return (
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar
        active={activeSidebarItem}
        isChatOpen={isChatOpen}
        onToggleChat={isOnChatPage ? undefined : toggleChat}
      />
      <Outlet />
      {!isOnChatPage && (
        <SidebarChat
          isOpen={isChatOpen}
          onClose={closeChat}
          viewContext={viewContext}
          openSessionKey={openSessionKey}
          onSessionKeyUsed={clearSessionKey}
        />
      )}
    </div>
  );
}
