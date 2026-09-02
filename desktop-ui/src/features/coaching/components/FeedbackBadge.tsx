const feedbackColors: Record<string, string> = {
  helpful: "text-status-success bg-status-success/10",
  dismissed: "text-status-warning bg-status-warning/10",
  stop: "text-status-danger bg-status-danger/10",
  ignored: "text-fg-dim bg-control-hover/30",
};

export function FeedbackBadge({ feedback }: { feedback: string | null }) {
  if (!feedback) return null;
  return (
    <span
      className={`text-[9px] px-1.5 py-0.5 rounded-full ${feedbackColors[feedback] ?? "text-fg-dim"}`}
    >
      {feedback}
    </span>
  );
}
