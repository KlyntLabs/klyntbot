import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/perf-proxy",
  testMatch: "**/*.perf.ts",
  outputDir: "test-results/perf-proxy/artifacts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "off",
  },
  projects: [
    {
      name: "webkit",
      use: {
        ...devices["Desktop Safari"],
        viewport: { width: 1280, height: 800 },
        deviceScaleFactor: 1,
      },
    },
  ],
  webServer: {
    command: "bun run dev",
    url: "http://localhost:1420",
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
