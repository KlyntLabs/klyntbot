import { expect, test } from "@playwright/test";
import { K_CAPTURES } from "../../scripts/perf-proxy/contract.ts";
import type { RowResult } from "../../scripts/perf-proxy/contract.ts";
import { buildMessages, installMocks, SESSION_KEY } from "./support/fixtures.ts";
import {
  expectPlainRendered,
  expectTheme,
  expectVirtualizedRendered,
  setTheme,
} from "./support/preconditions.ts";
import { writeRowFile } from "./support/rowfile.ts";
import {
  captureScreenshots,
  sampleRaf,
  SCREENSHOT_OPTIONS,
  stats,
} from "./support/sampler.ts";

const SCENARIOS = [
  { scenario: "idle-20", n: 20, scroll: false },
  { scenario: "idle-200", n: 200, scroll: false },
  { scenario: "scroll-200", n: 200, scroll: true },
] as const;

const THEMES = ["light", "dark"] as const;

export const ROWS = SCENARIOS.flatMap((scenario) =>
  THEMES.map((theme) => ({ ...scenario, theme })),
);

test.describe.configure({ mode: "serial" });

for (const row of ROWS) {
  const name = `${row.scenario}×${row.theme}`;

  test(name, async ({ browser }, testInfo) => {
    const started = Date.now();
    const context = await browser.newContext({
      reducedMotion: "no-preference",
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 1,
    });
    await setTheme(context, row.theme);
    const page = await context.newPage();

    try {
      const { unexpected } = await installMocks(page, { n: row.n });
      await page.goto(`/#/chat?thread=${SESSION_KEY}`);
      await expectTheme(page, row.theme);

      const messages = buildMessages(row.n);
      if (row.n === 20) {
        await expectPlainRendered(page, messages);
      } else {
        await expectVirtualizedRendered(page, messages);
      }
      expect(
        unexpected,
        unexpected.length > 0
          ? `unexpected command: ${unexpected.join(", ")}`
          : undefined,
      ).toEqual([]);

      await page.waitForTimeout(600);
      const rafGaps = await sampleRaf(page, {
        warmupFrames: 60,
        sampleFrames: 300,
        scroll: row.scroll ? { stepPx: 120 } : undefined,
      });
      await page.waitForTimeout(600);
      const shotTimes = await captureScreenshots(page, K_CAPTURES);

      const rafStats = stats(rafGaps);
      const shotStats = stats(shotTimes);
      const viewport = page.viewportSize() ?? { width: 1280, height: 800 };
      const result: RowResult = {
        row: name,
        scenario: row.scenario,
        theme: row.theme,
        n: row.n,
        renderPath: row.n === 20 ? "plain" : "virtualized",
        raf: {
          sampleCount: rafGaps.length,
          p50Ms: rafStats.p50,
          p95Ms: rafStats.p95,
          maxMs: rafStats.max,
        },
        screenshot: {
          captureCount: shotTimes.length,
          p50Ms: shotStats.p50,
          options: { ...SCREENSHOT_OPTIONS },
        },
        environment: {
          webkitVersion: browser.version(),
          headless: testInfo.project.use.headless ?? true,
          viewport: { width: viewport.width, height: viewport.height },
          dpr: await page.evaluate(() => window.devicePixelRatio),
          userAgent: await page.evaluate(() => navigator.userAgent),
        },
        durationMs: Date.now() - started,
      };
      writeRowFile(result);
    } finally {
      await context.close();
    }
  });
}
