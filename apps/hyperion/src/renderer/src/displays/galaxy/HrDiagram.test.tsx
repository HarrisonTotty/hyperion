import type { StellarBriefDto, SystemIdHex } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { hrXPx, hrYPx } from "../../lib/galaxy/hrProjection";
import { type ChartSystem, NOW_MODE, type QueryMode } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { aStellarBrief, aSystemsInRange } from "../../test/galaxyFixtures";
import {
  BANNED_SPATIAL_MEMBERS,
  type RecordingContext2D,
  stubCanvas,
} from "../../test/RecordingContext2D";
import { stylesheetRule } from "../../test/stylesheet";
import { type StarFilter } from "./chartModel";
import { HrDiagram } from "./HrDiagram";

/** The stage, and every other element, laid out 400 × 300 at 16 px to the rem. */
const WIDTH_PX = 400;
const HEIGHT_PX = 300;

/** The plot inside that stage: 4 rem in from the left, 1.5 rem from the other edges. */
const AREA = { leftPx: 64, topPx: 24, widthPx: 312, heightPx: 252 };

/** The Sun, a white dwarf, a neutron star, a star that left no remnant and a black hole. */
const SYSTEMS: ReadonlyArray<ChartSystem> = toChartResult(
  aSystemsInRange({
    systems: [
      { relLy: [1, 0, 0], layer: "c", stellar: aStellarBrief("c", "dwarf") },
      { relLy: [2, 0, 0], layer: "c", stellar: aStellarBrief("c", "white_dwarf") },
      { relLy: [3, 0, 0], layer: "e", stellar: aStellarBrief("e", "neutron_star") },
      { relLy: [4, 0, 0], layer: "e", stellar: aStellarBrief("e", "no_remnant") },
      { relLy: [5, 0, 0], layer: "e", stellar: aStellarBrief("e", "black_hole") },
    ],
  }),
).systems;

function stubLayout(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
}

interface RenderSpec {
  readonly systems?: ReadonlyArray<ChartSystem>;
  readonly total?: number;
  readonly starFilter?: StarFilter;
  readonly selectedId?: SystemIdHex | null;
  readonly stale?: boolean;
  readonly mode?: QueryMode;
  readonly timeYr?: number;
}

function renderDiagram({
  systems = SYSTEMS,
  total = systems.length,
  starFilter = "all",
  selectedId = null,
  stale = false,
  mode = NOW_MODE,
  timeYr = 0,
}: RenderSpec = {}) {
  const recorder = stubCanvas();
  stubLayout();
  const onSelect = vi.fn<(id: SystemIdHex) => void>();
  const view = (id: SystemIdHex | null) => (
    <HrDiagram
      systems={systems}
      total={total}
      starFilter={starFilter}
      selectedId={id}
      onSelect={onSelect}
      driveRangeLy={50}
      stale={stale}
      mode={mode}
      timeYr={timeYr}
    />
  );
  const { rerender } = render(view(selectedId));
  return {
    recorder,
    onSelect,
    select: (id: SystemIdHex | null) => {
      rerender(view(id));
    },
  };
}

function canvas(): HTMLElement {
  return screen.getByRole("img", { name: /^Hertzsprung-Russell diagram/u });
}

/** The count read beside `label` under the diagram. */
function count(label: string): string {
  return screen.getByText(label, { selector: "dt" }).nextElementSibling?.textContent ?? "";
}

/**
 * The last element of a count's value: its stale slot, which only the `SYSTEMS` count's filter
 * words, text and no element, follow.
 */
function countSlot(label: string): Element | null {
  return screen.getByText(label, { selector: "dt" }).nextElementSibling?.lastElementChild ?? null;
}

/**
 * The text of every label matching `selector`, in document order: by class, since the labels are
 * hidden from assistive technology (the canvas's name stands for the picture) and their order along
 * the axis is what is checked.
 */
function values(selector: string): Array<string | null> {
  return [...document.querySelectorAll(selector)].map((label) => label.textContent);
}

/** How many times the canvas was painted: each paint begins by clearing it. */
function paints(recorder: RecordingContext2D): number {
  return recorder.calls("fillRect").length;
}

describe("HrDiagram", () => {
  it("is an image named for the diagram, which takes no focus", () => {
    renderDiagram();

    expect(canvas().tagName).toBe("CANVAS");
    expect(canvas()).not.toHaveAttribute("tabindex");
  });

  it("titles the graph and labels each axis with its unit", () => {
    renderDiagram();

    // One name for the page: the tab's, with the canvas's accessible name spelled out (ruling 115.4).
    expect(screen.getByText("HR DIAGRAM")).toBeInTheDocument();
    expect(screen.queryByText("HERTZSPRUNG-RUSSELL DIAGRAM")).not.toBeInTheDocument();
    expect(canvas()).toHaveAccessibleName("Hertzsprung-Russell diagram");
    expect(screen.getByText(/^LUMINOSITY/u)).toContainElement(
      screen.getByRole("img", { name: "solar luminosities" }),
    );
    expect(screen.getByText("EFFECTIVE TEMPERATURE K, LOG SCALE")).toBeInTheDocument();
    expect(screen.getByText(/^LUMINOSITY/u)).toHaveTextContent(/, LOG SCALE$/u);
  });

  it("writes each axis's ticks in one form, and labels the spectral letters' scale", () => {
    renderDiagram();

    // Every temperature can be set plainly; the faintest luminosity cannot (ruling 115.1).
    expect(values(".hr-diagram__label--x")).toEqual([
      "100,000",
      "30,000",
      "10,000",
      "3000",
      "1000",
    ]);
    expect(values(".hr-diagram__label--y")).toEqual([
      "1E-6",
      "1E-4",
      "1E-2",
      "1E0",
      "1E2",
      "1E4",
      "1E6",
    ]);
    for (const letter of ["O", "B", "A", "F", "G", "K", "M", "L", "T"]) {
      expect(screen.getByText(letter, { selector: ".hr-diagram__label--class" })).toBeVisible();
    }
    expect(values(".hr-diagram__label--class-axis")).toEqual(["CLASS"]);
  });

  it("counts what it plots, then what it does not under NOT PLOTTED, by kind or state", () => {
    renderDiagram();

    expect(count("PLOTTED")).toBe("2");
    expect(count("OFF SCALE")).toBe("0");
    const notPlotted = screen.getByRole("heading", { name: "NOT PLOTTED" }).nextElementSibling;
    expect(notPlotted?.tagName).toBe("DL");
    expect([...(notPlotted?.querySelectorAll("dt") ?? [])].map((dt) => dt.textContent)).toEqual([
      "NEUTRON STAR",
      "BLACK HOLE",
      "NO REMNANT",
      "NOT YET FORMED",
    ]);
    expect(count("NEUTRON STAR")).toBe("1");
    expect(count("BLACK HOLE")).toBe("1");
    expect(count("NO REMNANT")).toBe("1");
    expect(count("NOT YET FORMED")).toBe("0");
    // Words ruling 115.3 retired: a neutron star has a photosphere, and "no data" reads as none.
    expect(screen.queryByText("NO PHOTOSPHERE")).not.toBeInTheDocument();
    expect(screen.queryByText("NO DATA")).not.toBeInTheDocument();
    expect(screen.queryByText("DATA INVALID")).not.toBeInTheDocument();
    // No free-floating planet, so no count of them (plan 13, P13.T8.d).
    expect(screen.queryByText("PLANET", { selector: "dt" })).not.toBeInTheDocument();
  });

  it("counts free-floating planets under NOT PLOTTED once there are some", () => {
    const planets = toChartResult(
      aSystemsInRange({
        minLayer: "rogue_planet",
        systems: [
          { relLy: [1, 0, 0], layer: "c" },
          { relLy: [2, 0, 0], layer: "rogue_planet" },
          { relLy: [3, 0, 0], layer: "rogue_planet" },
        ],
      }),
    ).systems;
    renderDiagram({ systems: planets });

    expect(count("PLOTTED")).toBe("1");
    expect(count("PLANET")).toBe("2");
  });

  it("counts a plotted kind whose values fail its checks as DATA INVALID", () => {
    const broken: StellarBriefDto = { ...aStellarBrief("c", "dwarf"), teff_k: null };
    const systems = toChartResult(
      aSystemsInRange({ systems: [{ relLy: [1, 0, 0], layer: "c", stellar: broken }] }),
    ).systems;

    renderDiagram({ systems });

    expect(count("DATA INVALID")).toBe("1");
    expect(count("PLOTTED")).toBe("0");
  });

  it("counts a point beyond an axis as off scale", () => {
    const hot: StellarBriefDto = {
      ...aStellarBrief("c", "white_dwarf"),
      teff_k: 250_000,
      log_luminosity_lsun: 1,
    };
    const systems = toChartResult(
      aSystemsInRange({ systems: [{ relLy: [1, 0, 0], layer: "c", stellar: hot }] }),
    ).systems;

    renderDiagram({ systems });

    expect(count("OFF SCALE")).toBe("1");
    expect(count("PLOTTED")).toBe("1");
  });

  it("renders with no points, its counts all zero", () => {
    const { recorder } = renderDiagram({ systems: [] });

    expect(canvas()).toBeInTheDocument();
    expect(count("PLOTTED")).toBe("0");
    expect(recorder.calls("arc")).toHaveLength(0);
    expect(paints(recorder)).toBe(1);
  });

  it("says what the STARS filter hides", () => {
    renderDiagram({ systems: SYSTEMS.slice(0, 1), total: 4, starFilter: "living" });

    expect(count("SYSTEMS")).toBe("1 OF 4 SHOWN: LIVING");
  });

  it("says its symbols are not to scale and names every shape", () => {
    renderDiagram();

    const legend = screen.getByRole("group", { name: "Hertzsprung-Russell diagram legend" });
    expect(within(legend).getByText("SYMBOLS NOT TO SCALE")).toBeInTheDocument();
    // The arrowhead's entry is the state's own name, as the caption's count is (ruling 115.2).
    expect(within(legend).getByText("OFF SCALE")).toBeInTheDocument();
    expect(within(legend).queryByText(/PEGGED/u)).not.toBeInTheDocument();
    for (const name of ["Circle", "Ringed circle", "Diamond"]) {
      expect(within(legend).getByRole("img", { name })).toBeInTheDocument();
    }
  });

  it("leaves out of its legend the shapes it counts and does not plot", () => {
    renderDiagram();

    const legend = screen.getByRole("group", { name: "Hertzsprung-Russell diagram legend" });
    expect(within(legend).queryByRole("img", { name: "Triangle" })).not.toBeInTheDocument();
    expect(within(legend).queryByRole("img", { name: "Square" })).not.toBeInTheDocument();
  });

  it("marks every count stale when the answer is", () => {
    renderDiagram({ stale: true });

    for (const label of [
      "PLOTTED",
      "OFF SCALE",
      "NEUTRON STAR",
      "BLACK HOLE",
      "NO REMNANT",
      "NOT YET FORMED",
    ]) {
      expect(count(label)).toMatch(/S\s*stale$/u);
    }
  });

  it("keeps an empty slot after every count while the answer is live (plan 12, P12.T11.d)", () => {
    renderDiagram({ starFilter: "living" });

    // jsdom lays nothing out, so the room is held by the slot after each count and its rule.
    for (const label of [
      "SYSTEMS",
      "PLOTTED",
      "OFF SCALE",
      "NEUTRON STAR",
      "BLACK HOLE",
      "NO REMNANT",
      "NOT YET FORMED",
    ]) {
      const slot = countSlot(label);
      // The shared slot (plan 05, P05.T14), whose rule the guard below reads.
      expect(slot).toHaveClass("stale-slot");
      expect(slot).toBeEmptyDOMElement();
    }
    expect(stylesheetRule(".stale-slot")).toMatch(
      /display: inline-block;[^}]*min-width: 2ch;[^}]*text-align: right;/u,
    );
  });

  it("sets a stale SYSTEMS count's S after its figure and before the filter's words (plan 05, P05.T14)", () => {
    // The white dwarf alone passes `REMNANTS` of the five.
    renderDiagram({ systems: SYSTEMS.slice(1, 2), total: 5, starFilter: "remnants", stale: true });

    expect(count("SYSTEMS")).toMatch(/^1 OF 5 S\s*stale SHOWN: REMNANTS$/u);
    expect(countSlot("SYSTEMS")).toHaveTextContent(/^S\s*stale$/u);
  });

  it("keeps the SYSTEMS count's slot empty before the filter's words while live, so the words stay put", () => {
    renderDiagram({ systems: SYSTEMS.slice(1, 2), total: 5, starFilter: "remnants" });

    const slot = countSlot("SYSTEMS");
    expect(slot).toBeEmptyDOMElement();
    // The words follow the slot, not the figure, live as stale.
    expect(slot?.previousSibling?.textContent).toBe("1 OF 5");
    expect(slot?.nextSibling?.textContent).toBe(" SHOWN: REMNANTS");
  });

  it("puts each count's stale mark in its slot", () => {
    renderDiagram({ stale: true });

    for (const label of ["PLOTTED", "OFF SCALE", "NEUTRON STAR", "NOT YET FORMED"]) {
      expect(countSlot(label)).toHaveTextContent(/^S\s*stale$/u);
    }
  });

  it("selects the system whose point is clicked", async () => {
    const user = userEvent.setup();
    const { onSelect } = renderDiagram();

    await user.pointer({
      keys: "[MouseLeft]",
      target: canvas(),
      coords: { clientX: hrXPx(5_772, AREA) + 10, clientY: hrYPx(0, AREA) },
    });

    expect(onSelect).toHaveBeenCalledWith(SYSTEMS[0]?.id);
  });

  it("selects nothing when the click is more than a rem from every point", async () => {
    const user = userEvent.setup();
    const { onSelect } = renderDiagram();

    await user.pointer({
      keys: "[MouseLeft]",
      target: canvas(),
      coords: { clientX: AREA.leftPx + 2, clientY: AREA.topPx + 2 },
    });

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("brackets the selection in --accent and repaints only when it changes", () => {
    const { recorder, select } = renderDiagram();
    const before = paints(recorder);

    select(null);
    expect(paints(recorder)).toBe(before);

    recorder.clear();
    select(SYSTEMS[0]?.id ?? null);

    expect(paints(recorder)).toBe(1);
    // The reticle's four corners are one path of four moves, stroked in --accent.
    expect(recorder.calls("moveTo").length).toBeGreaterThanOrEqual(4);
    expect(recorder.sets("strokeStyle")).toContain("#5cc8e6");
  });

  it("draws no text on its canvas, and nothing translucent, shadowed or graded", () => {
    const { recorder } = renderDiagram();

    const used = recorder.names();
    for (const member of BANNED_SPATIAL_MEMBERS) {
      expect(used.has(member)).toBe(false);
    }
  });

  it("is painted in --text-muted and named stale when the answer is stale", () => {
    const { recorder } = renderDiagram({ stale: true });

    expect(canvas()).toHaveAccessibleName("Hertzsprung-Russell diagram, stale");
    expect(recorder.sets("strokeStyle")).not.toContain("#c8d6e5");
    expect(recorder.sets("strokeStyle")).toContain("#8a9db3");
  });
});

/** An observer off the galactic axis: 10,000 ly out, 36.87° from +x, 12.34 ly south. */
const OBSERVED: QueryMode = { kind: "observed", observerLy: [8_000, 6_000, -12.34] };

/** An observer on the galactic axis, where its `ANGLE` is undefined. */
const OBSERVED_ON_AXIS: QueryMode = { kind: "observed", observerLy: [0, 0, 100] };

/** The term `label` of the mode line under the plot. */
function lineTerm(label: string): HTMLElement {
  return screen.getByText(label, { selector: ".hr-diagram__readings dt" });
}

/** The value read beside `label` on the mode line under the plot. */
function lineValue(label: string): string {
  return lineTerm(label).nextElementSibling?.textContent ?? "";
}

/** The mode line's terms, in the order they are read. */
function lineTerms(): Array<string | null> {
  const line = lineTerm("MODE").closest("dl");
  return [...(line?.querySelectorAll("dt") ?? [])].map((term) => term.textContent);
}

describe("HrDiagram's mode line (plan 12, P12.T11.b)", () => {
  it("reads MODE NOW and the chart time in UT in now, naming no frame and no observer", () => {
    renderDiagram({ timeYr: 12.5 });

    expect(lineTerms()).toEqual(["UT", "MODE"]);
    expect(lineValue("UT")).toBe("+12.50 yr");
    expect(lineValue("MODE")).toBe("NOW");
    expect(screen.queryByText("OBSERVED FROM")).not.toBeInTheDocument();
    expect(screen.queryByText("FRAME")).not.toBeInTheDocument();
  });

  it("stands under the plot, after the temperature axis's label and before the counts", () => {
    renderDiagram();

    const line = lineTerm("MODE").closest("dl");
    const axisLabel = screen.getByText("EFFECTIVE TEMPERATURE K, LOG SCALE");
    const firstCount = screen.getByText("PLOTTED", { selector: "dt" });
    expect(line).not.toBeNull();
    expect(line?.closest("figure")).toContainElement(canvas());
    expect(axisLabel.compareDocumentPosition(line ?? axisLabel)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(firstCount.compareDocumentPosition(line ?? firstCount)).toBe(
      Node.DOCUMENT_POSITION_PRECEDING,
    );
  });

  it("reads OBSERVED FROM and the observer at the chart's precisions in observed mode, after FRAME GALACTIC", () => {
    renderDiagram({ mode: OBSERVED, timeYr: -4_197.5 });

    expect(lineTerms()).toEqual(["FRAME", "UT", "MODE", "RADIUS", "ANGLE", "HEIGHT"]);
    expect(lineValue("FRAME")).toBe("GALACTIC");
    expect(lineValue("UT")).toBe("-4197.50 yr");
    expect(lineTerm("MODE").nextElementSibling).toHaveTextContent(/^OBSERVED FROM/u);
    expect(lineValue("RADIUS")).toBe("10,000.0 ly");
    expect(lineValue("ANGLE")).toBe("036.9°");
    expect(lineValue("HEIGHT")).toBe("-12.3 ly");
    // The observer's coordinates are the mode's, nested in its value as the chart's furniture has them.
    expect(lineTerm("MODE").nextElementSibling).toContainElement(lineTerm("HEIGHT"));
  });

  it("reads an observer's ANGLE on the galactic axis as an em dash", () => {
    renderDiagram({ mode: OBSERVED_ON_AXIS });

    expect(lineValue("RADIUS")).toBe("0.0 ly");
    expect(lineValue("ANGLE")).toBe("—");
    expect(lineValue("HEIGHT")).toBe("+100.0 ly");
  });

  it("keeps its accessible name for the plot alone in observed mode", () => {
    renderDiagram({ mode: OBSERVED });

    expect(canvas()).toHaveAccessibleName("Hertzsprung-Russell diagram");
  });

  it("marks the time and each coordinate stale when the answer is, and no word", () => {
    renderDiagram({ mode: OBSERVED, stale: true });

    for (const label of ["UT", "RADIUS", "ANGLE", "HEIGHT"]) {
      expect(lineValue(label)).toMatch(/S\s*stale$/u);
    }
    // Four marks in all, so neither the frame's GALACTIC nor the mode's name carries one.
    const line = lineTerm("MODE").closest("dl");
    expect(line === null ? [] : within(line).getAllByText("stale")).toHaveLength(4);
  });

  it("marks the time stale in now, but not NOW", () => {
    renderDiagram({ stale: true });

    expect(lineValue("UT")).toMatch(/S\s*stale$/u);
    expect(lineValue("MODE")).toBe("NOW");
  });

  it("marks no missing angle stale", () => {
    renderDiagram({ mode: OBSERVED_ON_AXIS, stale: true });

    expect(lineValue("ANGLE")).toBe("—");
    expect(lineValue("HEIGHT")).toMatch(/S\s*stale$/u);
  });

  it("keeps room for each value's stale mark while the answer is live", () => {
    renderDiagram({ mode: OBSERVED });

    // jsdom lays nothing out, so the room is held by the slot after each value and its rule.
    for (const label of ["UT", "RADIUS", "ANGLE", "HEIGHT"]) {
      expect(lineTerm(label).nextElementSibling?.lastElementChild).toHaveClass(
        "spatial-reading__stale-slot",
      );
    }
    expect(stylesheetRule(".spatial-reading__stale-slot")).toMatch(/min-width: 2ch;/u);
  });

  it("mutes the line with the rest of a stale diagram", () => {
    expect(
      stylesheetRule(
        ".hr-diagram--stale :is(.hr-diagram__labels, .hr-diagram__readings, .hr-diagram__counts)",
      ),
    ).toMatch(/color: var\(--text-muted\);/u);
  });
});
