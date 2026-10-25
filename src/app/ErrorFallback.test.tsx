import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ErrorFallback } from "./ErrorFallback";

describe("ErrorFallback", () => {
  it("is calm and offers a reload", () => {
    render(<ErrorFallback />);
    expect(screen.getByText("Something went wrong")).toBeVisible();
    expect(screen.getByRole("button", { name: "Reload Harneo" })).toBeVisible();
  });
});
