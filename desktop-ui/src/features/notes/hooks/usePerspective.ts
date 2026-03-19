import { ipc } from "@shared/hooks/useIpc";
import type { Editor } from "@tiptap/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

export type PerspectiveType = "linked-view" | "annotated" | "study-mode";

interface PerspectiveEntry {
  active: PerspectiveType;
  params?: Record<string, unknown>;
}

interface PerspectiveConfig {
  sections: Record<string, PerspectiveEntry | null>;
}

const EMPTY_CONFIG: PerspectiveConfig = { sections: {} };

export function usePerspective(
  noteId: string | null,
  editor: Editor | null,
  perspectiveConfigJson: string | null | undefined,
) {
  const [config, setConfig] = useState<PerspectiveConfig>(() => {
    if (!perspectiveConfigJson) return EMPTY_CONFIG;
    try {
      return JSON.parse(perspectiveConfigJson);
    } catch {
      return EMPTY_CONFIG;
    }
  });

  const [focusedSectionId, setFocusedSectionId] = useState<string | null>(null);

  // Track cursor position → heading ID
  useEffect(() => {
    if (!editor) return;
    const onSelectionUpdate = () => {
      const { from } = editor.state.selection;
      let headingId: string | null = null;

      // Walk backwards from cursor to find the nearest heading
      editor.state.doc.nodesBetween(0, from, (node) => {
        if (node.type.name === "heading" && node.attrs.id) {
          headingId = node.attrs.id;
        }
      });

      setFocusedSectionId(headingId);
    };

    editor.on("selectionUpdate", onSelectionUpdate);
    return () => {
      editor.off("selectionUpdate", onSelectionUpdate);
    };
  }, [editor]);

  const activePerspective = useMemo(() => {
    if (!focusedSectionId) return null;
    return config.sections[focusedSectionId]?.active ?? null;
  }, [focusedSectionId, config]);

  // Debounced save
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const saveConfig = useCallback(
    (newConfig: PerspectiveConfig) => {
      if (!noteId) return;
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => {
        ipc("note_update", {
          params: {
            id: noteId,
            perspectiveConfig: JSON.stringify(newConfig),
          },
        }).catch(() => {});
      }, 500);
    },
    [noteId],
  );

  const setPerspective = useCallback(
    (sectionId: string, type: PerspectiveType) => {
      setConfig((prev) => {
        const next = {
          ...prev,
          sections: {
            ...prev.sections,
            [sectionId]: { active: type },
          },
        };
        saveConfig(next);
        return next;
      });
    },
    [saveConfig],
  );

  const clearPerspective = useCallback(
    (sectionId: string) => {
      setConfig((prev) => {
        const { [sectionId]: _, ...rest } = prev.sections;
        const next = { ...prev, sections: rest };
        saveConfig(next);
        return next;
      });
    },
    [saveConfig],
  );

  return {
    activePerspective,
    focusedSectionId,
    allPerspectives: config.sections,
    setPerspective,
    clearPerspective,
  };
}
