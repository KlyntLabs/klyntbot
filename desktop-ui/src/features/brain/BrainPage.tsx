import { useQuery } from "@shared/hooks/useQuery";
import { Boxes, Brain, Crosshair, Eye } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router";
import { BrainCard } from "./components/BrainCard";
import { CoachingDetail } from "./components/CoachingDetail";
import { ContextsDetail } from "./components/ContextsDetail";
import { HealthStrip } from "./components/HealthStrip";
import { MemoryDetail } from "./components/MemoryDetail";
import { MirrorDetail } from "./components/MirrorDetail";

type BrainSection = "memory" | "coaching" | "mirror" | "contexts";

const DETAIL_RENDERERS: Record<BrainSection, () => ReactNode> = {
  memory: () => <MemoryDetail />,
  coaching: () => <CoachingDetail />,
  mirror: () => <MirrorDetail />,
  contexts: () => <ContextsDetail />,
};

interface MemoryStats {
  activeFacts: number;
  episodicCount: number;
  rulesCount: number;
}

interface MirrorState {
  latestTrendNarrative: { narrative: string } | null;
  latestBrainVersion: { version: number } | null;
  pendingMetaRules: unknown[];
  recentTrialPreviews: unknown[];
}

interface CoachingSituation {
  energyLevel?: number;
  focusState?: number;
  deadlinePressure?: number;
  distractionRisk?: number;
  coachingReceptivity?: number;
}

function MiniGauge({ label, value, color }: { label: string; value?: number; color: string }) {
  const pct = typeof value === "number" ? Math.round(value * 100) : 0;
  const hasValue = typeof value === "number";
  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative size-12">
        <svg className="size-12 -rotate-90" viewBox="0 0 36 36" aria-hidden="true">
          <circle
            className="text-white/[0.08]"
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
          />
          <circle
            className={color}
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
            strokeDasharray={`${pct} 100`}
            strokeLinecap="round"
          />
        </svg>
        <span className="absolute inset-0 flex items-center justify-center text-[10px] text-muted-foreground font-mono">
          {hasValue ? `${pct}%` : "—"}
        </span>
      </div>
      <span className="text-2xs text-dim text-center leading-tight">{label}</span>
    </div>
  );
}

interface InferenceStats {
  activeContextCount: number;
  archivedContextCount: number;
  assignmentRate: number;
}

export function BrainPage() {
  const navigate = useNavigate();
  const { section } = useParams<{ section?: string }>();
  const expanded = (section as BrainSection) || null;

  const setExpanded = (s: BrainSection | null) => {
    navigate(s ? `/brain/${s}` : "/brain", { replace: true });
  };

  const { data: memStats } = useQuery<MemoryStats>("cognitive_memory_stats", undefined, {
    activeFacts: 0,
    episodicCount: 0,
    rulesCount: 0,
  });

  const { data: mirrorState } = useQuery<MirrorState>("get_mirror_state", undefined, {
    latestTrendNarrative: null,
    latestBrainVersion: null,
    pendingMetaRules: [],
    recentTrialPreviews: [],
  });

  const { data: situation } = useQuery<CoachingSituation>("coaching_situation", undefined, {});

  const { data: inferenceStats } = useQuery<InferenceStats>("get_inference_stats", undefined, {
    activeContextCount: 0,
    archivedContextCount: 0,
    assignmentRate: 0,
  });

  const cards: {
    id: BrainSection;
    title: string;
    subtitle: string;
    icon: ReactNode;
    accentClass: string;
    summary: ReactNode;
  }[] = [
    {
      id: "memory",
      title: "Memory & Knowledge",
      subtitle: "User model, semantic facts, episodic memories",
      icon: <Brain className="size-4 text-success" strokeWidth={1.5} />,
      accentClass: "bg-success/15",
      summary: (
        <div className="flex gap-5 text-xs">
          <span>
            <span className="text-lg font-semibold text-success">{memStats.activeFacts}</span>{" "}
            <span className="text-muted-foreground">facts</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-info">{memStats.episodicCount}</span>{" "}
            <span className="text-muted-foreground">episodic</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-purple">{memStats.rulesCount}</span>{" "}
            <span className="text-muted-foreground">rules</span>
          </span>
        </div>
      ),
    },
    {
      id: "coaching",
      title: "Coaching & Patterns",
      subtitle: "Situation awareness, interventions, behavior patterns",
      icon: <Crosshair className="size-4 text-info" strokeWidth={1.5} />,
      accentClass: "bg-info/15",
      summary: (
        <div className="flex items-center gap-3">
          <MiniGauge label="Energy" value={situation.energyLevel} color="text-brand" />
          <MiniGauge label="Focus" value={situation.focusState} color="text-brand" />
          <MiniGauge label="Deadline" value={situation.deadlinePressure} color="text-destructive" />
          <MiniGauge label="Distraction" value={situation.distractionRisk} color="text-brand" />
          <MiniGauge label="Receptive" value={situation.coachingReceptivity} color="text-success" />
        </div>
      ),
    },
    {
      id: "mirror",
      title: "Mirror & Reflection",
      subtitle: "Weekly reflections, brain versions, skill routing",
      icon: <Eye className="size-4 text-purple" strokeWidth={1.5} />,
      accentClass: "bg-purple/15",
      summary: (
        <div>
          <div className="bg-surface-low rounded-lg p-3 mb-2">
            <p className="text-2xs text-dim mb-1">Latest Reflection</p>
            <p className="text-xs text-muted-foreground italic line-clamp-2">
              {mirrorState.latestTrendNarrative?.narrative ??
                "Your first weekly reflection will appear after 7 days of use."}
            </p>
          </div>
          <p className="text-2xs text-dim">
            Brain v{mirrorState.latestBrainVersion?.version ?? 1} ·{" "}
            {mirrorState.pendingMetaRules?.length ?? 0} meta-rules ·{" "}
            {mirrorState.recentTrialPreviews?.length ?? 0} trials
          </p>
        </div>
      ),
    },
    {
      id: "contexts",
      title: "Contexts & Inference",
      subtitle: "Work context detection, assignment, merging",
      icon: <Boxes className="size-4 text-warning" strokeWidth={1.5} />,
      accentClass: "bg-warning/15",
      summary: (
        <div className="flex gap-5 text-xs">
          <span>
            <span className="text-lg font-semibold text-warning">
              {inferenceStats.activeContextCount}
            </span>{" "}
            <span className="text-muted-foreground">active</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-dim">
              {inferenceStats.archivedContextCount}
            </span>{" "}
            <span className="text-muted-foreground">archived</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-info">
              {Math.round(inferenceStats.assignmentRate * 100)}%
            </span>{" "}
            <span className="text-muted-foreground">assignment</span>
          </span>
        </div>
      ),
    },
  ];

  const expandedCard = expanded ? cards.find((c) => c.id === expanded) : null;

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0 overflow-y-auto">
      <div className="flex flex-col gap-5 p-6 w-full">
        {!expanded && <HealthStrip />}

        {expandedCard ? (
          <BrainCard
            title={expandedCard.title}
            subtitle={expandedCard.subtitle}
            icon={expandedCard.icon}
            accentClass={expandedCard.accentClass}
            summary={expandedCard.summary}
            detail={DETAIL_RENDERERS[expandedCard.id]()}
            expanded={true}
            onExpand={() => {}}
            onCollapse={() => setExpanded(null)}
          />
        ) : (
          <div className="grid grid-cols-2 gap-4">
            {cards.map((card) => (
              <BrainCard
                key={card.id}
                title={card.title}
                subtitle={card.subtitle}
                icon={card.icon}
                accentClass={card.accentClass}
                summary={card.summary}
                detail={null}
                expanded={false}
                onExpand={() => setExpanded(card.id)}
                onCollapse={() => {}}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
