import { Archive } from "lucide-react";

export function ArchivedSettings() {
  // TODO: Wire to chat_threads with archive filter when backend supports thread archiving.
  // For now, show a placeholder empty state.

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-fg">Archived threads</h2>
        <p className="text-ui text-fg-secondary mt-1">
          View and restore archived conversations
        </p>
      </div>

      <div className="island rounded-lg p-8 flex flex-col items-center text-center">
        <Archive className="size-8 text-fg-dim mb-3" strokeWidth={1.5} />
        <p className="text-ui text-fg-secondary mb-1">No archived threads</p>
        <p className="text-ui-xs text-fg-dim">
          Archived conversations will appear here. Thread archiving is coming soon.
        </p>
      </div>
    </div>
  );
}
