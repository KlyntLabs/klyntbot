import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [
    react({
      babel: {
        plugins: ["babel-plugin-react-compiler"],
      },
    }),
    tailwindcss(),
  ],
  clearScreen: false,
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@shared": path.resolve(__dirname, "./src/shared"),
      "@features": path.resolve(__dirname, "./src/features"),
      "@app": path.resolve(__dirname, "./src/app"),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:3456",
        changeOrigin: true,
      },
      "/attachments": {
        target: "http://127.0.0.1:3456",
        changeOrigin: true,
      },
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "esnext",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'index.html'),
        statusBadge: path.resolve(__dirname, 'status-badge.html'),
      },
      output: {
        manualChunks: {
          three: ["three", "react-force-graph-2d", "react-force-graph-3d"],
          mermaid: ["mermaid"],
          recharts: ["recharts"],
          tiptap: [
            "@tiptap/react",
            "@tiptap/starter-kit",
            "@tiptap/pm/state",
            "@tiptap/pm/view",
            "@tiptap/pm/model",
            "@tiptap/pm/commands",
            "@tiptap/pm/history",
          ],
        },
      },
    },
  },
});
