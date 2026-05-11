import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelPlanMode,
  editPlanItems,
  openPlanFile,
  ratifyPlan,
  removePlanItems,
} from "@/api/endpoints/coding";
import { type TodoItem, useTodos } from "../state/todoStore";

type ConfirmAction = null | "ratify" | "cancel";

function PlanItemRow({
  item,
  onRemove,
  onTitleEdit,
}: {
  item: TodoItem;
  onRemove: () => void;
  onTitleEdit: (title: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(item.title);

  const handleBlur = useCallback(() => {
    setEditing(false);
    if (draft !== item.title) onTitleEdit(draft);
  }, [draft, item.title, onTitleEdit]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        (e.target as HTMLInputElement).blur();
      } else if (e.key === "Escape") {
        setDraft(item.title);
        setEditing(false);
      }
    },
    [item.title],
  );

  return (
    <li className="coding-todo__plan-banner-row">
      {editing ? (
        <input
          className="coding-todo__plan-banner-title-edit"
          value={draft}
          autoFocus
          onChange={(e) => setDraft(e.target.value)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
        />
      ) : (
        <span
          className="coding-todo__plan-banner-title"
          onClick={() => setEditing(true)}
          title="Click to edit"
        >
          {item.title}
        </span>
      )}
      <span className="coding-todo__plan-banner-concurrency" title="Concurrency class">
        {item.concurrency}
      </span>
      <button
        className="coding-todo__plan-banner-remove"
        aria-label={`Remove ${item.title}`}
        onClick={onRemove}
      >
        ×
      </button>
    </li>
  );
}

export function PlanModeBanner({ threadId }: { threadId: string }) {
  const { items, planModeState } = useTodos(threadId);
  const [confirming, setConfirming] = useState<ConfirmAction>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clean up debounce on unmount.
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  const removeItem = useCallback(
    async (itemId: string) => {
      if (!planModeState) return;
      await removePlanItems(threadId, planModeState.planSessionId, [itemId]);
    },
    [threadId, planModeState],
  );

  const editTitle = useCallback(
    (itemId: string, title: string) => {
      if (!planModeState) return;
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        const next = items.map((i) => (i.id === itemId ? { ...i, title } : i));
        await editPlanItems(threadId, planModeState.planSessionId, JSON.stringify(next));
      }, 400);
    },
    [threadId, planModeState, items],
  );

  const ratify = useCallback(async () => {
    if (!planModeState) return;
    await ratifyPlan(threadId, planModeState.planSessionId);
  }, [threadId, planModeState]);

  const cancelPlan = useCallback(async () => {
    await cancelPlanMode(threadId);
  }, [threadId]);

  const handleOpenFile = useCallback(() => {
    if (!planModeState) return;
    openPlanFile(planModeState.planFilePath);
  }, [planModeState]);

  const handleConfirm = useCallback(async () => {
    if (confirming === "ratify") await ratify();
    else if (confirming === "cancel") await cancelPlan();
    setConfirming(null);
  }, [confirming, ratify, cancelPlan]);

  if (!planModeState) return null;

  return (
    <div className="coding-todo__plan-banner">
      <div className="coding-todo__plan-banner-header">
        <button
          className="coding-todo__plan-banner-title-link"
          onClick={handleOpenFile}
          title={planModeState.planFilePath}
        >
          Plan mode · {planModeState.planFileSlug}.md
        </button>
        <button
          className="coding-todo__plan-banner-close"
          aria-label="Close plan mode"
          onClick={() => setConfirming("cancel")}
        >
          ×
        </button>
      </div>
      <div className="coding-todo__plan-banner-summary">
        Reviewing {items.length} proposed {items.length === 1 ? "item" : "items"}
      </div>
      <ul className="coding-todo__plan-banner-list">
        {items.map((item) => (
          <PlanItemRow
            key={item.id}
            item={item}
            onRemove={() => removeItem(item.id)}
            onTitleEdit={(t) => editTitle(item.id, t)}
          />
        ))}
      </ul>
      {confirming === null && (
        <div className="coding-todo__plan-banner-actions">
          <button
            className="coding-todo__plan-banner-primary"
            onClick={() => setConfirming("ratify")}
          >
            Ratify & Execute
          </button>
          <button
            className="coding-todo__plan-banner-danger"
            onClick={() => setConfirming("cancel")}
          >
            Cancel Plan
          </button>
        </div>
      )}
      {confirming !== null && (
        <div className="coding-todo__plan-banner-confirm">
          <span>
            {confirming === "ratify"
              ? `Ratify ${items.length} ${items.length === 1 ? "item" : "items"}?`
              : "Cancel plan and discard proposed items?"}
          </span>
          <button onClick={handleConfirm}>Confirm</button>
          <button onClick={() => setConfirming(null)}>Back</button>
        </div>
      )}
    </div>
  );
}
