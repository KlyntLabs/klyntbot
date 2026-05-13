import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AttachTerminal } from "./AttachTerminal";

const invokeMock = vi.fn();
vi.mock("@/api/client", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const xtermWrites: string[] = [];
const onDataCallbacks: ((data: string) => void)[] = [];
const onResizeCallbacks: ((size: { rows: number; cols: number }) => void)[] = [];

function MockTerminal() {
  return {
    loadAddon: vi.fn(),
    open: vi.fn(),
    write: (s: string) => xtermWrites.push(s),
    onData: (cb: (d: string) => void) => onDataCallbacks.push(cb),
    onResize: (cb: (s: { rows: number; cols: number }) => void) =>
      onResizeCallbacks.push(cb),
    dispose: vi.fn(),
  };
}

vi.mock("@xterm/xterm", () => ({
  Terminal: MockTerminal,
}));
function MockFitAddon() {
  return { fit: vi.fn() };
}

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: MockFitAddon,
}));

// Minimal WebSocket polyfill that captures sends and lets tests dispatch events.
class FakeWS {
  static instances: FakeWS[] = [];
  static OPEN = 1;
  readyState = 1;
  binaryType = "blob";
  url: string;
  onmessage: ((e: MessageEvent) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  sent: (string | ArrayBufferLike | Blob | ArrayBufferView)[] = [];
  constructor(url: string) {
    this.url = url;
    FakeWS.instances.push(this);
  }
  send(d: string | ArrayBufferLike | Blob | ArrayBufferView) {
    this.sent.push(d);
  }
  close() {
    this.readyState = 3;
    this.onclose?.();
  }
}
(globalThis as unknown as { WebSocket: typeof FakeWS }).WebSocket = FakeWS;

describe("AttachTerminal", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    xtermWrites.length = 0;
    onDataCallbacks.length = 0;
    onResizeCallbacks.length = 0;
    FakeWS.instances.length = 0;
  });
  afterEach(() => cleanup());

  it("calls coding_job_attach on mount and primes terminal with tail", async () => {
    invokeMock.mockResolvedValueOnce({
      wsUrl: "ws://localhost:3456/api/coding/jobs/bash-x/attach?token=abc",
      rows: 24,
      cols: 80,
      tailB64: btoa("prev output\n"),
    });
    invokeMock.mockResolvedValue(undefined); // for detach
    render(<AttachTerminal threadId="t1" jobId="bash-x" />);
    await new Promise((r) => setTimeout(r, 50));
    expect(invokeMock).toHaveBeenCalledWith("coding_job_attach", {
      jobId: "bash-x",
    });
    // Tail should have been written to the terminal.
    expect(xtermWrites.some((w) => w.includes("prev output"))).toBe(true);
  });

  it("sends user keystrokes as text frames", async () => {
    invokeMock.mockResolvedValueOnce({
      wsUrl: "ws://x/attach?token=abc",
      rows: 24,
      cols: 80,
      tailB64: "",
    });
    invokeMock.mockResolvedValue(undefined);
    render(<AttachTerminal threadId="t1" jobId="bash-x" />);
    await new Promise((r) => setTimeout(r, 50));
    const ws = FakeWS.instances[0];
    expect(ws).toBeDefined();
    onDataCallbacks[0]("hi\n");
    expect(ws.sent[0]).toBe("hi\n");
  });

  it("sends resize as JSON control frame", async () => {
    invokeMock.mockResolvedValueOnce({
      wsUrl: "ws://x/attach?token=abc",
      rows: 24,
      cols: 80,
      tailB64: "",
    });
    invokeMock.mockResolvedValue(undefined);
    render(<AttachTerminal threadId="t1" jobId="bash-x" />);
    await new Promise((r) => setTimeout(r, 50));
    onResizeCallbacks[0]({ rows: 30, cols: 120 });
    const ws = FakeWS.instances[0];
    const sent = ws.sent.find((s) => typeof s === "string" && s.includes("resize"));
    expect(sent).toBeDefined();
    expect(JSON.parse(sent as string)).toEqual({ kind: "resize", rows: 30, cols: 120 });
  });
});
