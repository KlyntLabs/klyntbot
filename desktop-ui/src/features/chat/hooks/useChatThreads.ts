import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ChatThread } from "../types";

export interface UseChatThreadsResult {
  threads: ChatThread[];
  refetch: () => Promise<void>;
  error: string | null;
}

export function useChatThreads(): UseChatThreadsResult {
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    try {
      const result = await invoke<ChatThread[]>("chat_threads");
      setThreads(result);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    refetch();
  }, [refetch]);

  useEffect(() => {
    const unsubCreated = listen("chat:thread_created", () => refetch());
    const unsubUpdated = listen("chat:thread_updated", () => refetch());
    return () => {
      unsubCreated.then((fn) => fn()).catch(() => {});
      unsubUpdated.then((fn) => fn()).catch(() => {});
    };
  }, [refetch]);

  return { threads, refetch, error };
}
