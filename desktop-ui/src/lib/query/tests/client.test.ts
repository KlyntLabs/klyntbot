import { describe, expect, it } from "vitest";
import { createQueryClient } from "../client";

describe("createQueryClient", () => {
  it("disables refetchOnWindowFocus to avoid alt-tab stampede", () => {
    const client = createQueryClient();
    const defaults = client.getDefaultOptions().queries;
    expect(defaults?.refetchOnWindowFocus).toBe(false);
  });

  it("uses 30s default staleTime", () => {
    const client = createQueryClient();
    expect(client.getDefaultOptions().queries?.staleTime).toBe(30_000);
  });

  it("retries once on failure (Tauri errors are usually deterministic)", () => {
    const client = createQueryClient();
    expect(client.getDefaultOptions().queries?.retry).toBe(1);
  });

  it("each invocation returns an independent client", () => {
    const a = createQueryClient();
    const b = createQueryClient();
    expect(a).not.toBe(b);
  });
});
