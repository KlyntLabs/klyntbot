const TYPE_LABELS: Record<string, string> = {
  fact: "fact",
  rule: "rule",
  episode: "episode",
};

interface MemoryReferenceProps {
  refType: string;
  refId: string;
}

/** Renders an inline (type) label for a memory reference marker. */
export function MemoryReference({ refType }: MemoryReferenceProps) {
  const label = TYPE_LABELS[refType] ?? refType;

  return <span className="text-[11px] text-muted-foreground">({label})</span>;
}
