import { vi } from "vitest";

export const invoke = vi.fn();
export const convertFileSrc = vi.fn((path: string) => `tauri://${path}`);
export const isTauri = vi.fn(() => true);
