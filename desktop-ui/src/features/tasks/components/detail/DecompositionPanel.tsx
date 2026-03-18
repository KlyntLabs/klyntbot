import { AlertTriangle, Bot, ChevronRight, Clock, X, Zap } from "lucide-react";
import { useState } from "react";

export interface PlannedSubtask {
  tempId: string;
  title: string;
  description: string | null;
  estimatedMinutes: number | null;
  energyLevel: string | null;
  priority: number | null;
  children: PlannedSubtask[];
}

export interface DecompositionResult {
  id: string;
  confidence: number;
  reasoning: string;
  subtasks: PlannedSubtask[];
  totalEstimatedMins: number | null;
  warnings: string[];
  autoApplied: boolean;
}

interface DecompositionPanelProps {
  result: DecompositionResult | null;
  loading: boolean;
  onDecompose: () => void;
  onApply: (id: string) => void;
  onClose: () => void;
  applying: boolean;
}

function confidenceBadgeClass(confidence: number): string {
  if (confidence >= 80) return "bg-green-500/20 text-green-400";
  if (confidence >= 60) return "bg-yellow-500/20 text-yellow-400";
  return "bg-red-500/20 text-red-400";
}

function countSubtasks(subtasks: PlannedSubtask[]): number {
  let count = 0;
  for (const s of subtasks) {
    count += 1 + countSubtasks(s.children);
  }
  return count;
}

function formatEstimate(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

function SubtaskItem({ subtask, depth }: { subtask: PlannedSubtask; depth: number }) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = subtask.children.length > 0;

  return (
    <div style={{ paddingLeft: `${depth * 16}px` }}>
      <div className="flex items-start gap-2 py-1.5">
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="mt-0.5 text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors"
          >
            <ChevronRight
              className={`size-3.5 transition-transform ${expanded ? "rotate-90" : ""}`}
            />
          </button>
        ) : (
          <span className="mt-0.5 size-3.5" />
        )}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm text-[hsl(var(--foreground))]">{subtask.title}</span>
            {subtask.estimatedMinutes != null && (
              <span className="flex items-center gap-0.5 text-xs text-[hsl(var(--muted-foreground))] shrink-0">
                <Clock className="size-3" />
                {formatEstimate(subtask.estimatedMinutes)}
              </span>
            )}
            {subtask.energyLevel != null && (
              <span className="flex items-center gap-0.5 text-xs text-[hsl(var(--muted-foreground))] shrink-0">
                <Zap className="size-3" />
                {subtask.energyLevel}
              </span>
            )}
          </div>
          {subtask.description && (
            <p className="text-xs text-[hsl(var(--muted-foreground))] truncate mt-0.5">
              {subtask.description}
            </p>
          )}
        </div>
      </div>
      {hasChildren && expanded && (
        <div>
          {subtask.children.map((child) => (
            <SubtaskItem key={child.tempId} subtask={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

export function DecompositionPanel({
  result,
  loading,
  onDecompose,
  onApply,
  onClose,
  applying,
}: DecompositionPanelProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[hsl(var(--border))] shrink-0">
        <div className="flex items-center gap-2">
          <Bot className="size-4 text-purple-400" />
          <span className="text-sm font-medium text-[hsl(var(--foreground))]">Task Breakdown</span>
          {result && (
            <span
              className={`text-[10px] font-medium px-1.5 py-0.5 rounded-full ${confidenceBadgeClass(result.confidence)}`}
            >
              {Math.round(result.confidence)}%
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="p-0.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] transition-colors"
          aria-label="Close breakdown"
        >
          <X className="size-3.5" />
        </button>
      </div>

      {/* Content */}
      {!result && !loading && <IntroView onDecompose={onDecompose} />}
      {loading && <LoadingView />}
      {result && (
        <ResultView result={result} onApply={onApply} onClose={onClose} applying={applying} />
      )}
    </div>
  );
}

function IntroView({ onDecompose }: { onDecompose: () => void }) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center px-6 py-8 text-center">
      <div className="size-10 rounded-full bg-purple-500/10 flex items-center justify-center mb-4">
        <Bot className="size-5 text-purple-400" />
      </div>
      <h3 className="text-sm font-medium text-[hsl(var(--foreground))] mb-2">AI Task Breakdown</h3>
      <p className="text-xs text-[hsl(var(--muted-foreground))] mb-1">
        Analyze this task and suggest a breakdown into smaller, actionable subtasks.
      </p>
      <p className="text-xs text-[hsl(var(--muted-foreground))] mb-6">
        The AI considers the description, acceptance criteria, and complexity to propose a
        structured plan.
      </p>
      <button
        type="button"
        onClick={onDecompose}
        className="flex items-center gap-2 px-4 py-2 text-sm rounded-md bg-purple-600 text-white hover:bg-purple-500 transition-colors"
      >
        <Bot className="size-4" />
        Break Down
      </button>
    </div>
  );
}

function LoadingView() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center px-6 py-8 text-center">
      <div className="size-10 rounded-full bg-purple-500/10 flex items-center justify-center mb-4 animate-pulse">
        <Bot className="size-5 text-purple-400" />
      </div>
      <p className="text-sm text-[hsl(var(--muted-foreground))]">Analyzing task...</p>
      <p className="text-xs text-[hsl(var(--muted-foreground))] mt-1">
        This may take a few seconds
      </p>
    </div>
  );
}

function ResultView({
  result,
  onApply,
  onClose,
  applying,
}: {
  result: DecompositionResult;
  onApply: (id: string) => void;
  onClose: () => void;
  applying: boolean;
}) {
  const totalCount = countSubtasks(result.subtasks);

  return (
    <>
      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-3">
        {/* Reasoning */}
        <div className="rounded-md bg-[hsl(var(--accent))]/50 px-3 py-2">
          <p className="text-xs text-[hsl(var(--muted-foreground))]">{result.reasoning}</p>
        </div>

        {/* Warnings */}
        {result.warnings.length > 0 && (
          <div className="rounded-md bg-yellow-500/10 border border-yellow-500/30 px-3 py-2 space-y-1">
            {result.warnings.map((warning) => (
              <div key={warning} className="flex items-start gap-2">
                <AlertTriangle className="size-3.5 text-yellow-400 shrink-0 mt-0.5" />
                <span className="text-xs text-yellow-300">{warning}</span>
              </div>
            ))}
          </div>
        )}

        {/* Subtask tree */}
        <div>
          <div className="text-[10px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider mb-2">
            Proposed Subtasks
          </div>
          <div className="border border-[hsl(var(--border))] rounded-md px-3 py-1">
            {result.subtasks.map((subtask) => (
              <SubtaskItem key={subtask.tempId} subtask={subtask} depth={0} />
            ))}
          </div>
        </div>

        {/* Total estimate */}
        {result.totalEstimatedMins != null && (
          <div className="flex items-center gap-1.5 text-xs text-[hsl(var(--muted-foreground))]">
            <Clock className="size-3" />
            Total estimate: {formatEstimate(result.totalEstimatedMins)}
          </div>
        )}
      </div>

      {/* Footer actions */}
      <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[hsl(var(--border))] shrink-0">
        <button
          type="button"
          onClick={onClose}
          className="px-3 py-1.5 text-xs rounded-md text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]/50 transition-colors"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={() => onApply(result.id)}
          disabled={applying}
          className="px-3 py-1.5 text-xs rounded-md bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-50 transition-colors"
        >
          {applying ? "Applying..." : `Create ${totalCount} subtask${totalCount !== 1 ? "s" : ""}`}
        </button>
      </div>
    </>
  );
}
