import { useTodos } from "../state/todoStore";

export function PlanModeBanner({ threadId }: { threadId: string }) {
  const { planMode, planSessionId } = useTodos(threadId);
  if (!planMode) return null;

  return (
    <div className="flex items-center gap-2 rounded-md bg-amber-500/10 border border-amber-500/20 px-3 py-2 text-sm text-amber-700">
      <span className="font-medium">Plan Mode</span>
      <span className="text-muted-foreground">
        Session: {planSessionId?.slice(0, 8) ?? "—"}
      </span>
    </div>
  );
}
