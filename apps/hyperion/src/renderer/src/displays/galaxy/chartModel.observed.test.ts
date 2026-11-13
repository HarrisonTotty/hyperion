import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { type ChartResult, NOW_MODE, type QueryMode } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import type { Camera, Viewport } from "../../spatial/camera";
import { buildDrawList, type LineOp } from "../../spatial/drawList";
import {
  anObservation,
  aSystemRecord,
  aSystemsInRange,
  type RelativeSystemSpec,
} from "../../test/galaxyFixtures";
import { answeredMode, chartDataFault, OBSERVER_LABEL, toScene } from "./chartModel";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(CENTRE[0], CENTRE[1], CENTRE[2]));
const OBSERVER_LY = [26_010, 5, -2] as const;
const OBSERVED: QueryMode = { kind: "observed", observerLy: OBSERVER_LY };

/** A chart asked in observed mode, its systems answered as given. */
function observedChart(systems: ReadonlyArray<RelativeSystemSpec>): ChartResult {
  return toChartResult(aSystemsInRange({ centreLy: CENTRE, systems }), OBSERVED);
}

/** A system of layer A `relLy` from the centre, seen `shiftLy` along +y from where it is. */
function seen(relLy: readonly [number, number, number], shiftLy: number): RelativeSystemSpec {
  return {
    relLy,
    layer: "a",
    observed: { lightAgeYr: 100, apparentRelLy: [relLy[0], relLy[1] + shiftLy, relLy[2]] },
  };
}

function sceneOf(result: ChartResult, selectedId: string | null = null) {
  return toScene(result, { frame: FRAME, driveRangeLy: 50, selectedId, destinationId: null });
}

describe("toScene's apparent positions (plan 12, P12.T11)", () => {
  it("draws no segment and no observer for a chart in now", () => {
    const scene = sceneOf(
      toChartResult(aSystemsInRange({ centreLy: CENTRE, systems: [seen([1, 0, 0], 1)] })),
    );

    expect(scene).not.toHaveProperty("segments");
    expect(scene).not.toHaveProperty("observer");
  });

  it("joins each observed system's mark to its apparent position with a dashed, ticked line", () => {
    const scene = sceneOf(observedChart([seen([1, 2, 3], 0.5)]));

    expect(scene.segments).toEqual([
      {
        id: "apparent:0000000000000001",
        from: expect.objectContaining({}),
        to: expect.objectContaining({}),
        line: "dashed",
        stroke: "textMuted",
        widthPx: 1,
        fromMark: "0000000000000001",
        endTick: true,
        minLengthPx: 4,
      },
    ]);
    const [segment] = scene.segments ?? [];
    expect(segment?.from.y).toBeCloseTo(2, 9);
    expect(segment?.to.y).toBeCloseTo(2.5, 9);
  });

  it("draws the selected system's in --text 2 px wide whatever its offset, and last", () => {
    const result = observedChart([seen([1, 0, 0], 0.5), seen([2, 0, 0], 0.5)]);
    const scene = sceneOf(result, "0000000000000001");

    expect(
      scene.segments?.map((each) => [each.fromMark, each.stroke, each.widthPx, each.minLengthPx]),
    ).toEqual([
      ["0000000000000002", "textMuted", 1, 4],
      ["0000000000000001", "text", 2, 0],
    ]);
  });

  it("draws none for a row without an observation, or for a system not drawn", () => {
    const scene = sceneOf(
      observedChart([
        { relLy: [1, 0, 0], layer: "a" },
        { ...seen([2, 0, 0], 0.5), stellar: null },
      ]),
    );

    expect(scene.segments ?? []).toEqual([]);
  });

  it("places the observer from the chart centre, labelled OBSERVER", () => {
    const scene = sceneOf(observedChart([]));

    expect(scene.observer).toEqual({ position: vec3(10, 5, -2), label: OBSERVER_LABEL });
    expect(OBSERVER_LABEL).toBe("OBSERVER");
  });
});

describe("answeredMode (plan 12, P12.T11)", () => {
  it("is the mode asked for an answer whose rows carry their observation", () => {
    expect(answeredMode(observedChart([seen([1, 0, 0], 0.5)]))).toEqual(OBSERVED);
  });

  it("is now for an answer asked observed whose rows all came without, and draws no observer", () => {
    const result = observedChart([{ relLy: [1, 0, 0], layer: "a" }]);

    expect(answeredMode(result)).toEqual(NOW_MODE);
    expect(sceneOf(result)).not.toHaveProperty("observer");
  });

  it("keeps the mode asked for an answer with no rows", () => {
    expect(answeredMode(observedChart([]))).toEqual(OBSERVED);
  });
});

describe("toScene's free-floating planets as observed (plan 12, P12.T11)", () => {
  /** A planet formed a thousand years ago, its light 5,000 years old. */
  const young: RelativeSystemSpec = {
    relLy: [1, 0, 0],
    layer: "rogue_planet",
    ageMyr: 0.001,
    observed: { lightAgeYr: 5_000, apparentRelLy: [1, 0, 0] },
  };

  /** The same planet as a `now` answer has it, with no observation. */
  const present: RelativeSystemSpec = { relLy: [1, 0, 0], layer: "rogue_planet", ageMyr: 0.001 };

  function chartOf(system: RelativeSystemSpec, mode: QueryMode | undefined): ChartResult {
    return toChartResult(
      aSystemsInRange({ centreLy: CENTRE, minLayer: "rogue_planet", systems: [system] }),
      mode,
    );
  }

  it("does not draw a planet that had not formed when its light left", () => {
    expect(sceneOf(chartOf(young, OBSERVED)).points).toEqual([]);
  });

  it("draws the same planet in now, where it has formed", () => {
    expect(sceneOf(chartOf(present, undefined)).points.map((point) => point.shape)).toEqual([
      "triangle-down",
    ]);
  });
});

describe("the apparent positions drawn (plan 12, P12.T11)", () => {
  // Straight down at 10 px a light-year, so that +y, spinward or not, is along the screen's x; a
  // root font of 4 px puts a layer-A mark's outline 1 px out, inside the 4 px threshold.
  const top: Camera = { azimuthDeg: 0, elevationDeg: 90, pxPerUnit: 10 };
  const viewport: Viewport = { widthPx: 800, heightPx: 600, remPx: 4 };

  function joinersOf(result: ChartResult, selectedId: string | null): string[] {
    const scene = sceneOf(result, selectedId);
    const list = buildDrawList(scene, top, viewport, 2);
    // A joiner is dashed; the only dashed lines are the joiners.
    const dashed = list.ops.filter(
      (op): op is LineOp => op.kind === "line" && op.dashPx !== undefined,
    );
    return dashed.map((op) => (op.widthPx === 2 ? "selected" : "other"));
  }

  it("draws a system's that is not selected only past 4 px", () => {
    expect(joinersOf(observedChart([seen([5, 0, 0], 0.39)]), null)).toEqual([]);
    expect(joinersOf(observedChart([seen([5, 0, 0], 0.41)]), null)).toEqual(["other"]);
  });

  it("draws the selected system's below 4 px, outside its mark's outline", () => {
    const result = observedChart([seen([5, 0, 0], 0.2), seen([6, 0, 0], 0.2)]);

    expect(joinersOf(result, "0000000000000001")).toEqual(["selected"]);
  });

  it("draws not even the selected system's inside its mark's outline", () => {
    expect(joinersOf(observedChart([seen([5, 0, 0], 0.05)]), "0000000000000001")).toEqual([]);
  });
});

describe("chartDataFault's observed rows (plan 12, P12.T11)", () => {
  /** A chart of one observed system whose observation is changed by `change`. */
  function withObservation(change: Partial<ReturnType<typeof anObservation>>): ChartResult {
    const record = aSystemRecord({
      positionLy: [26_001, 0, 0],
      observed: { ...anObservation({ apparentLy: [26_001, 0, 0], lightAgeYr: 1 }), ...change },
    });
    return toChartResult({ ...aSystemsInRange({ centreLy: CENTRE }), systems: [record] }, OBSERVED);
  }

  it("finds nothing wrong with an observed row the server built properly", () => {
    expect(chartDataFault(withObservation({}))).toBeNull();
    expect(
      chartDataFault(withObservation({ curvature_error_ly: 0, curvature_error_arcsec: 0 })),
    ).toBeNull();
  });

  it.each([
    ["a negative light age", { light_age_yr: -1 }],
    ["a light age that is not finite", { light_age_yr: Number.POSITIVE_INFINITY }],
    ["a negative bound", { curvature_error_ly: -1e-6 }],
    ["a bound that is not finite", { curvature_error_ly: Number.NaN }],
    ["an angle that is not finite", { curvature_error_arcsec: Number.POSITIVE_INFINITY }],
    ["a negative angle", { curvature_error_arcsec: -0.1 }],
  ] as const)("refuses an observed row with %s", (_what, change) => {
    expect(chartDataFault(withObservation(change))).toBe("observation unusable");
  });
});
