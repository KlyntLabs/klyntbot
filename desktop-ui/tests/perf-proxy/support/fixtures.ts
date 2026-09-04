import type { Page } from "@playwright/test";
import type { AppInfoResponse } from "../../../src/shared/types/common.ts";
import type { ChatMessage, ChatThread } from "../../../src/shared/types/chat.ts";

export const SESSION_KEY = "perf-proxy-session";

const EPOCH_MS = 1_756_800_000_000;
const FILLER = "plain filler text for rendering proxy rows";

export function buildMessages(n: number): ChatMessage[] {
  const messages: ChatMessage[] = [];
  for (let i = 0; i < n; i++) {
    messages.push({
      id: `m${i + 1}`,
      role: i % 2 === 0 ? "user" : "assistant",
      content: `msg ${i + 1} — ${FILLER}`,
      timestamp: new Date(EPOCH_MS + 60_000 * i).toISOString(),
    });
  }
  return messages;
}

export function buildThread(sessionKey: string, n: number): ChatThread {
  return {
    sessionKey,
    title: "Perf proxy thread",
    messageCount: n,
    updatedAt: new Date(EPOCH_MS).toISOString(),
  };
}

export const ALLOWLIST: Record<string, (n: number) => unknown> = {
  app_info: (): AppInfoResponse => ({
    version: "0.0.0-perf-proxy",
    dataDir: "/tmp/perf-proxy",
    setupCompleted: true,
  }),
  chat_threads: (n) => [buildThread(SESSION_KEY, n)],
  chat_messages: (n) => buildMessages(n),
  view_clear_active: () => null,
  autotuner_status: () => null,
  flashcard_total_due: () => 0,
  journey_item_count: () => 0,
  autotuner_get_toast_count: () => 0,
  journey_milestones: () => [],
};

export async function installMocks(
  page: Page,
  opts: { n: number },
): Promise<{ unexpected: string[] }> {
  const unexpected: string[] = [];
  await page.route("**/api/**", async (route) => {
    const url = new URL(route.request().url());
    const segments = url.pathname.split("/").filter(Boolean);
    const command = segments[segments.length - 1] ?? "";
    const handler = ALLOWLIST[command];
    if (!handler) {
      unexpected.push(command);
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ code: "UNEXPECTED", message: command }),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(handler(opts.n)),
    });
  });
  await page.addInitScript(() => {
    class StubEventSource {
      url: string;
      readyState = 2;
      constructor(url: string) {
        this.url = url;
      }
      close(): void {}
      addEventListener(): void {}
      removeEventListener(): void {}
      dispatchEvent(): boolean {
        return false;
      }
      onopen: ((ev: Event) => void) | null = null;
      onmessage: ((ev: MessageEvent) => void) | null = null;
      onerror: ((ev: Event) => void) | null = null;
    }
    Object.defineProperty(window, "EventSource", {
      configurable: true,
      writable: true,
      value: StubEventSource,
    });
  });
  return { unexpected };
}
