import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@/api/client", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { SettingsAppSection } from "./SettingsAppSection";

describe("SettingsAppSection", () => {
  beforeEach(() => invoke.mockReset());

  it("reads ui section on mount", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({ theme: "dark", uiScale: 1.25 });
      }
      return Promise.resolve({});
    });

    render(<SettingsAppSection />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_get_section", {
        section: "ui",
      });
    });
  });

  it("patches theme via config_update_section", async () => {
    invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({ theme: "system" });
      }
      if (cmd === "config_update_section") {
        return Promise.resolve({ ...(args.patch as object) });
      }
      return Promise.resolve({});
    });

    render(<SettingsAppSection />);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    const themeSelect = screen.getByLabelText("Theme");
    fireEvent.change(themeSelect, { target: { value: "dark" } });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_update_section", {
        section: "ui",
        patch: { theme: "dark" },
      });
    });
  });

  it("patches uiScale on blur", async () => {
    invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({ uiScale: 1.0 });
      }
      if (cmd === "config_update_section") {
        return Promise.resolve({ ...(args.patch as object) });
      }
      return Promise.resolve({});
    });

    render(<SettingsAppSection />);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    const slider = screen.getByRole("slider", { name: /ui scale/i });
    fireEvent.change(slider, { target: { value: "1.25" } });
    fireEvent.blur(slider);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_update_section", {
        section: "ui",
        patch: { uiScale: 1.25 },
      });
    });
  });

  it("patches toggle fields immediately", async () => {
    invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({ notificationSoundsEnabled: true });
      }
      if (cmd === "config_update_section") {
        return Promise.resolve({ ...(args.patch as object) });
      }
      return Promise.resolve({});
    });

    render(<SettingsAppSection />);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    const toggle = screen.getByRole("switch", { name: /notification sounds/i });
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_update_section", {
        section: "ui",
        patch: { notificationSoundsEnabled: false },
      });
    });
  });

  it("renders error state when config load fails", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "config_get_section") {
        return Promise.reject(new Error("network error"));
      }
      return Promise.resolve({});
    });

    render(<SettingsAppSection />);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    expect(screen.getByText(/network error/i)).toBeInTheDocument();
  });
});
