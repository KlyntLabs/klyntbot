import { bench, describe } from "vitest";

// Placeholder — the real `coalesceDeltas` ships in PR8 (Task 70).
// This is the perf-gate harness so subsequent PRs can update the number.
describe("coalesceDeltas", () => {
  bench("100 chunks", () => {
    const chunks: string[] = Array.from({ length: 100 }, (_, i) => `tok-${i}`);
    // mock coalescer: concat
    chunks.join("");
  });

  bench("10,000 chunks", () => {
    const chunks: string[] = Array.from({ length: 10_000 }, (_, i) => `tok-${i}`);
    chunks.join("");
  });
});
