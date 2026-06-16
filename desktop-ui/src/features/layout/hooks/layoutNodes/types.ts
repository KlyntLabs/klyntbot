import type { ApprovalToasts } from "@app/components/ApprovalToasts";
import type { MainHeader } from "@app/components/MainHeader";
import type { ComponentProps, ReactNode } from "react";
import type { Composer } from "@/features/composer/components/Composer";
import type { DebugPanel } from "@/features/debug/components/DebugPanel";
import type { FileTreePanel } from "@/features/files/components/FileTreePanel";
import type { GitDiffPanel } from "@/features/git/components/GitDiffPanel";
import type { GitDiffViewer } from "@/features/git/components/GitDiffViewer";
import type { Home } from "@/features/home/components/Home";
import type { Messages } from "@/features/messages/components/Messages";
import type { ErrorToasts } from "@/features/notifications/components/ErrorToasts";
import type { PlanPanel } from "@/features/plan/components/PlanPanel";
import type { PromptPanel } from "@/features/prompts/components/PromptPanel";
import type { TerminalDock } from "@/features/terminal/components/TerminalDock";
import type { TerminalSessionState } from "@/features/terminal/hooks/useTerminalSession";
import type { UpdateToast } from "@/features/update/components/UpdateToast";

export type WorktreeRenameState = {
  name: string;
  error: string | null;
  notice: string | null;
  isSubmitting: boolean;
  isDirty: boolean;
  upstream?: {
    oldBranch: string;
    newBranch: string;
    error: string | null;
    isSubmitting: boolean;
    onConfirm: () => void;
  } | null;
  onFocus: () => void;
  onChange: (value: string) => void;
  onCancel: () => void;
  onCommit: () => void;
};

export type ChatViewProps = {
  active: boolean;
  sessionKey: string | null;
  onThreadsChanged: () => void;
};

export type SidebarChatProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectCalendar?: () => void;
  onSelectFocus?: () => void;
  threads: import("@/features/chat/types").ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
  activeNavId?: string | null;
  workspaces: import("@/types").WorkspaceInfo[];
};

export type LayoutPrimarySurface = {
  sidebarProps: SidebarChatProps;
  messagesProps: ComponentProps<typeof Messages>;
  composerProps: ComponentProps<typeof Composer> | null;
  approvalToastsProps: ComponentProps<typeof ApprovalToasts>;
  updateToastProps: ComponentProps<typeof UpdateToast>;
  errorToastsProps: ComponentProps<typeof ErrorToasts>;
  homeProps: ComponentProps<typeof Home>;
  mainHeaderProps: ComponentProps<typeof MainHeader> | null;
  desktopTopbarProps: {
    showBackToChat: boolean;
    onExitDiff: () => void;
  };
  chatViewProps: ChatViewProps;
};

export type LayoutGitSurface = {
  filePanelMode: ComponentProps<typeof GitDiffPanel>["filePanelMode"];
  fileTreeProps: ComponentProps<typeof FileTreePanel> | null;
  promptPanelProps: ComponentProps<typeof PromptPanel>;
  gitDiffPanelProps: ComponentProps<typeof GitDiffPanel>;
  gitDiffViewerProps: ComponentProps<typeof GitDiffViewer>;
  diffViewProps: {
    centerMode: "chat" | "diff";
    splitChatDiffView: boolean;
    gitDiffViewStyle: "split" | "unified";
  };
};

export type LayoutSecondarySurface = {
  planPanelProps: ComponentProps<typeof PlanPanel>;
  terminalDockProps: Omit<ComponentProps<typeof TerminalDock>, "terminalNode">;
  terminalState: TerminalSessionState | null;
  debugPanelProps: ComponentProps<typeof DebugPanel>;
};

export type LayoutNodesOptions = {
  primary: LayoutPrimarySurface;
  git: LayoutGitSurface;
  secondary: LayoutSecondarySurface;
};

export type LayoutNodesResult = {
  sidebarNode: ReactNode;
  messagesNode: ReactNode;
  composerNode: ReactNode;
  approvalToastsNode: ReactNode;
  updateToastNode: ReactNode;
  errorToastsNode: ReactNode;
  homeNode: ReactNode;
  mainHeaderNode: ReactNode;
  desktopTopbarLeftNode: ReactNode;
  gitDiffPanelNode: ReactNode;
  gitDiffViewerNode: ReactNode;
  planPanelNode: ReactNode;
  debugPanelNode: ReactNode;
  terminalDockNode: ReactNode;
};
