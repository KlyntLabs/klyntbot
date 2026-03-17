import { AlertTriangle, Bot, ChevronRight, Clock, Zap } from "lucide-react";
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";

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

interface DecompositionModalProps {
  result: DecompositionResult | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onApply: (id: string) => void;
  onReject: (id: string) => void;
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

export function DecompositionModal({
  result,
  open,
  onOpenChange,
  onApply,
  onReject,
  applying,
}: DecompositionModalProps) {
  if (!result) return null;

  const totalCount = countSubtasks(result.subtasks);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bot className="size-5 text-purple-400" />
            Task Breakdown
            <span
              className={`ml-2 text-xs font-medium px-2 py-0.5 rounded-full ${confidenceBadgeClass(result.confidence)}`}
            >
              {Math.round(result.confidence)}%
            </span>
          </DialogTitle>
          <DialogDescription>
            Review the AI-generated subtask plan before applying.
          </DialogDescription>
        </DialogHeader>

        {/* Reasoning */}
        <div className="rounded-md bg-[hsl(var(--accent))]/50 px-3 py-2">
          <p className="text-sm text-[hsl(var(--muted-foreground))]">{result.reasoning}</p>
        </div>

        {/* Warnings */}
        {result.warnings.length > 0 && (
          <div className="rounded-md bg-yellow-500/10 border border-yellow-500/30 px-3 py-2 space-y-1">
            {result.warnings.map((warning) => (
              <div key={warning} className="flex items-start gap-2">
                <AlertTriangle className="size-4 text-yellow-400 shrink-0 mt-0.5" />
                <span className="text-sm text-yellow-300">{warning}</span>
              </div>
            ))}
          </div>
        )}

        {/* Subtask tree */}
        <div className="border border-[hsl(var(--border))] rounded-md px-3 py-2">
          {result.subtasks.map((subtask) => (
            <SubtaskItem key={subtask.tempId} subtask={subtask} depth={0} />
          ))}
        </div>

        {/* Total estimate */}
        {result.totalEstimatedMins != null && (
          <div className="flex items-center gap-1.5 text-sm text-[hsl(var(--muted-foreground))]">
            <Clock className="size-4" />
            Total estimate: {formatEstimate(result.totalEstimatedMins)}
          </div>
        )}

        <DialogFooter>
          <button
            type="button"
            onClick={() => onReject(result.id)}
            className="px-3 py-1.5 text-sm rounded-md border border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]/50 transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onApply(result.id)}
            disabled={applying}
            className="px-3 py-1.5 text-sm rounded-md bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-50 transition-colors"
          >
            {applying
              ? "Applying..."
              : `Create ${totalCount} subtask${totalCount !== 1 ? "s" : ""}`}
          </button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
