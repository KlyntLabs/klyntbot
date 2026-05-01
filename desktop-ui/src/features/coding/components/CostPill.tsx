import { useCodingThreadCost } from "../hooks/useCodingThreadCost";

export function CostPill({ threadId }: { threadId: string | null }) {
  const { cost, tokens } = useCodingThreadCost(threadId);
  return (
    <span className="cost-pill" title={`${tokens} tokens`}>
      ${cost.toFixed(4)}
    </span>
  );
}
