import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { useAtomActions, useKnowledgeAtoms } from "../hooks/useKnowledgeAtoms";
import { AtomCard } from "./AtomCard";
import { BulkAcceptModal } from "./BulkAcceptModal";

interface KnowledgeAtomsPanelProps {
  noteId: string | null;
  /** "inline" = below editor (returns null when empty), "panel" = inside Learn panel (shows empty state) */
  variant?: "inline" | "panel";
}

export function KnowledgeAtomsPanel({ noteId, variant = "inline" }: KnowledgeAtomsPanelProps) {
  const { activeAtoms, suggestedAtoms, refetch } = useKnowledgeAtoms(noteId);
  const { accept, dismiss } = useAtomActions(noteId);
  const [bulkModalOpen, setBulkModalOpen] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();
  const highlightAtomId = searchParams.get("atomId");
  const scrolledRef = useRef(false);

  useEffect(() => {
    if (!highlightAtomId || scrolledRef.current) return;
    const el = document.querySelector(`[data-atom-id="${highlightAtomId}"]`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      scrolledRef.current = true;
      setSearchParams({}, { replace: true });
    }
  }, [highlightAtomId, setSearchParams]);

  const totalCount = activeAtoms.length + suggestedAtoms.length;

  if (totalCount === 0) {
    if (variant === "inline") return null;
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <p className="text-[11px] text-muted-foreground text-center">
          No knowledge atoms yet. Atoms are auto-extracted when you write — check back after saving
          some content.
        </p>
      </div>
    );
  }

  const wrapperClass =
    variant === "panel" ? "flex-1 overflow-y-auto px-3 py-2" : "border-b border-border px-3 py-2";

  return (
    <div className={wrapperClass}>
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
            <div key={atom.id} data-atom-id={atom.id}>
              <AtomCard atom={atom} onReviewDone={refetch} />
            </div>
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
