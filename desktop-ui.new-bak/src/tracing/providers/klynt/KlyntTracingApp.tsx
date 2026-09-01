// SPDX-License-Identifier: Apache-2.0
//
// Standalone Klynt tracing app. Independent of KimiTracingApp and
// ClaudeCodeTracingApp: owns its own header, sessions list, session detail
// shell, tabs (Wire + Context + State + Agents), chips, and keyboard
// shortcuts. Shares data-layer primitives (api.ts, design tokens, Tooltip).

import { ArrowLeft, Bot, Check, Copy, Download, List, Moon, RefreshCw, Sun } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/tracing/components/ui/tooltip";
import { AgentScopeBar } from "@/tracing/features/agents-panel/agent-scope-bar";
import { AgentsPanel } from "@/tracing/features/agents-panel/agents-panel";
import { ContextViewer } from "@/tracing/features/context-viewer/context-viewer";
import { HeaderChips } from "@/tracing/features/session-detail/header-chips";
import { StateViewer } from "@/tracing/features/state-viewer/state-viewer";
import { WireViewer } from "@/tracing/features/wire-viewer/wire-viewer";
import { useTheme } from "@/tracing/hooks/use-theme";
import {
  fetchHeaderLayout,
  getSessionDownloadUrl,
  getSessionSummary,
  type HeaderLayoutResponse,
  listSessions,
  openInPath,
  type SessionInfo,
  type SessionSummary,
} from "@/tracing/lib/api";

const PROVIDER_ID = "klynt";
type Tab = "wire" | "context" | "state" | "agents";

export function KlyntTracingApp() {
  const { theme, toggleTheme } = useTheme();
  const [sessionId, setSessionId] = useState<string | null>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("klynt_session");
  });
  const [activeTab, setActiveTab] = useState<Tab>("wire");
  const [refreshKey, setRefreshKey] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [headerLayout, setHeaderLayout] = useState<HeaderLayoutResponse | null>(null);
  const [sessionSummary, setSessionSummary] = useState<SessionSummary | null>(null);
  const [openInSupported] = useState(false);
  const refreshTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [agentScope, setAgentScope] = useState<string | null>(null);
  const [contextScrollTarget, setContextScrollTarget] = useState<string | null>(null);
  const [wireScrollTarget, setWireScrollTarget] = useState<string | null>(null);

  useEffect(() => {
    return () => {
      if (refreshTimeoutRef.current) {
        clearTimeout(refreshTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    fetchHeaderLayout(PROVIDER_ID)
      .then(setHeaderLayout)
      .catch(() => setHeaderLayout(null));
  }, []);

  useEffect(() => {
    listSessions(PROVIDER_ID)
      .then(setSessions)
      .catch(() => setSessions([]));
  }, []);

  useEffect(() => {
    if (!sessionId) {
      setSessionSummary(null);
      return;
    }
    getSessionSummary(PROVIDER_ID, sessionId, refreshKey > 0)
      .then(setSessionSummary)
      .catch(() => setSessionSummary(null));
  }, [sessionId, refreshKey]);

  const handleSessionChange = useCallback((id: string | null) => {
    setSessionId(id);
    setAgentScope(null);
    const url = new URL(window.location.href);
    if (id) {
      url.searchParams.set("klynt_session", id);
    } else {
      url.searchParams.delete("klynt_session");
    }
    window.history.pushState({}, "", url.toString());
  }, []);

  useEffect(() => {
    const handler = () => {
      const params = new URLSearchParams(window.location.search);
      setSessionId(params.get("klynt_session"));
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "1") setActiveTab("wire");
      else if (e.key === "2") setActiveTab("context");
      else if (e.key === "3") setActiveTab("state");
      else if (e.key === "4") setActiveTab("agents");
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const currentSession = useMemo(() => {
    if (!sessionId) return null;
    return sessions.find((s) => `${s.work_dir_hash}/${s.session_id}` === sessionId) ?? null;
  }, [sessionId, sessions]);

  const handleNavigateToContext = useCallback((toolCallId: string) => {
    setContextScrollTarget(toolCallId);
    setActiveTab("context");
  }, []);

  const handleNavigateToWire = useCallback((toolCallId: string) => {
    setWireScrollTarget(toolCallId);
    setActiveTab("wire");
  }, []);

  return (
    <TooltipProvider>
      <div className="tracing-root flex h-full min-h-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b px-4 py-3">
          <h1
            className={`text-lg font-semibold tracking-tight flex items-center gap-2 ${
              sessionId ? "cursor-pointer hover:text-primary transition-colors" : ""
            }`}
            onClick={() => sessionId && handleSessionChange(null)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && sessionId) {
                handleSessionChange(null);
              }
            }}
            tabIndex={sessionId ? 0 : undefined}
            role={sessionId ? "button" : undefined}
            title={sessionId ? "Back to Sessions Explorer" : undefined}
          >
            {sessionId && <ArrowLeft size={16} className="text-muted-foreground" />}
            Klynt Tracing
          </h1>
          <button
            type="button"
            onClick={toggleTheme}
            className="rounded-md p-2 hover:bg-accent"
            title={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
          >
            {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
          </button>
        </header>

        {sessionId && (
          <>
            <div className="flex items-center border-b">
              <div className="flex-1 min-w-0 px-4 py-2">
                <SessionIdPill sessionId={sessionId} />
                {sessionSummary && headerLayout && (
                  <HeaderChips chips={headerLayout.chips} stats={sessionSummary} />
                )}
              </div>
              {currentSession && (
                <SessionDirectoryActions
                  session={currentSession}
                  openInSupported={openInSupported}
                />
              )}
              <a
                href={getSessionDownloadUrl(PROVIDER_ID, sessionId)}
                download
                className="shrink-0 rounded-md p-1.5 hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                title="Download session files as ZIP"
              >
                <Download size={14} />
              </a>
              <button
                type="button"
                onClick={() => {
                  setRefreshing(true);
                  setRefreshKey((k) => k + 1);
                  listSessions(PROVIDER_ID, true)
                    .then(setSessions)
                    .catch(() => {});
                  if (refreshTimeoutRef.current) {
                    clearTimeout(refreshTimeoutRef.current);
                  }
                  refreshTimeoutRef.current = setTimeout(() => setRefreshing(false), 600);
                }}
                className="mr-3 shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                title="Refresh session data"
              >
                <RefreshCw size={14} className={refreshing ? "animate-spin" : ""} />
              </button>
            </div>

            <div className="flex border-b px-4">
              {(
                [
                  { key: "wire", label: "Wire Events", icon: null },
                  { key: "context", label: "Context Messages", icon: null },
                  { key: "state", label: "State", icon: null },
                  { key: "agents", label: "Agents", icon: <Bot size={14} /> },
                ] as const
              ).map(({ key, label, icon }) => (
                <button
                  type="button"
                  key={key}
                  onClick={() => setActiveTab(key)}
                  className={`relative flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium transition-colors ${
                    activeTab === key
                      ? "text-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {icon}
                  {label}
                  {activeTab === key && (
                    <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary" />
                  )}
                </button>
              ))}
            </div>

            {(activeTab === "wire" || activeTab === "context") && (
              <AgentScopeBar
                sessionId={sessionId}
                refreshKey={refreshKey}
                selectedAgentId={agentScope}
                onSelectAgent={(id) => setAgentScope(id)}
                providerId={PROVIDER_ID}
              />
            )}

            <div className="flex-1 min-h-0 overflow-hidden">
              {activeTab === "wire" && (
                <WireViewer
                  sessionId={sessionId}
                  refreshKey={refreshKey}
                  onNavigateToContext={handleNavigateToContext}
                  scrollToToolCallId={wireScrollTarget}
                  onScrollTargetConsumed={() => setWireScrollTarget(null)}
                  agentScope={agentScope}
                  providerId={PROVIDER_ID}
                />
              )}
              {activeTab === "context" && (
                <ContextViewer
                  sessionId={sessionId}
                  refreshKey={refreshKey}
                  onNavigateToWire={handleNavigateToWire}
                  scrollToToolCallId={contextScrollTarget}
                  onScrollTargetConsumed={() => setContextScrollTarget(null)}
                  agentScope={agentScope}
                  providerId={PROVIDER_ID}
                />
              )}
              {activeTab === "state" && (
                <StateViewer
                  sessionId={sessionId}
                  refreshKey={refreshKey}
                  providerId={PROVIDER_ID}
                />
              )}
              {activeTab === "agents" && (
                <AgentsPanel
                  sessionId={sessionId}
                  refreshKey={refreshKey}
                  selectedAgentId={agentScope}
                  onSelectAgent={(agentId) => {
                    setAgentScope(agentId);
                    setActiveTab("wire");
                  }}
                  onSelectMain={() => {
                    setAgentScope(null);
                    setActiveTab("wire");
                  }}
                  providerId={PROVIDER_ID}
                />
              )}
            </div>
          </>
        )}

        {!sessionId && <SessionListView sessions={sessions} onSelect={handleSessionChange} />}
      </div>
    </TooltipProvider>
  );
}

function SessionIdPill({ sessionId }: { sessionId: string }) {
  const [copied, setCopied] = useState(false);
  const timeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timeout.current) clearTimeout(timeout.current);
    },
    [],
  );
  const bareId = sessionId.split("/").pop() ?? sessionId;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="inline-flex items-center gap-1 rounded-md bg-muted px-2 py-1 text-xs font-mono text-muted-foreground hover:text-foreground transition-colors"
          onClick={() => {
            navigator.clipboard.writeText(bareId).catch(() => {});
            setCopied(true);
            if (timeout.current) clearTimeout(timeout.current);
            timeout.current = setTimeout(() => setCopied(false), 2000);
          }}
        >
          {bareId}
        </button>
      </TooltipTrigger>
      <TooltipContent>{copied ? "Copied!" : "Click to copy"}</TooltipContent>
    </Tooltip>
  );
}

function SessionDirectoryActions({
  session,
  openInSupported,
}: {
  session: SessionInfo;
  openInSupported: boolean;
}) {
  const [copied, setCopied] = useState(false);
  const timeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timeout.current) clearTimeout(timeout.current);
    },
    [],
  );
  return (
    <div className="flex shrink-0 items-center gap-1 px-1.5">
      {openInSupported && (
        <button
          type="button"
          className="rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          onClick={async () => {
            try {
              await openInPath(PROVIDER_ID, "finder", session.session_id);
            } catch (err) {
              console.error("Open dir failed:", err);
            }
          }}
        >
          Open Dir
        </button>
      )}
      <button
        type="button"
        className="rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(session.session_dir);
            setCopied(true);
            if (timeout.current) clearTimeout(timeout.current);
            timeout.current = setTimeout(() => setCopied(false), 2000);
          } catch (err) {
            console.error("Copy failed:", err);
          }
        }}
      >
        {copied ? <Check size={13} /> : <Copy size={13} />}
        Copy DIR
      </button>
    </div>
  );
}

function SessionListView({
  sessions,
  onSelect,
}: {
  sessions: SessionInfo[];
  onSelect: (id: string) => void;
}) {
  const groups = useMemo(() => {
    const map = new Map<string, SessionInfo[]>();
    for (const s of sessions) {
      const key = s.imported ? "Imported" : (s.work_dir ?? "Unknown");
      const list = map.get(key) ?? [];
      list.push(s);
      map.set(key, list);
    }
    return Array.from(map.entries()).map(([dir, items]) => ({ dir, items }));
  }, [sessions]);

  if (sessions.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
        <List size={32} className="opacity-40" />
        <span>No Klynt sessions found.</span>
        <span className="text-xs opacity-60">Coding-mode sessions appear here automatically.</span>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto px-4 py-4">
      {groups.map((g) => (
        <section key={g.dir} className="mb-6">
          <header className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-foreground">{g.dir}</span>
            <span className="text-xs text-muted-foreground">
              {g.items.length} session{g.items.length === 1 ? "" : "s"}
            </span>
          </header>
          <ul className="space-y-1">
            {g.items.map((s) => (
              <li key={`${s.session_id}-${s.work_dir_hash}`}>
                <button
                  type="button"
                  className="w-full flex flex-col gap-0.5 rounded-md border px-3 py-2 text-left transition-colors hover:bg-accent"
                  onClick={() => onSelect(`${s.work_dir_hash}/${s.session_id}`)}
                >
                  <span className="text-sm font-medium">{s.title || s.session_id.slice(0, 8)}</span>
                  <span className="text-xs text-muted-foreground">
                    {s.turns} turns
                    {(s.subagent_count ?? 0) > 0 && (
                      <>
                        {" "}
                        <span>·</span> <span>{s.subagent_count} agents</span>{" "}
                      </>
                    )}
                    <span> · </span>
                    <span>{formatRelativeTime(s.last_updated)}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

function formatRelativeTime(unixSec: number): string {
  const now = Date.now() / 1000;
  const diff = now - unixSec;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}
