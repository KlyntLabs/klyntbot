// @vitest-environment node
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import type { RowResult } from "./contract.ts";
import { ALLOWLIST, buildMessages } from "../../tests/perf-proxy/support/fixtures.ts";
import {
  EXPECTED_ROWS,
  readRowFiles,
  writeRowFile,
} from "../../tests/perf-proxy/support/rowfile.ts";

const MARKDOWN_CHARS = /[*_~`#[\]()!>|]/;

describe("buildMessages", () => {
  it("builds 20 messages with unique ids, alternating roles, rising timestamps, and no markdown", () => {
    const messages = buildMessages(20);
    expect(messages).toHaveLength(20);

    const ids = messages.map((m) => m.id);
    expect(new Set(ids).size).toBe(20);
    expect(ids).toEqual(Array.from({ length: 20 }, (_, i) => `m${i + 1}`));

    for (let i = 0; i < messages.length; i++) {
      expect(messages[i].role).toBe(i % 2 === 0 ? "user" : "assistant");
    }

    for (let i = 1; i < messages.length; i++) {
      expect(messages[i].timestamp! > messages[i - 1].timestamp!).toBe(true);
    }

    for (const message of messages) {
      expect(message.content).not.toMatch(MARKDOWN_CHARS);
    }
  });
});

describe("ALLOWLIST", () => {
  it("has exactly the nine command names", () => {
    expect(Object.keys(ALLOWLIST).sort()).toEqual(
      [
        "app_info",
        "autotuner_get_toast_count",
        "autotuner_status",
        "chat_messages",
        "chat_threads",
        "flashcard_total_due",
        "journey_item_count",
        "journey_milestones",
        "view_clear_active",
      ].sort(),
    );
  });
});

describe("row files", () => {
  const sampleRow = (): RowResult => ({
    row: "idle-20×light",
    scenario: "idle-20",
    theme: "light",
    n: 20,
    renderPath: "plain",
    raf: { sampleCount: 300, p50Ms: 10, p95Ms: 11, maxMs: 12 },
    screenshot: {
      captureCount: 10,
      p50Ms: 40,
      options: { type: "png", scale: "css" },
    },
    environment: {
      webkitVersion: "WebKit/605.1.15",
      headless: true,
      viewport: { width: 1280, height: 800 },
      dpr: 1,
      userAgent: "Mozilla/5.0",
    },
    durationMs: 1000,
  });

  it("round-trips a RowResult through writeRowFile and readRowFiles", () => {
    const dir = mkdtempSync(join(tmpdir(), "perf-proxy-rows-"));
    try {
      const row = sampleRow();
      writeRowFile(row, dir);
      expect(readRowFiles(dir)).toEqual([row]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("returns an empty array for an empty directory", () => {
    const dir = mkdtempSync(join(tmpdir(), "perf-proxy-rows-empty-"));
    try {
      expect(readRowFiles(dir)).toEqual([]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("lists the six expected row names", () => {
    expect(EXPECTED_ROWS).toEqual([
      "idle-20×light",
      "idle-20×dark",
      "idle-200×light",
      "idle-200×dark",
      "scroll-200×light",
      "scroll-200×dark",
    ]);
  });
});
