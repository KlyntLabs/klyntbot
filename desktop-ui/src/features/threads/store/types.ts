/**
 * Turn generation — monotonically increasing per (threadId).
 *
 * Used to filter stale events: any event whose generation < current is
 * silently ignored. Replaces the brittle "active turn id ref" pattern.
 */
export type TurnGeneration = number;

export type TurnHandle = {
  threadId: string;
  turnId: string;
  generation: TurnGeneration;
};

export type ThreadStatus =
  | { kind: "idle"; lastDurationMs: number | null }
  | { kind: "streaming"; turn: TurnHandle; startedAt: number }
  | { kind: "tool_executing"; turn: TurnHandle; tool: string; callId: string; startedAt: number }
  | { kind: "stuck"; turn: TurnHandle; stuckSince: number } // watchdog fired
  | { kind: "error"; message: string; turn: TurnHandle | null };
