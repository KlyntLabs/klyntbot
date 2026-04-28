import { useEffect, useMemo, useState } from "react";
import { ProviderChips } from "./ProviderChips";
import { SessionList } from "./SessionList";
import { WireViewer } from "./WireViewer";
import { listCodingSessions } from "@/api/endpoints/codingMemory";
import type { ProviderId, SessionSummaryDto } from "./types";

export function CodingMemoryPlugin() {
  const [provider, setProvider] = useState<ProviderId | "all">("all");
  const [sessions, setSessions] = useState<SessionSummaryDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listCodingSessions({
      source: provider === "all" ? undefined : provider,
      sinceDays: 14,
      limit: 100,
      offset: 0,
    })
      .then((rows) => {
        if (cancelled) return;
        setSessions(rows);
        setSelectedId((prev) => prev ?? rows[0]?.sessionId ?? null);
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [provider, refreshKey]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: sessions.length };
    for (const s of sessions) c[s.source] = (c[s.source] ?? 0) + 1;
    return c;
  }, [sessions]);

  return (
    <div className="cm-plugin">
      <ProviderChips active={provider} onChange={setProvider} counts={counts} />
      <div className="cm-plugin__body">
        <aside className="cm-plugin__sidebar">
          <SessionList sessions={sessions} selectedId={selectedId} onSelect={setSelectedId} loading={loading} />
        </aside>
        <main className="cm-plugin__main">
          {selectedId
            ? <WireViewer sessionId={selectedId} refreshKey={refreshKey} />
            : <div className="cm-state cm-state--empty">Select a session to inspect.</div>}
        </main>
      </div>
    </div>
  );
}
