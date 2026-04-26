import { type ParsedFileLocation, parseFileLocation } from "@utils/fileLinks";
import { expect, type vi } from "vitest";

export function expectOpenedFileTarget(
  mock: ReturnType<typeof vi.fn>,
  path: string,
  line: number | null = null,
  column: number | null = null,
) {
  expect(mock).toHaveBeenCalledWith({ path, line, column });
}

export function fileTarget(rawPath: string): ParsedFileLocation {
  return parseFileLocation(rawPath);
}
