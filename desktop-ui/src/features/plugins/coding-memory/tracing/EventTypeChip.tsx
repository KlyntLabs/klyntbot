interface Props {
  rawKind: string;
  active?: boolean;
  onClick?: () => void;
}

const PALETTE: Record<string, string> = {
  TurnBegin: "var(--type-blue, #6aaaff)",
  TurnEnd: "var(--type-blue, #6aaaff)",
  StepBegin: "var(--type-green, #69d18a)",
  StepInterrupted: "var(--type-yellow, #d4a83a)",
  ContentPart: "var(--type-purple, #a07cff)",
  ThinkPart: "var(--type-purple, #a07cff)",
  TextPart: "var(--type-purple, #a07cff)",
  ToolCall: "var(--type-violet, #c060ff)",
  ToolCallPart: "var(--type-violet, #c060ff)",
  ToolResult: "var(--type-violet, #c060ff)",
  StatusUpdate: "var(--type-orange, #d68a3a)",
  SubagentEvent: "var(--type-blue, #6aaaff)",
  CompactionBegin: "var(--type-orange, #d68a3a)",
  CompactionEnd: "var(--type-orange, #d68a3a)",
  Error: "var(--accent-red, #ff6b6b)",
};

export function EventTypeChip({ rawKind, active, onClick }: Props) {
  const color = PALETTE[rawKind] ?? "var(--text-secondary)";
  return (
    <button
      type="button"
      className={"tracing-chip" + (active ? " tracing-chip--active" : "")}
      style={{ borderColor: color, color }}
      onClick={onClick}
    >
      {rawKind}
    </button>
  );
}
