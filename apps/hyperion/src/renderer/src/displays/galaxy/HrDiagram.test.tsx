import type { StellarBriefDto, SystemIdHex } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { hrXPx, hrYPx } from "../../lib/galaxy/hrProjection";
import type { ChartSystem } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { aStellarBrief, aSystemsInRange } from "../../test/galaxyFixtures";
import {
  BANNED_SPATIAL_MEMBERS,
  type RecordingContext2D,
  stubCanvas,
} from "../../test/RecordingContext2D";
import { type StarFilter } from "./chartModel";
import { HrDiagram } from "./HrDiagram";

/** The stage, and every other element, laid out 400 × 300 at 16 px to the rem. */
const WIDTH_PX = 400;
const HEIGHT_PX = 300;

/** The plot inside that stage: 3.5 rem in from the left, 1.5 rem from the other edges. */
const AREA = { leftPx: 56, topPx: 24, widthPx: 320, heightPx: 252 };

/** The Sun, a white dwarf, a neutron star and a star that left no remnant. */
const SYSTEMS: ReadonlyArray<ChartSystem> = toChartResult(
  aSystemsInRange({
    systems: [
      { relLy: [1, 0, 0], layer: "c", stellar: aStellarBrief("c", "dwarf") },
      { relLy: [2, 0, 0], layer: "c", stellar: aStellarBrief("c", "white_dwarf") },
      { relLy: [3, 0, 0], layer: "e", stellar: aStellarBrief("e", "neutron_star") },
      { relLy: [4, 0, 0], layer: "e", stellar: aStellarBrief("e", "no_remnant") },
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
}

function renderDiagram({
  systems = SYSTEMS,
  total = systems.length,
  starFilter = "all",
  selectedId = null,
  stale = false,
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

    expect(screen.getByText("HERTZSPRUNG-RUSSELL DIAGRAM")).toBeInTheDocument();
    expect(screen.getByText(/^LUMINOSITY/u)).toContainElement(
      screen.getByRole("img", { name: "solar luminosities" }),
    );
    expect(screen.getByText("EFFECTIVE TEMPERATURE K, LOG SCALE")).toBeInTheDocument();
    expect(screen.getByText(/^LUMINOSITY/u)).toHaveTextContent(/, LOG SCALE$/u);
  });

  it("writes the major ticks' values and the spectral letters", () => {
    renderDiagram();

    expect(values(".hr-diagram__label--x")).toEqual(["1E5", "30,000", "1E4", "3000", "1E3"]);
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
  });

  it("counts what it plots and what it cannot", () => {
    renderDiagram();

    expect(count("PLOTTED")).toBe("2");
    expect(count("OFF SCALE")).toBe("0");
    expect(count("NO PHOTOSPHERE")).toBe("1");
    expect(count("NO REMNANT")).toBe("1");
    expect(count("NOT YET FORMED")).toBe("0");
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
      "NO PHOTOSPHERE",
      "NO REMNANT",
      "NOT YET FORMED",
    ]) {
      expect(count(label)).toMatch(/S\s*stale$/u);
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
