import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Layers } from "lucide-react";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import { Badge, Button, EmptyState, Kbd, NavItem, Panel, Screen, Tag } from ".";

describe("Button", () => {
  it("renders a named button that is type=button by default", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Save</Button>);
    const button = screen.getByRole("button", { name: "Save" });
    expect(button).toHaveAttribute("type", "button");
    await userEvent.click(button);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it.each(["primary", "secondary", "ghost", "danger", "danger-solid"] as const)(
    "renders the %s variant",
    (variant) => {
      render(<Button variant={variant}>Go</Button>);
      expect(screen.getByRole("button", { name: "Go" }).className).toContain(
        variant,
      );
    },
  );

  it("supports small and disabled", () => {
    render(
      <Button small disabled>
        Go
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Go" });
    expect(button).toBeDisabled();
    expect(button.className).toContain("sm");
  });
});

describe("Panel", () => {
  it("is a region labelled by its title", () => {
    render(
      <Panel title="About" titleId="p-title">
        body
      </Panel>,
    );
    expect(screen.getByRole("region", { name: "About" })).toHaveTextContent(
      "body",
    );
  });
});

describe("EmptyState", () => {
  it("shows title and sentence with a hidden icon", () => {
    render(
      <EmptyState icon={Layers} title="Nothing yet">
        Add something.
      </EmptyState>,
    );
    expect(
      screen.getByRole("heading", { name: "Nothing yet" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Add something.")).toBeInTheDocument();
    expect(document.querySelector("svg")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });
});

describe("Badge and Tag", () => {
  it("badge carries its tone", () => {
    render(<Badge tone="ok">Current</Badge>);
    expect(screen.getByText("Current")).toHaveAttribute("data-tone", "ok");
  });

  it("badge defaults to neutral", () => {
    render(<Badge>Plain</Badge>);
    expect(screen.getByText("Plain")).toHaveAttribute("data-tone", "neutral");
  });

  it("tag carries its tint and an optional dot", () => {
    render(
      <Tag tint="violet" dot>
        agents
      </Tag>,
    );
    const tag = screen.getByText("agents");
    expect(tag).toHaveAttribute("data-tint", "violet");
    expect(tag.querySelector("i")).not.toBeNull();
  });
});

describe("Kbd", () => {
  it("renders a kbd element", () => {
    render(<Kbd>Ctrl K</Kbd>);
    expect(screen.getByText("Ctrl K").tagName).toBe("KBD");
  });
});

describe("NavItem", () => {
  it("is a link with an accessible name and current state", () => {
    render(
      <MemoryRouter initialEntries={["/profiles"]}>
        <NavItem to="/profiles" label="Profiles" icon={Layers} />
        <NavItem to="/library" label="Library" icon={Layers} />
      </MemoryRouter>,
    );
    const active = screen.getByRole("link", { name: "Profiles" });
    expect(active).toHaveAttribute("aria-current", "page");
    expect(active).toHaveAttribute("title", "Profiles");
    expect(screen.getByRole("link", { name: "Library" })).not.toHaveAttribute(
      "aria-current",
    );
  });
});

describe("Screen", () => {
  it("labels its section with the h1", () => {
    render(
      <Screen title="Scaffold project">
        <p>content</p>
      </Screen>,
    );
    expect(
      screen.getByRole("region", { name: "Scaffold project" }),
    ).toContainElement(
      screen.getByRole("heading", { level: 1, name: "Scaffold project" }),
    );
  });
});
