import { describe, expect, it } from "vitest";

type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

interface VoiceState {
  sessionState: VoiceSessionState;
  chips: RoutingChip[];
  transcript: string;
  segments: Array<{ text: string; confidence: number }>;
  ttsAudio: { base64: string; sampleRate: number; text: string } | null;
  memoryEcho: string | null;
}

function reduceVoiceEvent(
  state: VoiceState,
  event: { type: string; [key: string]: unknown },
): VoiceState {
  switch (event.type) {
    case "captureStarted":
      return {
        ...state,
        sessionState: "capturing" as const,
        transcript: "",
        chips: [],
        segments: [],
        ttsAudio: null,
        memoryEcho: null,
      };
    case "partialTranscript":
      return {
        ...state,
        transcript: event.text as string,
        segments: (event.segments as Array<{ text: string; confidence: number }>) ?? [],
      };
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
    case "speakResponse":
      return {
        ...state,
        sessionState: "response" as const,
        ttsAudio: {
          base64: event.audioBase64 as string,
          sampleRate: event.sampleRate as number,
          text: event.text as string,
        },
      };
    case "memoryEcho":
      return { ...state, memoryEcho: event.text as string };
    default:
      return state;
  }
}

describe("Voice event reducer", () => {
  const initial: VoiceState = {
    sessionState: "idle" as const,
    chips: [],
    transcript: "",
    segments: [],
    ttsAudio: null,
    memoryEcho: null,
  };

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
    const withChips: VoiceState = {
      ...initial,
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

  it("stores segments from partialTranscript", () => {
    const capturing: VoiceState = { ...initial, sessionState: "capturing" as const };
    const result = reduceVoiceEvent(capturing, {
      type: "partialTranscript",
      text: "bonjour monde",
      segments: [
        { text: "bonjour", confidence: 0.92 },
        { text: "monde", confidence: 0.71 },
      ],
    });
    expect(result.segments).toHaveLength(2);
    expect(result.segments[0].confidence).toBe(0.92);
    expect(result.segments[1].confidence).toBe(0.71);
  });

  it("stores ttsAudio from speakResponse", () => {
    const capturing: VoiceState = reduceVoiceEvent(initial, {
      type: "captureStarted",
      sessionId: "s1",
      engine: "local",
    });
    const result = reduceVoiceEvent(capturing, {
      type: "speakResponse",
      audioBase64: "AAAA",
      sampleRate: 24000,
      text: "Hello there",
    });
    expect(result.sessionState).toBe("response");
    expect(result.ttsAudio).toEqual({
      base64: "AAAA",
      sampleRate: 24000,
      text: "Hello there",
    });
  });

  it("memory echo stored on memoryEcho event", () => {
    const capturing: VoiceState = reduceVoiceEvent(initial, {
      type: "captureStarted",
      sessionId: "s1",
      engine: "local",
    });
    const result = reduceVoiceEvent(capturing, {
      type: "memoryEcho",
      text: "You mentioned learning French last week",
    });
    expect(result.memoryEcho).toBe("You mentioned learning French last week");
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
  const idle: VoiceState = {
    sessionState: "idle",
    chips: [],
    transcript: "",
    segments: [],
    ttsAudio: null,
    memoryEcho: null,
  };

  it("transitions from idle to capturing on captureStarted", () => {
    const state = reduceVoiceEvent(idle, {
      type: "captureStarted",
      sessionId: "s1",
      engine: "local",
    });
    expect(state.sessionState).toBe("capturing");
  });

  it("transcript available on finalized for chat handoff", () => {
    const processing: VoiceState = { ...idle, sessionState: "processing" as const };
    const state = reduceVoiceEvent(processing, {
      type: "finalized",
      text: "schedule dentist tomorrow",
      routedTo: "tasks",
      responsePreview: "",
    });
    expect(state.sessionState).toBe("response");
  });
});
