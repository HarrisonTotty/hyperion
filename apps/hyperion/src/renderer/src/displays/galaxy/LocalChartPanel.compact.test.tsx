/**
 * The compact `LOCAL CHART` page's room for plan 12's mode (P12.T11) and for the legend's seventh
 * circle kind (P06.T24.c): at 1280 × 720 the column of what the chart is asked for must end above
 * the stage's foot in `NOW` and in `OBSERVED`, or its last rows are drawn over the chart's
 * furniture, and the stage keeps 14rem (ruling 134.9, as lowered on 2026-10-21). jsdom lays nothing
 * out, so these tests hold the stylesheet's rules the fit rests on, and the order the view's parts
 * stand in; the fit itself was measured on the real client, hidden, at landing (look-72), again
 * for the stripped helium star's entry (stripped-class's look on look-72's harness), and again at
 * the landing look of #79 to #81 (look-79-81), whose two state lines at once, a census hint and a
 * summary on two lines, moved the census's hint into its row (plan 12, P12.T11.g; the figures are
 * in `styles.css` above `.local-chart--compact`).
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { localFrameAt, type LocalFrame } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import type { SpatialMode } from "../../spatial/ModeReading";
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
function renderView(
  frame: LocalFrame = localFrameAt(vec3(26_000, 0, 0)),
  mode?: SpatialMode,
): HTMLElement {
  stubCanvas();
  const { container } = render(
    <SpatialView
      scene={{
        frame,
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
      {...(mode === undefined ? {} : { mode })}
    />,
  );
  return container;
}

/** Lays every element out at 400 by 300 px, so that the view has a viewport and draws its scale bar. */
function stubLayout(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 400, height: 300 }),
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
    // Six rows, of which the stage's gives way; the census's hint has none of its own (P12.T11.g).
    expect(page).toMatch(/grid-template-rows: auto auto minmax\(0, 1fr\) auto auto auto;/u);
    expect(page).not.toMatch(/"hint\s+hint"/u);
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

describe("the compact LOCAL CHART page's state lines in the census's room (plan 12, P12.T11.g)", () => {
  it("sets the census row as a grid, the summary's column as wide as it is up to what its controls leave", () => {
    const row = stylesheetRule(".local-chart--compact .census-readout__row");
    expect(row).toMatch(/display: grid;/u);
    expect(row).toMatch(/grid-area: census;/u);
    expect(row).toMatch(
      /grid-template-columns: minmax\(0, max-content\) max-content max-content;/u,
    );
    expect(row).toMatch(/align-items: center;/u);
    expect(row).toMatch(/gap: 0;/u);
  });

  it("sets the summary and the hint 1rem a line, as the furniture's lines are", () => {
    expect(
      stylesheetRule(".local-chart--compact :is(.census-readout__summary, .census-readout__hint)"),
    ).toMatch(/line-height: 1rem;/u);
  });

  it("stands RETRY and CENSUS BY LAYER in columns of their own, 1rem after the summary by margin", () => {
    expect(stylesheetRule(".local-chart--compact .census-readout__summary")).toMatch(
      /grid-area: 1 \/ 1;/u,
    );
    const retry = stylesheetRule(".local-chart--compact .census-readout__retry");
    expect(retry).toMatch(/grid-area: 1 \/ 2;/u);
    expect(retry).toMatch(/margin-left: 1rem;/u);
    const toggle = stylesheetRule(".local-chart--compact .census-readout__toggle");
    expect(toggle).toMatch(/grid-area: 1 \/ 3;/u);
    expect(toggle).toMatch(/margin-left: 1rem;/u);
  });

  it("sets a hint under the summary, the toggle spanning both of its lines from the top", () => {
    expect(stylesheetRule(".local-chart--compact .census-readout__hint")).toMatch(
      /grid-area: 2 \/ 1 \/ 3 \/ 2;/u,
    );
    const toggle = stylesheetRule(
      ".local-chart--compact .census-readout__row:has(> .census-readout__hint) > .census-readout__toggle",
    );
    expect(toggle).toMatch(/grid-row: 1 \/ span 2;/u);
    expect(toggle).toMatch(/align-self: start;/u);
  });

  it("keeps the stacked page's hint on a line of its own under the census row", () => {
    expect(stylesheetRule(".census-readout__row > .census-readout__hint")).toMatch(
      /flex: 0 0 100%;/u,
    );
  });

  it("runs the axis note on in the furniture's line, whole, the readings' 0.75rem before what follows", () => {
    expect(stylesheetRule(".spatial-view__axis-note")).not.toMatch(/flex-basis/u);
    const note = stylesheetRule(".local-chart--compact .spatial-view__axis-note");
    expect(note).toMatch(/display: inline-block;/u);
    expect(note).toMatch(/margin-right: 0\.75rem;/u);
    expect(note).toMatch(/white-space: nowrap;/u);
  });

  it("sets the axis note after the time and the scale bar and before the mode", () => {
    stubLayout();
    const view = renderView(localFrameAt(vec3(0, 0, 3000)), { name: "NOW", from: [] });
    const time = screen.getByText("UT", { selector: "dt" });
    const bar = view.querySelector(".spatial-view__furniture > .scale-bar");
    const note = view.querySelector(".spatial-view__axis-note");
    const mode = screen.getByText("MODE", { selector: "dt" });
    if (bar === null || note === null) {
      throw new Error("the furniture has no scale bar or no axis note on the axis");
    }

    expect([follows(time, bar), follows(bar, note), follows(note, mode)]).toEqual([
      true,
      true,
      true,
    ]);
  });

  it("sets the axis note last, after the scale bar, on a view with no mode", () => {
    stubLayout();
    const view = renderView(localFrameAt(vec3(0, 0, 3000)));
    const furniture = view.querySelector(".spatial-view__furniture");
    const bar = view.querySelector(".spatial-view__furniture > .scale-bar");
    const note = view.querySelector(".spatial-view__axis-note");
    if (furniture === null || bar === null || note === null) {
      throw new Error("the furniture has no scale bar or no axis note on the axis");
    }

    expect([follows(bar, note), furniture.lastElementChild]).toEqual([true, note]);
  });
});
