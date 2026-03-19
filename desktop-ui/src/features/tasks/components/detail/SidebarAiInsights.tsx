import { Bot, ChevronDown, Sparkles, Zap } from "lucide-react";
import { useState } from "react";
import type { DetailTask, Suggestion, TaskState } from "../../lib/mappers";
import { SectionLabel } from "./SectionLabel";

interface SidebarAiInsightsProps {
  task: DetailTask;
  taskState: TaskState;
  suggestions: Suggestion[];
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
  onFetchSuggestions: () => void;
  suggestionsLoading: boolean;
  aiError: string | null;
}

export function SidebarAiInsights({
  task,
  taskState,
  suggestions,
  onApply,
  onDismiss,
  onFetchSuggestions,
  suggestionsLoading,
  aiError,
}: SidebarAiInsightsProps) {
  const pendingSuggestions = suggestions.filter((s) => s.status === "pending");

  return (
    <div className="px-4 py-3 space-y-4">
      <SectionLabel>AI Insights</SectionLabel>

      {aiError && (
        <div className="rounded-md bg-red-500/10 border border-red-500/30 p-2 text-xs text-red-300">
          {aiError}
        </div>
      )}

      {pendingSuggestions.length > 0 ? (
        <SuggestionsList suggestions={pendingSuggestions} onApply={onApply} onDismiss={onDismiss} />
      ) : (
        <WhyThisTaskNow task={task} />
      )}

      {taskState !== "completed" && pendingSuggestions.length === 0 && (
        <button
          type="button"
          onClick={onFetchSuggestions}
          disabled={suggestionsLoading}
          className="w-full flex items-center justify-center gap-1.5 text-xs px-3 py-1.5 rounded-md border border-purple-500/30 text-purple-300 hover:bg-purple-500/10 transition-colors disabled:opacity-50"
        >
          <Sparkles className="size-3" />
          {suggestionsLoading ? "Analyzing..." : "Get Suggestions"}
        </button>
      )}
    </div>
  );
}

function SuggestionsList({
  suggestions,
  onApply,
  onDismiss,
}: {
  suggestions: Suggestion[];
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const top = suggestions[0];
  const rest = suggestions.slice(1);

  if (!top) return null;

  return (
    <div className="space-y-3">
      <SuggestionCard suggestion={top} onApply={onApply} onDismiss={onDismiss} />

      {rest.length > 0 && !expanded && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors flex items-center gap-1"
        >
          <ChevronDown className="size-3" />
          See all ({rest.length} more)
        </button>
      )}

      {expanded &&
        rest.map((s) => (
          <SuggestionCard key={s.id} suggestion={s} onApply={onApply} onDismiss={onDismiss} />
        ))}
    </div>
  );
}

function SuggestionCard({
  suggestion,
  onApply,
  onDismiss,
}: {
  suggestion: Suggestion;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  return (
    <div className="rounded-md border border-border p-3 space-y-2">
      <div className="flex items-start gap-2">
        <Sparkles className="size-3.5 text-purple shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-foreground">{suggestion.title}</span>
            <span className="text-[10px] px-1 py-0.5 rounded bg-purple/20 text-purple shrink-0">
              {Math.round(suggestion.confidence * 100)}%
            </span>
          </div>
          <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
            {suggestion.description}
          </p>
        </div>
      </div>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onApply(suggestion.id)}
          className="text-xs px-2 py-1 rounded bg-purple/20 text-purple hover:bg-purple/30 transition-colors"
        >
          Apply
        </button>
        <button
          type="button"
          onClick={() => onDismiss(suggestion.id)}
          className="text-xs px-2 py-1 rounded text-muted-foreground hover:bg-accent transition-colors"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}

interface TaskReason {
  icon: typeof Zap;
  text: string;
  weight: number;
}

function computeReasons(task: DetailTask): TaskReason[] {
  const reasons: TaskReason[] = [];
  const now = new Date();

  // Priority
  if (task.priority?.id === "urgent") {
    reasons.push({ icon: Zap, text: "P1 — highest priority", weight: 100 });
  } else if (task.priority?.id === "high") {
    reasons.push({ icon: Zap, text: "P2 — high priority", weight: 80 });
  }

  // Due date
  if (task.dueDate) {
    const due = new Date(task.dueDate);
    const diffDays = Math.floor((due.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
    if (diffDays < 0) {
      const absDays = Math.abs(diffDays);
      reasons.push({
        icon: Zap,
        text: `Overdue by ${absDays} day${absDays !== 1 ? "s" : ""}`,
        weight: 95,
      });
    } else if (diffDays === 0) {
      reasons.push({ icon: Zap, text: "Due today", weight: 90 });
    } else if (diffDays <= 3) {
      reasons.push({
        icon: Zap,
        text: `Due in ${diffDays} day${diffDays !== 1 ? "s" : ""}`,
        weight: 70,
      });
    }
  }

  // Focus momentum
  if (task.focusedAt) {
    reasons.push({ icon: Zap, text: "You're already in flow", weight: 85 });
  }

  // Energy match
  if (task.energyLevel) {
    const hour = now.getHours();
    const currentEnergy =
      hour >= 6 && hour < 12 ? "high" : hour >= 12 && hour < 17 ? "medium" : "low";
    if (task.energyLevel === currentEnergy) {
      reasons.push({
        icon: Zap,
        text: "Matches your current energy window",
        weight: 60,
      });
    }
  }

  // Quick win
  if (task.complexityScore != null && task.complexityScore <= 30) {
    reasons.push({ icon: Zap, text: "Quick win — low complexity", weight: 50 });
  }

  return reasons.sort((a, b) => b.weight - a.weight).slice(0, 3);
}

function WhyThisTaskNow({ task }: { task: DetailTask }) {
  const reasons = computeReasons(task);
  if (reasons.length === 0) return null;

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-foreground">
        <Bot className="size-3.5 text-purple" />
        Why This Task Now?
      </div>
      <div className="space-y-1.5">
        {reasons.map((r) => (
          <div key={r.text} className="flex items-center gap-2 text-xs text-muted-foreground">
            <r.icon className="size-3 text-purple/60 shrink-0" />
            {r.text}
          </div>
        ))}
      </div>
    </div>
  );
}
