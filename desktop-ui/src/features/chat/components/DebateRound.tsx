import type { DebateRound as DebateRoundType } from "@shared/types";
import { ConsensusIndicator } from "./ConsensusIndicator";
import { PersonaMessageList } from "./PersonaMessageList";

interface DebateRoundProps {
  round: DebateRoundType;
  totalRounds: number;
  isCurrentRound: boolean;
  isConsensusRound: boolean;
}

export function DebateRound({
  round,
  totalRounds,
  isCurrentRound,
  isConsensusRound,
}: DebateRoundProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-medium text-muted-foreground">
          Round {round.round}
          {isCurrentRound && (
            <span className="ml-1 text-purple-400 animate-pulse">Active</span>
          )}
        </span>
        <ConsensusIndicator
          score={round.consensusScore}
          reached={isConsensusRound}
          round={round.round}
          totalRounds={totalRounds}
        />
      </div>
      <PersonaMessageList personaMessages={round.personaMessages} />
    </div>
  );
}
