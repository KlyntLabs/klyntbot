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
  energy: "bg-warning/20 text-warning",
  work: "bg-info/20 text-info",
  finance: "bg-success/20 text-success",
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
      {/* UserModel Summary Cards */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">User Model</h2>
        <div className="grid grid-cols-3 gap-3">
          {domainCards.map(({ domain, count, preview }) => (
            <button
              key={domain}
              type="button"
              onClick={() => setDomainFilter(domainFilter === domain ? null : domain)}
              className={`text-left p-3 rounded-lg border transition-all ${
                domainFilter === domain
                  ? "border-brand/50 bg-brand/10"
                  : "border-border bg-surface-low hover:bg-surface-base"
              }`}
            >
              <div className="flex items-center justify-between mb-1">
                <span className={`text-[11px] px-1.5 py-0.5 rounded ${domainColors[domain]}`}>
                  {domain}
                </span>
                <span className="text-[11px] text-muted">{count} facts</span>
              </div>
              <div className="space-y-0.5 mt-2">
                {preview.slice(0, 2).map((p) => (
                  <p key={p} className="text-[11px] text-muted truncate">
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
          <h2 className="text-[13px] font-medium text-secondary">
            Semantic Facts {domainFilter && <span className="text-muted">({domainFilter})</span>}
          </h2>
          <button
            type="button"
            className="flex items-center gap-1 text-[11px] text-muted hover:text-secondary"
          >
            <Plus className="w-3 h-3" /> Add Fact
          </button>
        </div>
        <div className="bg-surface-low rounded-lg border border-border overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-border-subtle">
                <th className="text-left p-2 text-muted font-normal">Domain</th>
                <th className="text-left p-2 text-muted font-normal">Subject</th>
                <th className="text-left p-2 text-muted font-normal">Predicate</th>
                <th className="text-left p-2 text-muted font-normal">Object</th>
                <th className="text-left p-2 text-muted font-normal">Conf</th>
                <th className="text-left p-2 text-muted font-normal">Stab</th>
                <th className="text-left p-2 text-muted font-normal">Retr</th>
                <th className="text-left p-2 text-muted font-normal">Accessed</th>
                <th className="text-left p-2 text-muted font-normal" />
              </tr>
            </thead>
            <tbody>
              {facts.map((f) => (
                <tr
                  key={f.id}
                  className={`border-b border-border-subtle hover:bg-surface-lowest ${
                    f.retrievability < 0.3 ? "opacity-40" : ""
                  }`}
                >
                  <td className="p-2">
                    <span
                      className={`text-[10px] px-1 py-0.5 rounded ${domainColors[f.domain] ?? "text-muted"}`}
                    >
                      {f.domain}
                    </span>
                  </td>
                  <td className="p-2 text-secondary">{f.subject}</td>
                  <td className="p-2 text-secondary">{f.predicate}</td>
                  <td className="p-2 text-primary">{f.object}</td>
                  <td className="p-2">
                    <div className="w-12 bg-surface-raised rounded-full h-1.5">
                      <div
                        className="bg-brand h-1.5 rounded-full"
                        style={{ width: `${f.confidence * 100}%` }}
                      />
                    </div>
                  </td>
                  <td className="p-2 text-muted">{f.stability.toFixed(1)}</td>
                  <td className="p-2 text-muted">{(f.retrievability * 100).toFixed(0)}%</td>
                  <td className="p-2 text-muted">{f.accessCount}x</td>
                  <td className="p-2">
                    <button
                      type="button"
                      onClick={() => handleDeleteFact(f.id)}
                      className="text-muted hover:text-destructive"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </td>
                </tr>
              ))}
              {facts.length === 0 && (
                <tr>
                  <td colSpan={9} className="p-4 text-center text-muted">
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
        <h2 className="text-[13px] font-medium text-secondary mb-3">Episodic Memories</h2>
        <div className="space-y-2">
          {episodic.map((m) => (
            <div key={m.id} className="p-3 bg-surface-low rounded-lg border border-border">
              <div className="flex items-center gap-2 mb-1">
                <span
                  className={`text-[10px] px-1 py-0.5 rounded ${domainColors[m.domain] ?? "text-muted"}`}
                >
                  {m.domain}
                </span>
                <span className="text-[10px] text-muted">{m.occurredAt}</span>
                <span className="text-[10px] text-muted">imp: {m.importance.toFixed(2)}</span>
              </div>
              <p className="text-[12px] text-secondary">{m.content}</p>
              {m.summary && <p className="text-[11px] text-muted mt-1">{m.summary}</p>}
            </div>
          ))}
          {episodic.length === 0 && <p className="text-[12px] text-muted">No episodic memories</p>}
        </div>
      </div>

      {/* Procedural Rules */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Procedural Rules</h2>
        <div className="bg-surface-low rounded-lg border border-border overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-border-subtle">
                <th className="text-left p-2 text-muted font-normal">Domain</th>
                <th className="text-left p-2 text-muted font-normal">Rule</th>
                <th className="text-left p-2 text-muted font-normal">Conf</th>
                <th className="text-left p-2 text-muted font-normal">Signals</th>
                <th className="text-left p-2 text-muted font-normal">Active</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((r) => (
                <tr key={r.id} className="border-b border-border-subtle">
                  <td className="p-2">
                    <span
                      className={`text-[10px] px-1 py-0.5 rounded ${domainColors[r.domain] ?? "text-muted"}`}
                    >
                      {r.domain}
                    </span>
                  </td>
                  <td className="p-2 text-secondary">{r.ruleText}</td>
                  <td className="p-2 text-muted">{r.confidence.toFixed(2)}</td>
                  <td className="p-2 text-muted">{r.signalCount}</td>
                  <td className="p-2">
                    <span
                      className={`text-[10px] ${r.active ? "text-success" : "text-destructive"}`}
                    >
                      {r.active ? "ON" : "OFF"}
                    </span>
                  </td>
                </tr>
              ))}
              {rules.length === 0 && (
                <tr>
                  <td colSpan={5} className="p-4 text-center text-muted">
                    No rules
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="flex items-center gap-4 p-3 bg-surface-low rounded-lg border border-border">
        <span className="text-[11px] text-muted">
          Active: <span className="text-secondary">{stats.activeFacts}</span>
        </span>
        <span className="text-[11px] text-muted">
          Archived: <span className="text-secondary">{stats.archivedFacts}</span>
        </span>
        <span className="text-[11px] text-muted">
          Episodic: <span className="text-secondary">{stats.episodicCount}</span>
        </span>
        <span className="text-[11px] text-muted">
          Rules: <span className="text-secondary">{stats.rulesCount}</span>
        </span>
        <div className="flex-1" />
        <button
          type="button"
          onClick={handleReflection}
          disabled={reflecting}
          className="flex items-center gap-1 text-[11px] text-brand hover:text-brand/80 disabled:opacity-50"
        >
          <Play className="w-3 h-3" />
          {reflecting ? "Reflecting..." : "Run Reflection"}
        </button>
        <button
          type="button"
          onClick={handleCompact}
          disabled={compacting}
          className="flex items-center gap-1 text-[11px] text-brand hover:text-brand/80 disabled:opacity-50"
        >
          <Play className="w-3 h-3" />
          {compacting ? "Running..." : "Run Compaction"}
        </button>
      </div>
    </div>
  );
}
