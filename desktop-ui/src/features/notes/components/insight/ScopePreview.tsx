import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import { FileText, Link, NotebookText } from "lucide-react";
import type { ScopeConfig } from "./InsightScopePopover";

interface ScopePreviewNote {
  id: string;
  title: string;
  notebookId: string | null;
}

interface ScopePreviewResponse {
  notes: ScopePreviewNote[];
}

interface ScopePreviewProps {
  noteId: string;
  scopeConfig: ScopeConfig;
}

const SCOPE_LABELS: Record<string, { label: string; icon: typeof Link }> = {
  backlinks: { label: "Linked", icon: Link },
  notebook: { label: "Notebook", icon: NotebookText },
};

export function ScopePreview({ noteId, scopeConfig }: ScopePreviewProps) {
  const { data, isLoading } = useQuery<ScopePreviewResponse>(
    "note_insight_preview_scope",
    { noteId, scopeType: scopeConfig.scopeType },
    { notes: [] },
  );

  const notes = data?.notes ?? [];
  const scope = SCOPE_LABELS[scopeConfig.scopeType] ?? SCOPE_LABELS.backlinks;
  const Icon = scope.icon;

  return (
    <div className="px-3 py-2 border-b border-border">
      {/* Scope label */}
      <div className="flex items-center gap-1.5 text-2xs text-muted-foreground mb-1.5">
        <Icon size={10} className="shrink-0" />
        <span className="font-medium">{scope.label} scope</span>
        <span className="text-dim">
          {isLoading ? "..." : `${notes.length} note${notes.length !== 1 ? "s" : ""}`}
        </span>
        {scopeConfig.deepDive && <span className="text-purple text-[9px] ml-1">(deep dive)</span>}
      </div>

      {/* Note list */}
      {notes.length > 0 && (
        <div className="flex flex-col gap-0.5 max-h-28 overflow-y-auto">
          {notes.map((note) => (
            <div
              key={note.id}
              className={cn(
                "flex items-center gap-1.5 px-1.5 py-0.5 rounded text-2xs",
                "text-muted-foreground bg-accent/30",
              )}
            >
              <FileText size={9} className="shrink-0 text-dim" />
              <span className="truncate">{note.title}</span>
            </div>
          ))}
        </div>
      )}

      {/* Empty state */}
      {!isLoading && notes.length === 0 && (
        <div className="text-[9px] text-dim">No related notes found for this scope</div>
      )}
    </div>
  );
}
