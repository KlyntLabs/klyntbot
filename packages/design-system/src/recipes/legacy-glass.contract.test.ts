import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));

function readCss(name: string): string {
  return readFileSync(join(here, name), "utf8");
}

/** Extract a top-level utility/class block body (brace-balanced). */
function blockBody(source: string, header: RegExp): string {
  const match = header.exec(source);
  expect(match, `expected block matching ${header}`).toBeTruthy();
  const start = source.indexOf("{", match!.index);
  expect(start).toBeGreaterThanOrEqual(0);
  let depth = 0;
  for (let i = start; i < source.length; i++) {
    const ch = source[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return source.slice(start + 1, i);
    }
  }
  throw new Error(`unclosed block for ${header}`);
}

describe("legacy-glass ↔ recipe material contract", () => {
  const legacy = readCss("legacy-glass.css");
  const recipes = readCss("recipes.css");

  it("glass-floating and liquid-glass both use blur + glass-border tokens", () => {
    const floating = blockBody(legacy, /\.glass-floating\s*\{/);
    const liquid = blockBody(recipes, /@utility\s+liquid-glass\s*\{/);

    for (const body of [floating, liquid]) {
      expect(body).toMatch(/--ds-blur/);
      expect(body).toMatch(/--ds-glass-border/);
    }
  });

  it("glass-dropdown shares blur + border + window elevation tokens with liquid-glass", () => {
    const dropdown = blockBody(legacy, /\.glass-dropdown\s*\{/);
    const liquid = blockBody(recipes, /@utility\s+liquid-glass\s*\{/);

    expect(dropdown).toMatch(/--ds-blur/);
    expect(dropdown).toMatch(/--ds-glass-border/);
    expect(dropdown).toMatch(/--ds-elevation-window/);
    expect(liquid).toMatch(/--ds-elevation-window/);
  });

  it("legacy glass file has no raw hex / rgb / rgba literals", () => {
    expect(legacy).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(legacy).not.toMatch(/\brgba?\(/);
  });

  it("glass-input uses tokenized subtle blur (not a bare blur())", () => {
    const input = blockBody(legacy, /\.glass-input\s*\{/);
    expect(input).toMatch(/backdrop-filter:\s*var\(--ds-blur-subtle\)/);
    expect(input).not.toMatch(/backdrop-filter:\s*blur\(/);
  });
});
