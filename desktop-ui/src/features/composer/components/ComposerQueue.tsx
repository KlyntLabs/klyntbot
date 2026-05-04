import { useMenuController } from "@app/hooks/useMenuController";
import type { MouseEvent as ReactMouseEvent } from "react";
import { memo, useCallback } from "react";
import {
  PopoverMenuItem,
  PopoverSurface,
} from "@/features/design-system/components/popover/PopoverPrimitives";
import type { QueuedMessage } from "@/types";

type ComposerQueueProps = {
  queuedMessages: QueuedMessage[];
  pausedReason?: string | null;
  onEditQueued?: (item: QueuedMessage) => void;
  onDeleteQueued?: (id: string) => void;
};

export const ComposerQueue = memo(function ComposerQueue({
  queuedMessages,
  pausedReason = null,
  onEditQueued,
  onDeleteQueued,
}: ComposerQueueProps) {
  if (queuedMessages.length === 0) {
    return null;
  }

  return (
    <div className="composer-queue">
      <div className="composer-queue-title">Queued</div>
      {pausedReason ? <div className="composer-queue-hint">{pausedReason}</div> : null}
      <div className="composer-queue-list">
        {queuedMessages.map((item) => (
          <div key={item.id} className="composer-queue-item">
            <span className="composer-queue-text">
              {item.text ||
                (item.images?.length ? (item.images.length === 1 ? "Image" : "Images") : "")}
              {item.images?.length
                ? ` · ${item.images.length} image${item.images.length === 1 ? "" : "s"}`
                : ""}
            </span>
            <QueueMenuButton
              item={item}
              onEditQueued={onEditQueued}
              onDeleteQueued={onDeleteQueued}
            />
          </div>
        ))}
      </div>
    </div>
  );
});

type QueueMenuButtonProps = {
  item: QueuedMessage;
  onEditQueued?: (item: QueuedMessage) => void;
  onDeleteQueued?: (id: string) => void;
};

const QueueMenuButton = memo(function QueueMenuButton({
  item,
  onEditQueued,
  onDeleteQueued,
}: QueueMenuButtonProps) {
  const menu = useMenuController();
  const handleToggleMenu = useCallback(
    (event: ReactMouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      menu.toggle();
    },
    [menu],
  );

  const handleEdit = useCallback(() => {
    menu.close();
    onEditQueued?.(item);
  }, [item, menu, onEditQueued]);

  const handleDelete = useCallback(() => {
    menu.close();
    onDeleteQueued?.(item.id);
  }, [item.id, menu, onDeleteQueued]);

  return (
    <div className="composer-queue-menu-wrap" ref={menu.containerRef}>
      <button
        type="button"
        className={`composer-queue-menu${menu.isOpen ? " is-open" : ""}`}
        onClick={handleToggleMenu}
        aria-label="Queue item menu"
        aria-haspopup="menu"
        aria-expanded={menu.isOpen}
      >
        ...
      </button>
      {menu.isOpen && (
        <PopoverSurface className="composer-queue-item-popover" role="menu">
          <PopoverMenuItem onClick={handleEdit}>Edit</PopoverMenuItem>
          <PopoverMenuItem onClick={handleDelete}>Delete</PopoverMenuItem>
        </PopoverSurface>
      )}
    </div>
  );
});
