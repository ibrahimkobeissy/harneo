import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Layers } from "lucide-react";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import {
  Badge,
  Button,
  EmptyState,
  FileTree,
  Kbd,
  NavItem,
  Panel,
  Screen,
  Tag,
} from ".";

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

  it("collapsible: starts closed, keeps the intro, opens from its title", async () => {
    const user = userEvent.setup();
    render(
      <Panel
        title="Files"
        titleId="f-title"
        collapsible
        defaultOpen={false}
        intro={<p>2 skills</p>}
      >
        <p>the tree</p>
      </Panel>,
    );
    const toggle = screen.getByRole("button", { name: "Files" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("2 skills")).toBeVisible();
    expect(screen.queryByText("the tree")).toBeNull();
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("the tree")).toBeVisible();
  });

  it("collapsible: forceOpen opens a closed panel", () => {
    const { rerender } = render(
      <Panel title="Files" titleId="f-title" collapsible defaultOpen={false}>
        <p>the tree</p>
      </Panel>,
    );
    expect(screen.queryByText("the tree")).toBeNull();
    rerender(
      <Panel
        title="Files"
        titleId="f-title"
        collapsible
        defaultOpen={false}
        forceOpen
      >
        <p>the tree</p>
      </Panel>,
    );
    expect(screen.getByText("the tree")).toBeVisible();
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
    expect(active).toHaveAttribute("data-tip", "Profiles");
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

  it("keeps the title row and toolbar outside the scrolling body", () => {
    render(
      <Screen title="Library" toolbar={<p>toolbar</p>}>
        <p>content</p>
      </Screen>,
    );
    const head = document.querySelector("[data-screen-head]");
    const body = document.querySelector("[data-screen-body]");
    expect(head).toContainElement(
      screen.getByRole("heading", { level: 1, name: "Library" }),
    );
    expect(head).toContainElement(screen.getByText("toolbar"));
    expect(body).toContainElement(screen.getByText("content"));
    expect(body).not.toContainElement(head as HTMLElement);
  });
});

describe("FileTree collapsible options", () => {
  const files = [{ path: "a/b/x.md" }, { path: "a/y.md" }, { path: "root.md" }];

  it("starts collapsed with folder counts, keeping root files visible", async () => {
    const onSelect = vi.fn();
    render(
      <FileTree
        files={files}
        selected={null}
        onSelect={onSelect}
        defaultCollapsed
        showCounts
        action={(p) => <button type="button">act {p}</button>}
      />,
    );
    const folder = screen.getByRole("treeitem", { name: /^a\// });
    expect(folder).toHaveAttribute("aria-expanded", "false");
    expect(folder).toHaveTextContent("2");
    expect(screen.queryByText("y.md")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "act root.md" }));
    expect(onSelect).not.toHaveBeenCalled();
    await userEvent.click(folder);
    expect(screen.getByText("y.md")).toBeInTheDocument();
    await userEvent.click(screen.getByText("y.md"));
    expect(onSelect).toHaveBeenCalledWith("a/y.md");
  });

  it("stays expanded without counts by default", () => {
    render(<FileTree files={files} selected={null} onSelect={() => {}} />);
    expect(screen.getByText("y.md")).toBeInTheDocument();
    expect(
      screen.getByRole("treeitem", { name: /^a\// }),
    ).not.toHaveTextContent(/\d/);
  });
});
