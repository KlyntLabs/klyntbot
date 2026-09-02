import { ArrowRight, X } from "lucide-react";
import { useState } from "react";

interface Props {
  summary: string;
}

export function ChangesBanner({ summary }: Props) {
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;

  // Don't show if nothing meaningful changed
  if (summary.includes("No significant changes")) return null;

  return (
    <div className="px-3 py-2 border-b border-separator bg-brand/5 flex items-start gap-2">
      <ArrowRight size={12} className="text-brand shrink-0 mt-0.5" />
      <p className="text-ui-xs text-fg-secondary leading-relaxed flex-1">{summary}</p>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="text-fg-dim hover:text-fg-secondary transition-colors shrink-0"
      >
        <X size={10} />
      </button>
    </div>
  );
}
