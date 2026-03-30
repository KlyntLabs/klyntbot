import { describe, expect, it } from "vitest";
import type { BrainAmbientEvent, SignalMode, SignalSummary } from "./useAmbientSignals";

describe("useAmbientSignals types", () => {
  it("should have correct signal modes", () => {
    const modes: SignalMode[] = ["pulse", "badge", "deferred", "merged"];
    expect(modes).toHaveLength(4);
  });

  it("should match backend event shape", () => {
    const event: BrainAmbientEvent = {
      mode: "pulse",
      signals: [{ signalType: "memory_promoted", entityPair: "fact:1", headline: "test" }],
      tooltip: "Test tooltip",
      detailRoute: "/brain",
    };
    expect(event.signals).toHaveLength(1);
  });

  it("should allow null detailRoute", () => {
    const event: BrainAmbientEvent = {
      mode: "badge",
      signals: [],
      tooltip: "2 new signals",
      detailRoute: null,
    };
    expect(event.detailRoute).toBeNull();
  });

  it("should support all signal modes on BrainAmbientEvent", () => {
    const modes: SignalMode[] = ["pulse", "badge", "deferred", "merged"];
    for (const mode of modes) {
      const event: BrainAmbientEvent = {
        mode,
        signals: [],
        tooltip: `mode: ${mode}`,
        detailRoute: null,
      };
      expect(event.mode).toBe(mode);
    }
  });

  it("should allow multiple signals in an event", () => {
    const signals: SignalSummary[] = [
      { signalType: "memory_promoted", entityPair: "fact:1", headline: "Promoted fact" },
      { signalType: "cross_domain", entityPair: "task:2:note:5", headline: "Cross-domain link" },
    ];
    const event: BrainAmbientEvent = {
      mode: "merged",
      signals,
      tooltip: "2 brain signals",
      detailRoute: "/brain/signals",
    };
    expect(event.signals).toHaveLength(2);
    expect(event.signals[0].signalType).toBe("memory_promoted");
    expect(event.signals[1].entityPair).toBe("task:2:note:5");
  });
});
