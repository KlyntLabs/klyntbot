import { ApprovalToasts } from "@app/components/ApprovalToasts";
import { MainHeader } from "@app/components/MainHeader";
import { SidebarChatLayout } from "@app/components/SidebarChatLayout";
import ArrowLeft from "lucide-react/dist/esm/icons/arrow-left";
import { CodeLanding } from "@/features/coding/components/CodeLanding";
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
  | "codeLandingNode"
  | "mainHeaderNode"
  | "desktopTopbarLeftNode"
>;

export function buildPrimaryNodes(options: PrimaryLayoutNodesOptions): PrimaryLayoutNodes {
  const sidebarNode = (
    <SidebarChatLayout
      onOpenSettings={options.sidebarProps.onOpenSettings}
      onNewChat={options.sidebarProps.onNewChat}
      onSelectPlugins={options.sidebarProps.onSelectPlugins}
      onSelectCalendar={options.sidebarProps.onSelectCalendar}
      activeNavId={options.sidebarProps.activeNavId}
      threads={options.sidebarProps.threads}
      selectedSessionKey={options.sidebarProps.selectedSessionKey}
      onSelectThread={options.sidebarProps.onSelectThread}
    />
  );

  const messagesNode = <Messages {...options.messagesProps} />;

  const composerNode = options.composerProps ? <Composer {...options.composerProps} /> : null;

  const approvalToastsNode = <ApprovalToasts {...options.approvalToastsProps} />;
  const updateToastNode = <UpdateToast {...options.updateToastProps} />;
  const errorToastsNode = <ErrorToasts {...options.errorToastsProps} />;
  const homeNode = <Home {...options.homeProps} />;
  const codeLandingNode = <CodeLanding {...options.codeLandingProps} />;
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
    codeLandingNode,
    mainHeaderNode,
    desktopTopbarLeftNode,
  };
}
