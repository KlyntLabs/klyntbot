// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { formatShortcut, matchesShortcut, toMenuAccelerator } from "./shortcuts";

function withNavigatorPlatform(platform: string, fn: () => void) {
  const originalUserAgentData = Object.getOwnPropertyDescriptor(navigator, "userAgentData");

  Object.defineProperty(navigator, "userAgentData", {
    value: { platform },
    configurable: true,
  });

  try {
    fn();
  } finally {
    if (originalUserAgentData) {
      Object.defineProperty(navigator, "userAgentData", originalUserAgentData);
    } else {
      delete (navigator as unknown as { userAgentData?: unknown }).userAgentData;
    }
  }
}

describe("shortcuts", () => {
  it("formats macOS shortcuts with symbols", () => {
    withNavigatorPlatform("MacIntel", () => {
      expect(formatShortcut("cmd+ctrl+a")).toBe("⌘⌃A");
      expect(formatShortcut("cmd+shift+enter")).toBe("⌘⇧Enter");
      expect(toMenuAccelerator("cmd+ctrl+a")).toBe("Cmd+Ctrl+A");
    });
  });

  it("requires both cmd and ctrl on macOS", () => {
    withNavigatorPlatform("MacIntel", () => {
      const cmdCtrl = new KeyboardEvent("keydown", {
        key: "a",
        metaKey: true,
        ctrlKey: true,
      });
      expect(matchesShortcut(cmdCtrl, "cmd+ctrl+a")).toBe(true);

      const ctrlOnly = new KeyboardEvent("keydown", { key: "a", ctrlKey: true });
      expect(matchesShortcut(ctrlOnly, "cmd+ctrl+a")).toBe(false);
    });
  });
});
