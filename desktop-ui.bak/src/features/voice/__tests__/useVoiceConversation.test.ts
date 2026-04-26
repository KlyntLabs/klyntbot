import { describe, expect, it } from "vitest";

// Test the event reducer logic (same pattern as useVoiceEvents tests)
type ConversationPhase = "idle" | "listening" | "reflecting" | "speaking";

interface ConversationState {
  phase: ConversationPhase;
  transcript: string;
  segments: Array<{ text: string; confidence: number }>;
  routingChips: Array<{ skill: string; confidence: number; label: string }>;
  memoryEcho: string | null;
  audioLevel: number;
  ttsAudio: { base64: string; sampleRate: number; text: string } | null;
  sessionInfo: { key: string; title: string; turnCount: number } | null;
  continueAvailable: boolean;
  engineKind: "local" | "cloud";
}

function initialState(): ConversationState {
  return {
    phase: "idle",
    transcript: "",
    segments: [],
    routingChips: [],
    memoryEcho: null,
    audioLevel: 0,
    ttsAudio: null,
    sessionInfo: null,
    continueAvailable: false,
    engineKind: "local",
  };
}

function reduceConversationEvent(
  state: ConversationState,
  event: Record<string, unknown>,
): ConversationState {
  const next = { ...state };
  switch (event.type) {
    case "phaseChanged":
      next.phase = event.phase as ConversationPhase;
      if (event.sessionTitle || event.turnCount !== undefined) {
        next.sessionInfo = {
          key: next.sessionInfo?.key ?? "",
          title: (event.sessionTitle as string) ?? next.sessionInfo?.title ?? "",
          turnCount: (event.turnCount as number) ?? 0,
        };
      }
      if (next.phase === "listening") {
        next.transcript = "";
        next.segments = [];
        next.routingChips = [];
        next.memoryEcho = null;
        next.continueAvailable = false;
      }
      break;
    case "audioLevel":
      next.audioLevel = event.rms as number;
      break;
    case "partialTranscript":
      next.transcript = event.text as string;
      if (event.segments) {
        next.segments = event.segments as Array<{ text: string; confidence: number }>;
      }
      break;
    case "routingSuggestion": {
      const skill = event.skill as string;
      if (!next.routingChips.some((c) => c.skill === skill)) {
        next.routingChips = [
          ...next.routingChips,
          { skill, confidence: event.confidence as number, label: event.label as string },
        ];
      }
      break;
    }
    case "memoryEcho":
      next.memoryEcho = event.text as string;
      break;
    case "reflecting":
      next.phase = "reflecting";
      break;
    case "speakResponse":
      next.phase = "speaking";
      next.ttsAudio = {
        base64: event.audioBase64 as string,
        sampleRate: event.sampleRate as number,
        text: event.text as string,
      };
      break;
    case "ttsFadeOut":
      next.ttsAudio = null;
      break;
    case "continueAvailable":
      next.continueAvailable = true;
      break;
    case "captureStarted":
      next.engineKind = event.engine === "Cloud" ? "cloud" : "local";
      break;
  }
  return next;
}

describe("useVoiceConversation reducer", () => {
  it("phaseChanged to listening resets turn state", () => {
    let state = initialState();
    state.transcript = "old text";
    state.memoryEcho = "old echo";
    state.continueAvailable = true;

    state = reduceConversationEvent(state, {
      type: "phaseChanged",
      phase: "listening",
      sessionTitle: "Test Session",
      turnCount: 0,
    });

    expect(state.phase).toBe("listening");
    expect(state.transcript).toBe("");
    expect(state.memoryEcho).toBeNull();
    expect(state.continueAvailable).toBe(false);
    expect(state.sessionInfo?.title).toBe("Test Session");
  });

  it("full multi-turn cycle", () => {
    let state = initialState();

    // Start listening
    state = reduceConversationEvent(state, {
      type: "phaseChanged",
      phase: "listening",
      turnCount: 0,
    });
    expect(state.phase).toBe("listening");

    // Partial transcript
    state = reduceConversationEvent(state, { type: "partialTranscript", text: "hello world" });
    expect(state.transcript).toBe("hello world");

    // Routing
    state = reduceConversationEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.8,
      label: "Task",
    });
    expect(state.routingChips).toHaveLength(1);

    // Reflecting
    state = reduceConversationEvent(state, { type: "reflecting" });
    expect(state.phase).toBe("reflecting");

    // Speaking
    state = reduceConversationEvent(state, {
      type: "speakResponse",
      audioBase64: "abc",
      sampleRate: 16000,
      text: "response",
    });
    expect(state.phase).toBe("speaking");
    expect(state.ttsAudio?.text).toBe("response");

    // Auto-resume → next turn
    state = reduceConversationEvent(state, {
      type: "phaseChanged",
      phase: "listening",
      turnCount: 1,
    });
    expect(state.phase).toBe("listening");
    expect(state.transcript).toBe(""); // Reset for new turn
    expect(state.sessionInfo?.turnCount).toBe(1);
  });

  it("interrupt sets continueAvailable", () => {
    let state = initialState();
    state.phase = "speaking";

    state = reduceConversationEvent(state, { type: "ttsFadeOut" });
    expect(state.ttsAudio).toBeNull();

    state = reduceConversationEvent(state, { type: "continueAvailable", timeoutSecs: 8 });
    expect(state.continueAvailable).toBe(true);
  });

  it("routing chips deduplicate by skill", () => {
    let state = initialState();
    state = reduceConversationEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.8,
      label: "Task",
    });
    state = reduceConversationEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.9,
      label: "Task",
    });
    expect(state.routingChips).toHaveLength(1);
  });
});
