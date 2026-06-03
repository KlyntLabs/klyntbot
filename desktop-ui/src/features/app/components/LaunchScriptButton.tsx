import Play from "lucide-react/dist/esm/icons/play";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
import type { LaunchScriptIconId } from "@/types";
import { useMenuController } from "../hooks/useMenuController";
import { DEFAULT_LAUNCH_SCRIPT_ICON } from "../utils/launchScriptIcons";
import { LaunchScriptIconPicker } from "./LaunchScriptIconPicker";

type LaunchScriptButtonProps = {
  launchScript: string | null;
  editorOpen: boolean;
  draftScript: string;
  isSaving: boolean;
  error: string | null;
  onRun: () => void;
  onOpenEditor: () => void;
  onCloseEditor: () => void;
  onDraftChange: (value: string) => void;
  onSave: () => void;
  showNew?: boolean;
  newEditorOpen?: boolean;
  newDraftScript?: string;
  newDraftIcon?: LaunchScriptIconId;
  newDraftLabel?: string;
  newError?: string | null;
  onOpenNew?: () => void;
  onCloseNew?: () => void;
  onNewDraftChange?: (value: string) => void;
  onNewDraftIconChange?: (value: LaunchScriptIconId) => void;
  onNewDraftLabelChange?: (value: string) => void;
  onCreateNew?: () => void;
};

export function LaunchScriptButton({
  launchScript,
  editorOpen,
  draftScript,
  isSaving,
  error,
  onRun,
  onOpenEditor,
  onCloseEditor,
  onDraftChange,
  onSave,
  showNew = false,
  newEditorOpen = false,
  newDraftScript = "",
  newDraftIcon = DEFAULT_LAUNCH_SCRIPT_ICON,
  newDraftLabel = "",
  newError = null,
  onOpenNew,
  onCloseNew,
  onNewDraftChange,
  onNewDraftIconChange,
  onNewDraftLabelChange,
  onCreateNew,
}: LaunchScriptButtonProps) {
  const editorMenu = useMenuController({
    open: editorOpen,
    onDismiss: () => {
      onCloseEditor();
      onCloseNew?.();
    },
  });
  const { containerRef: popoverRef } = editorMenu;
  const hasLaunchScript = Boolean(launchScript?.trim());

  return (
    <div className="relative" ref={popoverRef}>
      <div className="inline-flex items-center gap-0.5">
        <button
          type="button"
          className="ghost main-header-action ds-tooltip-trigger"
          onClick={onRun}
          onContextMenu={(event) => {
            event.preventDefault();
            onOpenEditor();
          }}
          data-tauri-drag-region="false"
          aria-label={hasLaunchScript ? "Run launch script" : "Set launch script"}
          title={hasLaunchScript ? "Run launch script" : "Set launch script"}
          data-tooltip={hasLaunchScript ? "Run launch script" : "Set launch script"}
          data-tooltip-placement="bottom"
        >
          <Play size={14} aria-hidden />
        </button>
      </div>
      {editorOpen && (
        <PopoverSurface
          className="absolute right-0 top-[calc(100%+8px)] min-w-[240px] p-3 grid gap-3 z-[10]"
          role="dialog"
        >
          <div className="text-ui-sm font-semibold text-text-stronger">Launch script</div>
          <textarea
            className="w-full rounded-lg border border-border-muted bg-surface-control text-text-strong text-ui-sm p-2 outline-none resize-y min-h-[96px] [-webkit-app-region:no-drag] placeholder:text-text-faint focus-visible:border-border-strong focus-visible:ring-2 focus-visible:ring-border-accent"
            placeholder="e.g. npm run dev"
            value={draftScript}
            onChange={(event) => onDraftChange(event.target.value)}
            rows={6}
            data-tauri-drag-region="false"
          />
          {error && <div className="mt-2 text-ui-xs text-status-error">{error}</div>}
          <div className="flex gap-2 justify-end flex-wrap">
            <button
              type="button"
              className="ghost"
              onClick={() => {
                onCloseEditor();
                onCloseNew?.();
              }}
              data-tauri-drag-region="false"
            >
              Cancel
            </button>
            {showNew && onOpenNew && (
              <button
                type="button"
                className="ghost"
                onClick={onOpenNew}
                data-tauri-drag-region="false"
              >
                New
              </button>
            )}
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
          {showNew && newEditorOpen && onNewDraftChange && onNewDraftIconChange && onCreateNew && (
            <div className="mt-2 pt-3 border-t border-border-subtle grid gap-3">
              <div className="text-ui-sm font-semibold text-text-stronger">New launch script</div>
              <LaunchScriptIconPicker value={newDraftIcon} onChange={onNewDraftIconChange} />
              <input
                className="w-full rounded-lg border border-border-muted bg-surface-control text-text-strong text-ui-sm p-2 outline-none [-webkit-app-region:no-drag] placeholder:text-text-faint focus-visible:border-border-strong focus-visible:ring-2 focus-visible:ring-border-accent"
                type="text"
                placeholder="Optional label"
                value={newDraftLabel}
                onChange={(event) => onNewDraftLabelChange?.(event.target.value)}
                data-tauri-drag-region="false"
              />
              <textarea
                className="w-full rounded-lg border border-border-muted bg-surface-control text-text-strong text-ui-sm p-2 outline-none resize-y min-h-[80px] [-webkit-app-region:no-drag] placeholder:text-text-faint focus-visible:border-border-strong focus-visible:ring-2 focus-visible:ring-border-accent"
                placeholder="e.g. npm run dev"
                value={newDraftScript}
                onChange={(event) => onNewDraftChange(event.target.value)}
                rows={5}
                data-tauri-drag-region="false"
              />
              {newError && <div className="mt-2 text-ui-xs text-status-error">{newError}</div>}
              <div className="flex gap-2 justify-end flex-wrap">
                <button
                  type="button"
                  className="ghost"
                  onClick={onCloseNew}
                  data-tauri-drag-region="false"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="primary"
                  onClick={onCreateNew}
                  disabled={isSaving}
                  data-tauri-drag-region="false"
                >
                  {isSaving ? "Saving..." : "Create"}
                </button>
              </div>
            </div>
          )}
        </PopoverSurface>
      )}
    </div>
  );
}
