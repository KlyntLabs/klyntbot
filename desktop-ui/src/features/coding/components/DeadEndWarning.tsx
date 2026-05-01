export function DeadEndWarning({
  approachSummary,
  confidence,
}: {
  approachSummary: string;
  priorAttemptId: string;
  confidence: number;
}) {
  return (
    <div className="dead-end-warning" role="alert">
      <span className="dead-end-warning__icon">⚠</span>
      <span className="dead-end-warning__body">
        Prior attempt: {approachSummary} ({Math.round(confidence * 100)}% confidence dead-end)
      </span>
    </div>
  );
}
