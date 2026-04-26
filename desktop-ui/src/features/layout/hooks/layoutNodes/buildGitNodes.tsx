import { FileTreePanel } from "@/features/files/components/FileTreePanel";
import { GitDiffPanel } from "@/features/git/components/GitDiffPanel";
import { GitDiffViewer } from "@/features/git/components/GitDiffViewer";
import { PromptPanel } from "@/features/prompts/components/PromptPanel";
import type {
  LayoutGitSurface,
  LayoutNodesResult,
} from "./types";

export type GitLayoutNodesOptions = LayoutGitSurface;

type GitLayoutNodes = Pick<LayoutNodesResult, "gitDiffPanelNode" | "gitDiffViewerNode">;

function resolveGitDiffStyle({
  splitChatDiffView,
  centerMode,
  userPreference,
}: {
  splitChatDiffView: boolean;
  centerMode: GitLayoutNodesOptions["diffViewProps"]["centerMode"];
  userPreference: GitLayoutNodesOptions["diffViewProps"]["gitDiffViewStyle"];
}): GitLayoutNodesOptions["diffViewProps"]["gitDiffViewStyle"] {
  const shouldForceSingleColumn = splitChatDiffView && centerMode === "chat";
  return shouldForceSingleColumn ? "unified" : userPreference;
}

function buildGitDiffPanelNode(options: GitLayoutNodesOptions) {
  const selectedDiffPath =
    options.diffViewProps.centerMode === "diff"
      ? options.gitDiffViewerProps.selectedPath
      : null;

  if (options.filePanelMode === "files" && options.fileTreeProps) {
    return <FileTreePanel {...options.fileTreeProps} />;
  }
  if (options.filePanelMode === "prompts") {
    return <PromptPanel {...options.promptPanelProps} />;
  }
  return (
    <GitDiffPanel
      {...options.gitDiffPanelProps}
      selectedPath={selectedDiffPath}
    />
  );
}

function buildGitDiffViewerNode(options: GitLayoutNodesOptions) {
  return (
    <GitDiffViewer
      {...options.gitDiffViewerProps}
      diffStyle={resolveGitDiffStyle({
        splitChatDiffView: options.diffViewProps.splitChatDiffView,
        centerMode: options.diffViewProps.centerMode,
        userPreference: options.diffViewProps.gitDiffViewStyle,
      })}
    />
  );
}

export function buildGitNodes(options: GitLayoutNodesOptions): GitLayoutNodes {
  return {
    gitDiffPanelNode: buildGitDiffPanelNode(options),
    gitDiffViewerNode: buildGitDiffViewerNode(options),
  };
}
