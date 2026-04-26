import type { ComponentProps, ReactNode } from "react";
import type { ApprovalToasts } from "../../../app/components/ApprovalToasts";
import type { MainHeader } from "../../../app/components/MainHeader";
import type { Sidebar } from "../../../app/components/Sidebar";
import type { TabBar } from "../../../app/components/TabBar";
import type { Composer } from "../../../composer/components/Composer";
import type { DebugPanel } from "../../../debug/components/DebugPanel";
import type { FileTreePanel } from "../../../files/components/FileTreePanel";
import type { GitDiffPanel } from "../../../git/components/GitDiffPanel";
import type { GitDiffViewer } from "../../../git/components/GitDiffViewer";
import type { Home } from "../../../home/components/Home";
import type { Messages } from "../../../messages/components/Messages";
import type { ErrorToasts } from "../../../notifications/components/ErrorToasts";
import type { PlanPanel } from "../../../plan/components/PlanPanel";
import type { PromptPanel } from "../../../prompts/components/PromptPanel";
import type { TerminalDock } from "../../../terminal/components/TerminalDock";
import type { TerminalSessionState } from "../../../terminal/hooks/useTerminalSession";
import type { UpdateToast } from "../../../update/components/UpdateToast";

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

export type LayoutPrimarySurface = {
	sidebarProps: ComponentProps<typeof Sidebar>;
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
	tabBarProps: ComponentProps<typeof TabBar>;
};

export type LayoutGitSurface = {
	filePanelMode: ComponentProps<typeof GitDiffPanel>["filePanelMode"];
	fileTreeProps: ComponentProps<typeof FileTreePanel> | null;
	promptPanelProps: ComponentProps<typeof PromptPanel>;
	gitDiffPanelProps: ComponentProps<typeof GitDiffPanel>;
	gitDiffViewerProps: ComponentProps<typeof GitDiffViewer>;
	diffViewProps: {
		centerMode: "chat" | "diff";
		isPhone: boolean;
		splitChatDiffView: boolean;
		gitDiffViewStyle: "split" | "unified";
	};
};

export type LayoutSecondarySurface = {
	planPanelProps: ComponentProps<typeof PlanPanel>;
	terminalDockProps: Omit<ComponentProps<typeof TerminalDock>, "terminalNode">;
	terminalState: TerminalSessionState | null;
	debugPanelProps: ComponentProps<typeof DebugPanel>;
	compactNavProps: {
		onGoProjects: () => void;
		centerMode: "chat" | "diff";
		selectedDiffPath: string | null;
		onBackFromDiff: () => void;
		onShowSelectedDiff: () => void;
		hasActiveGitDiffs: boolean;
	};
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
	tabBarNode: ReactNode;
	gitDiffPanelNode: ReactNode;
	gitDiffViewerNode: ReactNode;
	planPanelNode: ReactNode;
	debugPanelNode: ReactNode;
	debugPanelFullNode: ReactNode;
	terminalDockNode: ReactNode;
	compactEmptyCodexNode: ReactNode;
	compactEmptyGitNode: ReactNode;
	compactGitBackNode: ReactNode;
};
