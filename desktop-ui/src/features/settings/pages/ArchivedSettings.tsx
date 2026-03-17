import { Archive } from "lucide-react";

export function ArchivedSettings() {
  // TODO: Wire to chat_threads with archive filter when backend supports thread archiving.
  // For now, show a placeholder empty state.

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Archived threads</h2>
        <p className="text-[13px] text-muted-foreground mt-1">View and restore archived conversations</p>
      </div>

      <div className="bg-card rounded-lg border border-border p-8 flex flex-col items-center text-center">
        <Archive className="w-8 h-8 text-dim mb-3" strokeWidth={1.5} />
        <p className="text-[13px] text-muted-foreground mb-1">No archived threads</p>
        <p className="text-[11px] text-dim">
          Archived conversations will appear here. Thread archiving is coming soon.
        </p>
      </div>
    </div>
  );
}
