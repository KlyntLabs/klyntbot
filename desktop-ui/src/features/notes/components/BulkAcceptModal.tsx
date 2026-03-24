import { Dialog } from "@shared/composites";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { Check } from "lucide-react";
import { useState } from "react";
import type { KnowledgeAtomResponse } from "../hooks/useKnowledgeAtoms";

interface BulkAcceptModalProps {
  atoms: KnowledgeAtomResponse[];
  onClose: () => void;
}

const IMPORTANCE_OPTIONS = [
  { label: "Low", value: 0.3, color: "text-blue-400" },
  { label: "Medium", value: 0.7, color: "text-amber-400" },
  { label: "High", value: 1.0, color: "text-red-400" },
] as const;

interface BulkAcceptParams {
  atomIds: string[];
  personalImportance: number;
}

export function BulkAcceptModal({ atoms, onClose }: BulkAcceptModalProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set(atoms.map((a) => a.id)));
  const [importance, setImportance] = useState(0.7);

  const { mutate, loading: saving } = useMutation<void, BulkAcceptParams>(
    "atoms_bulk_accept",
    "params",
  );

  const toggleAtom = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleAll = () => {
    if (selected.size === atoms.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(atoms.map((a) => a.id)));
    }
  };

  const handleAccept = async () => {
    if (selected.size === 0) return;
    await mutate({ atomIds: [...selected], personalImportance: importance });
    invalidateQueries("atoms_for_note");
    onClose();
  };

  return (
    <Dialog open onClose={onClose} title="Accept Suggested Atoms" size="sm">
      {/* Atom list */}
      <div className="max-h-[40vh] overflow-y-auto space-y-0.5 -mx-5 px-5">
        {/* Select all */}
        <button
          type="button"
          onClick={toggleAll}
          className="flex items-center gap-2 w-full py-1.5 text-2xs text-muted-foreground hover:text-primary transition-colors"
        >
          <div
            className={`size-3.5 rounded border flex items-center justify-center transition-colors ${
              selected.size === atoms.length
                ? "bg-brand border-brand"
                : "border-border hover:border-muted-foreground"
            }`}
          >
            {selected.size === atoms.length && <Check size={10} className="text-white" />}
          </div>
          {selected.size === atoms.length ? "Deselect all" : "Select all"}
        </button>

        {atoms.map((atom) => (
          <button
            key={atom.id}
            type="button"
            onClick={() => toggleAtom(atom.id)}
            className="flex items-center gap-2 w-full rounded-md px-1.5 py-1.5 hover:bg-surface-hover transition-colors text-left"
          >
            <div
              className={`size-3.5 rounded border flex items-center justify-center shrink-0 transition-colors ${
                selected.has(atom.id)
                  ? "bg-brand border-brand"
                  : "border-border hover:border-muted-foreground"
              }`}
            >
              {selected.has(atom.id) && <Check size={10} className="text-white" />}
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-xs font-medium text-primary truncate">{atom.subject}</p>
              <p className="text-2xs text-muted truncate">{atom.domain}</p>
            </div>
          </button>
        ))}
      </div>

      {/* Importance picker + action */}
      <div className="pt-3 mt-3 border-t border-border/30 space-y-3">
        <div>
          <p className="text-2xs text-muted-foreground mb-1.5">Importance</p>
          <div className="flex gap-1.5">
            {IMPORTANCE_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => setImportance(opt.value)}
                className={`flex-1 rounded-md py-1 text-[11px] font-medium transition-colors ${
                  importance === opt.value
                    ? `${opt.color} bg-white/10 border border-white/20`
                    : "text-muted-foreground hover:text-primary border border-transparent"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        <button
          type="button"
          onClick={handleAccept}
          disabled={selected.size === 0 || saving}
          className="w-full rounded-lg bg-brand py-2 text-sm font-medium text-white hover:bg-brand/90 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          {saving
            ? "Accepting..."
            : `Accept ${selected.size} atom${selected.size !== 1 ? "s" : ""}`}
        </button>
      </div>
    </Dialog>
  );
}
