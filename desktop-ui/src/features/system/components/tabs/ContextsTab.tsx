import {
  ContextDetailPanel,
  ContextSearchDialog,
  ContextSidebar,
  useWorkContextDetail,
  useWorkContexts,
} from "@features/work-contexts";
import type { WorkContext } from "@shared/types";
import { useState } from "react";

export function ContextsTab() {
  const [statusFilter, setStatusFilter] = useState<string | undefined>(undefined);
  const { data: contexts } = useWorkContexts(statusFilter);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const { data: detail } = useWorkContextDetail(selectedId);

  const handleSelect = (ctx: WorkContext) => {
    setSelectedId(ctx.id);
  };

  return (
    <div className="flex flex-col h-full min-h-0 p-3 gap-3">
      {/* Status filter row */}
      <div className="flex items-center gap-1 shrink-0">
        {(
          [
            [undefined, "Active"],
            ["paused", "Paused"],
            ["archived", "Archived"],
          ] as const
        ).map(([value, label]) => (
          <button
            key={label}
            type="button"
            onClick={() => setStatusFilter(value as string | undefined)}
            className={`px-2.5 py-1 rounded-lg text-[11px] font-light transition-colors ${
              statusFilter === value
                ? "glass-button-active text-primary"
                : "text-muted hover:text-secondary hover:bg-surface-low"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Context list */}
      <div className="flex-1 overflow-y-auto">
        <ContextSidebar
          contexts={contexts}
          selectedId={selectedId ?? undefined}
          onSelect={handleSelect}
          onSearchClick={() => setSearchOpen(true)}
        />
      </div>

      {/* Detail slide panel */}
      <ContextDetailPanel
        open={selectedId !== null}
        onClose={() => setSelectedId(null)}
        detail={detail}
      />

      {/* Search dialog */}
      <ContextSearchDialog
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        onSelect={(ctx) => {
          setSelectedId(ctx.id);
          setSearchOpen(false);
        }}
      />
    </div>
  );
}
