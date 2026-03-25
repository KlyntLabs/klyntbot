import type { DebateRound as DebateRoundType, JudgeDecisionEntry } from "@shared/types";
import { DebateRound } from "./DebateRound";
import { JudgeAnnotation } from "./JudgeAnnotation";

interface DebateViewProps {
  rounds: DebateRoundType[];
  totalRounds: number;
  currentRound: number | null;
  consensusReached: boolean;
  consensusSummary: string | null;
  judgeDecisions: JudgeDecisionEntry[];
}

export function DebateView({
  rounds,
  totalRounds,
  currentRound,
  consensusReached,
  consensusSummary,
  judgeDecisions,
}: DebateViewProps) {
  if (rounds.length === 0) return null;
  return (
    <div className="space-y-3 border border-border/30 rounded-xl p-4 bg-white/[0.02]">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
          Squad Debate
        </span>
        {consensusReached && <span className="text-2xs text-green-400">Consensus reached</span>}
      </div>
      {rounds.map((round, i) => {
        const judgeForRound = judgeDecisions.find((j) => j.round === round.round);
        return (
          <div key={round.round} className="space-y-2">
            <DebateRound
              round={round}
              totalRounds={totalRounds}
              isCurrentRound={round.round === currentRound}
              isConsensusRound={consensusReached && i === rounds.length - 1}
            />
            {judgeForRound && <JudgeAnnotation decision={judgeForRound} />}
          </div>
        );
      })}
      {consensusSummary && (
        <div className="text-[11px] text-green-400/80 italic border-t border-border/20 pt-2">
          {consensusSummary}
        </div>
      )}
    </div>
  );
}
