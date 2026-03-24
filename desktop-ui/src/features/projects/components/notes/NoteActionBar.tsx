import { useClickOutside } from "@shared/hooks/useClickOutside";
import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import type { EntityLinkCreateParams, Task, TaskCreateParams } from "@shared/types";
import { BookMarked, ListTodo, Sparkles, Target } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";

interface EntityLinkResponse {
  id: string;
}

interface NoteActionBarProps {
  noteId: string;
  noteTitle?: string;
  onInsightGenerated?: () => void;
}

export function NoteActionBar({ noteId, noteTitle, onInsightGenerated }: NoteActionBarProps) {
  const { objectives, project } = useProjectContext();
  const [generatingInsight, setGeneratingInsight] = useState(false);
  const [showKrPicker, setShowKrPicker] = useState(false);
  const krPickerRef = useRef<HTMLDivElement>(null);

  useClickOutside(krPickerRef, () => setShowKrPicker(false), showKrPicker);

  const { mutate: createLink } = useMutation<EntityLinkResponse, EntityLinkCreateParams>(
    "entity_link_create",
    "params",
  );

  const { mutate: createTask } = useMutation<Task, TaskCreateParams>("task_create", "params");

  const allKrs = objectives.flatMap((o) =>
    (o.keyResults ?? []).map((kr) => ({ ...kr, objectiveTitle: o.title })),
  );

  const handleGenerateInsight = useCallback(async () => {
    setGeneratingInsight(true);
    try {
      await ipc("note_insight_review", { noteId });
      onInsightGenerated?.();
    } catch (e) {
      console.warn("Failed to generate insight:", e);
    } finally {
      setGeneratingInsight(false);
    }
  }, [noteId, onInsightGenerated]);

  const handleLinkToKR = useCallback(
    async (krId: string) => {
      await createLink({
        sourceKind: "note",
        sourceId: noteId,
        targetKind: "key_result",
        targetId: krId,
      });
      setShowKrPicker(false);
    },
    [noteId, createLink],
  );

  const handleCreateTask = useCallback(async () => {
    if (!noteId) return;
    await createTask({
      title: `Task from: ${noteTitle || "Untitled note"}`,
      projectId: project?.id,
    });
  }, [noteId, noteTitle, project?.id, createTask]);

  const handleFlashcards = useCallback(async () => {
    try {
      await ipc("flashcard_generate", { noteId });
    } catch (e) {
      console.warn("Failed to generate flashcards:", e);
    }
  }, [noteId]);

  return (
    <div className="flex items-center gap-2 px-4 py-2 border-t border-border">
      <button
        type="button"
        onClick={handleGenerateInsight}
        disabled={generatingInsight}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand text-white text-xs font-medium hover:bg-brand/90 transition-colors disabled:opacity-50"
      >
        <Sparkles className="size-3.5" />
        {generatingInsight ? "Generating..." : "Generate Insight"}
      </button>

      <div className="relative" ref={krPickerRef}>
        <button
          type="button"
          onClick={() => setShowKrPicker((prev) => !prev)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <Target className="size-3.5" />
          Link to KR
        </button>

        {showKrPicker && (
          <div className="glass-panel absolute bottom-full mb-1 left-0 w-64 max-h-60 overflow-y-auto rounded-lg border border-border shadow-lg z-50">
            {allKrs.length === 0 ? (
              <p className="px-3 py-2 text-[11px] text-muted-foreground">
                No key results found. Create objectives with KRs in the OKR tab.
              </p>
            ) : (
              <div className="py-1">
                {objectives
                  .filter((o) => (o.keyResults ?? []).length > 0)
                  .map((o) => (
                    <div key={o.id}>
                      <p className="px-3 pt-2 pb-1 text-2xs text-muted-foreground uppercase tracking-wider truncate">
                        {o.title}
                      </p>
                      {(o.keyResults ?? []).map((kr) => (
                        <button
                          key={kr.id}
                          type="button"
                          onClick={() => handleLinkToKR(kr.id)}
                          className="w-full text-left px-3 py-1.5 text-[11px] text-foreground hover:bg-accent transition-colors truncate"
                        >
                          {kr.title}
                        </button>
                      ))}
                    </div>
                  ))}
              </div>
            )}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={handleCreateTask}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
      >
        <ListTodo className="size-3.5" />
        Create Task
      </button>

      <button
        type="button"
        onClick={handleFlashcards}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
      >
        <BookMarked className="size-3.5" />
        Flashcards
      </button>
    </div>
  );
}
