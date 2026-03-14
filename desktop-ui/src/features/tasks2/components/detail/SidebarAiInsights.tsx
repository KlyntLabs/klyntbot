import { Bot, ChevronDown, Sparkles, Zap } from "lucide-react";
import { useState } from "react";
import type { MockSuggestion, MockTaskMemory, TaskState } from "../../mock-data/issue-detail";
import { SectionLabel } from "./SectionLabel";

interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: MockSuggestion[];
  taskMemory: MockTaskMemory;
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

      {taskState === "completed" ? (
        <WhatAiLearned memory={taskMemory} />
      ) : taskState === "new" && suggestions.filter((s) => s.status === "pending").length === 0 ? (
        <WhyThisTaskNow />
      ) : (
        <SuggestionsList suggestions={suggestions} onApply={onApply} onDismiss={onDismiss} />
      )}

      {taskState !== "completed" && taskState !== "new" && (
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
  suggestions: MockSuggestion[];
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
          className="text-xs text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors flex items-center gap-1"
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
  suggestion: MockSuggestion;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  return (
    <div className="rounded-md border border-[hsl(var(--border))] p-3 space-y-2">
      <div className="flex items-start gap-2">
        <Sparkles className="size-3.5 text-purple-400 shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">
              {suggestion.title}
            </span>
            <span className="text-[10px] px-1 py-0.5 rounded bg-purple-500/20 text-purple-300 shrink-0">
              {Math.round(suggestion.confidence * 100)}%
            </span>
          </div>
          <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5 line-clamp-2">
            {suggestion.description}
          </p>
        </div>
      </div>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onApply(suggestion.id)}
          className="text-xs px-2 py-1 rounded bg-purple-500/20 text-purple-300 hover:bg-purple-500/30 transition-colors"
        >
          Apply
        </button>
        <button
          type="button"
          onClick={() => onDismiss(suggestion.id)}
          className="text-xs px-2 py-1 rounded text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))] transition-colors"
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
      <div className="flex items-center gap-1.5 text-sm font-medium text-[hsl(var(--foreground))]">
        <Bot className="size-3.5 text-purple-400" />
        Why This Task Now?
      </div>
      <div className="space-y-1.5">
        {reasons.map((r) => (
          <div
            key={r.text}
            className="flex items-center gap-2 text-xs text-[hsl(var(--muted-foreground))]"
          >
            <r.icon className="size-3 text-purple-400/60 shrink-0" />
            {r.text}
          </div>
        ))}
      </div>
    </div>
  );
}

function WhatAiLearned({ memory }: { memory: MockTaskMemory }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-[hsl(var(--foreground))]">
        <Bot className="size-3.5 text-purple-400" />
        What AI Learned
      </div>
      <p className="text-xs text-[hsl(var(--muted-foreground))]">{memory.lastSessionSummary}</p>
      {memory.relatedFacts.map((fact) => (
        <div
          key={fact}
          className="flex items-start gap-1.5 text-xs text-[hsl(var(--muted-foreground))]"
        >
          <span className="text-purple-400/60 shrink-0">•</span>
          {fact}
        </div>
      ))}
    </div>
  );
}

function TaskMemorySection({ memory }: { memory: MockTaskMemory }) {
  return (
    <div className="space-y-2 pt-2 border-t border-[hsl(var(--border))]/50">
      <span className="text-[10px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
        Task Memory
      </span>
      <p className="text-xs text-[hsl(var(--muted-foreground))]">{memory.lastSessionSummary}</p>
      {memory.continuityNote && (
        <p className="text-xs text-[hsl(var(--muted-foreground))] italic">
          {memory.continuityNote}
        </p>
      )}
      {memory.relatedFacts.length > 0 && (
        <div className="space-y-0.5">
          {memory.relatedFacts.map((fact) => (
            <div
              key={fact}
              className="flex items-start gap-1.5 text-xs text-[hsl(var(--muted-foreground))]"
            >
              <span className="text-purple-400/60 shrink-0">•</span>
              {fact}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
