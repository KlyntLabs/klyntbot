import { ipc } from "@shared/hooks/useIpc";
import { BookMarked, ListTodo, Sparkles, Target } from "lucide-react";
import { useCallback, useState } from "react";

interface NoteActionBarProps {
  noteId: string;
  onInsightGenerated?: () => void;
}

export function NoteActionBar({ noteId, onInsightGenerated }: NoteActionBarProps) {
  const [generatingInsight, setGeneratingInsight] = useState(false);

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

  const handleLinkToKR = useCallback(() => {
    // TODO: Open KR picker dropdown from project context
    console.info("Link to KR — placeholder");
  }, []);

  const handleCreateTask = useCallback(() => {
    // TODO: Open task creation modal pre-scoped to project
    console.info("Create task from note — placeholder");
  }, []);

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
        <Sparkles className="w-3.5 h-3.5" />
        {generatingInsight ? "Generating..." : "Generate Insight"}
      </button>

      <button
        type="button"
        onClick={handleLinkToKR}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
      >
        <Target className="w-3.5 h-3.5" />
        Link to KR
      </button>

      <button
        type="button"
        onClick={handleCreateTask}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
      >
        <ListTodo className="w-3.5 h-3.5" />
        Create Task
      </button>

      <button
        type="button"
        onClick={handleFlashcards}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
      >
        <BookMarked className="w-3.5 h-3.5" />
        Flashcards
      </button>
    </div>
  );
}
