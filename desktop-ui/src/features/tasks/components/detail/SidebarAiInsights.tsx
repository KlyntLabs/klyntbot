import { Bot, ChevronDown, Sparkles, Zap } from "lucide-react";
import { useState } from "react";
import type { Suggestion, TaskMemory, TaskState } from "../../lib/mappers";
import { SectionLabel } from "./SectionLabel";

interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: Suggestion[];
  taskMemory: TaskMemory | null;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}

export function SidebarAiInsights({
  taskState,
  suggestions,
  taskMemory,
  onApply,
  onDismiss,
}: SidebarAiInsightsProps) {
  return (
    <div className="px-4 py-3 space-y-4">
      <SectionLabel>AI Insights</SectionLabel>

      {taskState === "completed" && taskMemory ? (
        <WhatAiLearned memory={taskMemory} />
      ) : taskState === "new" && suggestions.filter((s) => s.status === "pending").length === 0 ? (
        <WhyThisTaskNow />
      ) : (
        <SuggestionsList suggestions={suggestions} onApply={onApply} onDismiss={onDismiss} />
      )}

      {taskState !== "completed" && taskState !== "new" && taskMemory && (
        <TaskMemorySection memory={taskMemory} />
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
  const pending = suggestions.filter((s) => s.status === "pending");
  const top = pending[0];
  const rest = pending.slice(1);

  if (!top) return null;

  return (
    <div className="space-y-3">
      <SuggestionCard suggestion={top} onApply={onApply} onDismiss={onDismiss} />

      {rest.length > 0 && !expanded && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="text-xs text-muted hover:text-primary transition-colors flex items-center gap-1"
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
            <span className="text-sm font-medium text-primary">{suggestion.title}</span>
            <span className="text-[10px] px-1 py-0.5 rounded bg-purple/20 text-purple shrink-0">
              {Math.round(suggestion.confidence * 100)}%
            </span>
          </div>
          <p className="text-xs text-muted mt-0.5 line-clamp-2">{suggestion.description}</p>
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
          className="text-xs px-2 py-1 rounded text-muted hover:bg-surface-raised transition-colors"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}

function WhyThisTaskNow() {
  const reasons = [
    { icon: Zap, text: "High priority — P1" },
    { icon: Zap, text: "Due in 2 days" },
    { icon: Zap, text: "Matches your current energy window" },
  ];

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-primary">
        <Bot className="size-3.5 text-purple" />
        Why This Task Now?
      </div>
      <div className="space-y-1.5">
        {reasons.map((r) => (
          <div key={r.text} className="flex items-center gap-2 text-xs text-muted">
            <r.icon className="size-3 text-purple/60 shrink-0" />
            {r.text}
          </div>
        ))}
      </div>
    </div>
  );
}

function WhatAiLearned({ memory }: { memory: TaskMemory }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-primary">
        <Bot className="size-3.5 text-purple" />
        What AI Learned
      </div>
      <p className="text-xs text-muted">{memory.lastSessionSummary}</p>
      {memory.relatedFacts.map((fact) => (
        <div key={fact} className="flex items-start gap-1.5 text-xs text-muted">
          <span className="text-purple/60 shrink-0">•</span>
          {fact}
        </div>
      ))}
    </div>
  );
}

function TaskMemorySection({ memory }: { memory: TaskMemory }) {
  return (
    <div className="space-y-2 pt-2 border-t border-border/50">
      <span className="text-[10px] font-medium text-muted uppercase tracking-wider">
        Task Memory
      </span>
      <p className="text-xs text-muted">{memory.lastSessionSummary}</p>
      {memory.continuityNote && (
        <p className="text-xs text-muted italic">{memory.continuityNote}</p>
      )}
      {memory.relatedFacts.length > 0 && (
        <div className="space-y-0.5">
          {memory.relatedFacts.map((fact) => (
            <div key={fact} className="flex items-start gap-1.5 text-xs text-muted">
              <span className="text-purple/60 shrink-0">•</span>
              {fact}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
