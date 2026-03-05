import { Archive } from "lucide-react";

export function ArchivedSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">Archived threads</h2>
        <p className="text-[13px] text-muted mt-1">View and restore archived conversations</p>
      </div>

      <div className="bg-surface-low rounded-lg border border-border p-8 flex flex-col items-center text-center">
        <Archive className="w-8 h-8 text-dim mb-3" strokeWidth={1.5} />
        <p className="text-[13px] text-muted">No archived threads</p>
      </div>
    </div>
  );
}
