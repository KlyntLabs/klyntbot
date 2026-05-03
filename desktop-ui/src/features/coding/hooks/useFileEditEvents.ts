import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import type { ConversationItem } from "@/types";

type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

type FileEditPayload = {
  path: string;
  op: "edit" | "write" | "apply_patch" | "notebook_edit";
  bytes: number;
  diff: string;
};

export function useFileEditEvents(sessionKey: string) {
  useEffect(() => {
    const un = listen<FileEditPayload>("agent:file_edit_with_symbols", (e) => {
      const id = `diff-${sessionKey}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      const item: DiffItem = {
        id,
        kind: "diff",
        title: shortName(e.payload.path),
        diff: e.payload.diff,
        path: e.payload.path,
        op: e.payload.op,
        bytes: e.payload.bytes,
      };
      chatStreamStore.upsertFileEdit(sessionKey, item);
    });
    return () => {
      un.then((f) => f());
    };
  }, [sessionKey]);
}

function shortName(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? path : path.slice(i + 1);
}
