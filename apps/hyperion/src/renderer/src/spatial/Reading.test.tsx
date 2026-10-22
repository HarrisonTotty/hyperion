import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { stylesheetRule } from "../test/stylesheet";
import { Reading, type ReadingProps, type SpatialReading } from "./Reading";

const RADIUS: SpatialReading = { label: "RADIUS", value: "26,000.0", unit: "ly", widthCh: 9 };
const ANGLE: SpatialReading = { label: "ANGLE", value: "045.0°", unit: "", widthCh: 6 };
const MISSING_ANGLE: SpatialReading = { ...ANGLE, value: null };

/** Renders one reading in the `dl` it is written for. */
function renderReading(props: ReadingProps): void {
  render(
    <dl>
      <Reading {...props} />
    </dl>,
  );
}

/** The value a reading gives for its label, as its definition reads it. */
function definition(label: string): HTMLElement {
  const value = screen.getByText(label, { selector: "dt" }).nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`${label} has no value`);
  }
  return value;
}

/** The slot a reading keeps for its stale mark, or `null` where it has none. */
function staleSlot(label: string): Element | null {
  return definition(label).querySelector(".spatial-reading__stale-slot");
}

describe("Reading's stale slot (plan 12, P12.T11.d)", () => {
  it("gives a reading no stale slot when no stale is given", () => {
    renderReading({ reading: RADIUS });

    expect(definition("RADIUS")).toHaveTextContent(/^26,000\.0 ly$/u);
    expect(definition("RADIUS").lastElementChild).toHaveClass("spatial-reading__unit");
    expect(staleSlot("RADIUS")).toBeNull();
  });

  it("keeps an empty slot after the unit while the value is live", () => {
    renderReading({ reading: RADIUS, stale: false });

    const slot = staleSlot("RADIUS");
    expect(slot).not.toBeNull();
    expect(slot).toBeEmptyDOMElement();
    expect(definition("RADIUS").lastElementChild).toBe(slot);
  });

  it("holds the S, named stale, in its slot after the unit while the value is stale", () => {
    renderReading({ reading: RADIUS, stale: true });

    const slot = staleSlot("RADIUS");
    expect(slot).toHaveTextContent(/^S\s*stale$/u);
    expect(slot?.previousElementSibling).toHaveClass("spatial-reading__unit");
    expect(definition("RADIUS")).toHaveTextContent(/^26,000\.0 ly S\s*stale$/u);
  });

  it("holds the S after the value where the reading has no unit", () => {
    renderReading({ reading: ANGLE, stale: true });

    const slot = staleSlot("ANGLE");
    expect(slot).toHaveTextContent(/^S\s*stale$/u);
    expect(slot?.previousElementSibling).toHaveClass("spatial-reading__value");
  });

  it("keeps the slot after an em dash and marks no missing value stale", () => {
    renderReading({ reading: MISSING_ANGLE, stale: true });

    expect(definition("ANGLE")).toHaveTextContent(/^—$/u);
    expect(staleSlot("ANGLE")).toBeEmptyDOMElement();
  });

  it("keeps the slot 2ch wide, its mark right-aligned, in every state", () => {
    const slot = stylesheetRule(".spatial-reading__stale-slot");

    expect(slot).toMatch(/display: inline-block;/u);
    expect(slot).toMatch(/min-width: 2ch;/u);
    expect(slot).toMatch(/text-align: right;/u);
  });

  it("marks a value the console keeps itself as its own, with no slot", () => {
    renderReading({ reading: RADIUS, own: true });

    expect(definition("RADIUS").parentElement).toHaveClass("spatial-reading--own");
    expect(staleSlot("RADIUS")).toBeNull();
  });

  it("never marks a value the console keeps itself stale, even if given stale", () => {
    renderReading({ reading: RADIUS, own: true, stale: true });

    expect(staleSlot("RADIUS")).toBeNull();
    expect(definition("RADIUS")).toHaveTextContent(/^26,000\.0 ly$/u);
  });

  it("marks no other reading as the console's own", () => {
    renderReading({ reading: RADIUS, stale: true });

    expect(definition("RADIUS").parentElement).not.toHaveClass("spatial-reading--own");
  });
});
