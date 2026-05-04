import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ChatThread } from "@/features/chat/types";

type CodingThreadSummary = {
  id: string;
  title: string | null;
  updatedAt: number;
  workspaceId: string;
};

export interface UseCodingSessionsResult {
  threads: ChatThread[];
  workspaceIdBySessionKey: Map<string, string>;
  refetch: () => Promise<void>;
  error: string | null;
}

export interface UseCodingSessionsOptions {
  enabled?: boolean;
}

export function useCodingSessions(
  options: UseCodingSessionsOptions = {},
): UseCodingSessionsResult {
  const { enabled = true } = options;
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const workspaceIdBySessionKey = useRef<Map<string, string>>(new Map());
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    try {
      const raw = await invoke<CodingThreadSummary[]>("coding_thread_list", {
        workspaceId: null,
      });
      const map = new Map<string, string>();
      setThreads(
        raw.map((t) => {
          map.set(t.id, t.workspaceId);
          return {
            sessionKey: t.id,
            title: t.title ?? "Untitled session",
            updatedAt: new Date(t.updatedAt).toISOString(),
          };
        }),
      );
      workspaceIdBySessionKey.current = map;
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    refetch();
    let unlisten: (() => void) | undefined;
    const pending = listen("coding:thread_updated", () => refetch()).then((fn) => {
      unlisten = fn;
    });
    return () => {
      pending.then(() => unlisten?.());
    };
  }, [refetch, enabled]);

  return { threads, refetch, error, workspaceIdBySessionKey: workspaceIdBySessionKey.current };
}
