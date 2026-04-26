import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
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
    let active = true;
    let unsubCreated: (() => void) | undefined;
    let unsubUpdated: (() => void) | undefined;
    listen("chat:thread_created", () => refetch()).then((fn) => {
      if (active) unsubCreated = fn;
      else fn();
    });
    listen("chat:thread_updated", () => refetch()).then((fn) => {
      if (active) unsubUpdated = fn;
      else fn();
    });
    return () => {
      active = false;
      unsubCreated?.();
      unsubUpdated?.();
    };
  }, [refetch]);

  return { threads, refetch, error };
}
