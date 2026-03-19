import type { Editor } from "@tiptap/react";
import { useCallback } from "react";
import { ulid } from "ulid";
import type { useAnnotations } from "./useAnnotations";
import { useCardGeneration } from "./useCardGeneration";

/** Get selected text from whichever editor has the selection, falling back to DOM selection. */
function getSelectedText(editor: Editor | null): string | null {
  // Try the main editor first
  if (editor && !editor.state.selection.empty) {
    const { from, to } = editor.state.selection;
    return editor.state.doc.textBetween(from, to);
  }
  // Fall back to DOM selection (works for split-mode editors)
  const sel = window.getSelection();
  if (sel && !sel.isCollapsed && sel.toString().trim().length > 0) {
    return sel.toString();
  }
  return null;
}

export function useEditorActions(
  editor: Editor | null,
  noteId: string | null,
  createAnnotation: ReturnType<typeof useAnnotations>["createAnnotation"],
) {
  const { generateFromText } = useCardGeneration();

  const handleAnnotate = useCallback(() => {
    if (!noteId) return;

    const selectedText = getSelectedText(editor);
    if (!selectedText) return;

    const markId = ulid();

    // Try to apply the mark via the main editor if it has the selection
    if (editor && !editor.state.selection.empty) {
      editor.commands.setAnnotation(markId);
    }

    createAnnotation({
      noteId,
      markId,
      content: "",
      quotedText: selectedText,
    });
  }, [editor, noteId, createAnnotation]);

  const handleFlashcard = useCallback(() => {
    const selectedText = getSelectedText(editor);
    if (!selectedText) return;
    generateFromText(selectedText);
  }, [editor, generateFromText]);

  const handleAskAI = useCallback(() => {
    // TODO: Open inline AI prompt input
  }, []);

  return { handleAnnotate, handleFlashcard, handleAskAI };
}
