import { Brain } from "lucide-react";
import { useState } from "react";

export function ReasoningPart({ text, redacted }: { text: string; redacted: boolean }) {
  const [expanded, setExpanded] = useState(false);

  if (!text.trim()) return null;

  return (
    <div className="part-reasoning">
      <button
        type="button"
        className="part-reasoning__toggle"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        <Brain size={14} />
        <span>Reasoning{redacted ? " (redacted)" : ""}</span>
      </button>
      {expanded && (
        <div className="part-reasoning__content">
          {redacted ? <em>Content redacted by provider</em> : text}
        </div>
      )}
    </div>
  );
}
