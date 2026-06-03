import AlignLeft from "lucide-react/dist/esm/icons/align-left";
import Columns2 from "lucide-react/dist/esm/icons/columns-2";
import { memo } from "react";
import { cn } from "@/utils/cn";
import type { SidebarToggleProps } from "@/features/layout/components/SidebarToggleControls";
import {
  RightPanelCollapseButton,
  RightPanelExpandButton,
} from "@/features/layout/components/SidebarToggleControls";

type MainHeaderActionsProps = {
  centerMode: "chat" | "diff";
  gitDiffViewStyle: "split" | "unified";
  onSelectDiffViewStyle: (style: "split" | "unified") => void;
  isCompact: boolean;
  rightPanelCollapsed: boolean;
  sidebarToggleProps: SidebarToggleProps;
};

export const MainHeaderActions = memo(function MainHeaderActions({
  centerMode,
  gitDiffViewStyle,
  onSelectDiffViewStyle,
  isCompact,
  rightPanelCollapsed,
  sidebarToggleProps,
}: MainHeaderActionsProps) {
  return (
    <>
      {centerMode === "diff" && (
        <fieldset
          className="diff-view-toggle inline-flex items-center border border-border-strong rounded-lg overflow-hidden"
          aria-label="Diff view"
        >
          <button
            type="button"
            className={cn(
              "diff-view-toggle-button inline-flex items-center justify-center border-0 rounded-none bg-transparent text-text-muted p-[6px] transition-colors duration-[120ms] ease-out",
              "hover:not(:disabled):bg-surface-control-hover hover:not(:disabled):text-text-stronger hover:not(:disabled):shadow-none",
              "[&+&]:border-l [&+&]:border-border-subtle",
              gitDiffViewStyle === "split" && "is-active bg-white/[0.08] text-text-stronger",
              "ds-tooltip-trigger",
            )}
            onClick={() => onSelectDiffViewStyle("split")}
            aria-pressed={gitDiffViewStyle === "split"}
            title="Dual-panel diff"
            data-tooltip="Dual-panel diff"
            data-tooltip-placement="bottom"
            data-tauri-drag-region="false"
          >
            <Columns2 size={14} aria-hidden />
          </button>
          <button
            type="button"
            className={cn(
              "diff-view-toggle-button inline-flex items-center justify-center border-0 rounded-none bg-transparent text-text-muted p-[6px] transition-colors duration-[120ms] ease-out",
              "hover:not(:disabled):bg-surface-control-hover hover:not(:disabled):text-text-stronger hover:not(:disabled):shadow-none",
              "[&+&]:border-l [&+&]:border-border-subtle",
              gitDiffViewStyle === "unified" && "is-active bg-white/[0.08] text-text-stronger",
              "ds-tooltip-trigger",
            )}
            onClick={() => onSelectDiffViewStyle("unified")}
            aria-pressed={gitDiffViewStyle === "unified"}
            title="Single-column diff"
            data-tooltip="Single-column diff"
            data-tooltip-placement="bottom"
            data-tauri-drag-region="false"
          >
            <AlignLeft size={14} aria-hidden />
          </button>
        </fieldset>
      )}
      {!isCompact ? (
        rightPanelCollapsed ? (
          <RightPanelExpandButton {...sidebarToggleProps} />
        ) : (
          <RightPanelCollapseButton {...sidebarToggleProps} />
        )
      ) : null}
    </>
  );
});
