import type { CoverDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { norm, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { CoverageView, fieldCellIndex } from "./coverage";
import { faceUvToDir, stToUv } from "./cube";
import type { Face } from "./patchKey";
import {
  SURVEY_EDGE_NOTE,
  SURVEY_EDGE_TICK_REM,
  SURVEY_EDGE_TICK_SPACING_REM,
  surveyEdgeMark,
  surveyEdgeNote,
  type SurveyEdgeOptions,
  type SurveyEdgeView,
} from "./surveyEdge";

const RADIUS_M = 1e6;
const VIEWPORT: Viewport = { widthPx: 800, heightPx: 600 };
const REM_PX = 16;
const OPTIONS: SurveyEdgeOptions = { remPx: REM_PX, keepClear: [], rangeFromM: null };

/** A level-5 coverage of the cells `(face, i, j)` given, each surveyed at code 8. */
function surveyed(...cells: ReadonlyArray<readonly [Face, number, number]>): CoverageView {
  const cover: CoverDto = {
    ranges: cells.map(([face, i, j]) => {
      const start = fieldCellIndex({ face, level: 5, i, j });
      return { start, end: start + 1, resolution: 8 };
    }),
  };
  const merged = CoverageView.empty(5).merged(cover, 1);
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** The unit direction of the centre of cell (i, j) of `face` at level 5. */
function cellCentre(face: Face, i: number, j: number): Vec3 {
  return faceUvToDir(face, stToUv((i + 0.5) / 32), stToUv((j + 0.5) / 32));
}

/**
 * A sphere of 1,000 km seen from `heightM` above the centre of cell (16, 16) of face 0, looking at
 * the body's centre, with `coverage` and the ground at `groundM` above the datum.
 */
function viewOf(
  coverage: CoverageView,
  heightM: number,
  groundM: number | null = null,
): { view: SurveyEdgeView; camera: ProjectionCamera; cameraM: Vec3 } {
  const up = cellCentre(0, 16, 16);
  const cameraM = scale(up, RADIUS_M + heightM);
  return {
    view: {
      body: "test",
      coverage,
      figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
      rotation: IDENTITY_ROTATION,
      centreM: scale(cameraM, -1),
      heightAtM: () => groundM,
    },
    camera: { orientation: lookAlong(scale(up, -1), vec3(0, 0, 1)), fovXRad: Math.PI / 3 },
    cameraM,
  };
}

/** A packed segment array's segments, each end m from the camera. */
function segmentsOf(packed: Float32Array): Array<readonly [Vec3, Vec3]> {
  const out: Array<readonly [Vec3, Vec3]> = [];
  for (let n = 0; n + 5 < packed.length; n += 6) {
    out.push([
      vec3(packed[n] ?? 0, packed[n + 1] ?? 0, packed[n + 2] ?? 0),
      vec3(packed[n + 3] ?? 0, packed[n + 4] ?? 0, packed[n + 5] ?? 0),
    ]);
  }
  return out;
}

describe("the survey edge's mark (R10.T10.c)", () => {
  it("draws a surveyed cell's border as a closed line on the ground, with ticks every rem", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    const line = segmentsOf(mark.line);
    expect(line.length).toBeGreaterThan(4);
    // Every point on the datum, 1,000 km from the body's centre, and the line closed.
    for (const [a, b] of line) {
      for (const p of [a, b]) {
        expect(
          norm(vec3(p.x - view.centreM.x, p.y - view.centreM.y, p.z - view.centreM.z)),
        ).toBeCloseTo(RADIUS_M, -1);
      }
    }
    const ends = new Map<string, number>();
    for (const [a, b] of line) {
      for (const p of [a, b]) {
        const key = [p.x, p.y, p.z].map((c) => c.toFixed(0)).join(" ");
        ends.set(key, (ends.get(key) ?? 0) + 1);
      }
    }
    expect([...ends.values()].every((n) => n === 2)).toBe(true);
    // The ticks: about one each rem of the drawn perimeter, each 0.25 rem long on the view.
    const perimeterPx = line.reduce((sum, [a, b]) => {
      const pa = project(a, camera, VIEWPORT);
      const pb = project(b, camera, VIEWPORT);
      return sum + Math.hypot(pb.xPx - pa.xPx, pb.yPx - pa.yPx);
    }, 0);
    const ticks = segmentsOf(mark.ticks);
    const spacingPx = SURVEY_EDGE_TICK_SPACING_REM * REM_PX;
    expect(ticks.length).toBeGreaterThanOrEqual(Math.floor(perimeterPx / spacingPx) - 4);
    expect(ticks.length).toBeLessThanOrEqual(Math.ceil(perimeterPx / spacingPx));
    for (const [foot, end] of ticks) {
      const a = project(foot, camera, VIEWPORT);
      const b = project(end, camera, VIEWPORT);
      expect(Math.hypot(b.xPx - a.xPx, b.yPx - a.yPx)).toBeCloseTo(
        SURVEY_EDGE_TICK_REM * REM_PX,
        3,
      );
    }
  });

  it("points every tick into the unsurveyed side", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    // The surveyed cell lies about the view's centre, so each tick runs away from it.
    const centre = { xPx: VIEWPORT.widthPx / 2, yPx: VIEWPORT.heightPx / 2 };
    const ticks = segmentsOf(mark.ticks);
    expect(ticks.length).toBeGreaterThan(0);
    for (const [foot, end] of ticks) {
      const a = project(foot, camera, VIEWPORT);
      const b = project(end, camera, VIEWPORT);
      expect(Math.hypot(b.xPx - centre.xPx, b.yPx - centre.yPx)).toBeGreaterThan(
        Math.hypot(a.xPx - centre.xPx, a.yPx - centre.yPx),
      );
    }
    // Two cells side by side: the side between them is no edge, and the ticks point out of both.
    const pair = surveyEdgeMark(
      viewOf(surveyed([0, 16, 16], [0, 17, 16]), 200_000).view,
      camera,
      VIEWPORT,
      OPTIONS,
    );
    expect(segmentsOf(pair.ticks).length).toBeGreaterThan(ticks.length);
  });

  it("stands on the higher of the drawn ground and the datum", () => {
    const above = viewOf(surveyed([0, 16, 16]), 200_000, 5_000);
    const below = viewOf(surveyed([0, 16, 16]), 200_000, -5_000);
    for (const [{ view, camera }, expectedM] of [
      [above, RADIUS_M + 5_000],
      [below, RADIUS_M],
    ] as const) {
      const [first] = segmentsOf(surveyEdgeMark(view, camera, VIEWPORT, OPTIONS).line);
      const p = first?.[0] ?? vec3(0, 0, 0);
      expect(
        norm(vec3(p.x - view.centreM.x, p.y - view.centreM.y, p.z - view.centreM.z)),
      ).toBeCloseTo(expectedM, -1);
    }
  });

  it("draws nothing of a survey beyond the horizon, and gives it no range", () => {
    const { view, camera } = viewOf(surveyed([3, 16, 16]), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    expect([mark.line.length, mark.ticks.length, mark.nearestRangeM]).toEqual([0, 0, null]);
  });

  it("gives the range to the edge's nearest point in the view, from the own ship where there is one", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    // The camera stands 200 km above the cell's centre, so its border lies further off.
    expect(mark.nearestRangeM).toBeGreaterThan(200_000);
    const points = segmentsOf(mark.line).flat();
    const nearest = Math.min(...points.map((p) => norm(p)));
    expect(mark.nearestRangeM).toBeCloseTo(nearest, -1);
    // From an own ship 100 km beyond the camera along its look, the range is the ship's.
    const ship = scale(normalise(view.centreM), 100_000);
    const fromShip = surveyEdgeMark(view, camera, VIEWPORT, { ...OPTIONS, rangeFromM: ship });
    const nearestFromShip = Math.min(
      ...points.map((p) => norm(vec3(p.x - ship.x, p.y - ship.y, p.z - ship.z))),
    );
    expect(fromShip.nearestRangeM).toBeCloseTo(nearestFromShip, -1);
  });

  it("leaves out the ticks within 0.5 rem of the line-of-sight mark's", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const all = segmentsOf(surveyEdgeMark(view, camera, VIEWPORT, OPTIONS).ticks);
    const first = all[0];
    if (first === undefined) {
      throw new Error("no tick was drawn");
    }
    const foot = project(first[0], camera, VIEWPORT);
    const mark = { xPx: foot.xPx, yPx: foot.yPx };
    const kept = segmentsOf(
      surveyEdgeMark(view, camera, VIEWPORT, { ...OPTIONS, keepClear: [[mark, mark]] }).ticks,
    );
    expect(kept.length).toBeLessThan(all.length);
    for (const [footM] of kept) {
      const p = project(footM, camera, VIEWPORT);
      expect(Math.hypot(p.xPx - mark.xPx, p.yPx - mark.yPx)).toBeGreaterThanOrEqual(0.5 * REM_PX);
    }
  });

  it("draws no edge with no survey", () => {
    const { view, camera } = viewOf(CoverageView.empty(5), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    expect([mark.line.length, mark.ticks.length, mark.nearestRangeM]).toEqual([0, 0, null]);
  });
});

describe("the survey edge's note", () => {
  it("states the nearest range to three figures, from the camera where there is no own ship", () => {
    expect(SURVEY_EDGE_NOTE).toBe("SURVEY EDGE");
    expect(surveyEdgeNote(4_210, false, null)).toEqual({ text: "SURVEY EDGE 4.21 km", unit: "km" });
    expect(surveyEdgeNote(4_210, true, null).text).toBe("SURVEY EDGE 4.21 km FROM CAMERA");
    expect(surveyEdgeNote(3.84e8, false, null)).toEqual({ text: "SURVEY EDGE 384 Mm", unit: "Mm" });
  });
});
