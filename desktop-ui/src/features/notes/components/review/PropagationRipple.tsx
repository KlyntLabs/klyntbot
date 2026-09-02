import { Zap } from "lucide-react";

interface PropagationRippleProps {
  count: number;
}

export function PropagationRipple({ count }: PropagationRippleProps) {
  if (count <= 0) return null;

  return (
    <div className="flex items-center gap-1.5 py-1 text-[9px] text-brand/70">
      <Zap size={10} className="text-brand" />
      <span>
        This review strengthened <span className="font-medium text-brand">{count}</span> linked{" "}
        {count === 1 ? "concept" : "concepts"}
      </span>
    </div>
  );
}
