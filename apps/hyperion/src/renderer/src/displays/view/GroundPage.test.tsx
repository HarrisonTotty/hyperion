/**
 * An instrument's `GROUND` page (plan R10, R10.T12.d): the lower plate's parts as the place's body,
 * giving way as the lower plate's do, with their forms' hidden copies for its room.
 */
import { render, screen, within } from "@testing-library/react";
import { createRef, type ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { statusLine } from "../../test/statusLine";
import { GroundPage } from "./GroundPage";
import type { LowerPlatePart } from "./viewRoom";

/** A part whose text names it, and its tallest form's text. */
function part(kind: "survey" | "datum" | "readout", text: string): LowerPlatePart<ReactNode> {
  return { kind, node: <p>{text}</p>, tallest: <p>{`${text} TALLEST`}</p> };
}

const PARTS = [
  part("survey", "SURVEY COVERAGE 64 %"),
  part("datum", "DATUM AREOID"),
  part("readout", "GROUND AT LINE OF SIGHT 1.20 km"),
];

const READOUT_HELD = "NOT AVAILABLE: no room for GROUND AT LINE OF SIGHT at this window size";

/** The page with `PARTS` in a region standing for its place, its readout as given. */
function renderPage(readout: "stands" | "yields" = "stands"): void {
  render(
    <section aria-label="Ground INSTRUMENT 1">
      <GroundPage
        parts={PARTS}
        readout={readout}
        survey="stands"
        plateRef={createRef()}
        sizerRef={createRef()}
      />
    </section>,
  );
}

/** The page's live plate, its copies apart. */
function livePlate(): HTMLElement {
  const plate = screen
    .getByRole("region", { name: "Ground INSTRUMENT 1" })
    .querySelector(":scope > .view-label");
  if (!(plate instanceof HTMLElement)) {
    throw new Error("no live plate stands");
  }
  return plate;
}

/** The forms' copies' root. */
function sizer(): Element {
  const found = document.querySelector(".view-ground__sizer");
  if (found === null) {
    throw new Error("no copies are laid out");
  }
  return found;
}

describe("GroundPage (R10.T12.d; guide draft D2)", () => {
  it("stands the parts in the lower plate's order", () => {
    renderPage();
    expect([...livePlate().children].map((entry) => entry.textContent)).toEqual([
      "SURVEY COVERAGE 64 %",
      "DATUM AREOID",
      "GROUND AT LINE OF SIGHT 1.20 km",
    ]);
  });

  it("states the readout given way in its place, as the lower plate does", () => {
    renderPage("yields");
    expect(within(livePlate()).getByText(statusLine(READOUT_HELD))).toBeInTheDocument();
  });

  it("keeps its forms' copies away from assistive technology", () => {
    renderPage();
    expect(sizer().getAttribute("aria-hidden")).toBe("true");
  });

  it("keeps a copy of each form at its tallest: whole, the readout given way, the survey too", () => {
    renderPage();
    expect(
      [...sizer().querySelectorAll("[data-form]")].map((form) => form.getAttribute("data-form")),
    ).toEqual(["whole", "readout", "survey"]);
  });
});
