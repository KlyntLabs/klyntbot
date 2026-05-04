import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export interface RecallEvent {
  thread_id: string;
  memory_ids: string[];
  coverage_score: number;
  dead_end_warning: boolean;
  snippets: Array<{ kind: string; summary: string; source: string }>;
}

export interface DeadEndEvent {
  thread_id: string;
  approach_summary: string;
  prior_attempt_id: string;
  confidence: number;
}

export function useCodingRecallSnippets(threadId: string | null) {
  const [recall, setRecall] = useState<RecallEvent | null>(null);
  const [deadEnd, setDeadEnd] = useState<DeadEndEvent | null>(null);

  useEffect(() => {
    if (!threadId) return;
    let active = true;
    const unlistens: Array<(() => void) | undefined> = [];
    (async () => {
      const [u1, u2] = await Promise.all([
        listen<RecallEvent>("agent:recall_injected", (evt) => {
          if (evt.payload.thread_id === threadId) setRecall(evt.payload);
        }),
        listen<DeadEndEvent>("agent:dead_end_warning_surfaced", (evt) => {
          if (evt.payload.thread_id === threadId) setDeadEnd(evt.payload);
        }),
      ]);
      unlistens.push(u1, u2);
      if (!active) {
        u1();
        u2();
        unlistens.length = 0;
      }
    })();
    return () => {
      active = false;
      for (const fn of unlistens) {
        fn?.();
      }
    };
  }, [threadId]);

  return { recall, deadEnd };
}
