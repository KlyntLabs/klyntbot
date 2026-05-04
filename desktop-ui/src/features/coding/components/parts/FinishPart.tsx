import type { FinishReason } from "./types";

function formatFinishReason(reason: FinishReason): string {
  switch (reason.kind) {
    case "completed":
      return "Completed";
    case "tool_calls_exhausted":
      return "Tool call limit reached";
    case "cancelled":
      return "Cancelled";
    case "permission_denied":
      return `Permission denied: ${reason.reason}`;
    case "sandbox_violation":
      return `Sandbox violation: ${reason.reason}`;
    case "cost_ceiling_reached":
      return `Cost ceiling reached: $${reason.spend_usd.toFixed(2)} / $${reason.ceiling_usd.toFixed(2)}`;
    case "error":
      return `Error (${reason.code}): ${reason.message}`;
  }
}

export function FinishPart({ reason }: { reason: FinishReason }) {
  return (
    <div className={`part-finish part-finish--${reason.kind}`}>
      <span className="part-finish__label">{formatFinishReason(reason)}</span>
    </div>
  );
}
