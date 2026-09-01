import { vi } from "vitest";

/**
 * Mock helpers for Tauri APIs. Call inside a Vitest module factory:
 *
 *     import { mockTauriCore } from "@/test/mockTauri";
 *     vi.mock("@tauri-apps/api/core", () => mockTauriCore({ invoke: vi.fn() }));
 *
 * The default returns `invoke` and `convertFileSrc` mocks; pass overrides
 * to swap in test-specific behavior.
 */
export function mockTauriCore(
  overrides: {
    invoke?: ReturnType<typeof vi.fn>;
    convertFileSrc?: (path: string) => string;
    isTauri?: ReturnType<typeof vi.fn>;
  } = {},
) {
  return {
    invoke: overrides.invoke ?? vi.fn(),
    convertFileSrc: overrides.convertFileSrc ?? ((path: string) => `tauri://${path}`),
    isTauri: overrides.isTauri ?? vi.fn(() => true),
  };
}

/**
 * Mock helpers for `@tauri-apps/api/event`.
 */
export function mockTauriEvent(
  overrides: { listen?: ReturnType<typeof vi.fn>; emit?: ReturnType<typeof vi.fn> } = {},
) {
  return {
    listen: overrides.listen ?? vi.fn().mockResolvedValue(() => {}),
    emit: overrides.emit ?? vi.fn().mockResolvedValue(undefined),
  };
}
