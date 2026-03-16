interface ContextPanelProps {
  width: number;
  noteId: string | null;
}

export function ContextPanel({ width, noteId }: ContextPanelProps) {
  return (
    <div
      style={{ width }}
      className="border-l border-border flex flex-col flex-shrink-0 overflow-y-auto"
    >
      {noteId ? <div className="p-3 text-xs text-muted">Context panel (coming soon)</div> : null}
    </div>
  );
}
