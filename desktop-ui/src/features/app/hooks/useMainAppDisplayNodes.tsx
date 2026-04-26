import type { ComponentProps } from "react";
import { MainHeaderActions } from "@app/components/MainHeaderActions";
import { WorkspaceHome } from "@/features/workspaces/components/WorkspaceHome";

type UseMainAppDisplayNodesArgs = {
  centerMode: "chat" | "diff";
  gitDiffViewStyle: "split" | "unified";
  setGitDiffViewStyle: (style: "split" | "unified") => void;
  isCompact: boolean;
  rightPanelCollapsed: boolean;
  sidebarToggleProps: Parameters<typeof MainHeaderActions>[0]["sidebarToggleProps"];
  workspaceHomeProps: ComponentProps<typeof WorkspaceHome> | null;
};

export function useMainAppDisplayNodes({
  centerMode,
  gitDiffViewStyle,
  setGitDiffViewStyle,
  isCompact,
  rightPanelCollapsed,
  sidebarToggleProps,
  workspaceHomeProps,
}: UseMainAppDisplayNodesArgs) {
  const mainHeaderActionsNode = (
    <>
      <MainHeaderActions
        centerMode={centerMode}
        gitDiffViewStyle={gitDiffViewStyle}
        onSelectDiffViewStyle={setGitDiffViewStyle}
        isCompact={isCompact}
        rightPanelCollapsed={rightPanelCollapsed}
        sidebarToggleProps={sidebarToggleProps}
      />
    </>
  );

  const workspaceHomeNode = workspaceHomeProps ? <WorkspaceHome {...workspaceHomeProps} /> : null;

  return {
    mainHeaderActionsNode,
    workspaceHomeNode,
  };
}
