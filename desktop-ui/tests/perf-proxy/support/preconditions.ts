import type { BrowserContext, Page } from "@playwright/test";
import type { ChatMessage } from "../../../src/shared/types/chat.ts";

export async function setTheme(
  context: BrowserContext,
  theme: string,
): Promise<void> {
  await context.addInitScript((value) => {
    localStorage.setItem("klynt-theme", value);
  }, theme);
}

export async function expectTheme(page: Page, theme: string): Promise<void> {
  await page.waitForFunction(
    (expected) =>
      document.documentElement.getAttribute("data-theme") === expected,
    theme,
  );
}

export async function expectPlainRendered(
  page: Page,
  messages: ChatMessage[],
): Promise<void> {
  await page.locator('[data-render-path="plain"]').waitFor();
  await Promise.all(
    messages.map((message) =>
      page.getByText(message.content, { exact: true }).waitFor(),
    ),
  );
}

export async function expectVirtualizedRendered(
  page: Page,
  messages: ChatMessage[],
): Promise<void> {
  await page.locator('[data-render-path="virtualized"]').waitFor();
  const last = messages[messages.length - 1];
  if (!last) {
    throw new Error("virtualized precondition requires at least one message");
  }
  await page.getByText(last.content, { exact: true }).waitFor();
}
