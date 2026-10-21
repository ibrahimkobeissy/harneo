import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AppInfo } from "../bindings/AppInfo";
import { AboutPanel } from "./AboutPanel";

describe("AboutPanel", () => {
  it("shows the version and data dir from app_info", async () => {
    const info: AppInfo = { version: "1.2.3", dataDir: "/data/harneo" };
    mockIPC((cmd) => (cmd === "app_info" ? info : undefined));

    render(<AboutPanel />);

    expect(screen.getByRole("status")).toHaveTextContent("Loading");
    expect(await screen.findByText(/Version 1\.2\.3/)).toBeInTheDocument();
    expect(screen.getByText(/\/data\/harneo/)).toBeInTheDocument();
  });

  it("shows an error when app_info fails", async () => {
    mockIPC(() => {
      throw "no data dir";
    });

    render(<AboutPanel />);

    expect(await screen.findByRole("alert")).toHaveTextContent("no data dir");
  });
});
