import ArrowLeft from "lucide-react/dist/esm/icons/arrow-left";
import { ApprovalToasts } from "@app/components/ApprovalToasts";
import { MainHeader } from "@app/components/MainHeader";
import { SidebarChatLayout } from "@app/components/SidebarChatLayout";
import { ChatPanel } from "@/features/chat/components/ChatPanel";
import { Composer } from "@/features/composer/components/Composer";
import { Home } from "@/features/home/components/Home";
import { Messages } from "@/features/messages/components/Messages";
import { ErrorToasts } from "@/features/notifications/components/ErrorToasts";
import { UpdateToast } from "@/features/update/components/UpdateToast";
import type { LayoutNodesResult, LayoutPrimarySurface } from "./types";

export type PrimaryLayoutNodesOptions = LayoutPrimarySurface;

type PrimaryLayoutNodes = Pick<
	LayoutNodesResult,
	| "sidebarNode"
	| "messagesNode"
	| "composerNode"
	| "approvalToastsNode"
	| "updateToastNode"
	| "errorToastsNode"
	| "homeNode"
	| "mainHeaderNode"
	| "desktopTopbarLeftNode"
>;

export function buildPrimaryNodes(
	options: PrimaryLayoutNodesOptions,
): PrimaryLayoutNodes {
	const { chatViewProps } = options;
	const chatActive = chatViewProps.active && chatViewProps.sessionKey !== null;

	const sidebarNode = (
		<SidebarChatLayout
			onSelectHome={options.sidebarProps.onSelectHome}
			onOpenSettings={options.sidebarProps.onOpenSettings}
			onNewChat={options.sidebarProps.onNewChat}
			threads={options.sidebarProps.threads}
			selectedSessionKey={options.sidebarProps.selectedSessionKey}
			onSelectThread={options.sidebarProps.onSelectThread}
		/>
	);

	const messagesNode = chatActive ? (
		<ChatPanel
			sessionKey={chatViewProps.sessionKey as string}
			onThreadsChanged={chatViewProps.onThreadsChanged}
		/>
	) : (
		<Messages {...options.messagesProps} />
	);

	const composerNode = chatActive
		? null
		: options.composerProps
			? <Composer {...options.composerProps} />
			: null;

	const approvalToastsNode = (
		<ApprovalToasts {...options.approvalToastsProps} />
	);

	const updateToastNode = <UpdateToast {...options.updateToastProps} />;

	const errorToastsNode = <ErrorToasts {...options.errorToastsProps} />;

	const homeNode = <Home {...options.homeProps} />;

	const mainHeaderNode = options.mainHeaderProps ? (
		<MainHeader {...options.mainHeaderProps} />
	) : null;

	const desktopTopbarLeftNode = (
		<>
			{options.desktopTopbarProps.showBackToChat && (
				<button
					className="icon-button back-button"
					onClick={options.desktopTopbarProps.onExitDiff}
					aria-label="Back to chat"
				>
					<ArrowLeft aria-hidden />
				</button>
			)}
			{mainHeaderNode}
		</>
	);

	return {
		sidebarNode,
		messagesNode,
		composerNode,
		approvalToastsNode,
		updateToastNode,
		errorToastsNode,
		homeNode,
		mainHeaderNode,
		desktopTopbarLeftNode,
	};
}
