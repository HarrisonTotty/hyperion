/**
 * The compact `LOCAL CHART` page's room for plan 12's mode (P12.T11) and for the legend's seventh
 * circle kind (P06.T24.c): at 1280 × 720 the column of what the chart is asked for must end above
 * the stage's foot in `NOW` and in `OBSERVED`, or its last rows are drawn over the chart's
 * furniture, and the stage keeps 14rem (ruling 134.9, as lowered on 2026-10-21). jsdom lays nothing
 * out, so these tests hold the stylesheet's rules the fit rests on, and the order the view's parts
 * stand in; the fit itself was measured on the real client, hidden, at landing (look-72), and again
 * for the stripped helium star's entry (stripped-class's look on look-72's harness; the figures are
 * in `styles.css` above `.local-chart--compact`).
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { SpatialView } from "../../spatial/SpatialView";
import { stubCanvas } from "../../test/RecordingContext2D";
import { stylesheetRule } from "../../test/stylesheet";
import { StarShapeLegend } from "./StarShapeLegend";

/** The circle's kinds, as the legend names them. */
const CIRCLE_KINDS = [
  "PROTOSTAR",
  "PRE-MAIN-SEQUENCE STAR",
  "DWARF",
  "SUBGIANT",
  "HOT SUBDWARF",
  "STRIPPED HELIUM STAR",
  "BROWN DWARF",
];

/** A spatial view as the chart draws it, with one system, for the order of its parts. */
function renderView(): void {
  stubCanvas();
  render(
    <SpatialView
      scene={{
        frame: localFrameAt(vec3(26_000, 0, 0)),
        points: [],
        spheres: [{ radius: 10, role: "data_edge", label: "QUERY EDGE 10 ly" }],
        plane: { spacing: 5, extent: 10, rings: [] },
        selectedId: null,
        destinationId: null,
      }}
      fitRadius={10}
      formatLength={(length) => `${length} ly`}
      frameName="GALACTIC"
      centre={[{ label: "RADIUS", value: "26,000.0", unit: "ly", widthCh: 9 }]}
      time={{ label: "UT", value: "+0.00", unit: "yr", widthCh: 9 }}
      coreDistance={{ value: "26,000.0", unit: "ly" }}
      accessibleName="Local chart"
      stale={false}
      onSelect={() => undefined}
    />,
  );
}

/** Whether `later` comes after `earlier` in the document, and so in the reading order. */
function follows(earlier: Element, later: Element): boolean {
  return (earlier.compareDocumentPosition(later) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

describe("the compact LOCAL CHART page's room", () => {
  it("keeps the column beside the stage for what the chart is asked for alone", () => {
    const page = stylesheetRule(".local-chart--compact");

    expect(page).toMatch(/"stage\s+controls"/u);
    expect(page).not.toMatch(/"stage\s+angles"/u);
    expect(page).not.toMatch(/"stage\s+keys"/u);
    // Seven rows, of which the stage's gives way.
    expect(page).toMatch(/grid-template-rows: auto auto minmax\(0, 1fr\) auto auto auto auto;/u);
  });

  it("stands the camera's angles and keys at the end of the presets' row, in their order, as the stacked page does", () => {
    expect(stylesheetRule(".local-chart--compact .preset-buttons")).toMatch(
      /grid-area: presets;[^}]*justify-self: start;/u,
    );
    const angles = stylesheetRule(".local-chart--compact .spatial-view__angles");
    expect(angles).toMatch(/grid-area: presets;/u);
    expect(angles).toMatch(/align-self: start;/u);
    expect(angles).toMatch(/justify-self: end;/u);
    expect(angles).toMatch(/line-height: 1rem;/u);
    // The keys a line down, so that the pair sets its own height.
    const keys = stylesheetRule(".local-chart--compact .spatial-view__keys");
    expect(keys).toMatch(/grid-area: presets;/u);
    expect(keys).toMatch(/align-self: start;/u);
    expect(keys).toMatch(/justify-self: end;/u);
    expect(keys).toMatch(/margin-top: 1rem;/u);
    expect(keys).toMatch(/line-height: 1rem;/u);
  });

  it("reads the presets, the angles, the keys and then the chart, in the stacked page's order", () => {
    renderView();
    const presets = screen.getByRole("button", { name: "T TOP" });
    const angles = screen.getByText("AZM", { selector: "dt" });
    const keys = screen.getByText("ARROWS ROTATE");
    const canvas = screen.getByRole("application", { name: "Local chart" });

    expect(follows(presets, angles)).toBe(true);
    expect(follows(angles, keys)).toBe(true);
    expect(follows(keys, canvas)).toBe(true);
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

  it("sets the furniture's lines 1rem apart, so that the stage keeps its 14rem", () => {
    expect(stylesheetRule(".local-chart--compact .spatial-view__furniture")).toMatch(
      /line-height: 1rem;/u,
    );
  });

  it("sets the scale bar the readings' 0.75rem before the mode that follows it (P12.T11.d)", () => {
    const bar = stylesheetRule(".local-chart--compact .spatial-view__furniture > .scale-bar");
    expect(bar).toMatch(/display: inline-flex;/u);
    expect(bar).toMatch(/margin-right: 0\.75rem;/u);
  });

  it("runs the legend on as one text, its lines 1.125rem apart to hold its marks", () => {
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
