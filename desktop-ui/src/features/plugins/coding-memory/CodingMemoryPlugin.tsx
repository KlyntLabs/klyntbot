import { useEffect, useMemo, useRef, useState } from "react";
import { listCodingSessions } from "@/api/endpoints/codingMemory";
import { ProviderChips } from "./ProviderChips";
import { ReforgeCycleDiff } from "./ReforgeCycleDiff";
import { SessionList } from "./SessionList";
import { TracingTab } from "./tracing/TracingTab";
import type { ProviderId, SessionSummaryDto } from "./types";
import { WireViewer } from "./WireViewer";

const POLL_INTERVAL_MS = 4000;

export function CodingMemoryPlugin() {
  const [provider, setProvider] = useState<ProviderId | "all">("all");
  const [sessions, setSessions] = useState<SessionSummaryDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [secondaryTab, setSecondaryTab] = useState<"sessions" | "reforge" | "tracing">("sessions");
  const [live, setLive] = useState(true);
  const [lastSyncedAt, setLastSyncedAt] = useState<number | null>(null);
  const initialLoadRef = useRef(true);

  // Initial + filter-change load (shows skeleton). Re-runs only when provider changes.
  useEffect(() => {
    let cancelled = false;
    initialLoadRef.current = true;
    setLoading(true);
    listCodingSessions({
      source: provider === "all" ? null : provider,
      repoId: null,
      sinceDays: 14,
      limit: 100,
      offset: 0,
    })
      .then((rows) => {
        if (cancelled) return;
        setSessions(rows);
        setSelectedId((prev) => prev ?? rows[0]?.sessionId ?? null);
        setLastSyncedAt(Date.now());
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
          initialLoadRef.current = false;
        }
      });
    return () => {
      cancelled = true;
    };
  }, [provider]);

  // Background poll — silent refresh, preserves selection + scroll.
  useEffect(() => {
    if (!live) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const rows = await listCodingSessions({
          source: provider === "all" ? null : provider,
          repoId: null,
          sinceDays: 14,
          limit: 100,
          offset: 0,
        });
        if (cancelled) return;
        setSessions(rows);
        setSelectedId((prev) => prev ?? rows[0]?.sessionId ?? null);
        setLastSyncedAt(Date.now());
        setRefreshKey((k) => k + 1);
      } catch {
        // swallow — keep last good rows
      }
    };
    const id = window.setInterval(tick, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [live, provider]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: sessions.length };
    for (const s of sessions) c[s.source] = (c[s.source] ?? 0) + 1;
    return c;
  }, [sessions]);

  const lastSyncedLabel = lastSyncedAt ? new Date(lastSyncedAt).toLocaleTimeString() : "—";

  return (
    <div className="cm-plugin">
      <div className="cm-plugin__live-row">
        <ProviderChips active={provider} onChange={setProvider} counts={counts} />
        <button
          type="button"
          className={"cm-plugin__live-toggle" + (live ? " cm-plugin__live-toggle--on" : "")}
          onClick={() => setLive((v) => !v)}
          title={live ? "Click to pause auto-refresh" : "Click to resume auto-refresh"}
        >
          <span className="cm-plugin__live-dot" aria-hidden="true" />
          {live ? "Live" : "Paused"} · synced {lastSyncedLabel}
        </button>
      </div>
      <div className="cm-plugin__secondary-tabs">
        <button
          type="button"
          className={
            "cm-plugin__sec-tab" +
            (secondaryTab === "sessions" ? " cm-plugin__sec-tab--active" : "")
          }
          onClick={() => setSecondaryTab("sessions")}
        >
          Sessions
        </button>
        <button
          type="button"
          className={
            "cm-plugin__sec-tab" + (secondaryTab === "reforge" ? " cm-plugin__sec-tab--active" : "")
          }
          onClick={() => setSecondaryTab("reforge")}
        >
          Reforge
        </button>
        <button
          type="button"
          className={
            "cm-plugin__sec-tab" + (secondaryTab === "tracing" ? " cm-plugin__sec-tab--active" : "")
          }
          onClick={() => setSecondaryTab("tracing")}
        >
          Tracing
        </button>
      </div>
      <div className="cm-plugin__body">
        {secondaryTab === "sessions" ? (
          <>
            <aside className="cm-plugin__sidebar">
              <SessionList
                sessions={sessions}
                selectedId={selectedId}
                onSelect={setSelectedId}
                loading={loading}
              />
            </aside>
            <main className="cm-plugin__main">
              {selectedId ? (
                <WireViewer sessionId={selectedId} refreshKey={refreshKey} />
              ) : (
                <div className="cm-state cm-state--empty">Select a session to inspect.</div>
              )}
            </main>
          </>
        ) : secondaryTab === "reforge" ? (
          <main className="cm-plugin__main">
            <ReforgeCycleDiff />
          </main>
        ) : (
          <main className="cm-plugin__main">
            <TracingTab />
          </main>
        )}
      </div>
    </div>
  );
}
