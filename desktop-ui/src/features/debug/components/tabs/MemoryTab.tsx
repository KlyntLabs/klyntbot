import { KnowledgeTrustWidget } from "@features/autotuner";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { Play, Plus, Trash2 } from "lucide-react";
import { useState } from "react";

interface UserModelSummary {
  identityCount: number;
  energyCount: number;
  workCount: number;
  financeCount: number;
  learningCount: number;
  preferencesCount: number;
  identityPreview: string[];
  energyPreview: string[];
  workPreview: string[];
  financePreview: string[];
  learningPreview: string[];
  preferencesPreview: string[];
}

interface SemanticFact {
  id: string;
  domain: string;
  subject: string;
  predicate: string;
  object: string;
  confidence: number;
  source: string;
  validFrom: string;
  validUntil: string | null;
  stability: number;
  retrievability: number;
  lastAccessed: string | null;
  accessCount: number;
  status: string;
}

interface EpisodicMemory {
  id: string;
  domain: string;
  content: string;
  summary: string | null;
  importance: number;
  occurredAt: string;
  recordedAt: string;
  stability: number;
  accessCount: number;
}

interface ProceduralRule {
  id: string;
  domain: string;
  ruleText: string;
  confidence: number;
  source: string;
  signalCount: number;
  active: boolean;
  createdAt: string;
  updatedAt: string;
}

interface MemoryStats {
  activeFacts: number;
  archivedFacts: number;
  episodicCount: number;
  rulesCount: number;
  lastCompaction: string | null;
}

interface CompactionResult {
  archivedCount: number;
  deletedEpisodic: number;
}

const DOMAINS = ["identity", "energy", "work", "finance", "learning", "preferences"] as const;

const domainColors: Record<string, string> = {
  identity: "bg-purple/20 text-purple",
  energy: "bg-status-warning/20 text-status-warning",
  work: "bg-status-info/20 text-status-info",
  finance: "bg-status-success/20 text-status-success",
  learning: "bg-brand/20 text-brand",
  preferences: "bg-purple/20 text-purple",
};

export function MemoryTab() {
  const [domainFilter, setDomainFilter] = useState<string | null>(null);

  const { data: model } = useQuery<UserModelSummary>("cognitive_user_model", undefined, {
    identityCount: 0,
    energyCount: 0,
    workCount: 0,
    financeCount: 0,
    learningCount: 0,
    preferencesCount: 0,
    identityPreview: [],
    energyPreview: [],
    workPreview: [],
    financePreview: [],
    learningPreview: [],
    preferencesPreview: [],
  });

  const { data: facts } = useQuery<SemanticFact[]>(
    "cognitive_facts_list",
    domainFilter ? { domain: domainFilter } : {},
    [],
  );

  const { data: episodic } = useQuery<EpisodicMemory[]>(
    "cognitive_episodic_list",
    domainFilter ? { domain: domainFilter, limit: 20 } : { limit: 20 },
    [],
  );

  const { data: rules } = useQuery<ProceduralRule[]>(
    "cognitive_rules_list",
    domainFilter ? { domain: domainFilter } : {},
    [],
  );

  const { data: stats } = useQuery<MemoryStats>("cognitive_memory_stats", undefined, {
    activeFacts: 0,
    archivedFacts: 0,
    episodicCount: 0,
    rulesCount: 0,
    lastCompaction: null,
  });

  const { mutate: runCompaction, loading: compacting } = useMutation<CompactionResult>(
    "cognitive_run_compaction",
  );
  const { mutate: runReflection, loading: reflecting } = useMutation<{
    factUpdates: number;
    ruleUpdates: number;
    summary: string;
  }>("cognitive_run_reflection");
  const { mutate: deleteFact } = useMutation<boolean>("cognitive_fact_delete");
  const { mutate: deactivateRule } = useMutation<boolean>("cognitive_rule_deactivate");

  const handleCompact = async () => {
    await runCompaction({} as never);
    invalidateQueries("cognitive_");
  };

  const handleReflection = async () => {
    await runReflection({} as never);
    invalidateQueries("cognitive_");
  };

  const handleDeleteFact = async (id: string) => {
    await deleteFact({ id } as never);
    invalidateQueries("cognitive_");
  };

  const domainCards = DOMAINS.map((d) => {
    const count = model[`${d}Count` as keyof UserModelSummary] as number;
    const preview = model[`${d}Preview` as keyof UserModelSummary] as string[];
    return { domain: d, count, preview };
  });

  return (
    <div className="space-y-6">
      <KnowledgeTrustWidget />

      {/* UserModel Summary Cards */}
      <div>
        <h2 className="text-ui font-medium text-fg-secondary mb-3">User Model</h2>
        <div className="grid grid-cols-3 gap-3">
          {domainCards.map(({ domain, count, preview }) => (
            <button
              key={domain}
              type="button"
              onClick={() => setDomainFilter(domainFilter === domain ? null : domain)}
              className={`text-left p-3 rounded-panel border transition-all ${
                domainFilter === domain
                  ? "border-brand/50 bg-brand/10"
                  : "border-separator bg-bg-elevated hover:bg-control-hover"
              }`}
            >
              <div className="flex items-center justify-between mb-1">
                <span className={`text-ui-xs px-1.5 py-0.5 rounded ${domainColors[domain]}`}>
                  {domain}
                </span>
                <span className="text-ui-xs text-fg-secondary">{count} facts</span>
              </div>
              <div className="space-y-0.5 mt-2">
                {preview.slice(0, 2).map((p) => (
                  <p key={p} className="text-ui-xs text-fg-secondary truncate">
                    {p}
                  </p>
                ))}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Semantic Facts Table */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-ui font-medium text-fg-secondary">
            Semantic Facts{" "}
            {domainFilter && <span className="text-fg-secondary">({domainFilter})</span>}
          </h2>
          <button
            type="button"
            className="flex items-center gap-1 text-ui-xs text-fg-secondary hover:text-fg"
          >
            <Plus className="size-3" /> Add Fact
          </button>
        </div>
        <div className="bg-bg-elevated rounded-panel border border-separator overflow-hidden">
          <table className="w-full text-ui-sm">
            <thead>
              <tr className="border-b border-separator">
                <th className="text-left p-2 text-fg-secondary font-normal">Domain</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Subject</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Predicate</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Object</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Conf</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Stab</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Retr</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Accessed</th>
                <th className="text-left p-2 text-fg-secondary font-normal" />
              </tr>
            </thead>
            <tbody>
              {facts.map((f) => (
                <tr
                  key={f.id}
                  className={`border-b border-separator hover:bg-control-hover ${
                    f.retrievability < 0.3 ? "opacity-40" : ""
                  }`}
                >
                  <td className="p-2">
                    <span
                      className={`text-ui-xs px-1 py-0.5 rounded ${domainColors[f.domain] ?? "text-fg-secondary"}`}
                    >
                      {f.domain}
                    </span>
                  </td>
                  <td className="p-2 text-fg-secondary">{f.subject}</td>
                  <td className="p-2 text-fg-secondary">{f.predicate}</td>
                  <td className="p-2 text-fg">{f.object}</td>
                  <td className="p-2">
                    <div className="w-12 bg-control-hover rounded-full h-1.5">
                      <div
                        className="bg-brand h-1.5 rounded-full"
                        style={{ width: `${f.confidence * 100}%` }}
                      />
                    </div>
                  </td>
                  <td className="p-2 text-fg-secondary">{f.stability.toFixed(1)}</td>
                  <td className="p-2 text-fg-secondary">
                    {(f.retrievability * 100).toFixed(0)}%
                  </td>
                  <td className="p-2 text-fg-secondary">{f.accessCount}x</td>
                  <td className="p-2">
                    <button
                      type="button"
                      onClick={() => handleDeleteFact(f.id)}
                      className="text-fg-secondary hover:text-status-danger"
                    >
                      <Trash2 className="size-3" />
                    </button>
                  </td>
                </tr>
              ))}
              {facts.length === 0 && (
                <tr>
                  <td colSpan={9} className="p-4 text-center text-fg-secondary">
                    No facts found
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Episodic Memories */}
      <div>
        <h2 className="text-ui font-medium text-fg-secondary mb-3">Episodic Memories</h2>
        <div className="space-y-2">
          {episodic.map((m) => (
            <div key={m.id} className="p-3 bg-bg-elevated rounded-panel border border-separator">
              <div className="flex items-center gap-2 mb-1">
                <span
                  className={`text-ui-xs px-1 py-0.5 rounded ${domainColors[m.domain] ?? "text-fg-secondary"}`}
                >
                  {m.domain}
                </span>
                <span className="text-ui-xs text-fg-secondary">{m.occurredAt}</span>
                <span className="text-ui-xs text-fg-secondary">
                  imp: {m.importance.toFixed(2)}
                </span>
              </div>
              <p className="text-ui-sm text-fg-secondary">{m.summary || m.content}</p>
              {m.summary && m.summary !== m.content && (
                <p className="text-ui-xs text-fg-secondary/60 mt-1">{m.content}</p>
              )}
            </div>
          ))}
          {episodic.length === 0 && (
            <p className="text-ui-sm text-fg-secondary">No episodic memories</p>
          )}
        </div>
      </div>

      {/* Procedural Rules */}
      <div>
        <h2 className="text-ui font-medium text-fg-secondary mb-3">Procedural Rules</h2>
        <div className="bg-bg-elevated rounded-panel border border-separator overflow-hidden">
          <table className="w-full text-ui-sm">
            <thead>
              <tr className="border-b border-separator">
                <th className="text-left p-2 text-fg-secondary font-normal">Domain</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Rule</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Conf</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Signals</th>
                <th className="text-left p-2 text-fg-secondary font-normal">Active</th>
                <th className="p-2" />
              </tr>
            </thead>
            <tbody>
              {rules.map((r) => (
                <tr key={r.id} className="border-b border-separator">
                  <td className="p-2">
                    <span
                      className={`text-ui-xs px-1 py-0.5 rounded ${domainColors[r.domain] ?? "text-fg-secondary"}`}
                    >
                      {r.domain}
                    </span>
                  </td>
                  <td className="p-2 text-fg-secondary">{r.ruleText}</td>
                  <td className="p-2 text-fg-secondary">{r.confidence.toFixed(2)}</td>
                  <td className="p-2 text-fg-secondary">{r.signalCount}</td>
                  <td className="p-2">
                    <span className={`text-ui-xs ${r.active ? "text-status-success" : "text-status-danger"}`}>
                      {r.active ? "ON" : "OFF"}
                    </span>
                  </td>
                  <td className="p-2">
                    {r.active && (
                      <button
                        type="button"
                        className="text-ui-xs text-status-danger/60 hover:text-status-danger"
                        onClick={async () => {
                          await deactivateRule({ id: r.id } as never);
                          invalidateQueries("cognitive_");
                        }}
                      >
                        <Trash2 className="size-3" />
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {rules.length === 0 && (
                <tr>
                  <td colSpan={6} className="p-4 text-center text-fg-secondary">
                    No rules
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="flex items-center gap-4 p-3 bg-bg-elevated rounded-panel border border-separator">
        <span className="text-ui-xs text-fg-secondary">
          Active: <span className="text-fg-secondary">{stats.activeFacts}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          Archived: <span className="text-fg-secondary">{stats.archivedFacts}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          Episodic: <span className="text-fg-secondary">{stats.episodicCount}</span>
        </span>
        <span className="text-ui-xs text-fg-secondary">
          Rules: <span className="text-fg-secondary">{stats.rulesCount}</span>
        </span>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleReflection}
          disabled={reflecting}
          className="flex items-center gap-1 text-ui-xs text-brand hover:text-brand/80 disabled:opacity-50"
        >
          <Play className="size-3" />
          {reflecting ? "Reflecting..." : "Run Reflection"}
        </button>
        <button
          type="button"
          onClick={handleCompact}
          disabled={compacting}
          className="flex items-center gap-1 text-ui-xs text-brand hover:text-brand/80 disabled:opacity-50"
        >
          <Play className="size-3" />
          {compacting ? "Running..." : "Run Compaction"}
        </button>
      </div>
    </div>
  );
}
