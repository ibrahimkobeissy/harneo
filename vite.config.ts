import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must see Rust errors, so the screen isn't cleared.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // No .env files: the app reads only import.meta.env.DEV, and HARNEO_SENTRY_DSN comes from the
  // process environment. Loading them made the dev server restart each time the Claude Code
  // sandbox mounts over ./.env (on every sandboxed command an agent runs).
  envDir: false,
  test: {
    // Room for several 5 s async waits in one test (see src/test/setup.ts).
    testTimeout: 20000,
    environment: "jsdom",
    setupFiles: "src/test/setup.ts",
    // Only this checkout's tests: subagent worktrees under .claude/ hold copies of them.
    include: ["src/**/*.test.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["lcov"],
      reportsDirectory: "coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test/**",
        "src/bindings/**",
        "src/**/*.d.ts",
      ],
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
});
