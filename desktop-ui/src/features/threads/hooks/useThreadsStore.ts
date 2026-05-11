import { useEffect } from "react";
import { useChatStore, selectThreadState } from "../store/useChatStore";
import type { ThreadAction, ThreadState } from "./useThreadsReducer";

/**
 * Zustand-backed replacement for `useReducer(threadReducer, …)`.
 *
 * All thread state now lives in `useChatStore`; this hook is a thin shim
 * that returns the same `[state, dispatch]` shape so `useThreads.ts` and
 * its sub-hooks don't need changes during the migration window.
 */
export function useThreadsReducer(maxItemsPerThread: number | null): [ThreadState, (action: ThreadAction) => void] {
  const dispatch = useChatStore((s) => s.dispatchThreadAction);

  useEffect(() => {
    dispatch({ type: "setMaxItemsPerThread", maxItemsPerThread });
  }, [dispatch, maxItemsPerThread]);

  const state = useChatStore(selectThreadState);

  return [state, dispatch];
}
