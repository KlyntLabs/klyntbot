import { retentionBarColor, retentionTextColor } from "@shared/lib/retention";
import { Activity, ArrowLeft, Brain, Play } from "lucide-react";
import { lazy, Suspense, useState } from "react";
import { Link } from "react-router";
import type { TopicHealth } from "../hooks/useKnowledgeHealth";
import { useKnowledgeHealth } from "../hooks/useKnowledgeHealth";
import { useRetentionHistory } from "../hooks/useRetentionHistory";
import { AtomGraph } from "./AtomGraph";

const RetentionChart = lazy(() =>
  import("./RetentionChart").then((m) => ({ default: m.RetentionChart })),
);

function TopicRow({ topic }: { topic: TopicHealth }) {
  const pct = Math.round(topic.avgRetention * 100);

  return (
    <div className="flex items-center gap-3 py-2">
      {/* Name + domain */}
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-primary truncate">{topic.name}</p>
        <p className="text-2xs text-muted truncate">{topic.domain}</p>
      </div>

      {/* Atom count */}
      <span className="text-2xs text-muted-foreground tabular-nums shrink-0">
        {topic.atomCount} atom{topic.atomCount !== 1 ? "s" : ""}
      </span>

      {/* Retention bar */}
      <div className="w-24 shrink-0">
        <div className="h-2 rounded-full bg-white/5 overflow-hidden">
          <div
            className={`h-full rounded-full transition-all duration-500 ${retentionBarColor(topic.avgRetention)}`}
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      {/* Retention % */}
      <span
        className={`text-xs font-medium tabular-nums w-10 text-right shrink-0 ${retentionTextColor(topic.avgRetention)}`}
      >
        {pct}%
      </span>

      {/* Review button */}
      <Link
        to={`/learn/review/${topic.id}`}
        className="flex items-center gap-1 px-2 py-1 rounded-md text-2xs font-medium text-purple-400 hover:bg-purple-500/15 transition-colors shrink-0"
      >
        <Play size={10} strokeWidth={1.5} />
        Review
      </Link>
    </div>
  );
}

type Tab = "topics" | "trends" | "graph";

function TrendsTab() {
  const [days, setDays] = useState<30 | 90>(30);
  const { data: retentionData } = useRetentionHistory(days);
  return (
    <div className="glass-card rounded-xl p-5">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          Retention Trends
        </h2>
        <div className="flex items-center gap-1">
          {([30, 90] as const).map((d) => (
            <button
              type="button"
              key={d}
              onClick={() => setDays(d)}
              className={`px-2 py-0.5 text-2xs rounded-md transition-colors ${
                days === d
                  ? "bg-brand/20 text-brand font-medium"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {d}d
            </button>
          ))}
        </div>
      </div>
      <Suspense
        fallback={
          <div className="flex items-center justify-center h-full text-muted text-sm">
            Loading...
          </div>
        }
      >
        <RetentionChart data={retentionData.overall} />
      </Suspense>
    </div>
  );
}

export function KnowledgeHealth() {
  const { data: health, loading } = useKnowledgeHealth();
  const [activeTab, setActiveTab] = useState<Tab>("topics");

  const avgPct = Math.round(health.avgRetention * 100);
  const isEmpty = health.topics.length === 0 && !loading;

  return (
    <div className="flex-1 p-6 space-y-5 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Link
          to="/learn"
          className="p-1 rounded-md text-muted-foreground hover:text-primary hover:bg-surface-hover transition-colors"
        >
          <ArrowLeft size={18} strokeWidth={1.5} />
        </Link>
        <Activity size={20} className="text-brand" strokeWidth={1.5} />
        <h1 className="text-lg font-semibold text-foreground">Knowledge Health</h1>
      </div>

      {/* Summary stats */}
      <div className="grid grid-cols-3 gap-3">
        <div className="glass-card rounded-xl p-4 text-center">
          <p className="text-2xl font-bold text-foreground tabular-nums">{health.totalAtoms}</p>
          <p className="text-[11px] text-muted-foreground mt-0.5">Total Atoms</p>
        </div>
        <div className="glass-card rounded-xl p-4 text-center">
          <p className="text-2xl font-bold text-foreground tabular-nums">{health.activeAtoms}</p>
          <p className="text-[11px] text-muted-foreground mt-0.5">Active</p>
        </div>
        <div className="glass-card rounded-xl p-4 text-center">
          <p
            className={`text-2xl font-bold tabular-nums ${retentionTextColor(health.avgRetention)}`}
          >
            {avgPct}%
          </p>
          <p className="text-[11px] text-muted-foreground mt-0.5">Avg Retention</p>
        </div>
      </div>

      {/* Tab bar */}
      <div className="flex items-center gap-1 border-b border-border/30 pb-0">
        {(["topics", "trends", "graph"] as const).map((tab) => (
          <button
            type="button"
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`px-3 py-2 text-xs font-medium capitalize transition-colors border-b-2 -mb-px ${
              activeTab === tab
                ? "border-brand text-brand"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === "topics" &&
        (isEmpty ? (
          <div className="glass-card rounded-xl p-8 text-center">
            <Brain size={32} className="mx-auto text-muted-foreground mb-3" strokeWidth={1.5} />
            <p className="text-sm text-muted-foreground">
              No knowledge atoms yet. Accept suggested atoms from your notes to start tracking
              retention.
            </p>
          </div>
        ) : (
          <div className="glass-card rounded-xl p-5">
            <h2 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">
              Topics ({health.topics.length})
            </h2>
            <div className="divide-y divide-border/30">
              {health.topics.map((topic) => (
                <TopicRow key={topic.id} topic={topic} />
              ))}
            </div>
          </div>
        ))}

      {activeTab === "trends" && <TrendsTab />}

      {activeTab === "graph" && (
        <div className="glass-card rounded-xl p-5">
          <h2 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">
            Knowledge Graph
          </h2>
          <AtomGraph />
        </div>
      )}
    </div>
  );
}
