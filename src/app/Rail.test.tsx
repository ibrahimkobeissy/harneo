import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { initialsOf } from "../ui/initials";
import { CollectionsProvider } from "./collections";
import { Sidebar } from "./Sidebar";

describe("initialsOf", () => {
  it("takes the first letters of up to two words", () => {
    expect(initialsOf("Review kit")).toBe("RK");
    expect(initialsOf("Rust")).toBe("R");
    expect(initialsOf("my big review kit")).toBe("MB");
    expect(initialsOf("rust-tooling")).toBe("RT");
    expect(initialsOf("  ")).toBe("?");
  });
});

describe("sidebar collections in the rail", () => {
  it("carry their initials, with the full name as accessible name and tooltip", async () => {
    mockIPC((cmd) =>
      cmd === "collections_list"
        ? [
            { id: 1, name: "Review kit", kind: "manual", rule: null, count: 3 },
            { id: 2, name: "Testing", kind: "smart", rule: null, count: 4 },
          ]
        : undefined,
    );
    render(
      <MemoryRouter>
        <CollectionsProvider>
          <Sidebar onShortcuts={() => {}} onSearch={() => {}} />
        </CollectionsProvider>
      </MemoryRouter>,
    );
    const kit = await screen.findByRole("link", { name: "Review kit" });
    expect(kit).toHaveAttribute("data-tip", "Review kit");
    expect(kit).toHaveTextContent("RK");
    expect(screen.getByRole("link", { name: "Testing" })).toHaveTextContent(
      "T",
    );
  });
});
