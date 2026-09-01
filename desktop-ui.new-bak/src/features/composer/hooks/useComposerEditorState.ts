import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "composerEditorExpanded";

export function useComposerEditorState() {
  const [isExpanded, setIsExpanded] = useState(() => {
    try {
      return window.localStorage.getItem(STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, String(isExpanded));
    } catch {
      // Ignore storage failures.
    }
  }, [isExpanded]);

  const toggleExpanded = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  return { isExpanded, toggleExpanded };
}
