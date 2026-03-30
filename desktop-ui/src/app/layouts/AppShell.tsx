import { BrainEventBridge } from "@app/BrainEventBridge";
import { BrainOrb } from "@shared/components/BrainOrb";
import { SidebarChat } from "@shared/components/chat/SidebarChat";
import { KlyntLogo } from "@shared/components/ui/KlyntLogo";
import { useActiveView } from "@shared/hooks/useActiveView";
import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useUpdateChecker } from "@shared/hooks/useUpdateChecker";
import type { AppInfoResponse, SidebarItem } from "@shared/types";
import { ArrowDownCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, Outlet, useLocation, useNavigate } from "react-router";
import { Sidebar } from "./Sidebar";

export function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [openSessionKey, setOpenSessionKey] = useState<string | null>(null);
  const [setupState, setSetupState] = useState<"loading" | "needed" | "ready">("loading");
  const { updateAvailable, version, triggerInstall } = useUpdateChecker();

  const isOnChatPage = location.pathname.startsWith("/chat");

  // Push active view to backend for contextual query rewriting
  useActiveView();

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
    if (path.startsWith("/learn")) return "Learn";
    if (path.startsWith("/finance")) return "Finance";
    if (path.startsWith("/coaching")) return "Coaching";
    if (path.startsWith("/brain")) return "Brain";
    if (path.startsWith("/automations")) return "Automations";
    if (path.startsWith("/system")) return "System";
    if (path.startsWith("/settings")) return "Settings";
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
    if (path.startsWith("/system")) return { entityKind: "system" };
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
    <div className="h-screen w-screen bg-background text-foreground flex flex-col overflow-hidden">
      {/* Bridge global events (brain:ambient, etc.) from dev server SSE → window CustomEvents */}
      <BrainEventBridge />
      {/* ── Custom titlebar (drag region) ────────────────────────────── */}
      <div className="relative flex items-center shrink-0 h-[36px] px-3" data-tauri-drag-region>
        {/* macOS traffic light spacer — pointer-events-none so clicks fall through to drag region */}
        <div className="w-[68px] shrink-0 pointer-events-none" />
        {/* Center branding */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none select-none">
          <KlyntLogo className="w-3.5 h-3.5 text-muted-foreground" />
          <span className="ml-1.5 text-xs font-medium text-muted-foreground tracking-wide">
            Klynt
          </span>
        </div>
        {/* Right controls — container is pointer-events-none, only buttons are interactive */}
        <div className="ml-auto flex items-center gap-2 shrink-0 pointer-events-none">
          {updateAvailable && (
            <button
              type="button"
              onClick={triggerInstall}
              className="pointer-events-auto flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium
                bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
            >
              <ArrowDownCircle className="w-3 h-3" />
              <span>v{version}</span>
            </button>
          )}
          <div className="pointer-events-auto">
            <BrainOrb />
          </div>
        </div>
      </div>
      {/* ── Main content ────────────────────────────────────────────────── */}
      <div className="flex gap-2 px-2 pb-2 flex-1 overflow-hidden">
        <Sidebar
          active={activeSidebarItem}
          isChatOpen={isChatOpen}
          onToggleChat={isOnChatPage ? undefined : toggleChat}
        />
        <div className="flex-1 flex overflow-hidden">
          <Outlet />
        </div>
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
    </div>
  );
}
