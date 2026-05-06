// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import {
  ArrowLeft,
  BarChart3,
  Bot,
  Check,
  Columns,
  Copy,
  Download,
  FolderOpen,
  List,
  Moon,
  RefreshCw,
  Sun,
  X,
} from "lucide-react";
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
import { DualView } from "@/tracing/features/dual-view/dual-view";
import { SessionsExplorer } from "@/tracing/features/sessions-explorer/sessions-explorer";
import { StateViewer } from "@/tracing/features/state-viewer/state-viewer";
import { StatisticsView } from "@/tracing/features/statistics/statistics-view";
import { isErrorEvent } from "@/tracing/features/wire-viewer/wire-event-card";
import { WireViewer } from "@/tracing/features/wire-viewer/wire-viewer";
import { useTheme } from "@/tracing/hooks/use-theme";
import {
  getSessionDownloadUrl,
  getSubagents,
  getVisCapabilities,
  getWireEvents,
  listSessions,
  openInPath,
  type SessionInfo,
  type WireEvent,
} from "@/tracing/lib/api";

type Tab = "wire" | "context" | "state" | "dual" | "agents";

interface SessionStatsData {
  turns: number;
  steps: number;
  toolCalls: number;
  errors: number;
  compactions: number;
  durationSec: number;
  inputTokens: number;
  outputTokens: number;
  cacheRate: number;
}

function computeStats(events: WireEvent[]): SessionStatsData {
  let turns = 0;
  let steps = 0;
  let toolCalls = 0;
  let errors = 0;
  let compactions = 0;
  let inputTokens = 0;
  let outputTokens = 0;
  let totalCacheRead = 0;
  let totalInputOther = 0;
  let totalCacheCreation = 0;

  for (const e of events) {
    if (e.type === "TurnBegin") turns++;
    if (e.type === "StepBegin") steps++;
    if (e.type === "ToolCall") toolCalls++;
    if (e.type === "CompactionBegin") compactions++;
    if (isErrorEvent(e)) errors++;
    if (e.type === "StatusUpdate") {
      const tu = e.payload.token_usage as Record<string, number> | undefined;
      if (tu) {
        inputTokens +=
          (tu.input_other ?? 0) + (tu.input_cache_read ?? 0) + (tu.input_cache_creation ?? 0);
        outputTokens += tu.output ?? 0;
        totalCacheRead += tu.input_cache_read ?? 0;
        totalInputOther += tu.input_other ?? 0;
        totalCacheCreation += tu.input_cache_creation ?? 0;
      }
    }
    // Count tokens from SubagentEvent-wrapped StatusUpdate
    if (e.type === "SubagentEvent") {
      const inner = e.payload.event as Record<string, unknown> | undefined;
      if (inner?.type === "StatusUpdate") {
        const innerPayload = inner.payload as Record<string, unknown> | undefined;
        const tu = innerPayload?.token_usage as Record<string, number> | undefined;
        if (tu) {
          inputTokens +=
            (tu.input_other ?? 0) + (tu.input_cache_read ?? 0) + (tu.input_cache_creation ?? 0);
          outputTokens += tu.output ?? 0;
          totalCacheRead += tu.input_cache_read ?? 0;
          totalInputOther += tu.input_other ?? 0;
          totalCacheCreation += tu.input_cache_creation ?? 0;
        }
      }
    }
  }

  const durationSec =
    events.length >= 2 ? events[events.length - 1].timestamp - events[0].timestamp : 0;

  const totalInput = totalCacheRead + totalInputOther + totalCacheCreation;
  const cacheRate = totalInput > 0 ? (totalCacheRead / totalInput) * 100 : 0;

  return {
    turns,
    steps,
    toolCalls,
    errors,
    compactions,
    durationSec,
    inputTokens,
    outputTokens,
    cacheRate,
  };
}

function formatDuration(sec: number): string {
  if (sec < 1) return `${(sec * 1000).toFixed(0)}ms`;
  if (sec < 60) return `${sec.toFixed(1)}s`;
  return `${(sec / 60).toFixed(1)}min`;
}

function formatTokens(n: number): string {
  if (n === 0) return "0";
  if (n < 1000) return `${n}`;
  return `${(n / 1000).toFixed(1)}k`;
}

function getSessionDir(session: SessionInfo): string {
  return session.session_dir;
}

function SessionDirectoryActions({
  session,
  openInSupported,
  providerId,
}: {
  session: SessionInfo;
  openInSupported: boolean;
  providerId: string;
}) {
  const [copied, setCopied] = useState(false);

  const handleOpenSessionDir = useCallback(async () => {
    try {
      await openInPath(providerId, "finder", session.session_id);
    } catch (error) {
      console.error("Failed to open session directory:", error);
      window.alert(
        error instanceof Error
          ? `Failed to open session directory:\n${error.message}`
          : "Failed to open session directory",
      );
    }
  }, [session.session_id]);

  const copiedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copiedTimeoutRef.current) {
        clearTimeout(copiedTimeoutRef.current);
      }
    };
  }, []);

  const handleCopyDirInfo = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(getSessionDir(session));
      setCopied(true);
      if (copiedTimeoutRef.current) {
        clearTimeout(copiedTimeoutRef.current);
      }
      copiedTimeoutRef.current = setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error("Failed to copy DIR info:", error);
      window.alert("Failed to copy DIR info");
    }
  }, [session]);

  return (
    <div className="flex shrink-0 items-center gap-1 px-1.5">
      {openInSupported && (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={handleOpenSessionDir}
              className="rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              aria-label="Open current session directory"
            >
              <span className="flex items-center gap-1">
                <FolderOpen size={13} />
                Open Dir
              </span>
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom" className="max-w-md break-all">
            Open current session directory
            <div className="mt-1 font-mono text-[11px]">{session.session_dir}</div>
          </TooltipContent>
        </Tooltip>
      )}

      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={handleCopyDirInfo}
            className="rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label="Copy current session directory info"
          >
            <span className="flex items-center gap-1">
              {copied ? <Check size={13} /> : <Copy size={13} />}
              Copy DIR
            </span>
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom">
          {copied ? "Copied session directory" : "Copy session directory path"}
        </TooltipContent>
      </Tooltip>
    </div>
  );
}

function SessionStats({ sessionId, refreshKey, providerId }: { sessionId: string; refreshKey: number; providerId: string }) {
  const [copied, setCopied] = useState(false);
  const copiedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [events, setEvents] = useState<WireEvent[]>([]);

  useEffect(() => {
    return () => {
      if (copiedTimeoutRef.current) {
        clearTimeout(copiedTimeoutRef.current);
      }
    };
  }, []);
  const [loaded, setLoaded] = useState(false);
  const [agentCount, setAgentCount] = useState(0);

  useEffect(() => {
    setLoaded(false);
    getWireEvents(providerId, sessionId, refreshKey > 0)
      .then((res) => setEvents(res.events))
      .catch(() => setEvents([]))
      .finally(() => setLoaded(true));
    getSubagents(providerId, sessionId, refreshKey > 0)
      .then((agents) => setAgentCount(agents.length))
      .catch(() => setAgentCount(0));
  }, [sessionId, refreshKey]);

  const stats = useMemo(() => computeStats(events), [events]);

  if (!loaded || events.length === 0) return null;

  const parts: string[] = [
    `${stats.turns} turn${stats.turns !== 1 ? "s" : ""}`,
    `${stats.steps} step${stats.steps !== 1 ? "s" : ""}`,
    `${stats.toolCalls} tool call${stats.toolCalls !== 1 ? "s" : ""}`,
  ];
  if (stats.errors > 0) parts.push(`${stats.errors} error${stats.errors !== 1 ? "s" : ""}`);
  if (stats.compactions > 0)
    parts.push(`${stats.compactions} compaction${stats.compactions !== 1 ? "s" : ""}`);
  if (agentCount > 0) parts.push(`${agentCount} agent${agentCount !== 1 ? "s" : ""}`);

  return (
    <div className="min-w-0 flex flex-1 items-center gap-2 overflow-x-auto px-4 py-1.5 text-xs text-muted-foreground">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            className="font-mono shrink-0 cursor-pointer hover:text-foreground transition-colors bg-transparent border-none p-0"
            onClick={() => {
              const fullId = sessionId.split("/").pop() ?? sessionId;
              navigator.clipboard.writeText(fullId).catch(() => {});
              setCopied(true);
              if (copiedTimeoutRef.current) {
                clearTimeout(copiedTimeoutRef.current);
              }
              copiedTimeoutRef.current = setTimeout(() => setCopied(false), 2000);
            }}
          >
            {sessionId.split("/").pop() ?? sessionId}
          </button>
        </TooltipTrigger>
        <TooltipContent>{copied ? "Copied!" : "Click to copy"}</TooltipContent>
      </Tooltip>
      <span className="text-border">|</span>
      <span className="shrink-0">{parts.join(" · ")}</span>
      <span className="text-border">|</span>
      <span className="shrink-0">{formatDuration(stats.durationSec)}</span>
      {(stats.inputTokens > 0 || stats.outputTokens > 0) && (
        <>
          <span className="text-border">|</span>
          <span className="shrink-0">
            {formatTokens(stats.inputTokens)} in / {formatTokens(stats.outputTokens)} out
          </span>
          {stats.cacheRate > 0 && (
            <>
              <span className="text-border">|</span>
              <span className="shrink-0">{Math.round(stats.cacheRate)}% cache</span>
            </>
          )}
        </>
      )}
    </div>
  );
}

function ShortcutRow({ keys, desc }: { keys: string; desc: string }) {
  return (
    <div className="flex items-center gap-3">
      <kbd className="inline-flex min-w-[2rem] items-center justify-center rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">
        {keys}
      </kbd>
      <span className="text-muted-foreground">{desc}</span>
    </div>
  );
}

export interface TracingAppProps {
  providerId: string | null; // null when chip is "all" or unsupported
}

export function TracingApp({ providerId }: TracingAppProps) {
  if (!providerId) {
    return (
      <div className="cm-state cm-state--empty">
        Select a provider from the chips above to view its tracing data.
      </div>
    );
  }

  const { theme, toggleTheme } = useTheme();
  const [sessionId, setSessionId] = useState<string | null>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("session");
  });
  const [activeTab, setActiveTab] = useState<Tab>("wire");
  const [explorerView, setExplorerView] = useState<"sessions" | "statistics">("sessions");
  const [showShortcutHelp, setShowShortcutHelp] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const refreshTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [openInSupported, setOpenInSupported] = useState(false);

  useEffect(() => {
    return () => {
      if (refreshTimeoutRef.current) {
        clearTimeout(refreshTimeoutRef.current);
      }
    };
  }, []);
  // Agent scope: null = main agent, string = sub-agent ID
  const [agentScope, setAgentScope] = useState<string | null>(null);
  // Cross-reference navigation targets
  const [contextScrollTarget, setContextScrollTarget] = useState<string | null>(null);
  const [wireScrollTarget, setWireScrollTarget] = useState<string | null>(null);

  const handleNavigateToContext = useCallback((toolCallId: string) => {
    setContextScrollTarget(toolCallId);
    setActiveTab("context");
  }, []);

  const handleNavigateToWire = useCallback((toolCallId: string) => {
    setWireScrollTarget(toolCallId);
    setActiveTab("wire");
  }, []);

  const handleSessionChange = useCallback((id: string | null) => {
    setSessionId(id);
    setAgentScope(null);
    const url = new URL(window.location.href);
    if (id) {
      url.searchParams.set("session", id);
    } else {
      url.searchParams.delete("session");
    }
    window.history.pushState({}, "", url.toString());
  }, []);

  useEffect(() => {
    const handler = () => {
      const params = new URLSearchParams(window.location.search);
      setSessionId(params.get("session"));
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  // Dynamic page title
  const [sessions, setSessions] = useState<Awaited<ReturnType<typeof listSessions>>>([]);
  useEffect(() => {
    listSessions(providerId)
      .then(setSessions)
      .catch(() => {});
  }, []);
  useEffect(() => {
    getVisCapabilities()
      .then((capabilities) => setOpenInSupported(capabilities.open_in_supported))
      .catch((error) => {
        console.error("Failed to load vis capabilities:", error);
        setOpenInSupported(false);
      });
  }, []);
  const currentSession = useMemo(() => {
    if (!sessionId) return null;
    return sessions.find((s) => `${s.work_dir_hash}/${s.session_id}` === sessionId) ?? null;
  }, [sessionId, sessions]);
  useEffect(() => {
    const previousTitle = document.title;
    if (!sessionId) {
      document.title = "Agent Tracing";
      return () => {
        document.title = previousTitle;
      };
    }
    const rawId = sessionId.split("/").pop() ?? sessionId;
    const label = currentSession?.metadata?.title || currentSession?.title || rawId.slice(0, 8);
    document.title = `${label} — Agent Tracing`;
    return () => {
      document.title = previousTitle;
    };
  }, [currentSession, sessionId]);

  // Global keyboard shortcuts: 1/2/3 to switch tabs
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Don't intercept when focused on input elements
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      if (e.key === "Escape" && showShortcutHelp) {
        setShowShortcutHelp(false);
        return;
      }

      if (e.key === "?") {
        setShowShortcutHelp((prev) => !prev);
        return;
      }

      if (e.key === "1") setActiveTab("wire");
      else if (e.key === "2") setActiveTab("context");
      else if (e.key === "3") setActiveTab("state");
      else if (e.key === "4") setActiveTab("dual");
      else if (e.key === "5") setActiveTab("agents");
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [showShortcutHelp]);

  return (
    <TooltipProvider>
      <div className="tracing-root flex h-full min-h-0 flex-1 flex-col">
        {/* Header */}
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
            Agent Tracing
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

        {/* Session Stats */}
        {sessionId && (
          <div className="flex items-center border-b">
            <SessionStats sessionId={sessionId} refreshKey={refreshKey} providerId={providerId} />
            {currentSession && (
              <SessionDirectoryActions session={currentSession} openInSupported={openInSupported} providerId={providerId} />
            )}
            <a
              href={getSessionDownloadUrl(providerId, sessionId)}
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
                listSessions(providerId, true)
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
        )}

        {/* Tabs */}
        {sessionId && (
          <>
            <div className="flex border-b px-4">
              {(
                [
                  { key: "wire", label: "Wire Events", icon: null },
                  { key: "context", label: "Context Messages", icon: null },
                  { key: "state", label: "State", icon: null },
                  { key: "dual", label: "Dual", icon: <Columns size={14} /> },
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

            {/* Agent Scope Bar - shown on wire/context/dual tabs */}
            {(activeTab === "wire" || activeTab === "context" || activeTab === "dual") && (
              <AgentScopeBar
                sessionId={sessionId}
                refreshKey={refreshKey}
                selectedAgentId={agentScope}
                onSelectAgent={(id) => {
                  setAgentScope(id);
                }}
                providerId={providerId}
              />
            )}

            {/* Tab Content */}
            <div className="flex-1 min-h-0 overflow-hidden">
              {activeTab === "wire" && (
                <WireViewer
                  sessionId={sessionId}
                  refreshKey={refreshKey}
                  onNavigateToContext={handleNavigateToContext}
                  scrollToToolCallId={wireScrollTarget}
                  onScrollTargetConsumed={() => setWireScrollTarget(null)}
                  agentScope={agentScope}
                  providerId={providerId}
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
                  providerId={providerId}
                />
              )}
              {activeTab === "state" && (
                <StateViewer sessionId={sessionId} refreshKey={refreshKey} providerId={providerId} />
              )}
              {activeTab === "dual" && (
                <DualView sessionId={sessionId} refreshKey={refreshKey} agentScope={agentScope} providerId={providerId} />
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
                  providerId={providerId}
                />
              )}
            </div>
          </>
        )}

        {!sessionId && (
          <div className="flex h-full flex-col overflow-hidden">
            {/* Explorer view tabs */}
            <div className="flex items-center gap-1 border-b px-4 py-1.5">
              {(
                [
                  { key: "sessions", label: "Sessions", icon: <List size={14} /> },
                  { key: "statistics", label: "Statistics", icon: <BarChart3 size={14} /> },
                ] as const
              ).map(({ key, label, icon }) => (
                <button
                  type="button"
                  key={key}
                  onClick={() => setExplorerView(key)}
                  className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
                    explorerView === key
                      ? "bg-accent text-foreground"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                  }`}
                >
                  {icon}
                  {label}
                </button>
              ))}
            </div>

            {/* Explorer content */}
            {explorerView === "sessions" ? (
              <SessionsExplorer onSelectSession={handleSessionChange} providerId={providerId} />
            ) : (
              <StatisticsView providerId={providerId} />
            )}
          </div>
        )}

        {/* Shortcut Help Overlay */}
        {showShortcutHelp && (
          <>
            <button
              type="button"
              className="fixed inset-0 z-50 bg-black/50"
              aria-label="Close"
              onClick={() => setShowShortcutHelp(false)}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none">
              <div
                role="dialog"
                aria-modal="true"
                className="relative w-full max-w-lg rounded-lg border bg-popover p-6 shadow-lg pointer-events-auto"
                onClick={(e) => e.stopPropagation()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                <div className="flex items-center justify-between mb-4">
                  <h2 className="text-lg font-semibold">Keyboard Shortcuts</h2>
                  <button
                    type="button"
                    onClick={() => setShowShortcutHelp(false)}
                    className="rounded-md p-1 hover:bg-accent"
                  >
                    <X size={16} />
                  </button>
                </div>

                <div className="space-y-4 text-sm">
                  <div>
                    <h3 className="text-xs font-medium uppercase text-muted-foreground mb-2">
                      Global
                    </h3>
                    <div className="space-y-1.5">
                      <ShortcutRow keys="1" desc="Wire Events" />
                      <ShortcutRow keys="2" desc="Context Messages" />
                      <ShortcutRow keys="3" desc="State" />
                      <ShortcutRow keys="4" desc="Dual View" />
                      <ShortcutRow keys="5" desc="Agents" />
                      <ShortcutRow keys="?" desc="Show shortcuts" />
                    </div>
                  </div>

                  <div>
                    <h3 className="text-xs font-medium uppercase text-muted-foreground mb-2">
                      Wire Events
                    </h3>
                    <div className="space-y-1.5">
                      <ShortcutRow keys="j / k" desc="Navigate events" />
                      <ShortcutRow keys="Enter" desc="Expand / collapse" />
                      <ShortcutRow keys="e" desc="Next error" />
                      <ShortcutRow keys="/" desc="Search" />
                      <ShortcutRow keys="Esc" desc="Close panel" />
                    </div>
                  </div>

                  <div>
                    <h3 className="text-xs font-medium uppercase text-muted-foreground mb-2">
                      Context Messages
                    </h3>
                    <div className="space-y-1.5">
                      <ShortcutRow keys="/" desc="Search" />
                      <ShortcutRow keys="Enter" desc="Next match" />
                      <ShortcutRow keys="Shift+Enter" desc="Previous match" />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </TooltipProvider>
  );
}
