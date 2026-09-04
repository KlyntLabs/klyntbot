import type { Page } from "@playwright/test";

export const SCREENSHOT_OPTIONS = {
  type: "png" as const,
  scale: "css" as const,
  animations: "allow" as const,
  caret: "hide" as const,
  fullPage: false,
};

export async function sampleRaf(
  page: Page,
  opts: {
    warmupFrames: number;
    sampleFrames: number;
    scroll?: { stepPx: number };
  },
): Promise<number[]> {
  return page.evaluate(async (options) => {
    const gaps: number[] = [];
    const scroller = options.scroll
      ? (document.querySelector(
          '[data-render-path="virtualized"] > div',
        ) as HTMLElement | null)
      : null;

    if (scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }

    await new Promise<void>((resolve) => {
      let previous: number | null = null;
      let discarded = 0;

      const onFrame = (now: number) => {
        if (previous !== null) {
          const gap = now - previous;
          if (discarded < options.warmupFrames) {
            discarded += 1;
          } else {
            gaps.push(gap);
          }
        }
        previous = now;

        if (scroller && options.scroll) {
          scroller.scrollTop = Math.max(
            0,
            scroller.scrollTop - options.scroll.stepPx,
          );
        }

        if (gaps.length >= options.sampleFrames) {
          resolve();
          return;
        }
        requestAnimationFrame(onFrame);
      };

      requestAnimationFrame(onFrame);
    });

    return gaps;
  }, opts);
}

export async function captureScreenshots(
  page: Page,
  k: number,
): Promise<number[]> {
  const times: number[] = [];
  for (let i = 0; i < k; i++) {
    const start = performance.now();
    await page.screenshot(SCREENSHOT_OPTIONS);
    times.push(performance.now() - start);
  }
  return times;
}

export function stats(values: number[]): {
  p50: number;
  p95: number;
  max: number;
} {
  if (values.length === 0) {
    throw new Error("stats requires at least one sample");
  }
  const sorted = [...values].sort((a, b) => a - b);
  const nearestRank = (percentile: number): number => {
    const rank = Math.ceil((percentile / 100) * sorted.length);
    return sorted[Math.max(0, rank - 1)]!;
  };
  return {
    p50: nearestRank(50),
    p95: nearestRank(95),
    max: sorted[sorted.length - 1]!,
  };
}
