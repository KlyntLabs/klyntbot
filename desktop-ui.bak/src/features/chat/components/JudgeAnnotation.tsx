import type { JudgeDecisionEntry } from "@shared/types";

interface JudgeAnnotationProps {
  decision: JudgeDecisionEntry;
}

export function JudgeAnnotation({ decision }: JudgeAnnotationProps) {
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
          Consensus: {Math.round(decision.consensusScore)}% —{" "}
          {decision.decision === "continue"
            ? "Continuing discussion"
            : decision.decision === "final_round"
              ? "Moving to final round"
              : "Consensus reached"}
        </p>
      </div>
    </div>
  );
}
