import type { SchemaEvolution } from "@shared/types";

interface SuggestionCardProps {
  suggestion: SchemaEvolution;
  onAccept: (id: string) => void;
  onDismiss: (id: string) => void;
}

export function SuggestionCard({ suggestion, onAccept, onDismiss }: SuggestionCardProps) {
  const confidencePercent = Math.round(suggestion.confidence * 100);

  return (
    <div className="flex items-start gap-3 rounded-lg border border-accent/20 bg-accent/5 p-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{formatAction(suggestion.actionType)}</span>
          <span className="text-xs text-muted tabular-nums">{confidencePercent}%</span>
        </div>
        <p className="mt-0.5 line-clamp-2 text-xs text-muted">{suggestion.reasoning}</p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={() => onAccept(suggestion.id)}
          className="rounded bg-accent px-2 py-1 text-xs text-white hover:bg-accent/90"
        >
          Accept
        </button>
        <button
          type="button"
          onClick={() => onDismiss(suggestion.id)}
          className="rounded px-2 py-1 text-xs text-muted hover:bg-surface-hover"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}

function formatAction(actionType: string): string {
  switch (actionType) {
    case "add_field":
      return "Add field";
    case "remove_field":
      return "Remove field";
    case "modify_field":
      return "Modify field";
    case "hide_field":
      return "Hide field";
    default:
      return actionType.replace(/_/g, " ");
  }
}
