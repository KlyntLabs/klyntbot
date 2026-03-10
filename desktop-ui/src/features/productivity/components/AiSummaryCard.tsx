interface AiSummaryCardProps {
  summary: string | null;
}

export function AiSummaryCard({ summary }: AiSummaryCardProps) {
  if (!summary) return null;

  return (
    <div className="glass-card p-4 flex flex-col gap-2">
      <h2 className="text-[13px] font-medium text-secondary">AI Summary</h2>
      <p className="text-[12px] font-light text-muted leading-relaxed">{summary}</p>
    </div>
  );
}
