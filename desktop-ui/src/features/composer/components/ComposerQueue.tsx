import { useMenuController } from "@app/hooks/useMenuController";
import type { MouseEvent as ReactMouseEvent } from "react";
import { memo, useCallback } from "react";
import { cn } from "@/utils/cn";
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
    <div className="flex flex-col gap-2 p-[10px_12px] rounded-[16px] bg-[var(--cm-surface-panel)] border border-[var(--cm-border-emphasis)]">
      <div className="text-ui-2xs uppercase tracking-[0.1em] text-text-fainter">Queued</div>
      {pausedReason ? <div className="text-ui-xs leading-[1.4] text-text-faint">{pausedReason}</div> : null}
      <div className="flex flex-col gap-1">
        {queuedMessages.map((item) => (
          <div key={item.id} className="flex items-center gap-2 p-1 px-1.5 rounded-lg bg-surface-item text-text-quiet text-ui-xs">
            <span className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
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
    <div className="relative flex-shrink-0" ref={menu.containerRef}>
      <button
        type="button"
        className={cn("composer-queue-menu text-text-faint text-ui-xs px-1 py-0.5 cursor-pointer border-0 bg-transparent", menu.isOpen && "is-open")}
        onClick={handleToggleMenu}
        aria-label="Queue item menu"
        aria-haspopup="menu"
        aria-expanded={menu.isOpen}
      >
        ...
      </button>
      {menu.isOpen && (
        <PopoverSurface className="absolute right-0 bottom-[calc(100%+4px)] min-w-[110px] p-1 z-40" role="menu">
          <PopoverMenuItem onClick={handleEdit}>Edit</PopoverMenuItem>
          <PopoverMenuItem onClick={handleDelete}>Delete</PopoverMenuItem>
        </PopoverSurface>
      )}
    </div>
  );
});
