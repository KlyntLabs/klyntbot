import { MessageSquare } from "lucide-react";
import { useMemo } from "react";
import type { InsightReviewState, PersonaMeta, TabStatus } from "../../hooks/useInsightReview";
import { PersonaCard } from "./PersonaCard";

type DebateState = InsightReviewState["tabs"]["perspectives"]["debate"];

interface PerspectivesTabProps {
  status: TabStatus;
  content: string;
  personas: PersonaMeta[];
  personaPerspectives?: Record<string, string>;
  noteId?: string | null;
  squadId?: string | null;
  onSquadChange?: (squadId: string) => void;
  debate?: DebateState;
  onStartDebate?: () => void;
}

/** Parse the perspectives markdown into per-persona sections by splitting on `---` and `## ` headings. */
function parsePersonaSections(
  content: string,
  personas: PersonaMeta[],
): { persona: PersonaMeta; section: string }[] {
  if (!content || personas.length === 0) return [];
  const sections = content.split(/\n---\n/).filter((s) => s.trim().length > 0);
  return personas.map((persona, i) => ({
    persona,
    section: sections[i]?.trim() ?? "",
  }));
}

function SkeletonLoader() {
  return (
    <div className="space-y-4">
      {[1, 2, 3].map((i) => (
        <div key={i} className="glass-card rounded-lg p-3 space-y-2 animate-pulse">
          <div className="flex items-center gap-2">
            <div className="size-7 rounded-full bg-card" />
            <div className="space-y-1">
              <div className="h-3 bg-card rounded w-24" />
              <div className="h-2 bg-card rounded w-16" />
            </div>
          </div>
          <div className="h-3 bg-card rounded w-full" />
          <div className="h-3 bg-card rounded w-4/5" />
          <div className="h-3 bg-card rounded w-3/4" />
        </div>
      ))}
    </div>
  );
}

function DebateRoundView({ round }: { round: DebateState["rounds"][number] }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <span className="text-2xs font-medium text-muted-foreground">Round {round.round}</span>
        <span className="text-[9px] text-dim capitalize">({round.phase})</span>
      </div>
      <div className="space-y-2">
        {round.personas.map((p) => (
          <div key={`${p.personaId}-${round.round}`} className="glass-card rounded-lg p-3">
            <div className="flex items-center gap-2 mb-1.5">
              <span className="text-sm">{p.personaIcon}</span>
              <div>
                <span className="text-[11px] font-medium text-foreground">{p.personaName}</span>
                <span className="text-[9px] text-dim ml-1.5">{p.personaRole}</span>
              </div>
            </div>
            {p.challenge && (
              <div className="text-2xs text-purple-400/80 italic mb-1.5 pl-2 border-l-2 border-purple-400/30">
                {p.challenge}
              </div>
            )}
            <div className="text-[11px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
              {p.content}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function JudgeDecisionView({ decision }: { decision: DebateState["judgeDecisions"][number] }) {
  const dotColor =
    decision.consensusScore > 85
      ? "bg-green-400"
      : decision.consensusScore > 60
        ? "bg-yellow-400"
        : "bg-red-400";

  return (
    <div className="glass-panel rounded-lg px-3 py-2 flex items-start gap-2 text-2xs">
      <div className={`size-2 rounded-full mt-0.5 shrink-0 ${dotColor}`} />
      <div className="flex-1 min-w-0">
        <p className="text-dim italic">{decision.reasoning}</p>
        <p className="text-muted-foreground mt-0.5">
          Consensus: {Math.round(decision.consensusScore)}%
        </p>
      </div>
    </div>
  );
}

function DebateView({ debate }: { debate: DebateState }) {
  const judgeByRound = useMemo(
    () => new Map(debate.judgeDecisions.map((j) => [j.round, j])),
    [debate.judgeDecisions],
  );

  return (
    <div className="space-y-3 border border-border/30 rounded-xl p-3 bg-white/[0.02]">
      <div className="flex items-center justify-between">
        <span className="text-2xs font-medium text-muted-foreground uppercase tracking-wider">
          Squad Debate
        </span>
        {debate.consensusReached && (
          <span className="text-2xs text-green-400">Consensus reached</span>
        )}
      </div>
      {debate.rounds.map((round) => {
        const judge = judgeByRound.get(round.round);
        return (
          <div key={round.round} className="space-y-2">
            <DebateRoundView round={round} />
            {judge && <JudgeDecisionView decision={judge} />}
          </div>
        );
      })}
      {debate.consensusSummary && (
        <div className="text-[11px] text-green-400/80 italic border-t border-border/20 pt-2">
          {debate.consensusSummary}
        </div>
      )}
    </div>
  );
}

function FallbackSections({
  content,
  personas,
  noteId,
}: {
  content: string;
  personas: PersonaMeta[];
  noteId?: string | null;
}) {
  const sections = parsePersonaSections(content, personas);
  if (sections.length === 0 && content) {
    return (
      <div className="space-y-4">
        <div className="text-2xs text-dim italic">Perspectives (persona details unavailable)</div>
        <div className="text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    );
  }
  return (
    <div className="grid grid-cols-2 gap-3">
      {sections.map(
        ({ persona, section }) =>
          section && (
            <PersonaCard
              key={persona.id}
              name={persona.name}
              role={persona.role}
              icon={persona.icon}
              tone={persona.tone}
              content={section}
              noteId={noteId ?? undefined}
              personaId={persona.id}
            />
          ),
      )}
    </div>
  );
}

export function PerspectivesTab({
  status,
  content,
  personas,
  personaPerspectives,
  noteId,
  debate,
  onStartDebate,
}: PerspectivesTabProps) {
  const debateState = debate ?? {
    active: false,
    rounds: [],
    judgeDecisions: [],
    consensusReached: false,
    consensusSummary: null,
  };
  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">
        Start an insight review to see expert perspectives
      </p>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate perspectives. Try regenerating.
      </p>
    );
  }

  const hasPerPersona = personaPerspectives && Object.keys(personaPerspectives).length > 0;

  return (
    <div className="space-y-4">
      {/* Debate button */}
      {(hasPerPersona || content) && onStartDebate && (
        <div className="flex justify-end">
          <button
            type="button"
            onClick={onStartDebate}
            disabled={debateState.active && !debateState.consensusReached}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-2xs font-medium transition-colors ${
              debateState.active && !debateState.consensusReached
                ? "bg-purple-500/20 text-purple-300 border border-purple-500/30 cursor-wait"
                : "bg-white/[0.04] text-dim hover:text-purple-300 hover:bg-purple-500/10 border border-transparent hover:border-purple-500/20"
            }`}
          >
            <MessageSquare size={11} />
            {debateState.active && !debateState.consensusReached ? "Debating..." : "Debate"}
          </button>
        </div>
      )}

      {/* Debate view (when active) */}
      {debateState.active && debateState.rounds.length > 0 && <DebateView debate={debateState} />}

      {/* Per-persona cards (existing behavior) */}
      {hasPerPersona ? (
        <div className="grid grid-cols-2 gap-3">
          {personas.map((persona) => {
            const personaContent = personaPerspectives[persona.id];
            if (!personaContent) {
              return (
                <div key={persona.id} className="glass-card rounded-lg p-3 space-y-2 animate-pulse">
                  <div className="flex items-center gap-2">
                    <span className="text-sm">{persona.icon}</span>
                    <div>
                      <div className="text-[11px] font-medium text-foreground">{persona.name}</div>
                      <div className="text-[9px] text-dim">{persona.role}</div>
                    </div>
                  </div>
                  <div className="text-2xs text-dim italic">Generating...</div>
                </div>
              );
            }
            return (
              <PersonaCard
                key={persona.id}
                name={persona.name}
                role={persona.role}
                icon={persona.icon}
                tone={persona.tone}
                content={personaContent}
                noteId={noteId ?? undefined}
                personaId={persona.id}
              />
            );
          })}
        </div>
      ) : (
        <FallbackSections content={content} personas={personas} noteId={noteId} />
      )}
    </div>
  );
}
