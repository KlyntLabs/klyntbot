import { useAtomActions, useKnowledgeAtoms } from "../hooks/useKnowledgeAtoms";
import { AtomCard } from "./AtomCard";

interface KnowledgeAtomsPanelProps {
  noteId: string | null;
}

export function KnowledgeAtomsPanel({ noteId }: KnowledgeAtomsPanelProps) {
  const { activeAtoms, suggestedAtoms, loading, refetch } = useKnowledgeAtoms(noteId);
  const { accept, dismiss, acceptAll } = useAtomActions(noteId);

  const totalCount = activeAtoms.length + suggestedAtoms.length;

  if (totalCount === 0) return null;

  return (
    <div className="border-b border-border px-3 py-3">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Knowledge Atoms ({totalCount})
        </span>
        {suggestedAtoms.length > 0 && (
          <button
            type="button"
            onClick={() => acceptAll(suggestedAtoms)}
            className="text-[10px] text-brand hover:text-brand/80 transition-colors"
          >
            Accept all ({suggestedAtoms.length})
          </button>
        )}
      </div>

      {/* Active atoms */}
      {activeAtoms.length > 0 && (
        <div className="space-y-1">
          {activeAtoms.map((atom) => (
            <AtomCard key={atom.id} atom={atom} onReviewDone={refetch} />
          ))}
        </div>
      )}

      {/* Suggested atoms */}
      {suggestedAtoms.length > 0 && (
        <>
          {activeAtoms.length > 0 && (
            <div className="my-2 border-t border-border/50" />
          )}
          <span className="text-[9px] text-muted-foreground uppercase tracking-wider mb-1 block">
            Suggested
          </span>
          <div className="space-y-1">
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
    </div>
  );
}
