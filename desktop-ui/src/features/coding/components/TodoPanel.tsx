import { countBlocked, countInProgress, countPending, useTodos } from "../state/todoStore";

export function TodoPanel({ threadId }: { threadId: string }) {
  const { items, planModeState } = useTodos(threadId);
  const planMode = !!planModeState;

  const summary =
    items.length === 0
      ? null
      : `${countInProgress(items)} in progress · ${countPending(items)} pending · ${countBlocked(
          items,
        )} blocked`;

  return (
    <div className="coding-todo-panel">
      <div className="coding-todo-panel__header">
        <h3>
          Todo List
          {planMode && <span className="coding-todo-panel__plan-tag">Plan Mode</span>}
        </h3>
        {summary && <span className="coding-todo-panel__summary">{summary}</span>}
      </div>
      {items.length === 0 ? (
        <p className="coding-todo-panel__empty">No items yet.</p>
      ) : (
        <ul className="coding-todo-panel__list">
          {items.map((item) => (
            <li
              key={item.id}
              className={`coding-todo-panel__row coding-todo-panel__row--${item.status}`}
            >
              <StatusDot status={item.status} />
              <span className="coding-todo-panel__title" title={item.title}>
                {item.title}
              </span>
              {item.blockedReason && (
                <span className="coding-todo-panel__blocked" title={item.blockedReason}>
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
  return <span className={`coding-todo-panel__dot coding-todo-panel__dot--${status}`} />;
}
