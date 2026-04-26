import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default defineConfig(async () => {
  const resolved = await viteConfig;
  return mergeConfig(resolved, {
    test: {
      environment: "node",
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
      setupFiles: ["src/test/vitest.setup.ts"],
    },
  });
});
