import { describe, it, expect, vi } from "vitest";
import { mockTauriCore, mockTauriEvent } from "./mockTauri";

describe("mockTauriCore", () => {
    it("provides default invoke + convertFileSrc", () => {
        const mocks = mockTauriCore();
        expect(typeof mocks.invoke).toBe("function");
        expect(mocks.convertFileSrc("foo.png")).toBe("tauri://foo.png");
    });

    it("respects override invoke", () => {
        const customInvoke = vi.fn();
        const mocks = mockTauriCore({ invoke: customInvoke });
        expect(mocks.invoke).toBe(customInvoke);
    });
});

describe("mockTauriEvent", () => {
    it("provides default listen + emit as resolving fns", async () => {
        const mocks = mockTauriEvent();
        const unlisten = await mocks.listen();
        expect(typeof unlisten).toBe("function");
        await expect(mocks.emit("test", {})).resolves.toBeUndefined();
    });
});
