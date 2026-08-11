/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  plugins: [react()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
    // Globs, not bare paths: `exclude` REPLACES Vitest's defaults, and the bare
    // "node_modules" entry did not match nested paths, so every dependency's
    // own test suite under .worktrees/*/node_modules was being collected and run.
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.worktrees/**",
      "**/src-tauri/**",
      "e2e/**",
    ],
  },
});
