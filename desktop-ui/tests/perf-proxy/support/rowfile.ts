import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { RowResult } from "../../../scripts/perf-proxy/contract.ts";

export const ROWS_DIR = "test-results/perf-proxy/rows";

export const EXPECTED_ROWS: string[] = [
  "idle-20×light",
  "idle-20×dark",
  "idle-200×light",
  "idle-200×dark",
  "scroll-200×light",
  "scroll-200×dark",
];

export function writeRowFile(row: RowResult, dir: string = ROWS_DIR): void {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${row.row}.json`), JSON.stringify(row), "utf8");
}

export function readRowFiles(dir: string = ROWS_DIR): RowResult[] {
  const names = readdirSync(dir)
    .filter((name) => name.endsWith(".json"))
    .sort();
  return names.map((name) => {
    const raw = readFileSync(join(dir, name), "utf8");
    return JSON.parse(raw) as RowResult;
  });
}
