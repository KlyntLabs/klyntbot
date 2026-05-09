import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { invoke } from "@/api/client";
import {
  applyJobsView,
  applyJobUpdate,
  type BashJobsPanelView,
  type BashJobView,
} from "@/features/coding/state/jobsStore";

const EVENTS = [
  "coding:job_started",
  "coding:job_completed",
  "coding:job_failed",
  "coding:job_cancelled",
  "coding:job_lost",
] as const;

export function useThreadJobs(threadId: string, agentChain: string[] = ["root"]) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: agentChain.join(",") is the stable dependency key
  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const view = await invoke<BashJobsPanelView>("coding_job_list", {
          threadId,
          agentChain,
          activeOnly: false,
        });
        if (!cancelled) applyJobsView(threadId, view.jobs);
      } catch (e) {
        // soft fail: store keeps prior state
        console.warn("coding_job_list failed", e);
      }
    };
    void refresh();

    const unsubs = EVENTS.map((evt) =>
      listen<BashJobView | { thread_id: string; job_id: string }>(evt, (e) => {
        if (cancelled) return;
        const payload = e.payload as BashJobView | { thread_id: string; job_id: string };
        const tid =
          "thread_id" in payload ? payload.thread_id : (payload as BashJobView).session_id;
        if (tid !== threadId) return;
        if ("id" in payload) {
          applyJobUpdate(threadId, payload as BashJobView);
        } else if ("job_id" in payload) {
          // Lifecycle event without full view → trigger a refresh
          void refresh();
        }
      }),
    );

    return () => {
      cancelled = true;
      // biome-ignore lint/suspicious/useIterableCallbackReturn: Promise return is intentionally discarded
      unsubs.forEach((p) => p.then((fn) => fn()));
    };
  }, [threadId, agentChain.join(",")]);
}
