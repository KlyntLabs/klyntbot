import { countBlocked, countPending, useTodos } from "../state/todoStore";

export function TodoSidebarBadge({ threadId }: { threadId: string }) {
  const { items } = useTodos(threadId);
  const pending = countPending(items);
  const blocked = countBlocked(items);

  if (items.length === 0) return null;

  return (
    <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
      <span className="inline-block w-1.5 h-1.5 rounded-full bg-amber-400" />
      {pending > 0 && `${pending} pending`}
      {blocked > 0 && ` · ${blocked} blocked`}
    </span>
  );
}
