// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { __testing, setAppMode } from "@app/hooks/useAppMode";
import { DesktopLayout } from "./DesktopLayout";

afterEach(() => cleanup());

const baseProps = {
  sidebarNode: <aside data-testid="sidebar">Sidebar</aside>,
  updateToastNode: null,
  approvalToastsNode: null,
  errorToastsNode: null,
  homeNode: <div data-testid="home">Home</div>,
  codeLandingNode: <div data-testid="code-landing">CodeLanding</div>,
  showHome: true,
  showWorkspace: false,
  topbarLeftNode: null,
  centerMode: "chat" as const,
  preloadGitDiffs: false,
  splitChatDiffView: false,
  messagesNode: null,
  gitDiffViewerNode: null,
  gitDiffPanelNode: null,
  planPanelNode: null,
  composerNode: null,
  terminalDockNode: null,
  debugPanelNode: null,
  hasActivePlan: false,
  onSidebarResizeStart: vi.fn(),
  onChatDiffSplitPositionResizeStart: vi.fn(),
  onRightPanelResizeStart: vi.fn(),
  onPlanPanelResizeStart: vi.fn(),
};

describe("DesktopLayout mode swap", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  afterEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  it("renders homeNode when mode is assistant", () => {
    render(<DesktopLayout {...baseProps} />);
    expect(screen.getByTestId("home")).toBeTruthy();
    expect(screen.queryByTestId("code-landing")).toBeNull();
  });

  it("renders codeLandingNode instead of homeNode when mode is code", () => {
    render(<DesktopLayout {...baseProps} />);
    act(() => setAppMode("code"));
    expect(screen.queryByTestId("home")).toBeNull();
    expect(screen.getByTestId("code-landing")).toBeTruthy();
  });

  it("renders neither node when showHome is false", () => {
    render(<DesktopLayout {...baseProps} showHome={false} />);
    expect(screen.queryByTestId("home")).toBeNull();
    expect(screen.queryByTestId("code-landing")).toBeNull();
  });

  it("flips back to homeNode when mode returns to assistant", () => {
    __testing.reset("code");
    render(<DesktopLayout {...baseProps} />);
    expect(screen.queryByTestId("home")).toBeNull();
    expect(screen.getByTestId("code-landing")).toBeTruthy();

    act(() => setAppMode("assistant"));
    expect(screen.getByTestId("home")).toBeTruthy();
    expect(screen.queryByTestId("code-landing")).toBeNull();
  });
});
