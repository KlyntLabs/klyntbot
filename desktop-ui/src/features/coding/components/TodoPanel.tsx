import { useTodos } from "../state/todoStore";

export function TodoPanel({ threadId }: { threadId: string }) {
  const { items, planMode } = useTodos(threadId);

  return (
    <div className="flex flex-col gap-2 p-3 text-sm">
      <div className="font-medium text-foreground">
        Todo List {planMode && <span className="text-amber-500">(Plan Mode)</span>}
      </div>
      {items.length === 0 ? (
        <div className="text-muted-foreground">No items yet.</div>
      ) : (
        <ul className="flex flex-col gap-1">
          {items.map((item) => (
            <li
              key={item.id}
              className="flex items-center gap-2 rounded px-2 py-1 hover:bg-muted/50"
            >
              <StatusDot status={item.status} />
              <span className="flex-1 truncate">{item.title}</span>
              {item.blockedReason && (
                <span className="text-xs text-amber-500 truncate max-w-[120px]">
                  {item.blockedReason}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function StatusDot({ status }: { status: string }) {
  const color =
    status === "done"
      ? "bg-green-500"
      : status === "in_progress"
        ? "bg-blue-500"
        : status === "blocked"
          ? "bg-red-500"
          : "bg-gray-400";
  return <span className={`inline-block w-2 h-2 rounded-full ${color}`} />;
}
