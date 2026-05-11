import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { useChatStore } from "@/features/threads/store/useChatStore";
import { fetchCodingTodos } from "@/api/endpoints/coding";
import { applyView } from "../state/todoStore";
import type { MessageDto } from "../state/codingEventReducer";
import { initialCodingState } from "../state/codingEventReducer";

// Re-exports for backward compatibility during migration
export type { MessageDto, ThreadEvent } from "../state/codingEventReducer";
export { applyThreadEvent } from "../state/codingEventReducer";

const OPTIMISTIC_USER_PREFIX = "local-user-";

function makeOptimisticUserMessage(threadId: string, text: string): MessageDto {
  return {
    id: `${OPTIMISTIC_USER_PREFIX}${Date.now()}`,
    session_id: threadId,
    role: "user",
    parts: [{ kind: "text", text }],
    model: null,
    turn_id: null,
    created_at: Date.now(),
    finish_reason: null,
  };
}

/**
 * Reads reduced coding thread state from `useChatStore`.
 *
 * Events are applied globally by `ThreadEventBuffer`, so this hook no
 * longer needs its own subscription — it simply selects the slice for
 * `threadId` from the unified store.
 */
export function useThreadEvents(threadId: string | null, seedThreadPrompt?: string | null) {
  const state = useChatStore(
    (store) => (threadId ? store.codingStateByThread[threadId] : undefined) ?? initialCodingState,
  );
  const seedAppliedRef = useRef<Record<string, boolean>>({});

  // Inject optimistic user message when seedThreadPrompt arrives.
  useEffect(() => {
    if (!seedThreadPrompt || !threadId) return;
    if (seedAppliedRef.current[threadId]) return;
    seedAppliedRef.current[threadId] = true;
    const store = useChatStore.getState();
    const prev = store.codingStateByThread[threadId] ?? initialCodingState;
    const exists = prev.items.some(
      (m) =>
        m.id.startsWith(OPTIMISTIC_USER_PREFIX) &&
        m.parts.some((p) => p.kind === "text" && p.text === seedThreadPrompt),
    );
    if (exists) return;
    store.applyCodingThreadEvent(threadId, {
      kind: "item_started",
      thread_id: threadId,
      turn_id: "",
      item: makeOptimisticUserMessage(threadId, seedThreadPrompt),
    });
  }, [threadId, seedThreadPrompt]);

  // Reset state when threadId changes (unmount) and merge resume items.
  useEffect(() => {
    if (!threadId) {
      useChatStore.getState().resetCodingThreadState(threadId ?? "");
      return;
    }
    let cancelled = false;

    invoke<{ items?: MessageDto[] }>("coding_thread_resume", {
      threadId,
      includeItems: true,
    })
      .then((thread: { items?: MessageDto[] }) => {
        if (cancelled) return;
        const items = thread?.items ?? [];
        if (items.length === 0) return;
        const prev = useChatStore.getState().codingStateByThread[threadId] ?? initialCodingState;
        const hasOptimistic = prev.items.some((it) => it.id.startsWith(OPTIMISTIC_USER_PREFIX));
        if (!hasOptimistic) {
          useChatStore.setState((s) => ({
            codingStateByThread: {
              ...s.codingStateByThread,
              [threadId]: { ...prev, items },
            },
          }));
          return;
        }
        const serverIds = new Set(items.map((it) => it.id));
        const localOnly = prev.items.filter(
          (it) => it.id.startsWith(OPTIMISTIC_USER_PREFIX) && !serverIds.has(it.id),
        );
        useChatStore.setState((s) => ({
          codingStateByThread: {
            ...s.codingStateByThread,
            [threadId]: { ...prev, items: [...items, ...localOnly] },
          },
        }));
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      useChatStore.getState().resetCodingThreadState(threadId);
    };
  }, [threadId]);

  // Clean up seed tracking for unmounted threads to avoid unbounded growth.
  useEffect(() => {
    if (!threadId) return;
    return () => {
      delete seedAppliedRef.current[threadId];
    };
  }, [threadId]);

  // Todo refresh effect
  useEffect(() => {
    if (!threadId) return;

    let refreshing = false;
    let cancelled = false;

    const eventNames = [
      "coding:todos_updated",
      "coding:plan_entered",
      "coding:plan_updated",
      "coding:plan_exited",
    ];
    const handlers = eventNames.map((name) =>
      listen(name, (e) => {
        if (cancelled) return;
        const payload = e.payload as any;
        const matches =
          name === "coding:todos_updated" ? payload?.thread_id === threadId : payload === threadId;
        if (matches) refresh();
      }),
    );

    async function refresh() {
      if (refreshing) return;
      refreshing = true;
      try {
        const view = await fetchCodingTodos(threadId!);
        applyView(threadId!, view as any);
      } finally {
        refreshing = false;
      }
    }

    return () => {
      cancelled = true;
      for (const h of handlers) h.then((fn) => fn());
    };
  }, [threadId]);

  return state;
}
