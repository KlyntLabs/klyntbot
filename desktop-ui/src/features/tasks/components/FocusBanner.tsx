import { useFocusSession } from "../hooks/useFocusSession";

interface ActiveTask {
  id: string;
  title: string;
  focusedAt: string;
}

interface Props {
  activeTask: ActiveTask | null;
  onEndFocus: (taskId: string) => void;
}

export function FocusBanner({ activeTask, onEndFocus }: Props) {
  const { isActive, elapsedSecs, formatElapsed } = useFocusSession(activeTask?.focusedAt ?? null);

  if (!activeTask || !isActive) return null;

  return (
    <div className="fixed top-0 left-0 right-0 z-50 flex items-center justify-between px-4 py-2 bg-surface-elevated border-b border-border">
      <div className="flex items-center gap-3">
        <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
        <span className="text-sm font-medium text-primary truncate max-w-xs">
          {activeTask.title}
        </span>
        <span className="text-xs text-muted font-mono tabular-nums">
          {formatElapsed(elapsedSecs)}
        </span>
      </div>
      <button
        type="button"
        onClick={() => onEndFocus(activeTask.id)}
        className="text-xs text-muted hover:text-primary px-2 py-1 rounded hover:bg-surface-hover transition-colors"
      >
        End Focus
      </button>
    </div>
  );
}
