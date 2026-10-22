import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

// findBy/waitFor wait up to 5 s instead of 1 s: check:full runs the suite with coverage while
// cargo builds, and a chain of async UI steps can outlast 1 s under that load. Assertions are
// unchanged; a passing UI still resolves as soon as it renders.
configure({ asyncUtilTimeout: 5000 });

afterEach(() => {
  cleanup();
  clearMocks();
});
