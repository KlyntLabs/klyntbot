import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
import type { LaunchScriptEntry, LaunchScriptIconId } from "@/types";
import { useMenuController } from "../hooks/useMenuController";
import { getLaunchScriptIcon, getLaunchScriptIconLabel } from "../utils/launchScriptIcons";
import { LaunchScriptIconPicker } from "./LaunchScriptIconPicker";

type LaunchScriptEntryButtonProps = {
  entry: LaunchScriptEntry;
  editorOpen: boolean;
  draftScript: string;
  draftIcon: LaunchScriptIconId;
  draftLabel: string;
  isSaving: boolean;
  error: string | null;
  onRun: () => void;
  onOpenEditor: () => void;
  onCloseEditor: () => void;
  onDraftChange: (value: string) => void;
  onDraftIconChange: (value: LaunchScriptIconId) => void;
  onDraftLabelChange: (value: string) => void;
  onSave: () => void;
  onDelete: () => void;
};

export function LaunchScriptEntryButton({
  entry,
  editorOpen,
  draftScript,
  draftIcon,
  draftLabel,
  isSaving,
  error,
  onRun,
  onOpenEditor,
  onCloseEditor,
  onDraftChange,
  onDraftIconChange,
  onDraftLabelChange,
  onSave,
  onDelete,
}: LaunchScriptEntryButtonProps) {
  const editorMenu = useMenuController({
    open: editorOpen,
    onDismiss: onCloseEditor,
  });
  const { containerRef: popoverRef } = editorMenu;
  const Icon = getLaunchScriptIcon(entry.icon);
  const iconLabel = getLaunchScriptIconLabel(entry.icon);

  return (
    <div className="relative" ref={popoverRef}>
      <div className="inline-flex items-center gap-0.5">
        <button
          type="button"
          className="ghost main-header-action launch-script-run ds-tooltip-trigger"
          onClick={onRun}
          onContextMenu={(event) => {
            event.preventDefault();
            onOpenEditor();
          }}
          data-tauri-drag-region="false"
          aria-label={entry.label?.trim() || iconLabel}
          title={entry.label?.trim() || iconLabel}
          data-tooltip={entry.label?.trim() || iconLabel}
          data-tooltip-placement="bottom"
        >
          <Icon size={14} aria-hidden />
        </button>
      </div>
      {editorOpen && (
        <PopoverSurface className="absolute right-0 top-[calc(100%+8px)] min-w-[240px] p-3 z-5" role="dialog">
          <div className="text-ui-sm font-semibold text-text-stronger mb-2">{entry.label?.trim() || "Launch script"}</div>
          <LaunchScriptIconPicker value={draftIcon} onChange={onDraftIconChange} />
          <input
            className="w-full rounded-lg border border-border-muted bg-surface-control text-text-strong text-ui-sm px-2 py-1.5 outline-none mb-2 [-webkit-app-region:no-drag]"
            type="text"
            placeholder="Optional label"
            value={draftLabel}
            onChange={(event) => onDraftLabelChange(event.target.value)}
            data-tauri-drag-region="false"
          />
          <textarea
            className="w-full rounded-lg border border-border-muted bg-surface-control text-text-strong text-ui-sm p-2 outline-none resize-y min-h-[96px] [-webkit-app-region:no-drag]"
            placeholder="e.g. npm run dev"
            value={draftScript}
            onChange={(event) => onDraftChange(event.target.value)}
            rows={6}
            data-tauri-drag-region="false"
          />
          {error && <div className="mt-2 text-ui-xs text-status-error">{error}</div>}
          <div className="mt-2.5 flex justify-end gap-2">
            <button
              type="button"
              className="ghost"
              onClick={onCloseEditor}
              data-tauri-drag-region="false"
            >
              Cancel
            </button>
            <button
              type="button"
              className="ghost text-status-error"
              onClick={onDelete}
              data-tauri-drag-region="false"
            >
              Delete
            </button>
            <button
              type="button"
              className="primary"
              onClick={onSave}
              disabled={isSaving}
              data-tauri-drag-region="false"
            >
              {isSaving ? "Saving..." : "Save"}
            </button>
          </div>
        </PopoverSurface>
      )}
    </div>
  );
}
