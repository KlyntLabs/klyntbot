import { Button } from "../../primitives/button";
import { Dialog } from "../dialog";
import type { ConfirmDialogProps } from "./ConfirmDialog.types";

export type { ConfirmDialogProps };

/** Confirm / cancel prompt built on Dialog. */
export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = false,
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onClose={onClose} title={title} size="sm">
      <div className="flex flex-col gap-4">
        <p className="text-ui text-fg-secondary">{message}</p>
        <div className="flex items-center justify-end gap-2">
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            {cancelLabel}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="primary"
            className={
              destructive
                ? "bg-status-danger hover:bg-status-danger/90 active:bg-status-danger/80"
                : undefined
            }
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
