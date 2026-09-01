import { DebugPanel } from "@/features/debug/components/DebugPanel";
import { PlanPanel } from "@/features/plan/components/PlanPanel";
import { TerminalDock } from "@/features/terminal/components/TerminalDock";
import { TerminalPanel } from "@/features/terminal/components/TerminalPanel";
import type { LayoutNodesResult, LayoutSecondarySurface } from "./types";

export type SecondaryLayoutNodesOptions = LayoutSecondarySurface;

type SecondaryLayoutNodes = Pick<
  LayoutNodesResult,
  "planPanelNode" | "debugPanelNode" | "terminalDockNode"
>;

function buildTerminalPanelNode(terminalState: SecondaryLayoutNodesOptions["terminalState"]) {
  if (!terminalState) {
    return null;
  }

  return (
    <TerminalPanel
      containerRef={terminalState.containerRef}
      status={terminalState.status}
      message={terminalState.message}
    />
  );
}

export function buildSecondaryNodes(options: SecondaryLayoutNodesOptions): SecondaryLayoutNodes {
  const planPanelNode = <PlanPanel {...options.planPanelProps} />;
  const terminalPanelNode = buildTerminalPanelNode(options.terminalState);

  const terminalDockNode = (
    <TerminalDock {...options.terminalDockProps} terminalNode={terminalPanelNode} />
  );

  const debugPanelNode = <DebugPanel {...options.debugPanelProps} />;

  return {
    planPanelNode,
    debugPanelNode,
    terminalDockNode,
  };
}
