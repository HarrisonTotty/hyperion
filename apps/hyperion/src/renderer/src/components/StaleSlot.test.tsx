import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { stylesheetRule } from "../test/stylesheet";
import { StaleSlot } from "./StaleSlot";

/** A value with its slot after it, in an `output` of its own, as a reading sets one. */
function renderValue(stale: boolean) {
  render(
    <output>
      12.4
      <StaleSlot stale={stale} />
    </output>,
  );
  const value = screen.getByRole("status");
  const slot = value.querySelector(".stale-slot");
  if (slot === null) {
    throw new Error("the value has no stale slot");
  }
  return { value, slot };
}

describe("StaleSlot", () => {
  it("stands empty after a live value, keeping its room", () => {
    const { value, slot } = renderValue(false);

    expect(slot).toBeEmptyDOMElement();
    expect(value).toHaveTextContent(/^12\.4$/u);
  });

  it("holds the guide's S while the value is stale, with its word for assistive technology", () => {
    const { slot } = renderValue(true);

    expect(slot).toHaveTextContent(/^S\s*stale$/u);
    expect(screen.getByText("S")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("stale")).toHaveClass("visually-hidden");
  });

  it("is 2 ch wide, live or stale, with its mark at its right", () => {
    // jsdom lays nothing out, so the width is held by the rule as written.
    expect(stylesheetRule(".stale-slot")).toMatch(
      /display: inline-block;[^}]*min-width: 2ch;[^}]*text-align: right;/u,
    );
  });
});
