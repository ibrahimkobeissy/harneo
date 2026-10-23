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
    expect(await screen.findByText("1.2.3")).toBeInTheDocument();
    expect(screen.getByText("Version")).toBeInTheDocument();
    expect(screen.getByText(/\/data\/harneo/)).toBeInTheDocument();
  });

  it("leaves the data folder out of the grouped variant", async () => {
    mockIPC((cmd) =>
      cmd === "app_info" ? { version: "1.2.3", dataDir: "/data/harneo" } : null,
    );
    render(<AboutPanel grouped dataFolder={false} />);
    expect(await screen.findByText("1.2.3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "About Harneo" })).toBeVisible();
    expect(screen.queryByText("Data folder")).toBeNull();
    expect(screen.queryByText(/\/data\/harneo/)).toBeNull();
  });

  it("shows an error when app_info fails", async () => {
    mockIPC(() => {
      throw "no data dir";
    });

    render(<AboutPanel />);

    expect(await screen.findByRole("alert")).toHaveTextContent("no data dir");
  });
});
