/**
 * The compact `LOCAL CHART` page's room for plan 12's mode (P12.T11): at 1280 × 720 the column of
 * what the chart is asked for must end above the stage's foot in `NOW` and in `OBSERVED`, or its
 * last rows are drawn over the chart's furniture. jsdom lays nothing out, so these tests hold the
 * stylesheet's rules the fit rests on; the fit itself was measured on the real client, hidden, at
 * landing (look-72; the figures are in `styles.css` above `.local-chart--compact`).
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { stylesheetRule } from "../../test/stylesheet";
import { StarShapeLegend } from "./StarShapeLegend";

/** The circle's kinds, as the legend names them. */
const CIRCLE_KINDS = [
  "PROTOSTAR",
  "PRE-MAIN-SEQUENCE STAR",
  "DWARF",
  "SUBGIANT",
  "HOT SUBDWARF",
  "BROWN DWARF",
];

describe("the compact LOCAL CHART page's room", () => {
  it("lays the camera's angles and keys on one row beside the chart, in their order", () => {
    expect(stylesheetRule(".local-chart--compact")).not.toMatch(/"stage\s+keys"/u);
    expect(stylesheetRule(".local-chart--compact .spatial-view__angles")).toMatch(
      /grid-area: angles;[^}]*justify-self: start;/u,
    );
    expect(stylesheetRule(".local-chart--compact .spatial-view__keys")).toMatch(
      /grid-area: angles;[^}]*justify-self: end;/u,
    );
  });

  it("takes the choices' bottom margin off in the column, as the fields have none", () => {
    expect(stylesheetRule(".local-chart--compact .chart-controls > .form-choice")).toMatch(
      /margin-bottom: 0;/u,
    );
  });

  it("sets the furniture's readings 0.75rem apart, so that UT ends the first line", () => {
    expect(
      stylesheetRule(".local-chart--compact .spatial-view__readings > .spatial-reading"),
    ).toMatch(/margin-right: 0\.75rem;/u);
    expect(stylesheetRule(".local-chart--compact .spatial-view__observer")).toMatch(
      /gap: 0 0\.75rem;[^}]*margin-left: 0\.75rem;/u,
    );
  });

  it("runs the legend on as one text at the furniture's line height", () => {
    const legend = stylesheetRule(".local-chart--compact > .symbol-legend");
    expect(legend).toMatch(/display: block;/u);
    expect(legend).toMatch(/line-height: 1\.125rem;/u);
  });

  it("breaks an entry of several kinds only after a comma, each kind whole", () => {
    expect(stylesheetRule(".local-chart--compact .symbol-legend__item--kinds")).toMatch(
      /display: inline;/u,
    );
    expect(stylesheetRule(".local-chart--compact .symbol-legend__kind")).toMatch(
      /white-space: nowrap;/u,
    );
  });

  it("holds each of the circle's kinds whole in an element of its own", () => {
    render(<StarShapeLegend compactRemnants planets />);

    for (const kind of CIRCLE_KINDS) {
      expect(screen.getByText(kind)).toHaveClass("symbol-legend__kind");
    }
  });

  it("keeps the mark and its words the entry's only two children, as the flex row lays them out", () => {
    render(<StarShapeLegend compactRemnants planets />);
    const entry = screen.getByRole("img", { name: "Circle" }).parentElement;

    expect(entry).toHaveClass("symbol-legend__item--kinds");
    expect(entry?.children).toHaveLength(2);
  });
});
