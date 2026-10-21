import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must see Rust errors, so the screen isn't cleared.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: { environment: "jsdom", setupFiles: "src/test/setup.ts" },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
});
