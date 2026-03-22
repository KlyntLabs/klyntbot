import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { Editor } from "@tiptap/react";
import { useCallback, useEffect } from "react";

export interface AnnotationResponse {
  id: string;
  noteId: string;
  markId: string | null;
  content: string;
  quotedText: string | null;
  rangeStart: number | null;
  rangeEnd: number | null;
  aiSuggestion: string | null;
  tags: string;
  createdAt: string;
  updatedAt: string;
}

interface AnnotationCreateParams {
  noteId: string;
  markId: string;
  content: string;
  quotedText?: string;
  rangeStart?: number;
  rangeEnd?: number;
  aiSuggestion?: string;
  tags?: string;
}

interface AnnotationUpdateParams {
  id: string;
  content?: string;
  tags?: string;
}

const EMPTY_ANNOTATIONS: AnnotationResponse[] = [];

export function useAnnotations(noteId: string | null, editor: Editor | null) {
  const {
    data: annotations,
    loading,
    refetch,
  } = useQuery<AnnotationResponse[]>(
    "annotation_list_for_note",
    noteId ? { noteId, limit: 200 } : null,
    EMPTY_ANNOTATIONS,
  );

  // Clean up orphan marks that have no matching DB record (only after data loaded)
  useEffect(() => {
    if (!editor || loading) return;
    const validIds = new Set(annotations.map((a) => a.markId ?? a.id));
    const { tr, doc } = editor.state;
    let modified = false;
    doc.descendants((node, pos) => {
      for (const mark of node.marks) {
        if (mark.type.name === "annotation" && !validIds.has(mark.attrs.annotationId)) {
          tr.removeMark(pos, pos + node.nodeSize, mark);
          modified = true;
        }
      }
    });
    if (modified) editor.view.dispatch(tr);
  }, [editor, annotations, loading]);

  const createAnnotation = useCallback(
    async (params: AnnotationCreateParams) => {
      if (!editor) return;

      // Apply optimistic mark
      editor.commands.setAnnotation(params.markId);

      try {
        const response = await ipc<AnnotationResponse>("annotation_create", {
          params,
        });

        // Confirm mark: remove pending attribute
        const { tr, doc } = editor.state;
        let modified = false;
        doc.descendants((node, pos) => {
          for (const mark of node.marks) {
            if (
              mark.type.name === "annotation" &&
              mark.attrs.annotationId === response.markId &&
              mark.attrs.pending
            ) {
              tr.removeMark(pos, pos + node.nodeSize, mark);
              tr.addMark(
                pos,
                pos + node.nodeSize,
                mark.type.create({ ...mark.attrs, pending: false }),
              );
              modified = true;
            }
          }
        });
        if (modified) editor.view.dispatch(tr);

        refetch();
      } catch {
        // Rollback: remove the optimistic mark
        editor.commands.unsetAnnotation(params.markId);
      }
    },
    [editor, refetch],
  );

  const updateAnnotation = useCallback(
    async (params: AnnotationUpdateParams) => {
      await ipc<AnnotationResponse>("annotation_update", { params });
      refetch();
    },
    [refetch],
  );

  const deleteAnnotation = useCallback(
    async (id: string) => {
      // Remove mark from editor
      if (editor) {
        editor.commands.unsetAnnotation(id);
      }
      await ipc<void>("annotation_delete", { id });
      refetch();
    },
    [editor, refetch],
  );

  return {
    annotations: annotations ?? EMPTY_ANNOTATIONS,
    createAnnotation,
    updateAnnotation,
    deleteAnnotation,
    refetch,
  };
}
