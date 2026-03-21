import { useState } from "react";
import { useAtomActions, useKnowledgeAtoms } from "../hooks/useKnowledgeAtoms";
import { AtomCard } from "./AtomCard";
import { BulkAcceptModal } from "./BulkAcceptModal";

interface KnowledgeAtomsPanelProps {
  noteId: string | null;
}

export function KnowledgeAtomsPanel({ noteId }: KnowledgeAtomsPanelProps) {
  const { activeAtoms, suggestedAtoms, refetch } = useKnowledgeAtoms(noteId);
  const { accept, dismiss } = useAtomActions(noteId);
  const [bulkModalOpen, setBulkModalOpen] = useState(false);

  const totalCount = activeAtoms.length + suggestedAtoms.length;

  if (totalCount === 0) return null;

  return (
    <div className="border-b border-border px-3 py-2">
      {/* Header */}
      <div className="flex items-center justify-between mb-1">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Knowledge Atoms ({totalCount})
        </span>
        {suggestedAtoms.length > 0 && (
          <button
            type="button"
            onClick={() => setBulkModalOpen(true)}
            className="text-[10px] text-brand hover:text-brand/80 transition-colors"
          >
            Accept all ({suggestedAtoms.length})
          </button>
        )}
      </div>

      {/* Active atoms */}
      {activeAtoms.length > 0 && (
        <div className="-mx-1">
          {activeAtoms.map((atom) => (
            <AtomCard key={atom.id} atom={atom} onReviewDone={refetch} />
          ))}
        </div>
      )}

      {/* Suggested atoms */}
      {suggestedAtoms.length > 0 && (
        <>
          {activeAtoms.length > 0 && <div className="my-1 border-t border-border/30" />}
          <span className="text-[9px] text-muted-foreground uppercase tracking-wider mb-0.5 block px-1">
            Suggested
          </span>
          <div className="-mx-1">
            {suggestedAtoms.map((atom) => (
              <AtomCard
                key={atom.id}
                atom={atom}
                onAccept={accept}
                onDismiss={dismiss}
                onReviewDone={refetch}
              />
            ))}
          </div>
        </>
      )}

      {/* Bulk accept modal */}
      {bulkModalOpen && noteId && (
        <BulkAcceptModal
          atoms={suggestedAtoms}
          onClose={() => {
            setBulkModalOpen(false);
            refetch();
          }}
        />
      )}
    </div>
  );
}
