import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must see Rust errors, so the screen isn't cleared.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: {
    environment: "jsdom",
    setupFiles: "src/test/setup.ts",
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
