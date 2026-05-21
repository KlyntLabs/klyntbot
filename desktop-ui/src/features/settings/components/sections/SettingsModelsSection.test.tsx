import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@/api/client", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { SettingsModelsSection } from "./SettingsModelsSection";

describe("SettingsModelsSection", () => {
  beforeEach(() => invoke.mockReset());

  it("reads agents and providers on mount", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({
          defaults: { model: "anthropic/claude-opus-4-5", temperature: 0.7 },
        });
      }
      return Promise.resolve({});
    });

    render(<SettingsModelsSection />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_get_section", {
        section: "agents",
      });
    });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_get_section", {
        section: "providers",
      });
    });
  });

  it("patches temperature via config_update_section", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "config_get_section") {
        return Promise.resolve({
          defaults: { model: "anthropic/claude-opus-4-5", temperature: 0.7 },
        });
      }
      if (cmd === "config_update_section") {
        return Promise.resolve({
          defaults: { model: "anthropic/claude-opus-4-5", temperature: 0.5 },
        });
      }
      return Promise.resolve({});
    });

    render(<SettingsModelsSection />);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    const temperatureSlider = screen.getByRole("slider");
    fireEvent.change(temperatureSlider, { target: { value: "0.5" } });
    fireEvent.blur(temperatureSlider);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("config_update_section", {
        section: "agents",
        patch: expect.objectContaining({
          defaults: expect.objectContaining({ temperature: 0.5 }),
        }),
      });
    });
  });
});
