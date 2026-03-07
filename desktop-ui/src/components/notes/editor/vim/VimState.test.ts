import { describe, expect, it } from "vitest";
import {
  accumulateCount,
  createVimState,
  effectiveCount,
  enterInsertMode,
  enterNormalMode,
  enterVisualLineMode,
  enterVisualMode,
  resetOperatorState,
  type VimState,
} from "./VimState";

describe("createVimState", () => {
  it("returns correct defaults", () => {
    const state = createVimState();
    expect(state.mode).toBe("normal");
    expect(state.count).toBeNull();
    expect(state.operator).toBeNull();
    expect(state.awaitingChar).toBeNull();
    expect(state.registers).toEqual({});
    expect(state.marks).toEqual({});
    expect(state.lastAction).toBeNull();
    expect(state.searchPattern).toBeNull();
    expect(state.searchDirection).toBe("forward");
    expect(state.visualAnchor).toBeNull();
    expect(state.gPending).toBe(false);
    expect(state.recording).toEqual([]);
  });
});

describe("resetOperatorState", () => {
  it("clears count, operator, awaitingChar, and gPending", () => {
    const state: VimState = {
      ...createVimState(),
      count: 5,
      operator: "d",
      awaitingChar: "f",
      gPending: true,
    };
    const result = resetOperatorState(state);
    expect(result.count).toBeNull();
    expect(result.operator).toBeNull();
    expect(result.awaitingChar).toBeNull();
    expect(result.gPending).toBe(false);
  });

  it("preserves mode", () => {
    const state: VimState = { ...createVimState(), mode: "visual" };
    const result = resetOperatorState(state);
    expect(result.mode).toBe("visual");
  });

  it("preserves registers and marks", () => {
    const state: VimState = {
      ...createVimState(),
      registers: { a: "hello", '"': "world" },
      marks: { a: 42, b: 100 },
    };
    const result = resetOperatorState(state);
    expect(result.registers).toEqual({ a: "hello", '"': "world" });
    expect(result.marks).toEqual({ a: 42, b: 100 });
  });

  it("preserves searchPattern and searchDirection", () => {
    const state: VimState = {
      ...createVimState(),
      searchPattern: "foo",
      searchDirection: "backward",
    };
    const result = resetOperatorState(state);
    expect(result.searchPattern).toBe("foo");
    expect(result.searchDirection).toBe("backward");
  });

  it("preserves lastAction and recording", () => {
    const state: VimState = {
      ...createVimState(),
      lastAction: { keys: ["d", "w"] },
      recording: ["i", "h", "i"],
    };
    const result = resetOperatorState(state);
    expect(result.lastAction).toEqual({ keys: ["d", "w"] });
    expect(result.recording).toEqual(["i", "h", "i"]);
  });

  it("preserves visualAnchor", () => {
    const state: VimState = { ...createVimState(), visualAnchor: 15 };
    const result = resetOperatorState(state);
    expect(result.visualAnchor).toBe(15);
  });

  it("does not mutate the original state", () => {
    const state: VimState = {
      ...createVimState(),
      count: 3,
      operator: "y",
    };
    resetOperatorState(state);
    expect(state.count).toBe(3);
    expect(state.operator).toBe("y");
  });
});

describe("enterInsertMode", () => {
  it("sets mode to insert", () => {
    const state = createVimState();
    const result = enterInsertMode(state);
    expect(result.mode).toBe("insert");
  });

  it("clears operator state", () => {
    const state: VimState = {
      ...createVimState(),
      count: 3,
      operator: "c",
      awaitingChar: "f",
      gPending: true,
    };
    const result = enterInsertMode(state);
    expect(result.count).toBeNull();
    expect(result.operator).toBeNull();
    expect(result.awaitingChar).toBeNull();
    expect(result.gPending).toBe(false);
  });

  it("records lastAction from recording when recording is non-empty", () => {
    const state: VimState = {
      ...createVimState(),
      recording: ["c", "w", "h", "i"],
    };
    const result = enterInsertMode(state);
    expect(result.lastAction).toEqual({ keys: ["c", "w", "h", "i"] });
    expect(result.recording).toEqual([]);
  });

  it("preserves existing lastAction when recording is empty", () => {
    const existingAction = { keys: ["d", "d"] };
    const state: VimState = {
      ...createVimState(),
      lastAction: existingAction,
      recording: [],
    };
    const result = enterInsertMode(state);
    expect(result.lastAction).toBe(existingAction);
  });

  it("sets lastAction to null when both recording and lastAction are empty/null", () => {
    const state = createVimState();
    const result = enterInsertMode(state);
    expect(result.lastAction).toBeNull();
  });

  it("clears recording after capturing lastAction", () => {
    const state: VimState = {
      ...createVimState(),
      recording: ["x", "y"],
    };
    const result = enterInsertMode(state);
    expect(result.recording).toEqual([]);
  });

  it("makes a copy of recording keys, not a reference", () => {
    const recording = ["a", "b"];
    const state: VimState = { ...createVimState(), recording };
    const result = enterInsertMode(state);
    recording.push("c");
    expect(result.lastAction).toEqual({ keys: ["a", "b"] });
  });
});

describe("enterNormalMode", () => {
  it("sets mode to normal", () => {
    const state: VimState = { ...createVimState(), mode: "insert" };
    const result = enterNormalMode(state);
    expect(result.mode).toBe("normal");
  });

  it("clears visualAnchor", () => {
    const state: VimState = {
      ...createVimState(),
      mode: "visual",
      visualAnchor: 42,
    };
    const result = enterNormalMode(state);
    expect(result.visualAnchor).toBeNull();
  });

  it("clears operator state", () => {
    const state: VimState = {
      ...createVimState(),
      count: 2,
      operator: "d",
      awaitingChar: "t",
      gPending: true,
    };
    const result = enterNormalMode(state);
    expect(result.count).toBeNull();
    expect(result.operator).toBeNull();
    expect(result.awaitingChar).toBeNull();
    expect(result.gPending).toBe(false);
  });

  it("preserves registers and marks", () => {
    const state: VimState = {
      ...createVimState(),
      mode: "visual",
      registers: { '"': "yanked text" },
      marks: { a: 10 },
    };
    const result = enterNormalMode(state);
    expect(result.registers).toEqual({ '"': "yanked text" });
    expect(result.marks).toEqual({ a: 10 });
  });
});

describe("enterVisualMode", () => {
  it("sets mode to visual", () => {
    const state = createVimState();
    const result = enterVisualMode(state, 10);
    expect(result.mode).toBe("visual");
  });

  it("sets visualAnchor to the given position", () => {
    const state = createVimState();
    const result = enterVisualMode(state, 42);
    expect(result.visualAnchor).toBe(42);
  });

  it("clears operator state", () => {
    const state: VimState = {
      ...createVimState(),
      count: 5,
      operator: "y",
      gPending: true,
    };
    const result = enterVisualMode(state, 0);
    expect(result.count).toBeNull();
    expect(result.operator).toBeNull();
    expect(result.gPending).toBe(false);
  });

  it("works when transitioning from insert mode", () => {
    const state: VimState = { ...createVimState(), mode: "insert" };
    const result = enterVisualMode(state, 7);
    expect(result.mode).toBe("visual");
    expect(result.visualAnchor).toBe(7);
  });

  it("accepts anchor of 0", () => {
    const state = createVimState();
    const result = enterVisualMode(state, 0);
    expect(result.visualAnchor).toBe(0);
  });
});

describe("enterVisualLineMode", () => {
  it("sets mode to visual-line", () => {
    const state = createVimState();
    const result = enterVisualLineMode(state, 5);
    expect(result.mode).toBe("visual-line");
  });

  it("sets visualAnchor to the given position", () => {
    const state = createVimState();
    const result = enterVisualLineMode(state, 99);
    expect(result.visualAnchor).toBe(99);
  });

  it("clears operator state", () => {
    const state: VimState = {
      ...createVimState(),
      count: 3,
      operator: "d",
      awaitingChar: "F",
      gPending: true,
    };
    const result = enterVisualLineMode(state, 0);
    expect(result.count).toBeNull();
    expect(result.operator).toBeNull();
    expect(result.awaitingChar).toBeNull();
    expect(result.gPending).toBe(false);
  });

  it("works when transitioning from visual mode", () => {
    const state: VimState = {
      ...createVimState(),
      mode: "visual",
      visualAnchor: 10,
    };
    const result = enterVisualLineMode(state, 20);
    expect(result.mode).toBe("visual-line");
    expect(result.visualAnchor).toBe(20);
  });
});

describe("accumulateCount", () => {
  it("sets count from null", () => {
    const state = createVimState();
    const result = accumulateCount(state, 5);
    expect(result.count).toBe(5);
  });

  it("builds two-digit count", () => {
    const state = createVimState();
    const r1 = accumulateCount(state, 1);
    const r2 = accumulateCount(r1, 2);
    expect(r2.count).toBe(12);
  });

  it("builds three-digit count", () => {
    const state = createVimState();
    const r1 = accumulateCount(state, 1);
    const r2 = accumulateCount(r1, 2);
    const r3 = accumulateCount(r2, 3);
    expect(r3.count).toBe(123);
  });

  it("handles leading zero (0 from null starts at 0)", () => {
    const state = createVimState();
    const result = accumulateCount(state, 0);
    expect(result.count).toBe(0);
  });

  it("handles 0 after existing digit", () => {
    const state: VimState = { ...createVimState(), count: 5 };
    const result = accumulateCount(state, 0);
    expect(result.count).toBe(50);
  });

  it("does not mutate the original state", () => {
    const state = createVimState();
    accumulateCount(state, 7);
    expect(state.count).toBeNull();
  });

  it("preserves other state fields", () => {
    const state: VimState = {
      ...createVimState(),
      mode: "visual",
      operator: "d",
    };
    const result = accumulateCount(state, 3);
    expect(result.mode).toBe("visual");
    expect(result.operator).toBe("d");
  });
});

describe("effectiveCount", () => {
  it("returns 1 when count is null", () => {
    const state = createVimState();
    expect(effectiveCount(state)).toBe(1);
  });

  it("returns the count when set", () => {
    const state: VimState = { ...createVimState(), count: 5 };
    expect(effectiveCount(state)).toBe(5);
  });

  it("returns 0 when count is 0", () => {
    const state: VimState = { ...createVimState(), count: 0 };
    expect(effectiveCount(state)).toBe(0);
  });

  it("returns large counts correctly", () => {
    const state: VimState = { ...createVimState(), count: 999 };
    expect(effectiveCount(state)).toBe(999);
  });
});
