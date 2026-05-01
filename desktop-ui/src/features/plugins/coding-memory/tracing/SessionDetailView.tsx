import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { DetailTab, Scope, SubagentSummary } from "./types";
import { useTracingSession } from "./useTracingSession";
import { useDetail } from "./useDetail";
import { chromeFor } from "./providers";
import { DetailHeader } from "./DetailHeader";
import { ScopeSelector } from "./ScopeSelector";
import { WireEventsTab } from "./WireEventsTab";
import { ContextMessagesTab } from "./ContextMessagesTab";
import { StateTab } from "./StateTab";
import { DualTab } from "./DualTab";
import { AgentsTab } from "./AgentsTab";

interface Props {
  providerId: string;
  sessionId: string;
  onBack: () => void;
}

export function SessionDetailView({ providerId, sessionId, onBack }: Props) {
  const [scope, setScope] = useState<Scope>({ kind: "main" });
  const [refreshKey, setRefreshKey] = useState(0);
  const [tab, setTab] = useState<DetailTab>("wireEvents");
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);

  const { detail, loading, error } = useTracingSession(providerId, sessionId, scope, refreshKey);
  const detailCtl = useDetail();

  useEffect(() => {
    let cancelled = false;
    invoke<SubagentSummary[]>("tracing_list_subagents", { providerId, sessionId })
      .then((s) => { if (!cancelled) setSubagents(s); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [providerId, sessionId, refreshKey]);

  const chrome = chromeFor(providerId);

  const handleOpenDir = useCallback(() => {
    invoke("tracing_open_dir", { providerId, sessionId });
  }, [providerId, sessionId]);

  const handleCopyDir = useCallback(async () => {
    const path: string = await invoke("tracing_get_dir", { providerId, sessionId });
    await navigator.clipboard.writeText(path);
  }, [providerId, sessionId]);

  const handleRefresh = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  const handleOpenSubagent = useCallback((agentId: string) => {
    setScope({ kind: "subagent", agentId });
  }, []);

  if (loading && !detail) return <div className="tracing-state">Loading session…</div>;
  if (error) return <div className="tracing-state tracing-state--error">{error}</div>;
  if (!detail) return null;

  return (
    <div className="tracing-detail">
      <DetailHeader
        providerDisplayName={chrome.displayName}
        sessionId={sessionId}
        stats={detail.stats}
        onBack={onBack}
        onOpenDir={handleOpenDir}
        onCopyDir={handleCopyDir}
        onDownload={() => { /* deferred — see "Out of scope for v1" */ }}
        onRefresh={handleRefresh}
      />
      <div className="tracing-tabs">
        {(["wireEvents", "contextMessages", "state", "dual", "agents"] as DetailTab[]).map((t) => (
          <button
            key={t}
            type="button"
            className={"tracing-tab" + (tab === t ? " tracing-tab--active" : "")}
            onClick={() => setTab(t)}
          >{labelFor(t)}</button>
        ))}
      </div>
      {tab === "wireEvents" && (
        <>
          <ScopeSelector scope={scope} onScopeChange={setScope} subagents={subagents} />
          <WireEventsTab
            events={detail.events}
            providerName={chrome.displayName}
            scope={scope}
            providerId={providerId}
            sessionId={sessionId}
            filterKind={detailCtl.filterKind}
            onFilterKind={detailCtl.setFilterKind}
            expandedSeq={detailCtl.expandedSeq}
            onToggleEvent={detailCtl.toggleEvent}
            selectedToolId={detailCtl.selectedToolId}
            onSelectTool={detailCtl.selectToolId}
            onOpenSubagent={handleOpenSubagent}
          />
        </>
      )}
      {tab === "contextMessages" && <ContextMessagesTab providerId={providerId} sessionId={sessionId} scope={scope} />}
      {tab === "state" && <StateTab providerId={providerId} sessionId={sessionId} />}
      {tab === "dual" && <DualTab providerId={providerId} sessionId={sessionId} scope={scope} />}
      {tab === "agents" && <AgentsTab providerName={chrome.displayName} scope={scope} providerId={providerId} sessionId={sessionId} />}
    </div>
  );
}

function labelFor(t: DetailTab): string {
  switch (t) {
    case "wireEvents": return "Wire Events";
    case "contextMessages": return "Context Messages";
    case "state": return "State";
    case "dual": return "Dual";
    case "agents": return "Agents";
  }
}
