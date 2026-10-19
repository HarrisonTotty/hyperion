import type { CoverDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { CoverageView, fieldCellIndex } from "./coverage";
import { faceUvToDir, stToUv } from "./cube";
import type { Face } from "./patchKey";
import {
  groundHidesPoint,
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

/** The least distance, m, from `from` to the segments, every segment in the view. */
function nearestOn(segments: ReadonlyArray<readonly [Vec3, Vec3]>, from: Vec3): number {
  return Math.min(
    ...segments.map(([a, b]) => {
      const d = sub(b, a);
      const t = Math.min(1, Math.max(0, dot(sub(from, a), d) / dot(d, d)));
      return norm(sub(add(a, scale(d, t)), from));
    }),
  );
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

  it("gives the range to the edge's nearest point in the view", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    // The camera stands 200 km above the cell's centre, so its border lies further off.
    expect(mark.nearestRangeM).toBeCloseTo(nearestOn(segmentsOf(mark.line), vec3(0, 0, 0)), -1);
  });

  it("measures the range from the own ship where there is one", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    // From an own ship 100 km beyond the camera along its look, the range is the ship's.
    const ship = scale(normalise(view.centreM), 100_000);
    const fromShip = surveyEdgeMark(view, camera, VIEWPORT, { ...OPTIONS, rangeFromM: ship });
    expect(fromShip.nearestRangeM).toBeCloseTo(nearestOn(segmentsOf(fromShip.line), ship), -1);
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

/** The unit direction of the point (a, b) in cells of `face` at level 5. */
function cellPoint(face: Face, a: number, b: number): Vec3 {
  return faceUvToDir(face, stToUv(a / 32), stToUv(b / 32));
}

/**
 * The camera 100 m over the edge of cell (16, 16) of face 0 that it shares with cell (16, 15), 0.6
 * of the way through the 33rd of the 64 pieces the side is first cut into, looking along it 15°
 * down and 10° to its side: the piece beneath it reaches behind the camera, and the side runs on
 * past the horizon some 14 km ahead.
 */
function lowOver(): {
  view: SurveyEdgeView;
  camera: ProjectionCamera;
  sideAt: (t: number) => Vec3;
} {
  const along = (t: number): Vec3 => cellPoint(0, 16 + t, 16);
  const footT = 32.6 / 64;
  const up = along(footT);
  const ahead = normalise(sub(along(footT + 1e-6), along(footT - 1e-6)));
  const aside = cross(up, ahead);
  const yaw = (10 * Math.PI) / 180;
  const pitch = (15 * Math.PI) / 180;
  const forward = add(
    scale(add(scale(ahead, Math.cos(yaw)), scale(aside, Math.sin(yaw))), Math.cos(pitch)),
    scale(up, -Math.sin(pitch)),
  );
  const cameraM = scale(up, RADIUS_M + 100);
  return {
    view: {
      body: "test",
      coverage: surveyed([0, 16, 16]),
      figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
      rotation: IDENTITY_ROTATION,
      centreM: scale(cameraM, -1),
      heightAtM: () => null,
    },
    camera: { orientation: lookAlong(forward, up), fovXRad: Math.PI / 3 },
    sideAt: (t) => sub(scale(along(t), RADIUS_M), cameraM),
  };
}

describe("the survey edge's mark beneath a camera low over it (R10.T10.c)", () => {
  it("runs the line on to the view's bottom edge", () => {
    const { view, camera } = lowOver();
    const line = segmentsOf(surveyEdgeMark(view, camera, VIEWPORT, OPTIONS).line);
    // A drawn segment crosses the bottom edge between the view's sides.
    const crossings = line.filter(([a, b]) => {
      const p = project(a, camera, VIEWPORT);
      const q = project(b, camera, VIEWPORT);
      const h = VIEWPORT.heightPx;
      if ((p.yPx - h) * (q.yPx - h) > 0 || p.yPx === q.yPx) {
        return false;
      }
      const x = p.xPx + ((h - p.yPx) / (q.yPx - p.yPx)) * (q.xPx - p.xPx);
      return x >= 0 && x <= VIEWPORT.widthPx;
    });
    expect(crossings.length).toBeGreaterThan(0);
  });

  it("ends the line at the horizon", () => {
    const { view, camera } = lowOver();
    const line = segmentsOf(surveyEdgeMark(view, camera, VIEWPORT, OPTIONS).line);
    const farthestM = Math.max(...line.flat().map((p) => norm(p)));
    // On a sphere the horizon lies √((R + h)² − R²) from the camera.
    expect(farthestM / Math.sqrt((RADIUS_M + 100) ** 2 - RADIUS_M ** 2)).toBeCloseTo(1, 6);
  });

  it("gives the range to the line where it leaves the view, to three figures", () => {
    const { view, camera, sideAt } = lowOver();
    const mark = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    // The side sampled every centimetre within 500 m of the camera's foot.
    let sampledM = Number.POSITIVE_INFINITY;
    for (let k = -50_000; k <= 50_000; k += 1) {
      const p = sideAt(32.6 / 64 + k / 5_000_000);
      const at = project(p, camera, VIEWPORT);
      const inView =
        at.inFront &&
        at.xPx >= 0 &&
        at.xPx <= VIEWPORT.widthPx &&
        at.yPx >= 0 &&
        at.yPx <= VIEWPORT.heightPx;
      if (inView) {
        sampledM = Math.min(sampledM, norm(p));
      }
    }
    expect(Math.abs((mark.nearestRangeM ?? 0) / sampledM - 1)).toBeLessThan(5e-4);
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

describe("the drawn ground between the camera and an edge's point (R10.T16.c)", () => {
  const FIGURE = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M };
  // The camera 1 km over the pole, a point on the datum 20 km away along the surface.
  const cameraM = vec3(0, 0, RADIUS_M + 1_000);
  const pointM = scale(normalise(vec3(20_000 / RADIUS_M, 0, 1)), RADIUS_M);
  // A ridge 2 km high across the line of sight, between 8 and 12 km from the pole.
  const ridgeAt =
    (fromKm: number, toKm: number): ((d: Vec3) => number) =>
    (d) => {
      const alongKm = (Math.atan2(Math.hypot(d.x, d.y), d.z) * RADIUS_M) / 1000;
      return alongKm >= fromKm && alongKm <= toKm ? 2_000 : 0;
    };

  it("hides a point behind a ridge across the line of sight", () => {
    expect(
      groundHidesPoint(cameraM, pointM, FIGURE, { heightAtM: ridgeAt(8, 12), ceilingM: 2_000 }),
    ).toBe(true);
  });

  it("hides no point where the ground stands clear of the line of sight, before or behind it", () => {
    expect([
      groundHidesPoint(cameraM, pointM, FIGURE, { heightAtM: () => 0, ceilingM: 2_000 }),
      groundHidesPoint(cameraM, pointM, FIGURE, { heightAtM: ridgeAt(25, 30), ceilingM: 2_000 }),
      groundHidesPoint(cameraM, pointM, FIGURE, { heightAtM: () => null, ceilingM: 0 }),
    ]).toEqual([false, false, false]);
  });
});

describe("the survey edge's note counts only the points the operator can see (R10.T16.c)", () => {
  it("gives the same range where the drawn ground hides nothing", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const bare = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    const clear = surveyEdgeMark(
      { ...view, ground: { heightAtM: () => 0, ceilingM: 1_000 } },
      camera,
      VIEWPORT,
      OPTIONS,
    );
    expect(clear.nearestRangeM).toBe(bare.nearestRangeM);
  });

  it("gives no range where the drawn ground hides every point, though the line is drawn", () => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const hidden = surveyEdgeMark(
      { ...view, ground: { heightAtM: () => 150_000, ceilingM: 150_000 } },
      camera,
      VIEWPORT,
      OPTIONS,
    );
    expect([hidden.line.length > 0, hidden.nearestRangeM]).toEqual([true, null]);
  });

  it("gives the range of the nearest point seen past ground that hides a nearer one", () => {
    // The camera 200 km over a point of the cell nearer its west side (s = 16 ÷ 32), whose
    // middle is then the line's one nearest point.
    const up = faceUvToDir(0, stToUv(16.2 / 32), stToUv(16.5 / 32));
    const cameraM = scale(up, RADIUS_M + 200_000);
    const view: SurveyEdgeView = {
      body: "test",
      coverage: surveyed([0, 16, 16]),
      figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
      rotation: IDENTITY_ROTATION,
      centreM: scale(cameraM, -1),
      heightAtM: () => null,
    };
    const camera = { orientation: lookAlong(scale(up, -1), vec3(0, 0, 1)), fovXRad: Math.PI / 3 };
    const bare = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    // A tower halfway between the camera's foot and that point, which hides it and its
    // neighbours alone.
    const tower = normalise(add(up, nearestPointDir(view, camera)));
    const ground = {
      heightAtM: (d: Vec3): number =>
        Math.acos(Math.min(1, dot(normalise(d), tower))) * RADIUS_M < 3_000 ? 180_000 : 0,
      ceilingM: 180_000,
    };
    const seen = surveyEdgeMark({ ...view, ground }, camera, VIEWPORT, OPTIONS);
    expect(seen.nearestRangeM).not.toBeNull();
    expect(seen.nearestRangeM ?? 0).toBeGreaterThan((bare.nearestRangeM ?? 0) + 10);
  });
});

/** The body-fixed direction of the drawn line's point nearest the camera. */
function nearestPointDir(view: SurveyEdgeView, camera: ProjectionCamera): Vec3 {
  const { line } = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
  let best = vec3(0, 0, 1);
  let least = Number.POSITIVE_INFINITY;
  for (const [a] of segmentsOf(line)) {
    if (norm(a) < least) {
      least = norm(a);
      best = normalise(sub(a, view.centreM));
    }
  }
  return best;
}
