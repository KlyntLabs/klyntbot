import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "@/features/threads/store/useChatStore";
import { useFileEditEvents } from "./useFileEditEvents";

describe("useFileEditEvents", () => {
  beforeEach(() => {
    useChatStore.setState({
      streamApprovals: {},
      streamFileEdits: {},
      streamSnapshots: {},
    });
  });

  it("upserts a kind: diff item on agent:file_edit_with_symbols", async () => {
    let handler: ((e: { payload: Record<string, unknown> }) => void) | undefined;
    vi.mocked(listen).mockImplementation((_event, h) => {
      handler = h as (e: { payload: Record<string, unknown> }) => void;
      return Promise.resolve(() => {});
    });
    renderHook(() => useFileEditEvents("s1"));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    act(() => {
      handler?.({
        payload: {
          path: "/repo/src/x.rs",
          op: "edit",
          bytes: 100,
          diff: "--- /repo/src/x.rs\n+++ /repo/src/x.rs\n@@ -1 +1 @@\n-old\n+new\n",
        },
      });
    });
    const items = useChatStore.getState().streamFileEdits["s1"] ?? [];
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("diff");
    expect(items[0].path).toBe("/repo/src/x.rs");
    expect(items[0].op).toBe("edit");
    expect(items[0].diff).toContain("+new");
  });
});
