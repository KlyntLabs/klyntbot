import { describe, expect, it, vi } from "vitest";
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

  it("respects override isTauri", () => {
    const customIsTauri = vi.fn(() => false);
    const mocks = mockTauriCore({ isTauri: customIsTauri });
    expect(mocks.isTauri).toBe(customIsTauri);
    expect((mocks.isTauri as unknown as () => boolean)()).toBe(false);
  });
});

describe("mockTauriEvent", () => {
  it("provides default listen + emit as resolving fns", async () => {
    const mocks = mockTauriEvent();
    const unlisten = await (mocks.listen as unknown as () => Promise<() => void>)();
    expect(typeof unlisten).toBe("function");
    await expect(
      (mocks.emit as unknown as (e: string, p: unknown) => Promise<void>)("test", {}),
    ).resolves.toBeUndefined();
  });
});
