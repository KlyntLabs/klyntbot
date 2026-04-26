import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

function isTauriAlias(find: string | RegExp): boolean {
  const pattern = typeof find === "string" ? find : find.source;
  return pattern.includes("tauri");
}

export default defineConfig(async () => {
  const resolved = await viteConfig;
  const originalAliases: Array<{ find: string | RegExp; replacement: string }> =
    (resolved as any).resolve?.alias ?? [];

  // Remove Tauri plugin aliases in tests so vi.mock() can mock the real
  // modules directly. When multiple module names alias to the same file
  // (tauri-shims.ts), Vitest v4 strict mock validation rejects partial
  // vi.mock() factories because it sees exports from the shared file.
  const testAliases = originalAliases.filter((a) => !isTauriAlias(a.find));

  const withoutTauri = {
    ...resolved,
    resolve: {
      ...resolved.resolve,
      alias: testAliases,
    },
  };

  return mergeConfig(withoutTauri, {
    test: {
      environment: "node",
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
      setupFiles: ["src/test/vitest.setup.ts"],
    },
  });
});
