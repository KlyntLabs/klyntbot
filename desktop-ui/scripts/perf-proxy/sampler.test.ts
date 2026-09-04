// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Page } from "@playwright/test";
import { sampleRaf } from "../../tests/perf-proxy/support/sampler.ts";

describe("sampleRaf", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("throws when scroll is set and the scroller querySelector misses", async () => {
    vi.stubGlobal("document", {
      querySelector: () => null,
    });

    const page = {
      evaluate: async <T, A>(
        fn: (arg: A) => T | Promise<T>,
        arg: A,
      ): Promise<T> => fn(arg),
    } as Pick<Page, "evaluate">;

    await expect(
      sampleRaf(page as Page, {
        warmupFrames: 1,
        sampleFrames: 1,
        scroll: { stepPx: 120 },
      }),
    ).rejects.toThrow(/virtualized|scroller/i);
  });
});
