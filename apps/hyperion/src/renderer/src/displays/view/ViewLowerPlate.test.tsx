/**
 * The `PRIMARY` view's lower plate (plan R10, R10.T12.b; decision-r10-view-block-room §3): its
 * parts in the guide's order, each yield line in the place of what it holds back, the state lines
 * that never give way, and each part's tallest form for the hidden sizer.
 */
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { statusLine } from "../../test/statusLine";
import { ViewLowerPlate } from "./ViewLowerPlate";
import type { LowerPlatePart } from "./viewRoom";

/** A part whose text names it, and its tallest form's text. */
function part(kind: "survey" | "datum" | "readout", text: string): LowerPlatePart<ReactNode> {
  return { kind, node: <p>{text}</p>, tallest: <p>{`${text} TALLEST`}</p> };
}

const SURVEY = part("survey", "SURVEY COVERAGE 64 %");
const DATUM = part("datum", "DATUM AREOID");
const READOUT = part("readout", "GROUND AT LINE OF SIGHT 1.20 km");

const PENDING = "SURVEY COVERAGE: PENDING";
const STATE: LowerPlatePart<ReactNode> = {
  kind: "state",
  key: PENDING,
  node: <p>{PENDING}</p>,
};

const READOUT_HELD = "NOT AVAILABLE: no room for GROUND AT LINE OF SIGHT at this window size";

/** The plate's entries as they read, in order. */
function entries(): string[] {
  const plate = document.querySelector(".view-label--lower");
  if (plate === null) {
    throw new Error("no lower plate is rendered");
  }
  return [...plate.children].map((entry) => entry.textContent);
}

describe("VIEW's lower plate (R10.T12.b)", () => {
  it("sets its parts in the order given: the survey line, DATUM, then the readout", () => {
    render(<ViewLowerPlate parts={[SURVEY, DATUM, READOUT]} readout="stands" survey="stands" />);
    expect(entries()).toEqual([
      "SURVEY COVERAGE 64 %",
      "DATUM AREOID",
      "GROUND AT LINE OF SIGHT 1.20 km",
    ]);
  });

  it("states the readout's yield line in its place", () => {
    render(<ViewLowerPlate parts={[SURVEY, DATUM, READOUT]} readout="yields" survey="stands" />);
    expect(entries()).toEqual(["SURVEY COVERAGE 64 %", "DATUM AREOID", READOUT_HELD]);
  });

  it("states one yield line for the survey line, DATUM and the readout, in the survey line's place", () => {
    render(<ViewLowerPlate parts={[SURVEY, DATUM, READOUT]} readout="stands" survey="yields" />);
    expect(entries()).toEqual([
      "NOT AVAILABLE: no room for SURVEY COVERAGE, DATUM or GROUND AT LINE OF SIGHT at this window size",
    ]);
  });

  it("names the survey line alone where it stands alone", () => {
    render(<ViewLowerPlate parts={[SURVEY]} readout="yields" survey="yields" />);
    expect(entries()).toEqual(["NOT AVAILABLE: no room for SURVEY COVERAGE at this window size"]);
  });

  it("sets the yield line as a statement, its state tracked", () => {
    render(<ViewLowerPlate parts={[SURVEY, DATUM, READOUT]} readout="yields" survey="stands" />);
    expect(screen.getByText(statusLine(READOUT_HELD))).toHaveClass(
      "view-label__statement",
      "view-label__yield",
    );
  });

  it.each([
    ["stands", "stands"],
    ["yields", "stands"],
    ["yields", "yields"],
  ] as const)(
    "keeps the survey's state line standing with the readout %s and the survey %s",
    (readout, survey) => {
      render(<ViewLowerPlate parts={[STATE, DATUM, READOUT]} readout={readout} survey={survey} />);
      expect(entries()[0]).toBe(PENDING);
    },
  );

  it("names only the parts it holds back beside a state line", () => {
    render(<ViewLowerPlate parts={[STATE, DATUM, READOUT]} readout="yields" survey="yields" />);
    expect(entries()).toEqual([
      PENDING,
      "NOT AVAILABLE: no room for DATUM or GROUND AT LINE OF SIGHT at this window size",
    ]);
  });

  it("shows each part in its tallest form for the sizer", () => {
    render(
      <ViewLowerPlate parts={[SURVEY, DATUM, READOUT]} readout="stands" survey="stands" tallest />,
    );
    expect(entries()).toEqual([
      "SURVEY COVERAGE 64 % TALLEST",
      "DATUM AREOID TALLEST",
      "GROUND AT LINE OF SIGHT 1.20 km TALLEST",
    ]);
  });

  it("carries the class it is given beside the slots", () => {
    render(
      <ViewLowerPlate
        parts={[SURVEY]}
        readout="stands"
        survey="stands"
        className="view-label--beside"
      />,
    );
    expect(document.querySelector(".view-label--lower")).toHaveClass("view-label--beside");
  });
});
