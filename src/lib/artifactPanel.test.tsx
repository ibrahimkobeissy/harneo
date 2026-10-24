import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  ArtifactPanelProvider,
  OpenArtifactButton,
  useArtifactPanel,
  useArtifactPanelChanged,
} from "./artifactPanel";

function Probe({ onChanged }: { onChanged: () => void }) {
  const panel = useArtifactPanel();
  useArtifactPanelChanged(onChanged);
  return (
    <div>
      <output aria-label="request">
        {panel.request
          ? `${panel.request.id}:${panel.request.focus ?? "none"}`
          : "closed"}
      </output>
      <button type="button" onClick={() => panel.swapArtifact("b")}>
        swap
      </button>
      <button type="button" onClick={panel.notifyChanged}>
        change
      </button>
      <button type="button" onClick={panel.closeArtifact}>
        close
      </button>
    </div>
  );
}

describe("artifact panel context", () => {
  it("opens, swaps without the focus target, signals changes and closes", async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    render(
      <ArtifactPanelProvider>
        <OpenArtifactButton id="a" focus="trust">
          open a
        </OpenArtifactButton>
        <Probe onChanged={onChanged} />
      </ArtifactPanelProvider>,
    );
    const request = screen.getByLabelText("request");
    expect(request).toHaveTextContent("closed");

    await user.click(screen.getByRole("button", { name: "open a" }));
    expect(request).toHaveTextContent("a:trust");

    await user.click(screen.getByRole("button", { name: "swap" }));
    expect(request).toHaveTextContent("b:none");

    expect(onChanged).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "change" }));
    expect(onChanged).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "close" }));
    expect(request).toHaveTextContent("closed");
  });

  it("ignores requests outside a provider, so a screen can render alone", async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    render(
      <>
        <OpenArtifactButton id="a">open a</OpenArtifactButton>
        <Probe onChanged={onChanged} />
      </>,
    );
    await user.click(screen.getByRole("button", { name: "open a" }));
    await user.click(screen.getByRole("button", { name: "swap" }));
    await user.click(screen.getByRole("button", { name: "change" }));
    await act(async () => {
      await user.click(screen.getByRole("button", { name: "close" }));
    });
    expect(screen.getByLabelText("request")).toHaveTextContent("closed");
    expect(onChanged).not.toHaveBeenCalled();
  });
});
