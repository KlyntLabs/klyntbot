import { ipc } from "@shared/hooks/useIpc";
import { retentionCssColor, retentionLabel } from "@shared/lib/retention";
import { BookOpen, ChevronDown, ChevronRight, Layers, Sparkles } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import type { TopicHealth } from "../hooks/useKnowledgeHealth";
import { useKnowledgeHealth } from "../hooks/useKnowledgeHealth";

interface AtomResponse {
  id: string;
  subject: string;
  atomType: string;
  retentionPct: number;
  personalImportance: number;
  salience: number;
  sourceContext: string | null;
  status: string;
}

interface TopicDetail {
  topic: TopicHealth;
  atoms: AtomResponse[];
}

interface TopicGroup {
  key: string;
  label: string;
  topics: TopicHealth[];
  totalAtoms: number;
  avgRetention: number;
}

function titleCase(s: string): string {
  return s
    .split("-")
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

function groupKey(domain: string): string {
  return domain.split(/[/:]/g)[0];
}

function topicLabel(domain: string): string {
  const parts = domain.split(/[/:]/g);
  if (parts.length <= 1) return titleCase(parts[0]);
  return parts
    .slice(1)
    .map((s) => titleCase(s))
    .join(" / ");
}

function groupTopics(topics: TopicHealth[]): TopicGroup[] {
  const map = new Map<string, TopicHealth[]>();
  for (const t of topics) {
    const key = groupKey(t.domain);
    const list = map.get(key) ?? [];
    list.push(t);
    map.set(key, list);
  }

  const groups: TopicGroup[] = [];
  for (const [key, items] of map) {
    const totalAtoms = items.reduce((s, t) => s + t.atomCount, 0);
    const avgRetention =
      totalAtoms > 0
        ? items.reduce((s, t) => s + t.avgRetention * t.atomCount, 0) / totalAtoms
        : 1.0;
    // Pre-sort topics by atom count so GroupCard doesn't need to sort during render
    items.sort((a, b) => b.atomCount - a.atomCount);
    groups.push({ key, label: titleCase(key), topics: items, totalAtoms, avgRetention });
  }

  groups.sort((a, b) => b.totalAtoms - a.totalAtoms);
  return groups;
}

function RetentionBar({ value }: { value: number }) {
  const pct = Math.round(value * 100);
  const color = retentionCssColor(value);
  return (
    <div className="flex items-center gap-1.5">
      <div className="w-12 h-1.5 rounded-full bg-white/[0.06] overflow-hidden">
        <div
          className="h-full rounded-full transition-all duration-500"
          style={{ width: `${pct}%`, backgroundColor: color }}
        />
      </div>
      <span className="text-[10px] tabular-nums w-7 text-right" style={{ color }}>
        {pct}%
      </span>
    </div>
  );
}

function AtomRow({ atom }: { atom: AtomResponse }) {
  const pct = Math.round(atom.retentionPct * 100);
  const color = retentionCssColor(atom.retentionPct);

  return (
    <div className="flex items-center gap-3 py-1.5">
      <div className="w-1.5 h-1.5 rounded-full shrink-0" style={{ backgroundColor: color }} />
      <span className="text-ui-sm text-fg truncate flex-1 min-w-0">{atom.subject}</span>
      <span className="text-[9px] text-fg-dim uppercase tracking-wider shrink-0">{atom.atomType}</span>
      <span className="text-ui-xs tabular-nums shrink-0 w-8 text-right" style={{ color }}>
        {pct}%
      </span>
    </div>
  );
}

function TopicRow({ topic, isOnlyChild }: { topic: TopicHealth; isOnlyChild: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const [atoms, setAtoms] = useState<AtomResponse[] | null>(null);
  const [loading, setLoading] = useState(false);

  // If it's the only topic in the group, show the group name; otherwise show child label
  const displayName = isOnlyChild ? titleCase(groupKey(topic.domain)) : topicLabel(topic.domain);

  const handleToggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (atoms) return;

    setLoading(true);
    try {
      const detail = await ipc<TopicDetail>("knowledge_topic_detail", {
        params: { topicId: topic.id },
      });
      setAtoms(detail.atoms);
    } catch {
      setAtoms([]);
    } finally {
      setLoading(false);
    }
  }, [expanded, atoms, topic.id]);

  return (
    <div>
      <button
        type="button"
        onClick={handleToggle}
        className="w-full flex items-center gap-2.5 px-3 py-2 text-left hover:bg-white/[0.02] rounded-lg transition-colors"
      >
        <ChevronRight
          size={12}
          strokeWidth={1.5}
          className={`text-fg-dim transition-transform duration-150 shrink-0 ${expanded ? "rotate-90" : ""}`}
        />
        <span className="text-ui-sm text-fg truncate flex-1 min-w-0">{displayName}</span>
        <span className="text-[10px] text-fg-dim tabular-nums shrink-0">{topic.atomCount}</span>
        <RetentionBar value={topic.avgRetention} />
      </button>

      {expanded && (
        <div className="ml-7 mr-2 mb-1 animate-[fade-in-up_0.12s_ease-out]">
          {loading ? (
            <div className="flex items-center gap-2 py-2 px-3">
              <div className="w-3 h-3 border border-dim border-t-foreground rounded-full animate-spin" />
              <span className="text-ui-xs text-fg-dim">Loading…</span>
            </div>
          ) : atoms && atoms.length > 0 ? (
            <div className="border-l border-white/[0.06] pl-3 divide-y divide-white/[0.03]">
              {atoms.map((a) => (
                <AtomRow key={a.id} atom={a} />
              ))}
            </div>
          ) : (
            <p className="text-ui-xs text-fg-dim px-3 py-2">No atoms</p>
          )}
        </div>
      )}
    </div>
  );
}

function GroupCard({ group }: { group: TopicGroup }) {
  const [expanded, setExpanded] = useState(group.totalAtoms >= 10);
  const color = retentionCssColor(group.avgRetention);
  const pct = Math.round(group.avgRetention * 100);
  const hasSubTopics = group.topics.length > 1 || group.topics[0].domain !== group.key;

  return (
    <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] overflow-hidden transition-colors hover:border-white/[0.10]">
      {/* Group header */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-3.5 py-3"
      >
        <div className="flex items-center gap-3">
          {/* Retention indicator */}
          <div
            className="w-2 h-8 rounded-full shrink-0"
            style={{ backgroundColor: color, opacity: 0.6 }}
          />
          <div className="flex-1 min-w-0">
            <div className="text-ui font-medium text-fg truncate">{group.label}</div>
            <div className="flex items-center gap-1.5 mt-0.5">
              <span className="text-[10px] text-fg-dim tabular-nums">{group.totalAtoms} atoms</span>
              {hasSubTopics && (
                <>
                  <span className="text-fg-dim">·</span>
                  <span className="text-[10px] text-fg-dim tabular-nums">
                    {group.topics.length} topics
                  </span>
                </>
              )}
              <span className="text-fg-dim">·</span>
              <span className="text-[10px] tabular-nums" style={{ color }}>
                {pct}% {retentionLabel(group.avgRetention).toLowerCase()}
              </span>
            </div>
          </div>
          <ChevronDown
            size={14}
            strokeWidth={1.5}
            className={`text-fg-dim transition-transform duration-200 shrink-0 ${expanded ? "rotate-180" : ""}`}
          />
        </div>
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="border-t border-white/[0.06] py-1 animate-[fade-in-up_0.12s_ease-out]">
          {hasSubTopics ? (
            group.topics.map((t) => <TopicRow key={t.id} topic={t} isOnlyChild={false} />)
          ) : (
            <TopicRow topic={group.topics[0]} isOnlyChild />
          )}
        </div>
      )}
    </div>
  );
}

export function AtomGraph() {
  const { data: health } = useKnowledgeHealth();
  const groups = useMemo(() => groupTopics(health.topics), [health.topics]);

  if (health.topics.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 gap-3">
        <div className="p-2.5 rounded-xl bg-white/[0.04]">
          <Layers size={20} className="text-fg-dim" strokeWidth={1.5} />
        </div>
        <div className="text-center">
          <p className="text-ui-sm text-fg-secondary">No knowledge topics yet</p>
          <p className="text-ui-xs text-fg-dim mt-0.5">
            Knowledge atoms are extracted from your notes automatically.
          </p>
        </div>
      </div>
    );
  }

  const totalAtoms = health.topics.reduce((s, t) => s + t.atomCount, 0);
  const avgRet = Math.round(health.avgRetention * 100);

  return (
    <div className="space-y-3">
      {/* Summary stats */}
      <div className="flex items-center gap-4 px-1">
        <div className="flex items-center gap-1.5">
          <Sparkles size={12} className="text-brand" strokeWidth={1.5} />
          <span className="text-ui-xs text-fg-secondary">
            <span className="text-fg font-medium tabular-nums">{totalAtoms}</span> atoms in{" "}
            <span className="text-fg font-medium tabular-nums">{groups.length}</span>{" "}
            {groups.length === 1 ? "domain" : "domains"}
          </span>
        </div>
        <span className="text-fg-dim">·</span>
        <div className="flex items-center gap-1.5">
          <BookOpen size={12} className="text-status-success" strokeWidth={1.5} />
          <span className="text-ui-xs text-fg-secondary">
            <span className="text-fg font-medium tabular-nums">{avgRet}%</span> avg
            retention
          </span>
        </div>
      </div>

      {/* Grouped topic cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {groups.map((g) => (
          <GroupCard key={g.key} group={g} />
        ))}
      </div>
    </div>
  );
}
