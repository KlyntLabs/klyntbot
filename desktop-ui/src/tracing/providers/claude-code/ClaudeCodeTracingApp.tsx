// SPDX-License-Identifier: Apache-2.0
//
// Standalone Claude Code tracing app. Independent of KimiTracingApp:
// owns its own header, sessions list, session detail shell, tabs (Wire +
// Agents only), chips, and keyboard shortcuts. Shares only data-layer
// primitives (api.ts, design tokens, Tooltip).

import { ArrowLeft, Bot, Check, Copy, Download, Moon, RefreshCw, Sun } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/tracing/components/ui/tooltip";
import { ClaudeCodeAgentsPanel } from "@/tracing/features/providers/claude-code/agents-panel/claude-code-agents-panel";
import { ClaudeCodeWireViewer } from "@/tracing/features/providers/claude-code/wire-viewer/claude-code-wire-viewer";
import { useTheme } from "@/tracing/hooks/use-theme";
import {
  fetchHeaderLayout,
  getSessionDownloadUrl,
  getSessionSummary,
  getSubagents,
  getWireEvents,
  type HeaderLayoutResponse,
  listSessions,
  openInPath,
  type SessionInfo,
  type SessionSummary,
  type SubagentInfo,
  type WireEvent,
} from "@/tracing/lib/api";

const PROVIDER_ID = "claudeCode";
type Tab = "wire" | "agents";

export function ClaudeCodeTracingApp() {
  const { theme, toggleTheme } = useTheme();
  const [sessionId, setSessionId] = useState<string | null>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("session");
  });
  const [activeTab, setActiveTab] = useState<Tab>("wire");
  const [refreshKey, setRefreshKey] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [wireEvents, setWireEvents] = useState<WireEvent[]>([]);
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [summary, setSummary] = useState<SessionSummary | null>(null);
  const [layout, setLayout] = useState<HeaderLayoutResponse | null>(null);
  const refreshTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (refreshTimeout.current) clearTimeout(refreshTimeout.current);
    };
  }, []);

  // Header layout (chip set) — fetched once.
  useEffect(() => {
    fetchHeaderLayout(PROVIDER_ID)
      .then(setLayout)
      .catch(() => setLayout(null));
  }, []);

  // Session list — refetched on refresh.
  useEffect(() => {
    listSessions(PROVIDER_ID)
      .then(setSessions)
      .catch(() => setSessions([]));
  }, [refreshKey]);

  // Session detail data when a session is selected.
  useEffect(() => {
    if (!sessionId) {
      setWireEvents([]);
      setSubagents([]);
      setSummary(null);
      return;
    }
    getWireEvents(PROVIDER_ID, sessionId, refreshKey > 0)
      .then((r) => setWireEvents(r.events))
      .catch(() => setWireEvents([]));
    getSubagents(PROVIDER_ID, sessionId, refreshKey > 0)
      .then(setSubagents)
      .catch(() => setSubagents([]));
    getSessionSummary(PROVIDER_ID, sessionId, refreshKey > 0)
      .then(setSummary)
      .catch(() => setSummary(null));
  }, [sessionId, refreshKey]);

  // URL <-> sessionId sync.
  const goToSession = useCallback((id: string | null) => {
    setSessionId(id);
    setActiveTab("wire");
    const url = new URL(window.location.href);
    if (id) url.searchParams.set("session", id);
    else url.searchParams.delete("session");
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

  // Keyboard: 1=wire, 2=agents (CC only has these two).
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "1") setActiveTab("wire");
      else if (e.key === "2") setActiveTab("agents");
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const currentSession = useMemo(
    () => sessions.find((s) => `${s.work_dir_hash}/${s.session_id}` === sessionId) ?? null,
    [sessions, sessionId],
  );

  const refresh = useCallback(() => {
    setRefreshing(true);
    setRefreshKey((k) => k + 1);
    if (refreshTimeout.current) clearTimeout(refreshTimeout.current);
    refreshTimeout.current = setTimeout(() => setRefreshing(false), 600);
  }, []);

  return (
    <TooltipProvider>
      <div className="cc-app">
        <header className="cc-app__header">
          <h1
            className={`cc-app__title${sessionId ? " cc-app__title--clickable" : ""}`}
            onClick={() => sessionId && goToSession(null)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && sessionId) goToSession(null);
            }}
            tabIndex={sessionId ? 0 : undefined}
            role={sessionId ? "button" : undefined}
          >
            {sessionId && <ArrowLeft size={16} />}
            Claude Code · Tracing
          </h1>
          <button
            type="button"
            onClick={toggleTheme}
            className="cc-app__theme-toggle"
            title={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
          >
            {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
          </button>
        </header>

        {sessionId ? (
          <SessionDetailView
            sessionId={sessionId}
            session={currentSession}
            summary={summary}
            layout={layout}
            wireEvents={wireEvents}
            subagents={subagents}
            activeTab={activeTab}
            onTabChange={setActiveTab}
            onRefresh={refresh}
            refreshing={refreshing}
          />
        ) : (
          <SessionListView sessions={sessions} onSelect={goToSession} />
        )}
      </div>
    </TooltipProvider>
  );
}

// ── Session detail view ───────────────────────────────────────────────

function SessionDetailView({
  sessionId,
  session,
  summary,
  layout,
  wireEvents,
  subagents,
  activeTab,
  onTabChange,
  onRefresh,
  refreshing,
}: {
  sessionId: string;
  session: SessionInfo | null;
  summary: SessionSummary | null;
  layout: HeaderLayoutResponse | null;
  wireEvents: WireEvent[];
  subagents: SubagentInfo[];
  activeTab: Tab;
  onTabChange: (t: Tab) => void;
  onRefresh: () => void;
  refreshing: boolean;
}) {
  const bareId = sessionId.split("/").pop() ?? sessionId;
  return (
    <>
      <div className="cc-app__session-bar">
        <SessionIdPill bareId={bareId} />
        {summary && layout && <CcHeaderChips chips={layout.chips} stats={summary} />}
        <div className="cc-app__session-actions">
          {session && <SessionDirActions session={session} />}
          <a
            href={getSessionDownloadUrl(PROVIDER_ID, sessionId)}
            download
            className="cc-app__icon-btn"
            title="Download session as ZIP"
          >
            <Download size={14} />
          </a>
          <button
            type="button"
            onClick={onRefresh}
            className="cc-app__icon-btn"
            title="Refresh session"
          >
            <RefreshCw size={14} className={refreshing ? "cc-spin" : ""} />
          </button>
        </div>
      </div>

      <div className="cc-app__tabs">
        <TabButton
          label="Wire Events"
          active={activeTab === "wire"}
          onClick={() => onTabChange("wire")}
        />
        <TabButton
          label="Agents"
          icon={<Bot size={14} />}
          active={activeTab === "agents"}
          onClick={() => onTabChange("agents")}
          badge={subagents.length || undefined}
        />
      </div>

      <div className="cc-app__content">
        {activeTab === "wire" && <ClaudeCodeWireViewer events={wireEvents} />}
        {activeTab === "agents" && <ClaudeCodeAgentsPanel agents={subagents} onSelect={() => {}} />}
      </div>
    </>
  );
}

function SessionIdPill({ bareId }: { bareId: string }) {
  const [copied, setCopied] = useState(false);
  const timeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timeout.current) clearTimeout(timeout.current);
    },
    [],
  );
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="cc-app__id-pill"
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

function SessionDirActions({ session }: { session: SessionInfo }) {
  const [copied, setCopied] = useState(false);
  const timeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timeout.current) clearTimeout(timeout.current);
    },
    [],
  );
  return (
    <>
      <button
        type="button"
        className="cc-app__icon-btn"
        title={`Open ${session.session_dir}`}
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
      <button
        type="button"
        className="cc-app__icon-btn"
        title={copied ? "Copied" : "Copy directory path"}
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
    </>
  );
}

function TabButton({
  label,
  icon,
  active,
  onClick,
  badge,
}: {
  label: string;
  icon?: React.ReactNode;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`cc-app__tab${active ? " cc-app__tab--active" : ""}`}
    >
      {icon}
      {label}
      {badge !== undefined && <span className="cc-app__tab-badge">{badge}</span>}
    </button>
  );
}

// ── Header chips (CC-only; data-driven by header_layout) ──────────────

function CcHeaderChips({
  chips,
  stats,
}: {
  chips: HeaderLayoutResponse["chips"];
  stats: SessionSummary;
}) {
  return (
    <div className="cc-app__chips">
      {chips.map((c) => (
        <span key={c} className="cc-app__chip">
          <span className="cc-app__chip-label">{chipLabel(c)}</span>
          <strong>{chipValue(c, stats)}</strong>
        </span>
      ))}
    </div>
  );
}

function chipLabel(c: string): string {
  switch (c) {
    case "turns":
      return "Turns";
    case "toolCalls":
      return "Tools";
    case "errors":
      return "Errors";
    case "compactions":
      return "Compacts";
    case "agents":
      return "Agents";
    case "duration":
      return "Dur";
    case "tokens":
      return "Tokens";
    case "cacheHitPct":
      return "Cache%";
    case "model":
      return "Model";
    default:
      return c;
  }
}

function chipValue(c: string, s: SessionSummary): string {
  switch (c) {
    case "turns":
      return String(s.turns);
    case "toolCalls":
      return String(s.tool_calls);
    case "errors":
      return String(s.errors);
    case "compactions":
      return String(s.compactions);
    case "duration":
      return formatDuration(s.duration_sec);
    case "tokens":
      return `${formatNum(s.input_tokens)} / ${formatNum(s.output_tokens)}`;
    default:
      return "—";
  }
}

function formatNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return m > 0 ? `${m}m ${s}s` : `${s}s`;
}

function formatRelativeTime(unixSec: number): string {
  const now = Date.now() / 1000;
  const diff = now - unixSec;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

// ── Session list view ─────────────────────────────────────────────────

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
      <div className="cc-app__empty">
        <span>No Claude Code sessions found.</span>
        <span className="cc-app__empty-hint">
          Sessions are auto-discovered from <code>~/.claude/projects/</code>.
        </span>
      </div>
    );
  }

  return (
    <div className="cc-app__list">
      {groups.map((g) => (
        <section key={g.dir} className="cc-app__group">
          <header className="cc-app__group-header">
            <span className="cc-app__group-name">{g.dir}</span>
            <span className="cc-app__group-count">
              {g.items.length} session{g.items.length === 1 ? "" : "s"}
            </span>
          </header>
          <ul className="cc-app__rows">
            {g.items.map((s) => (
              <li key={`${s.session_id}-${s.work_dir_hash}`}>
                <button
                  type="button"
                  className="cc-app__row"
                  onClick={() => onSelect(`${s.work_dir_hash}/${s.session_id}`)}
                >
                  <span className="cc-app__row-id">{s.session_id.slice(0, 8)}</span>
                  <span className="cc-app__row-title">{s.title || "(untitled)"}</span>
                  <span className="cc-app__row-meta">
                    <span>{s.turns} turns</span>
                    {(s.subagent_count ?? 0) > 0 && (
                      <>
                        <span>·</span>
                        <span>{s.subagent_count} agents</span>
                      </>
                    )}
                    <span>·</span>
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
