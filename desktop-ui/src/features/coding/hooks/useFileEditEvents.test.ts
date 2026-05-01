import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { listen } from "@tauri-apps/api/event";
import { useFileEditEvents } from "./useFileEditEvents";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";

describe("useFileEditEvents", () => {
  it("upserts a kind: diff item on agent:file_edit_with_symbols", async () => {
    let handler: any;
    (listen as any).mockImplementation((_c: string, h: any) => {
      handler = h; return Promise.resolve(() => {});
    });
    renderHook(() => useFileEditEvents("s1"));
    await waitFor(() => expect(listen).toHaveBeenCalled());
    act(() => {
      handler({ payload: { path: "/repo/src/x.rs", op: "edit", bytes: 100,
        diff: "--- /repo/src/x.rs\n+++ /repo/src/x.rs\n@@ -1 +1 @@\n-old\n+new\n" } });
    });
    const items = chatStreamStore.getFileEdits("s1");
    expect(items).toHaveLength(1);
    expect(items[0].kind).toBe("diff");
    expect(items[0].path).toBe("/repo/src/x.rs");
    expect(items[0].op).toBe("edit");
    expect(items[0].diff).toContain("+new");
  });
});
