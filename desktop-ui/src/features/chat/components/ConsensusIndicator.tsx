interface ConsensusIndicatorProps {
  score: number | null;
  reached: boolean;
  round?: number;
  totalRounds?: number;
}

export function ConsensusIndicator({
  score,
  reached,
  round,
  totalRounds,
}: ConsensusIndicatorProps) {
  if (score === null && !round) return null;

  // Show round progress instead of raw Jaccard score (which is misleading for natural language)
  if (round && totalRounds) {
    return (
      <div className="flex items-center gap-1.5 text-ui-xs text-fg-dim">
        <span>
          Round {round}/{totalRounds}
        </span>
        <div className="flex gap-0.5">
          {Array.from({ length: totalRounds }, (_, i) => (
            <div
              // biome-ignore lint/suspicious/noArrayIndexKey: static indicator dots from Array.from
              key={`round-dot-${i}`}
              className={`w-1.5 h-1.5 rounded-full transition-all ${
                i < (round ?? 0)
                  ? reached && i === (round ?? 0) - 1
                    ? "bg-green-400"
                    : "bg-purple-400"
                  : "bg-white/[0.08]"
              }`}
            />
          ))}
        </div>
        {reached && <span className="text-green-400">Consensus</span>}
      </div>
    );
  }

  // Fallback: simple consensus badge
  if (reached) {
    return <span className="text-ui-xs text-green-400">Consensus reached</span>;
  }

  return null;
}
