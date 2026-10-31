import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TagPicker } from ".";

function rect(top: number, bottom: number, left = 100): DOMRect {
  return {
    top,
    bottom,
    left,
    right: left + 80,
    width: 80,
    height: bottom - top,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

function openAt(anchor: DOMRect, menuHeight: number): HTMLElement {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(
    menuHeight,
  );
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(260);
  render(
    <TagPicker
      existing={[]}
      skip={new Set()}
      maxChars={40}
      onPick={() => {}}
    />,
  );
  const button = screen.getByRole("button", { name: /tag all/i });
  vi.spyOn(button, "getBoundingClientRect").mockReturnValue(anchor);
  fireEvent.click(button);
  const menu = screen.getByRole("combobox").parentElement;
  if (!menu) throw new Error("no menu");
  return menu;
}

describe("the tag menu's placement", () => {
  afterEach(() => vi.restoreAllMocks());

  it("opens just below its button when it fits, fixed to the window so no list clips it", () => {
    window.innerHeight = 800;
    const menu = openAt(rect(100, 122), 200);
    expect(menu.style.top).toBe("126px");
    expect(menu.style.left).toBe("100px");
  });

  it("opens above its button near the bottom of the window", () => {
    window.innerHeight = 800;
    const menu = openAt(rect(700, 722), 200);
    expect(menu.style.top).toBe("496px");
  });
});
