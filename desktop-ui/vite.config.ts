import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf-8"),
) as {
  version: string;
};

function resolveCommitHash() {
  const ciCommit =
    process.env.GIT_COMMIT ??
    process.env.GITHUB_SHA ??
    process.env.CI_COMMIT_SHA;
  if (typeof ciCommit === "string" && ciCommit.trim().length > 0) {
    return ciCommit.trim().slice(0, 12);
  }
  try {
    return execSync("git rev-parse --short=12 HEAD", {
      encoding: "utf8",
    }).trim();
  } catch {
    return "unknown";
  }
}

function resolveBuildDate() {
  const sourceDateEpoch = process.env.SOURCE_DATE_EPOCH;
  if (typeof sourceDateEpoch === "string" && /^\d+$/.test(sourceDateEpoch)) {
    const milliseconds = Number(sourceDateEpoch) * 1000;
    if (Number.isFinite(milliseconds) && milliseconds > 0) {
      return new Date(milliseconds).toISOString();
    }
  }
  return new Date().toISOString();
}

function resolveGitBranch() {
  const ciBranch =
    process.env.GIT_BRANCH ??
    process.env.GITHUB_REF_NAME ??
    process.env.CI_COMMIT_REF_NAME;
  if (typeof ciBranch === "string" && ciBranch.trim().length > 0) {
    return ciBranch.trim();
  }
  try {
    const branch = execSync("git rev-parse --abbrev-ref HEAD", {
      encoding: "utf8",
    }).trim();
    if (!branch || branch === "HEAD") {
      return "unknown";
    }
    return branch;
  } catch {
    return "unknown";
  }
}

const appCommitHash = resolveCommitHash();
const appBuildDate = resolveBuildDate();
const appGitBranch = resolveGitBranch();

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      // Path aliases (more specific prefixes first).
      {
        find: "@app",
        replacement: fileURLToPath(
          new URL("./src/features/app", import.meta.url),
        ),
      },
      {
        find: "@settings",
        replacement: fileURLToPath(
          new URL("./src/features/settings", import.meta.url),
        ),
      },
      {
        find: "@threads",
        replacement: fileURLToPath(
          new URL("./src/features/threads", import.meta.url),
        ),
      },
      {
        find: "@services",
        replacement: fileURLToPath(new URL("./src/services", import.meta.url)),
      },
      {
        find: "@utils",
        replacement: fileURLToPath(new URL("./src/utils", import.meta.url)),
      },
      {
        find: /^@\/(.*)$/,
        replacement: fileURLToPath(new URL("./src/$1", import.meta.url)),
      },
      // TODO(klynt-integration): remove these Tauri-mock aliases once the Rust
      // backend is wired up. The mock modules log every call and return
      // null/empty defaults so the UI can boot without a Tauri runtime.
      {
        find: /^@tauri-apps\/api\/core$/,
        replacement: fileURLToPath(
          new URL(
            "./src/services/__mocks__/tauri-api-core.ts",
            import.meta.url,
          ),
        ),
      },
      {
        find: /^@tauri-apps\/api\/event$/,
        replacement: fileURLToPath(
          new URL(
            "./src/services/__mocks__/tauri-api-event.ts",
            import.meta.url,
          ),
        ),
      },
      {
        find: /^@tauri-apps\/api\/app$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/api\/dpi$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/api\/menu$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/api\/webview$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/api\/window$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/plugin-dialog$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/plugin-notification$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/plugin-opener$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/plugin-process$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^@tauri-apps\/plugin-updater$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
      {
        find: /^tauri-plugin-liquid-glass-api$/,
        replacement: fileURLToPath(
          new URL("./src/services/__mocks__/tauri-shims.ts", import.meta.url),
        ),
      },
    ],
  },
  worker: {
    format: "es",
  },
  define: {
    __APP_VERSION__: JSON.stringify(packageJson.version),
    __APP_COMMIT_HASH__: JSON.stringify(appCommitHash),
    __APP_BUILD_DATE__: JSON.stringify(appBuildDate),
    __APP_GIT_BRANCH__: JSON.stringify(appGitBranch),
  },

  build: {
    target: "es2022",
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (
            id.includes("node_modules/react") ||
            id.includes("node_modules/react-dom") ||
            id.includes("node_modules/react-markdown") ||
            id.includes("node_modules/remark-gfm")
          ) {
            return "vendor-react";
          }
          if (
            id.includes("node_modules/@tauri-apps") ||
            id.includes("node_modules/tauri-plugin-")
          ) {
            return "vendor-tauri";
          }
          if (
            id.includes("node_modules/lucide-react") ||
            id.includes("node_modules/@tanstack/react-virtual") ||
            id.includes("node_modules/prismjs")
          ) {
            return "vendor-ui";
          }
          if (id.includes("node_modules/@xterm")) {
            return "vendor-xterm";
          }
        },
      },
    },
  },

  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react-markdown",
      "lucide-react",
      "@tanstack/react-virtual",
      "prismjs",
      "@xterm/xterm",
      "@xterm/addon-fit",
    ],
  },

  server: {
    port: 1420,
    strictPort: true,
  },
});
