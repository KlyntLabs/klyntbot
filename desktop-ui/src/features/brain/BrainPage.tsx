import { useQuery } from "@shared/hooks/useQuery";
import { Boxes, Brain, Crosshair, Eye } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router";
import { ActivityStream } from "./components/ActivityStream";
import { BrainCard } from "./components/BrainCard";
import { CoachingDetail } from "./components/CoachingDetail";
import { ContextsDetail } from "./components/ContextsDetail";
import { HealthStrip } from "./components/HealthStrip";
import { MemoryDetail } from "./components/MemoryDetail";
import { MirrorDetail } from "./components/MirrorDetail";

type BrainSection = "memory" | "coaching" | "mirror" | "contexts";

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
  coachingReceptivity?: number;
}

interface InferenceStats {
  activeContexts: number;
  archivedContexts: number;
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
    activeContexts: 0,
    archivedContexts: 0,
    assignmentRate: 0,
  });

  const DETAIL_RENDERERS: Record<BrainSection, () => ReactNode> = {
    memory: () => <MemoryDetail />,
    coaching: () => <CoachingDetail />,
    mirror: () => <MirrorDetail />,
    contexts: () => <ContextsDetail />,
  };

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
        <div className="flex items-center gap-4">
          {[
            { label: "Energy", value: situation.energyLevel },
            { label: "Focus", value: situation.focusState },
            { label: "Deadline", value: situation.deadlinePressure },
            { label: "Receptive", value: situation.coachingReceptivity },
          ].map((g) => (
            <div key={g.label} className="text-center">
              <div className="size-9 rounded-full border-2 border-info/40 flex items-center justify-center text-2xs font-semibold text-info">
                {g.value ?? "—"}
              </div>
              <p className="text-2xs text-dim mt-1">{g.label}</p>
            </div>
          ))}
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
              {inferenceStats.activeContexts}
            </span>{" "}
            <span className="text-muted-foreground">active</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-dim">
              {inferenceStats.archivedContexts}
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
      <div className="flex flex-col gap-5 p-6 max-w-4xl w-full mx-auto">
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

        {!expanded && <ActivityStream />}
      </div>
    </div>
  );
}
