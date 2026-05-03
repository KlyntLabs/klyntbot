import type { WireEventDto } from "./types";

export type ChipColor =
  | "blue"
  | "purple"
  | "green"
  | "orange"
  | "amber"
  | "cyan"
  | "indigo"
  | "teal"
  | "pink"
  | "neutral";

const KIND_TO_COLOR: Record<string, ChipColor> = {
  // turn
  turnBegin: "blue",
  turnEnd: "blue",
  steerInput: "blue",
  // step
  stepBegin: "green",
  stepInterrupted: "amber",
  // compaction
  compactionBegin: "orange",
  compactionEnd: "orange",
  compactionApplied: "orange",
  // mcp
  mcpLoadingBegin: "cyan",
  mcpLoadingEnd: "cyan",
  // status / notifications
  statusUpdate: "neutral",
  notification: "amber",
  // text
  textPart: "neutral",
  thinkPart: "neutral",
  planDisplay: "teal",
  // tools
  toolCall: "purple",
  toolResult: "purple",
  toolCallPart: "purple",
  toolCallRequest: "purple",
  // approvals
  questionRequest: "amber",
  approvalRequest: "amber",
  approvalResponse: "amber",
  approvalDecision: "amber",
  // sub-agents
  subagentEvent: "indigo",
  // media
  imageUrlPart: "pink",
  videoUrlPart: "pink",
  audioUrlPart: "pink",
  // klynt-internal-rich
  skillActivated: "teal",
  recallInjected: "indigo",
  providerCall: "cyan",
  fileEdit: "purple",
  testRun: "green",
  error: "neutral",
  gitCommit: "blue",
  mirrorAlert: "amber",
  sessionStart: "blue",
  sessionEnd: "blue",
  userPrompt: "blue",
  assistantMsg: "neutral",
};

export function eventChipColor(kind: string): ChipColor {
  return KIND_TO_COLOR[kind] ?? "neutral";
}

export function isErrorEvent(e: WireEventDto): boolean {
  if (e.kind === "error") return true;
  if (e.kind === "toolResult") {
    const rv = (e.payloadDecoded as any)?.return_value;
    if (rv && typeof rv === "object" && rv.is_error === true) return true;
  }
  if (e.kind === "stepInterrupted") return true;
  if (e.kind === "approvalDecision" || e.kind === "approvalResponse") {
    const r = (e.payloadDecoded as any)?.response;
    if (r === "reject") return true;
  }
  return false;
}

export function formatTimestamp(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    fractionalSecondDigits: 3,
  });
}

export function formatTimeDelta(current: Date, prev: Date): string {
  const ms = current.getTime() - prev.getTime();
  if (ms < 1) return "";
  if (ms < 1000) return `+${Math.round(ms)}ms`;
  const s = ms / 1000;
  if (s < 60) return `+${s.toFixed(2)}s`;
  return `+${(s / 60).toFixed(1)}min`;
}

export function timeDeltaSeverity(ms: number): "ok" | "warn" | "danger" {
  if (ms > 60_000) return "danger";
  if (ms > 10_000) return "warn";
  return "ok";
}

export function summarizeEvent(e: WireEventDto): string {
  const p = (e.payloadDecoded ?? {}) as Record<string, unknown>;
  switch (e.kind) {
    case "turnBegin":
    case "userPrompt":
      return truncate(extractText((p as any).user_input ?? (p as any).text), 120);
    case "toolCall":
      return String((p as any).function?.name ?? (p as any).tool_name ?? "");
    case "toolResult":
      return truncate(JSON.stringify((p as any).return_value ?? p), 160);
    case "thinkPart":
      return truncate(String((p as any).think ?? ""), 120);
    case "textPart":
      return truncate(String((p as any).text ?? ""), 120);
    default:
      return truncate(JSON.stringify(p), 120);
  }
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + "…" : s;
}
function extractText(x: unknown): string {
  if (typeof x === "string") return x;
  if (Array.isArray(x) && x.length > 0) {
    const first = x[0] as Record<string, unknown>;
    return String(first.text ?? "");
  }
  return "";
}
