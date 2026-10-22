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

  it("sets the furniture's readings 0.75rem apart, so that it keeps two lines in NOW and three observed", () => {
    expect(
      stylesheetRule(".local-chart--compact .spatial-view__readings > .spatial-reading"),
    ).toMatch(/margin-right: 0\.75rem;/u);
    expect(stylesheetRule(".local-chart--compact .spatial-view__observer")).toMatch(
      /gap: 0 0\.75rem;[^}]*margin-left: 0\.75rem;/u,
    );
  });

  it("sets the scale bar the readings' 0.75rem before the mode that follows it (P12.T11.d)", () => {
    const bar = stylesheetRule(".local-chart--compact .spatial-view__furniture > .scale-bar");
    expect(bar).toMatch(/display: inline-flex;/u);
    expect(bar).toMatch(/margin-right: 0\.75rem;/u);
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

describe("the compact LOCAL CHART page's refusals (plan 12, P12.T11.c)", () => {
  it("stands a refusal on its field's row, taking no line of its own and never wrapping", () => {
    const refusal = stylesheetRule(".local-chart--compact .chart-controls__error");

    expect(refusal).toMatch(/flex-basis: auto;/u);
    expect(refusal).toMatch(/margin: 0;/u);
    expect(refusal).toMatch(/white-space: nowrap;/u);
  });

  it("gives the hint way while a refusal stands, with display none, which the description still reaches", () => {
    expect(
      stylesheetRule(
        ".local-chart--compact .chart-controls__field--refused > .chart-controls__hint",
      ),
    ).toMatch(/display: none;/u);
  });

  it("hides the refusal's field name from sight alone, as .visually-hidden does", () => {
    const hidden = stylesheetRule(".local-chart--compact .chart-controls__error-name");

    for (const declaration of stylesheetRule(".visually-hidden")
      .trim()
      .split(/\s*\n\s*/u)) {
      expect(hidden).toContain(declaration);
    }
    expect(hidden).not.toMatch(/display: none|visibility: hidden/u);
  });

  it("keeps the stacked page's refusal on a line of its own under its field", () => {
    expect(stylesheetRule(".chart-controls__error")).toMatch(/flex-basis: 100%;/u);
    expect(stylesheetRule(".chart-controls__hint")).not.toMatch(/display:/u);
  });
});

describe("the compact LOCAL CHART page's mode row (plan 12, P12.T11.c)", () => {
  it("sets the mode and the command 0.5rem apart, a field's own gap", () => {
    expect(stylesheetRule(".local-chart--compact .chart-controls__modes")).toMatch(
      /gap: 0 0\.5rem;/u,
    );
    expect(stylesheetRule(".chart-controls__field")).toMatch(/gap: 0 0\.5rem;/u);
  });

  it("keeps the row on one line, its parts unshrunk, while no held-back reason of its own stands", () => {
    expect(
      stylesheetRule(
        ".local-chart--compact .chart-controls__modes:not(.chart-controls__modes--reason)",
      ),
    ).toMatch(/flex-wrap: nowrap;/u);
    expect(
      stylesheetRule(
        ".local-chart--compact .chart-controls__modes:not(.chart-controls__modes--reason) > *",
      ),
    ).toMatch(/flex: none;/u);
  });

  it("lets the row wrap its reasons under it with no chart on show, as the stacked page does", () => {
    expect(stylesheetRule(".chart-controls__modes")).toMatch(/flex-wrap: wrap;/u);
    expect(stylesheetRule(".local-chart--compact .chart-controls__modes")).not.toMatch(
      /flex-wrap/u,
    );
  });

  it("leaves the controls' own padding and letter spacing alone, every station's", () => {
    expect(stylesheetRule(".control")).toMatch(/padding: 0 0\.5rem;/u);
    expect(stylesheetRule(".control")).toMatch(/letter-spacing: 0\.15em;/u);
  });
});
