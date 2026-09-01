import type { AppView } from "@app/constants/appViews";
import { type CSSProperties, useMemo } from "react";
import type { AppSettings } from "@/types";

type UseAppShellOrchestrationOptions = {
  sidebarCollapsed: boolean;
  rightPanelCollapsed: boolean;
  shouldReduceTransparency: boolean;
  isWorkspaceDropActive: boolean;
  centerMode: "chat" | "diff" | "calendar";
  appView: AppView;
  selectedDiffPath: string | null;
  showComposer: boolean;
  activeThreadId: string | null;
  sidebarWidth: number;
  rightPanelWidth: number;
  chatDiffSplitPositionPercent: number;
  planPanelHeight: number;
  terminalPanelHeight: number;
  debugPanelHeight: number;
  appSettings: Pick<AppSettings, "uiFontFamily" | "codeFontFamily" | "codeFontSize">;
};

export function useAppShellOrchestration({
  sidebarCollapsed,
  rightPanelCollapsed,
  shouldReduceTransparency,
  isWorkspaceDropActive,
  centerMode,
  appView,
  selectedDiffPath,
  showComposer,
  activeThreadId,
  sidebarWidth,
  rightPanelWidth,
  chatDiffSplitPositionPercent,
  planPanelHeight,
  terminalPanelHeight,
  debugPanelHeight,
  appSettings,
}: UseAppShellOrchestrationOptions) {
  const showGitDetail = Boolean(selectedDiffPath) && centerMode === "diff";
  const isThreadOpen = Boolean(activeThreadId && showComposer);

  const appClassName = `app layout-desktop${
    shouldReduceTransparency ? " reduced-transparency" : ""
  }${sidebarCollapsed ? " sidebar-collapsed" : ""}${
    rightPanelCollapsed ? " right-panel-collapsed" : ""
  }${appView === "calendar" ? " is-calendar" : ""}`;

  const appStyle = useMemo<CSSProperties>(
    () =>
      ({
        "--sidebar-width": `${sidebarCollapsed ? 0 : sidebarWidth}px`,
        "--right-panel-width": `${rightPanelCollapsed ? 0 : rightPanelWidth}px`,
        "--chat-diff-split-position-percent": `${chatDiffSplitPositionPercent}%`,
        "--plan-panel-height": `${planPanelHeight}px`,
        "--terminal-panel-height": `${terminalPanelHeight}px`,
        "--debug-panel-height": `${debugPanelHeight}px`,
        "--ui-font-family": appSettings.uiFontFamily,
        "--code-font-family": appSettings.codeFontFamily,
        "--code-font-size": `${appSettings.codeFontSize}px`,
        "--sidebar-top-padding": "36px",
        "--right-panel-top-padding": "12px",
        "--home-scroll-offset": "0px",
        "--window-caption-width": "0px",
        "--window-caption-gap": "0px",
      }) as CSSProperties,
    [
      appSettings.codeFontFamily,
      appSettings.codeFontSize,
      appSettings.uiFontFamily,
      chatDiffSplitPositionPercent,
      debugPanelHeight,
      planPanelHeight,
      rightPanelCollapsed,
      rightPanelWidth,
      sidebarCollapsed,
      sidebarWidth,
      terminalPanelHeight,
    ],
  );

  return {
    showGitDetail,
    isThreadOpen,
    dropOverlayActive: isWorkspaceDropActive,
    dropOverlayText: "Drop Project Here",
    appClassName,
    appStyle,
  };
}
