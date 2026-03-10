export type VimMode = "normal" | "insert" | "visual" | "visual-line";
export type Operator = "d" | "c" | "y" | ">" | "<" | "gu" | "gU";
export type AwaitingChar = "f" | "F" | "t" | "T" | "r";
export type SearchDirection = "forward" | "backward";

export interface RecordedAction {
  keys: string[];
}

export interface VimState {
  mode: VimMode;
  count: number | null;
  operator: Operator | null;
  awaitingChar: AwaitingChar | null;
  registers: Record<string, string>;
  marks: Record<string, number>;
  lastAction: RecordedAction | null;
  searchPattern: string | null;
  searchDirection: SearchDirection;
  visualAnchor: number | null;
  gPending: boolean;
  recording: string[];
}

export function createVimState(): VimState {
  return {
    mode: "normal",
    count: null,
    operator: null,
    awaitingChar: null,
    registers: {},
    marks: {},
    lastAction: null,
    searchPattern: null,
    searchDirection: "forward",
    visualAnchor: null,
    gPending: false,
    recording: [],
  };
}

export function resetOperatorState(state: VimState): VimState {
  return {
    ...state,
    count: null,
    operator: null,
    awaitingChar: null,
    gPending: false,
  };
}

export function enterInsertMode(state: VimState): VimState {
  return {
    ...resetOperatorState(state),
    mode: "insert",
    lastAction: state.recording.length > 0 ? { keys: [...state.recording] } : state.lastAction,
    recording: [],
  };
}

export function enterNormalMode(state: VimState): VimState {
  return { ...resetOperatorState(state), mode: "normal", visualAnchor: null };
}

export function enterVisualMode(state: VimState, anchor: number): VimState {
  return {
    ...resetOperatorState(state),
    mode: "visual",
    visualAnchor: anchor,
  };
}

export function enterVisualLineMode(state: VimState, anchor: number): VimState {
  return {
    ...resetOperatorState(state),
    mode: "visual-line",
    visualAnchor: anchor,
  };
}

export function accumulateCount(state: VimState, digit: number): VimState {
  const current = state.count ?? 0;
  return { ...state, count: current * 10 + digit };
}

export function effectiveCount(state: VimState): number {
  return state.count ?? 1;
}
