import { useCallback, useRef, useState } from "react";
import type { Task } from "../lib/types";
import { ipc } from "./useIpc";
import { useSetToggle } from "./useSetToggle";

export function useSubtasks() {
  const [childrenCache, setChildrenCache] = useState<Map<string, Task[]>>(new Map());
  const [expandedTasks, toggleExpand] = useSetToggle();
  // Generation counter: incremented on invalidation so the useEffect re-runs.
  const [generation, setGeneration] = useState(0);
  const inFlightRef = useRef<Set<string>>(new Set());

  const fetchChildren = useCallback(async (parentId: string) => {
    if (inFlightRef.current.has(parentId)) return;

    inFlightRef.current.add(parentId);
    try {
      const children = await ipc<Task[]>("task_list_children", { parentId });
      setChildrenCache((prev) => new Map(prev).set(parentId, children));
    } finally {
      inFlightRef.current.delete(parentId);
    }
  }, []);

  const invalidateCache = useCallback(() => {
    setChildrenCache(new Map());
    setGeneration((g) => g + 1);
  }, []);

  return {
    expandedTasks,
    childrenCache,
    generation,
    toggleExpand,
    fetchChildren,
    invalidateCache,
  };
}
