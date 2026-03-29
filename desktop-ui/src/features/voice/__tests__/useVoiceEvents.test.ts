import { describe, expect, it } from "vitest";

type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

function reduceVoiceEvent(
  state: { sessionState: VoiceSessionState; chips: RoutingChip[]; transcript: string },
  event: { type: string; [key: string]: unknown },
) {
  switch (event.type) {
    case "captureStarted":
      return { ...state, sessionState: "capturing" as const, transcript: "", chips: [] };
    case "partialTranscript":
      return { ...state, transcript: event.text as string };
    case "routingSuggestion": {
      const chip = {
        skill: event.skill as string,
        confidence: event.confidence as number,
        label: event.label as string,
      };
      if (state.chips.find((c) => c.skill === chip.skill)) return state;
      return { ...state, chips: [...state.chips, chip] };
    }
    case "captureEnded":
      return { ...state, sessionState: "processing" as const };
    case "finalized":
      return { ...state, sessionState: "response" as const, transcript: event.text as string };
    default:
      return state;
  }
}

describe("Voice event reducer", () => {
  const initial = { sessionState: "idle" as const, chips: [], transcript: "" };

  it("transitions idle -> capturing on captureStarted", () => {
    const result = reduceVoiceEvent(initial, {
      type: "captureStarted",
      sessionId: "s1",
      engine: "local",
    });
    expect(result.sessionState).toBe("capturing");
  });

  it("updates transcript on partialTranscript", () => {
    const capturing = { ...initial, sessionState: "capturing" as const };
    const result = reduceVoiceEvent(capturing, {
      type: "partialTranscript",
      text: "hello world",
    });
    expect(result.transcript).toBe("hello world");
  });

  it("adds routing chips without duplicates", () => {
    let state = { ...initial, sessionState: "capturing" as const };
    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.8,
      label: "Task",
    });
    expect(state.chips).toHaveLength(1);

    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.9,
      label: "Task",
    });
    expect(state.chips).toHaveLength(1);

    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "learning",
      confidence: 0.7,
      label: "Learning",
    });
    expect(state.chips).toHaveLength(2);
  });

  it("transitions capturing -> processing on captureEnded", () => {
    const capturing = { ...initial, sessionState: "capturing" as const };
    const result = reduceVoiceEvent(capturing, { type: "captureEnded", durationMs: 3000 });
    expect(result.sessionState).toBe("processing");
  });

  it("transitions processing -> response on finalized", () => {
    const processing = { ...initial, sessionState: "processing" as const };
    const result = reduceVoiceEvent(processing, {
      type: "finalized",
      text: "done",
      routedTo: "tasks",
    });
    expect(result.sessionState).toBe("response");
  });

  it("resets chips on new capture", () => {
    const withChips = {
      sessionState: "response" as const,
      chips: [{ skill: "tasks", confidence: 0.8, label: "Task" }],
      transcript: "old text",
    };
    const result = reduceVoiceEvent(withChips, {
      type: "captureStarted",
      sessionId: "s2",
      engine: "local",
    });
    expect(result.chips).toHaveLength(0);
    expect(result.transcript).toBe("");
  });
});

describe("Pronunciation confidence thresholds", () => {
  it("classifies word confidence into correct tiers", () => {
    const classify = (c: number) => (c >= 0.85 ? "good" : c >= 0.6 ? "fair" : "poor");

    expect(classify(0.95)).toBe("good");
    expect(classify(0.85)).toBe("good");
    expect(classify(0.84)).toBe("fair");
    expect(classify(0.6)).toBe("fair");
    expect(classify(0.59)).toBe("poor");
    expect(classify(0.1)).toBe("poor");
  });
});

describe("Launcher recording mode flow", () => {
  it("transitions from idle to capturing on captureStarted", () => {
    const state = reduceVoiceEvent(
      { sessionState: "idle" as const, chips: [], transcript: "" },
      { type: "captureStarted", sessionId: "s1", engine: "local" },
    );
    expect(state.sessionState).toBe("capturing");
  });

  it("transcript available on finalized for chat handoff", () => {
    const processing = { sessionState: "processing" as const, chips: [], transcript: "" };
    const state = reduceVoiceEvent(processing, {
      type: "finalized",
      text: "schedule dentist tomorrow",
      routedTo: "tasks",
      responsePreview: "",
    });
    expect(state.sessionState).toBe("response");
  });
});
