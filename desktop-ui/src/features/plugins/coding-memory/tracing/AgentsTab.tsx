import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SubagentSummary } from "./types";

interface Props {
  providerName: string;
  scope: { kind: string };
  providerId?: string;
  sessionId?: string;
}

export function AgentsTab({
  providerName: _providerName,
  scope: _scope,
  providerId,
  sessionId,
}: Props) {
  const [agents, setAgents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!providerId || !sessionId) {
      setAgents([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    invoke<SubagentSummary[]>("tracing_list_subagents", { providerId, sessionId })
      .then((rows) => {
        if (!cancelled) setAgents(rows);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [providerId, sessionId]);

  if (loading) return <div className="tracing-state">Loading agents…</div>;

  return (
    <div className="tracing-agents">
      {agents.length === 0 && <div className="tracing-state">No subagents.</div>}
      {agents.map((a) => (
        <div key={a.agentId} className="tracing-agents__card">
          <div className="tracing-agents__id">{a.agentId}</div>
          <div className="tracing-agents__type">{a.subagentType}</div>
          <div className="tracing-agents__desc">{a.description}</div>
          <div className="tracing-agents__stats">
            {a.eventCount} events · {a.status}
          </div>
        </div>
      ))}
    </div>
  );
}
