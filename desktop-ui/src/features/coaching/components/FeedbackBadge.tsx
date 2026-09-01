const feedbackColors: Record<string, string> = {
  helpful: "text-success bg-success/10",
  dismissed: "text-warning bg-warning/10",
  stop: "text-destructive bg-destructive/10",
  ignored: "text-dim bg-accent/30",
};

export function FeedbackBadge({ feedback }: { feedback: string | null }) {
  if (!feedback) return null;
  return (
    <span
      className={`text-[9px] px-1.5 py-0.5 rounded-full ${feedbackColors[feedback] ?? "text-dim"}`}
    >
      {feedback}
    </span>
  );
}
