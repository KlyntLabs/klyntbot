import type { MouseEvent, ReactNode } from "react";
import { memo } from "react";
import { DesktopLayout } from "@/features/layout/components/DesktopLayout";

type AppLayoutProps = {
  showHome: boolean;
  centerMode: "chat" | "diff" | "calendar";
  preloadGitDiffs: boolean;
  splitChatDiffView: boolean;
  hasActivePlan: boolean;
  activeWorkspace: boolean;
  sidebarNode: ReactNode;
  messagesNode: ReactNode;
  composerNode: ReactNode;
  approvalToastsNode: ReactNode;
  updateToastNode: ReactNode;
  errorToastsNode: ReactNode;
  homeNode: ReactNode;
  dashboardNode?: ReactNode;
  desktopTopbarLeftNode: ReactNode;
  topbarActionsNode?: ReactNode;
  gitDiffPanelNode: ReactNode;
  gitDiffViewerNode: ReactNode;
  planPanelNode: ReactNode;
  debugPanelNode: ReactNode;
  terminalDockNode: ReactNode;
  onSidebarResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onChatDiffSplitPositionResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onRightPanelResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onPlanPanelResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
};

export const AppLayout = memo(function AppLayout({
  showHome,
  centerMode,
  preloadGitDiffs,
  splitChatDiffView,
  hasActivePlan,
  activeWorkspace,
  sidebarNode,
  messagesNode,
  composerNode,
  approvalToastsNode,
  updateToastNode,
  errorToastsNode,
  homeNode,
  dashboardNode,
  desktopTopbarLeftNode,
  topbarActionsNode,
  gitDiffPanelNode,
  gitDiffViewerNode,
  planPanelNode,
  debugPanelNode,
  terminalDockNode,
  onSidebarResizeStart,
  onChatDiffSplitPositionResizeStart,
  onRightPanelResizeStart,
  onPlanPanelResizeStart,
}: AppLayoutProps) {
  return (
    <DesktopLayout
      sidebarNode={sidebarNode}
      updateToastNode={updateToastNode}
      approvalToastsNode={approvalToastsNode}
      errorToastsNode={errorToastsNode}
      homeNode={homeNode}
      dashboardNode={dashboardNode}
      showHome={showHome}
      showWorkspace={activeWorkspace && !showHome}
      topbarLeftNode={desktopTopbarLeftNode}
      topbarActionsNode={topbarActionsNode}
      centerMode={centerMode}
      preloadGitDiffs={preloadGitDiffs}
      splitChatDiffView={splitChatDiffView}
      messagesNode={messagesNode}
      gitDiffViewerNode={gitDiffViewerNode}
      gitDiffPanelNode={gitDiffPanelNode}
      planPanelNode={planPanelNode}
      composerNode={composerNode}
      terminalDockNode={terminalDockNode}
      debugPanelNode={debugPanelNode}
      hasActivePlan={hasActivePlan}
      onSidebarResizeStart={onSidebarResizeStart}
      onChatDiffSplitPositionResizeStart={onChatDiffSplitPositionResizeStart}
      onRightPanelResizeStart={onRightPanelResizeStart}
      onPlanPanelResizeStart={onPlanPanelResizeStart}
    />
  );
});
