import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  ArtifactPanelProvider,
  OpenArtifactButton,
  useArtifactPanelChanged,
} from "../lib/artifactPanel";
import { ArtifactPanelHost } from "./ArtifactPanelHost";

// The real panel loads an artifact over IPC; this stand-in exposes the callbacks the host wires.
vi.mock("../features/library", () => ({
  DetailPanel: (props: {
    id: string;
    previousName: string | null;
    onOpen: (id: string, fromName: string) => void;
    onBack: () => void;
    onClose: () => void;
    onRemoved: () => void;
  }) => (
    <aside aria-label="details">
      <p>showing {props.id}</p>
      <p>back to {props.previousName ?? "nothing"}</p>
      <button type="button" onClick={() => props.onOpen("b", props.id)}>
        follow
      </button>
      <button type="button" onClick={props.onBack}>
        back
      </button>
      <button type="button" onClick={props.onClose}>
        close panel
      </button>
      <button type="button" onClick={props.onRemoved}>
        delete
      </button>
    </aside>
  ),
}));

function Host({ onChanged }: { onChanged: () => void }) {
  useArtifactPanelChanged(onChanged);
  return (
    <>
      <OpenArtifactButton id="a">row a</OpenArtifactButton>
      <ArtifactPanelHost />
    </>
  );
}

function setup() {
  const onChanged = vi.fn();
  render(
    <ArtifactPanelProvider>
      <Host onChanged={onChanged} />
    </ArtifactPanelProvider>,
  );
  return { user: userEvent.setup(), onChanged };
}

describe("ArtifactPanelHost", () => {
  it("follows a reference and comes back along the trail", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "row a" }));
    expect(screen.getByText("showing a")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "follow" }));
    expect(screen.getByText("showing b")).toBeInTheDocument();
    expect(screen.getByText("back to a")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "back" }));
    expect(screen.getByText("showing a")).toBeInTheDocument();
    expect(screen.getByText("back to nothing")).toBeInTheDocument();

    // Back with an empty trail does nothing.
    await user.click(screen.getByRole("button", { name: "back" }));
    expect(screen.getByText("showing a")).toBeInTheDocument();
  });

  it("closes after a delete, tells the host page and gives focus back", async () => {
    const { user, onChanged } = setup();
    const row = screen.getByRole("button", { name: "row a" });
    await user.click(row);
    await user.click(screen.getByRole("button", { name: "delete" }));
    expect(onChanged).toHaveBeenCalledTimes(1);
    expect(screen.queryByLabelText("details")).not.toBeInTheDocument();
    expect(row).toHaveFocus();
  });
});
