import ArrowLeft from "lucide-react/dist/esm/icons/arrow-left";
import { ApprovalToasts } from "../../../app/components/ApprovalToasts";
import { MainHeader } from "../../../app/components/MainHeader";
import { Sidebar } from "../../../app/components/Sidebar";
import { Composer } from "../../../composer/components/Composer";
import { Home } from "../../../home/components/Home";
import { Messages } from "../../../messages/components/Messages";
import { ErrorToasts } from "../../../notifications/components/ErrorToasts";
import { UpdateToast } from "../../../update/components/UpdateToast";
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
	const sidebarNode = <Sidebar {...options.sidebarProps} />;

	const messagesNode = <Messages {...options.messagesProps} />;

	const composerNode = options.composerProps ? (
		<Composer {...options.composerProps} />
	) : null;

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
