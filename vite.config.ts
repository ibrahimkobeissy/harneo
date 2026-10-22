import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must see Rust errors, so the screen isn't cleared.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
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
