import type { CoverDto } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M, type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { CoverageView, fieldCellIndex } from "./coverage";
import { DrawnGround, type DrawnInstance } from "./drawnGround";
import { DATUM_SLOT, fadeWord, MIX_STEPS, PARENT_MIX_SLOT } from "./gpu/uniforms";
import { type PatchKey, patchKeyString } from "./patchKey";
import type { ResidentBake } from "./residentHeights";
import { fnv1a64 } from "./workers/f32Digest";
import { faceUvToDir, stToUv } from "./cube";
import type { Face } from "./patchKey";
import {
  groundHidesPoint,
  groundHidesTick,
  SURVEY_EDGE_NOTE,
  SURVEY_EDGE_TICK_REM,
  SURVEY_EDGE_TICK_SPACING_REM,
  TICK_SIGHT_SAMPLES,
  surveyEdgeMark,
  surveyEdgeNote,
  surveyEdgeNoteName,
  type SurveyEdgeMark,
  type SurveyEdgeOptions,
  type SurveyEdgeRimStamps,
  type SurveyEdgeView,
} from "./surveyEdge";

const RADIUS_M = 1e6;
const VIEWPORT: Viewport = { widthPx: 800, heightPx: 600 };
const REM_PX = 16;
const OPTIONS: SurveyEdgeOptions = {
  remPx: REM_PX,
  keepClear: [],
  rangeFromM: null,
  lineWidthPx: 2,
};

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

  it("makes a coverage's edge once, and draws every later frame of it the same from it", () => {
    const coverage = surveyed([0, 16, 16], [0, 17, 16], [0, 3, 30]);
    const read = vi.spyOn(coverage, "maskTexels");
    const near = viewOf(coverage, 200_000);
    const first = surveyEdgeMark(near.view, near.camera, VIEWPORT, OPTIONS);
    expect(read).toHaveBeenCalled();
    read.mockClear();
    // Later frames, from the same height and from farther off, read the coverage's cells no more.
    expect(surveyEdgeMark(near.view, near.camera, VIEWPORT, OPTIONS)).toEqual(first);
    const far = viewOf(coverage, 900_000);
    const farMark = surveyEdgeMark(far.view, far.camera, VIEWPORT, OPTIONS);
    expect(read).not.toHaveBeenCalled();
    // And each is the frame a coverage of the same cells, never drawn before, gives.
    const fresh = surveyed([0, 16, 16], [0, 17, 16], [0, 3, 30]);
    const again = viewOf(fresh, 900_000);
    expect(surveyEdgeMark(again.view, again.camera, VIEWPORT, OPTIONS)).toEqual(farMark);
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

describe("the survey edge's note (decision-r10-t16-review §1)", () => {
  it("is named as the guide names the mark", () => {
    expect(SURVEY_EDGE_NOTE).toBe("SURVEY EDGE");
  });

  it("gives the body's designation and its range from the own ship", () => {
    expect(surveyEdgeNote("TEST MOON", 4_210, false, null)).toEqual({
      text: "SURVEY EDGE TEST MOON 4.21 km",
      range: "4.21 km",
      reading: "4.21 km",
      unit: "km",
    });
  });

  it("labels its range FROM CAMERA after the unit where there is no own ship", () => {
    expect(surveyEdgeNote("TEST MOON", 412, true, null).text).toBe(
      "SURVEY EDGE TEST MOON 412 m FROM CAMERA",
    );
  });

  it("reads a range below 1 km in m", () => {
    expect(surveyEdgeNote("TEST MOON", 412, false, null)).toEqual({
      text: "SURVEY EDGE TEST MOON 412 m",
      range: "412 m",
      reading: "412 m",
      unit: "m",
    });
  });

  it("keeps the unit it was last shown in within its hysteresis", () => {
    expect(surveyEdgeNote("TEST MOON", 1_020, false, "m").reading).toBe("1020 m");
  });

  it.each([
    [false, false, "TEST MOON, survey edge, range 4.21 km"],
    [true, false, "TEST MOON, survey edge, range 4.21 km FROM CAMERA"],
    [false, true, "TEST MOON, survey edge, range 4.21 km, stale"],
  ])("is named in the options' form (from camera %s, stale %s)", (fromCamera, stale, name) => {
    const note = surveyEdgeNote("TEST MOON", 4_210, fromCamera, null);
    expect(surveyEdgeNoteName("TEST MOON", note, stale)).toBe(name);
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

describe("the drawn ground between the camera and a tick's foot (R10.T16.c)", () => {
  const FIGURE = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M };
  // The camera 1 km over the pole, a foot on the datum 20 km away along the surface, a pixel 1 mrad.
  const cameraM = vec3(0, 0, RADIUS_M + 1_000);
  const footM = scale(normalise(vec3(20_000 / RADIUS_M, 0, 1)), RADIUS_M);
  const PIXEL_RAD = 1e-3;
  /** Half the 2 px line's width, the tolerance the mark gives the march. */
  const TOLERANCE_PX = 1;
  /** The along-surface distance from the pole of the point `q`, m. */
  const alongOf = (q: Vec3): number => Math.atan2(Math.hypot(q.x, q.y), q.z) * RADIUS_M;
  /** The line of sight's height over the datum `alongM` from the pole, and a pixel there, m. */
  const sightAt = (alongM: number): { readonly heightM: number; readonly pixelM: number } => {
    const back = sub(cameraM, footM);
    let low = 0;
    let high = 1;
    for (let step = 0; step < 60; step += 1) {
      const mid = (low + high) / 2;
      if (alongOf(add(footM, scale(back, mid))) > alongM) {
        low = mid;
      } else {
        high = mid;
      }
    }
    const q = add(footM, scale(back, low));
    return { heightM: norm(q) - RADIUS_M, pixelM: norm(sub(cameraM, q)) * PIXEL_RAD };
  };
  /** Ground `px` pixels above the line of sight between 9 and 11 km from the pole, datum elsewhere. */
  const ridge = (px: number): { heightAtM: (d: Vec3) => number; ceilingM: number } => ({
    heightAtM: (d) => {
      const alongM = alongOf(d);
      if (alongM < 9_000 || alongM > 11_000) {
        return 0;
      }
      const { heightM, pixelM } = sightAt(alongM);
      return heightM + px * pixelM;
    },
    ceilingM: 1_000,
  });

  it("hides a foot behind a ridge standing more than a pixel above its line of sight", () => {
    expect(groundHidesTick(cameraM, footM, FIGURE, ridge(2), PIXEL_RAD, TOLERANCE_PX)).toBe(true);
  });

  it("hides no foot behind a ridge within a pixel of its line of sight", () => {
    expect(groundHidesTick(cameraM, footM, FIGURE, ridge(0.5), PIXEL_RAD, TOLERANCE_PX)).toBe(
      false,
    );
  });

  it("hides no foot where no ground is drawn", () => {
    expect(
      groundHidesTick(
        cameraM,
        footM,
        FIGURE,
        { heightAtM: () => null, ceilingM: 0 },
        PIXEL_RAD,
        TOLERANCE_PX,
      ),
    ).toBe(false);
  });

  it.each([
    { name: "a sphere", a: RADIUS_M, c: RADIUS_M, latDeg: 90 },
    { name: "an Earth's spheroid, at 45°", a: 6_378_137, c: 6_356_752.3, latDeg: 45 },
  ])(
    "hides no foot over flat ground on $name with the camera inside the ground's highest shell",
    ({ a, c, latDeg }) => {
      // The camera 1 km over the datum, the shell 2 km above the equatorial radius; feet on the
      // datum 2 to 30 km away along the meridian, each its line of sight marched to the camera.
      const figure = { equatorialRadiusM: a, polarRadiusM: c };
      const pointAt = (lat: number, upM: number): Vec3 => {
        const d = normalise(vec3(Math.cos(lat), 0, Math.sin(lat)));
        const datum = vec3(a * d.x, a * d.y, c * d.z);
        return add(datum, scale(normalise(vec3(d.x / a, d.y / a, d.z / c)), upM));
      };
      const lat = (latDeg * Math.PI) / 180;
      const camera = pointAt(lat, 1_000);
      const hidden = Array.from({ length: 2_000 }, (_, k) =>
        groundHidesTick(
          camera,
          pointAt(lat - (2_000 + 14 * k) / a, 0),
          figure,
          { heightAtM: () => 0, ceilingM: 2_000 },
          PIXEL_RAD,
          TOLERANCE_PX,
        ),
      ).filter(Boolean);
      expect(hidden).toEqual([]);
    },
  );

  it("marches open ground seen steeply in under a third of its most samples", () => {
    // The camera 5 km over the pole, the foot 10 km away along the surface, over the datum.
    const steepCameraM = vec3(0, 0, RADIUS_M + 5_000);
    const nearFootM = scale(normalise(vec3(10_000 / RADIUS_M, 0, 1)), RADIUS_M);
    let samples = 0;
    const flat = {
      heightAtM: (): number => {
        samples += 1;
        return 0;
      },
      ceilingM: 1_000,
    };
    const hidden = groundHidesTick(steepCameraM, nearFootM, FIGURE, flat, PIXEL_RAD, TOLERANCE_PX);
    expect([hidden, samples < TICK_SIGHT_SAMPLES / 3]).toEqual([false, true]);
  });

  it("finds a ridge rising at 40° from open ground seen steeply, its top over the tolerance", () => {
    // The camera 5 km over the pole, the foot 10 km away along the surface; 2 km before the foot a
    // plateau 300 m long stands 1.5 px above the line of sight, its flanks falling at 40° to the
    // datum.
    const steepCameraM = vec3(0, 0, RADIUS_M + 5_000);
    const nearFootM = scale(normalise(vec3(10_000 / RADIUS_M, 0, 1)), RADIUS_M);
    const back = sub(steepCameraM, nearFootM);
    const alongOfPoint = (q: Vec3): number => Math.atan2(Math.hypot(q.x, q.y), q.z) * RADIUS_M;
    // The line of sight's height over the datum and a pixel there at 8 km from the pole.
    let lowT = 0;
    let highT = 1;
    for (let step = 0; step < 60; step += 1) {
      const mid = (lowT + highT) / 2;
      if (alongOfPoint(add(nearFootM, scale(back, mid))) > 8_000) {
        lowT = mid;
      } else {
        highT = mid;
      }
    }
    const crestPoint = add(nearFootM, scale(back, lowT));
    const topM =
      norm(crestPoint) - RADIUS_M + 1.5 * norm(sub(steepCameraM, crestPoint)) * PIXEL_RAD;
    const flank = Math.tan((40 * Math.PI) / 180);
    const plateau = {
      heightAtM: (d: Vec3): number => {
        const offM = Math.max(0, Math.abs(alongOfPoint(d) - 8_000) - 150);
        return Math.max(0, topM - offM * flank);
      },
      ceilingM: topM,
    };
    expect(groundHidesTick(steepCameraM, nearFootM, FIGURE, plateau, PIXEL_RAD, TOLERANCE_PX)).toBe(
      true,
    );
  });

  it("reads no ground within its line's footprint before the foot", () => {
    // A wall 100 m high from 19.8 km to the foot: within a pixel of the datum's depth slope, 330 m.
    const wall = {
      heightAtM: (d: Vec3): number =>
        Math.atan2(Math.hypot(d.x, d.y), d.z) * RADIUS_M >= 19_800 ? 100 : 0,
      ceilingM: 100,
    };
    expect(groundHidesTick(cameraM, footM, FIGURE, wall, PIXEL_RAD, TOLERANCE_PX)).toBe(false);
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

  it("keeps the nearest drawn point's range once its sight tests are used up with none seen", () => {
    // Twenty-five cells apart, whose edges' pieces are far more than the tests' cap.
    const cells: Array<readonly [Face, number, number]> = [];
    for (let i = 12; i <= 20; i += 2) {
      for (let j = 12; j <= 20; j += 2) {
        cells.push([0, i, j]);
      }
    }
    const { view, camera } = viewOf(surveyed(...cells), 200_000);
    const bare = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
    const hidden = surveyEdgeMark(
      { ...view, ground: { heightAtM: () => 150_000, ceilingM: 150_000 } },
      camera,
      VIEWPORT,
      OPTIONS,
    );
    expect(hidden.nearestRangeM).toBe(bare.nearestRangeM);
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

/** How many ticks a mark draws. */
function ticksOf(mark: { readonly ticks: Float32Array }): number {
  return mark.ticks.length / 6;
}

/** The segments of a packed line any of whose points falls in the view, as `segmentsOf` gives them. */
function inViewOf(packed: Float32Array, camera: ProjectionCamera): Array<readonly [Vec3, Vec3]> {
  return segmentsOf(packed).filter(([a, b]) =>
    Array.from({ length: 33 }, (_, k) => k / 32).some((u) => {
      const at = project(add(a, scale(sub(b, a), u)), camera, VIEWPORT);
      return (
        at.inFront &&
        at.xPx >= 0 &&
        at.xPx <= VIEWPORT.widthPx &&
        at.yPx >= 0 &&
        at.yPx <= VIEWPORT.heightPx
      );
    }),
  );
}

describe("the survey edge's pieces off the view (R10.T16.c)", () => {
  // 200 km over the cell near its south-west corner through a field 30 km across: its south and
  // west sides cross the view, its north and east sides lie off it, and its border is one run.
  const corner = cellPoint(0, 16.15, 16.15);
  const cornerM = scale(corner, RADIUS_M + 200_000);
  const view: SurveyEdgeView = {
    body: "test",
    coverage: surveyed([0, 16, 16]),
    figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
    rotation: IDENTITY_ROTATION,
    centreM: scale(cornerM, -1),
    heightAtM: () => null,
  };
  const camera = { orientation: lookAlong(scale(corner, -1), vec3(0, 0, 1)), fovXRad: 0.15 };
  // The datum, the rim nowhere above it.
  const flat = { heightAtM: () => 0, ceilingM: 0 };
  /** The mark of the view, over `ground` where given. */
  const markOf = (ground: typeof flat | null): SurveyEdgeMark =>
    surveyEdgeMark(ground === null ? view : { ...view, ground }, camera, VIEWPORT, OPTIONS);

  it("draws fewer segments, over ground whose highest it knows", () => {
    expect(markOf(flat).line.length).toBeLessThan(markOf(null).line.length);
  });

  it("draws in the view each segment it draws with every piece drawn", () => {
    const shown = inViewOf(markOf(flat).line, camera);
    expect([shown.length > 0, shown]).toEqual([true, inViewOf(markOf(null).line, camera)]);
  });

  it("places the same ticks, their spacing carried on over the pieces it does not draw", () => {
    expect([markOf(flat).ticks.length > 0, [...markOf(flat).ticks]]).toEqual([
      true,
      [...markOf(null).ticks],
    ]);
  });

  it("gives the same range to the edge's nearest point in the view", () => {
    expect(markOf(flat).nearestRangeM).toBe(markOf(null).nearestRangeM);
  });

  it("draws in the view each segment of a rim raised to the ground's highest, as it comes in", () => {
    // 3 km over the datum 0.8 of a cell south of the cell, looking north across its south side,
    // the rim 1.5 km up, 3.3° below the horizontal where its datum stands 5.5° below: as the view
    // pitches from 10° up to 2° down, its bottom edge 10.8° below its axis passes between them.
    const up = cellPoint(0, 16.5, 15.2);
    const cameraM = scale(up, RADIUS_M + 3_000);
    const north = normalise(sub(cellPoint(0, 16.5, 15.21), up));
    const raised: SurveyEdgeView = {
      body: "test",
      coverage: surveyed([0, 16, 16]),
      figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
      rotation: IDENTITY_ROTATION,
      centreM: scale(cameraM, -1),
      heightAtM: () => 1_500,
    };
    const ground = { heightAtM: () => 1_500, ceilingM: 1_500 };
    const differ = Array.from({ length: 48 }, (_, k) => ((k * 0.25 - 10) * Math.PI) / 180).filter(
      (pitch) => {
        const forward = sub(scale(north, Math.cos(pitch)), scale(up, Math.sin(pitch)));
        const pitched = { orientation: lookAlong(forward, up), fovXRad: 0.5 };
        const culled = surveyEdgeMark({ ...raised, ground }, pitched, VIEWPORT, OPTIONS);
        const whole = surveyEdgeMark(raised, pitched, VIEWPORT, OPTIONS);
        return (
          JSON.stringify(inViewOf(culled.line, pitched)) !==
          JSON.stringify(inViewOf(whole.line, pitched))
        );
      },
    );
    expect(differ).toEqual([]);
  });

  it("places each tick of a side just past the view's edge whose tick reaches into it", () => {
    // 200 km over unsurveyed ground east of the cell through a field 30 km across, looking down,
    // stepped west 100 m at a time, so that the cell's east side comes in across the view's left
    // edge, its ticks pointing east, into the view, first.
    const ticksDiffer = Array.from({ length: 60 }, (_, k) => 17.0 + 0.32 - k * 0.002).filter(
      (a) => {
        const up = cellPoint(0, a, 16.5);
        const cameraM = scale(up, RADIUS_M + 200_000);
        const east: SurveyEdgeView = {
          body: "test",
          coverage: surveyed([0, 16, 16]),
          figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
          rotation: IDENTITY_ROTATION,
          centreM: scale(cameraM, -1),
          heightAtM: () => null,
        };
        const down = { orientation: lookAlong(scale(up, -1), vec3(0, 0, 1)), fovXRad: 0.15 };
        const culled = surveyEdgeMark({ ...east, ground: flat }, down, VIEWPORT, OPTIONS);
        const whole = surveyEdgeMark(east, down, VIEWPORT, OPTIONS);
        return JSON.stringify([...culled.ticks]) !== JSON.stringify([...whole.ticks]);
      },
    );
    expect(ticksDiffer).toEqual([]);
  });
});

describe("the survey edge's rim kept from frame to frame (R10.T16.c)", () => {
  const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
  /** The view reading its rim from `key` at `revision`, and how many times it read it. */
  const counting = (
    key: object,
    revision: number,
  ): { readonly view: SurveyEdgeView; readonly reads: () => number } => {
    let reads = 0;
    return {
      view: {
        ...view,
        heightAtM: () => {
          reads += 1;
          return 500;
        },
        ground: { heightAtM: () => 0, ceilingM: 500, rimSource: { key, revision } },
      },
      reads: () => reads,
    };
  };

  it("reads the rim again at no place while its source's revision stands", () => {
    const key = {};
    surveyEdgeMark(counting(key, 1).view, camera, VIEWPORT, OPTIONS);
    const again = counting(key, 1);
    surveyEdgeMark(again.view, camera, VIEWPORT, OPTIONS);
    expect(again.reads()).toBe(0);
  });

  it("reads it again at every place once the revision moves", () => {
    const key = {};
    const first = counting(key, 1);
    surveyEdgeMark(first.view, camera, VIEWPORT, OPTIONS);
    const moved = counting(key, 2);
    surveyEdgeMark(moved.view, camera, VIEWPORT, OPTIONS);
    expect(moved.reads()).toBe(first.reads());
  });

  it("draws from the kept rim the mark it draws from the rim read afresh", () => {
    const key = {};
    surveyEdgeMark(counting(key, 1).view, camera, VIEWPORT, OPTIONS);
    const fresh = {
      ...view,
      heightAtM: () => 500,
      ground: { heightAtM: () => 0, ceilingM: 500 },
    };
    expect(surveyEdgeMark(counting(key, 1).view, camera, VIEWPORT, OPTIONS)).toEqual(
      surveyEdgeMark(fresh, camera, VIEWPORT, OPTIONS),
    );
  });

  /**
   * The view reading its rim from `key` at `revision`, its source stamping each read `stampOf` its
   * count, the stamps in `standing` standing; and how many times it read it.
   */
  const stamping = (
    key: object,
    revision: number,
    standing: ReadonlySet<number>,
    stampOf: (reads: number) => number,
  ): { readonly view: SurveyEdgeView; readonly reads: () => number } => {
    let reads = 0;
    const stamps: SurveyEdgeRimStamps = {
      lastRimStamp: () => stampOf(reads),
      rimStampStands: (stamp) => standing.has(stamp),
    };
    return {
      view: {
        ...view,
        heightAtM: () => {
          reads += 1;
          return 500;
        },
        ground: { heightAtM: () => 0, ceilingM: 500, rimSource: { key, revision, stamps } },
      },
      reads: () => reads,
    };
  };

  it("reads the rim again, where its source stamps its reads, only where a read's stamp falls, however the revision moves (R10.T16.i)", () => {
    const key = {};
    const both = new Set([1, 2]);
    // Its reads stamped 2 and 1 in turn.
    const first = stamping(key, 1, both, (reads) => (reads % 2) + 1);
    surveyEdgeMark(first.view, camera, VIEWPORT, OPTIONS);
    const moved = stamping(key, 2, both, () => 1);
    surveyEdgeMark(moved.view, camera, VIEWPORT, OPTIONS);
    const fallen = stamping(key, 3, new Set([1]), () => 1);
    surveyEdgeMark(fallen.view, camera, VIEWPORT, OPTIONS);
    expect([first.reads() > 2, moved.reads(), fallen.reads()]).toEqual([
      true,
      0,
      Math.floor(first.reads() / 2),
    ]);
  });

  it("reads a place whose stamp fell again by its read's token where its source can", () => {
    const key = {};
    let reads = 0;
    let agains = 0;
    let stamp = 1;
    const stamps: SurveyEdgeRimStamps = {
      lastRimStamp: () => stamp,
      rimStampStands: (s) => s === stamp,
      lastRimToken: () => "read",
      // As a fresh read would read it: the same height, so that the pieces and places are too.
      rimAgain: (token) => {
        agains += token === "read" ? 1 : 0;
        return 500;
      },
    };
    const marked = (): SurveyEdgeMark =>
      surveyEdgeMark(
        {
          ...view,
          heightAtM: () => {
            reads += 1;
            return 500;
          },
          ground: { heightAtM: () => 0, ceilingM: 900, rimSource: { key, revision: 1, stamps } },
        },
        camera,
        VIEWPORT,
        OPTIONS,
      );
    const before = marked();
    const first = reads;
    stamp = 2;
    const after = marked();
    expect([reads - first, agains, after.line]).toEqual([0, first, before.line]);
  });

  it("keeps two sources' rims apart over one coverage", () => {
    const at = (key: object, rimM: number): SurveyEdgeMark =>
      surveyEdgeMark(
        {
          ...view,
          heightAtM: () => rimM,
          ground: { heightAtM: () => 0, ceilingM: 900, rimSource: { key, revision: 1 } },
        },
        camera,
        VIEWPORT,
        OPTIONS,
      );
    const first = {};
    const second = {};
    at(first, 500);
    expect(at(second, 900).line).not.toEqual(at(first, 500).line);
  });
});

describe("the survey edge's pieces over a bump in its rim (R10.T16.c)", () => {
  // A bump in the rim at the middle of the cell's side at t = 16, a Gaussian 1 km wide along it.
  const middle = faceUvToDir(0, stToUv(16.5 / 32), stToUv(16 / 32));
  const bump =
    (heightM: number): ((d: Vec3) => number) =>
    (d) => {
      const offM = Math.acos(Math.min(1, dot(normalise(d), middle))) * RADIUS_M;
      return heightM * Math.exp(-((offM / 1_000) ** 2) / 2);
    };
  const piecesOf = (heightM: number | null): number => {
    const { view, camera } = viewOf(surveyed([0, 16, 16]), 200_000);
    const rim = heightM === null ? view : { ...view, heightAtM: bump(heightM) };
    return surveyEdgeMark(rim, camera, VIEWPORT, OPTIONS).line.length / 6;
  };

  it("halves the pieces whose middle stands more than half a pixel off their chord", () => {
    // 5 km high, the rim's points move some 2 px on the view, its curve sharp across a piece.
    expect(piecesOf(5_000)).toBeGreaterThan(piecesOf(null));
  });

  it("leaves the pieces whole where the rim's curve stays within half a pixel of their chords", () => {
    // 0.2 km high, the rim's points move a tenth of a pixel at the most.
    expect(piecesOf(200)).toBe(piecesOf(null));
  });
});

describe("the survey edge's ticks seen obliquely (R10.T16.c)", () => {
  // The camera 50 km over cell (16, 14), looking at cell (16, 16)'s centre, so that the ticks of
  // the cell's nearer side point towards the camera, over nearer ground.
  const up = cellCentre(0, 16, 14);
  const cameraM = scale(up, RADIUS_M + 50_000);
  const targetM = scale(cellCentre(0, 16, 16), RADIUS_M);
  const view: SurveyEdgeView = {
    body: "test",
    coverage: surveyed([0, 16, 16]),
    figure: { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
    rotation: IDENTITY_ROTATION,
    centreM: scale(cameraM, -1),
    heightAtM: () => null,
  };
  const camera = { orientation: lookAlong(normalise(sub(targetM, cameraM)), up), fovXRad: 1 };
  const ticks = segmentsOf(surveyEdgeMark(view, camera, VIEWPORT, OPTIONS).ticks);
  /**
   * How far a tick's end stands off the plane through its foot parallel to the datum's, m: its
   * `f32` ends 50 km out are some 4 mm apart.
   */
  const offPlaneM = ([foot, end]: readonly [Vec3, Vec3]): number =>
    Math.abs(dot(sub(end, foot), normalise(sub(foot, view.centreM))));
  /** How much nearer the camera a tick's end stands than its foot, along the view's axis, m. */
  const nearerM = ([foot, end]: readonly [Vec3, Vec3]): number =>
    project(foot, camera, VIEWPORT).depth > 0
      ? NEAR_PLANE_M / project(foot, camera, VIEWPORT).depth -
        NEAR_PLANE_M / project(end, camera, VIEWPORT).depth
      : 0;

  it("lays each tick towards nearer ground on the plane through its foot, the plane its line is pulled to", () => {
    const towards = ticks.filter((tick) => nearerM(tick) > 1);
    expect([towards.length > 0, towards.every((tick) => offPlaneM(tick) < 0.05)]).toEqual([
      true,
      true,
    ]);
  });

  it("keeps every tick the guide's length on the view", () => {
    const lengths = ticks.map(([foot, end]) => {
      const a = project(foot, camera, VIEWPORT);
      const b = project(end, camera, VIEWPORT);
      return Math.hypot(b.xPx - a.xPx, b.yPx - a.yPx);
    });
    expect(lengths.every((px) => Math.abs(px - SURVEY_EDGE_TICK_REM * REM_PX) < 1e-3)).toBe(true);
  });
});

describe("the survey edge's ticks behind nearer ground (R10.T16.c)", () => {
  // The camera 200 km over a point of the cell nearer its west side, and a tower halfway to the
  // line's nearest point, which hides that stretch of it.
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
  const tower = normalise(add(up, nearestPointDir(view, camera)));
  const ground = {
    heightAtM: (d: Vec3): number =>
      Math.acos(Math.min(1, dot(normalise(d), tower))) * RADIUS_M < 3_000 ? 180_000 : 0,
    ceilingM: 180_000,
  };
  const bare = surveyEdgeMark(view, camera, VIEWPORT, OPTIONS);
  const behind = surveyEdgeMark({ ...view, ground }, camera, VIEWPORT, OPTIONS);

  it("leaves out the ticks whose feet the nearer tower hides, and keeps others", () => {
    expect([ticksOf(behind) > 0, ticksOf(behind) < ticksOf(bare)]).toEqual([true, true]);
  });

  it("draws the line behind the tower whole, for the depth test to hide", () => {
    expect(behind.line).toEqual(bare.line);
  });

  it("keeps every tick over ground that hides none of them", () => {
    const flat = { heightAtM: () => 0, ceilingM: 1_000 };
    expect(surveyEdgeMark({ ...view, ground: flat }, camera, VIEWPORT, OPTIONS).ticks).toEqual(
      bare.ticks,
    );
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

/** An edge bake sloped across its grid, its morph target below it. */
function sloped(baseM: number): ResidentBake {
  return {
    standing: "edge",
    ownAtM: (x, y) => baseM + 2 * x + 0.5 * y + 0.01 * x * y,
    morphAtM: (x, y) => baseM - 30 + x - 0.25 * y,
    surveyedAt: (x, y) => x + y < 110,
  };
}

describe("the survey edge's mark over the ground as drawn, its reads and its segments' packing (lane perf-render-thread-ground)", () => {
  /** The level-6 patches of level-5 cells (16, 16) to (17, 17) of face 0, and their parents. */
  const keys: PatchKey[] = [];
  for (let i = 32; i < 36; i += 1) {
    for (let j = 32; j < 36; j += 1) {
      keys.push({ face: 0, level: 6, i, j });
    }
  }
  const bakes = new Map<string, ResidentBake>(
    [
      ...keys,
      ...[16, 17].flatMap((i) => [16, 17].map((j): PatchKey => ({ face: 0, level: 5, i, j }))),
    ].map((key, n) => [patchKeyString(key), sloped(40 + 7 * n)]),
  );
  /** Each patch's instance: plain, refining from its parent, rising from the datum, fading in turn. */
  const instances = keys.map((key, n): DrawnInstance => {
    const mix = Math.round(((n % 5) * MIX_STEPS) / 5);
    const fades = [
      0,
      fadeWord(0, mix, PARENT_MIX_SLOT),
      fadeWord(0, mix, DATUM_SLOT),
      fadeWord(60, 0, 0),
    ][n % 4];
    return {
      key,
      area: key,
      morphStartM: 800,
      morphEndM: 12_000,
      hasParent: true,
      holds: 0,
      edgeFades: n % 3 === 0 ? 0x0f : 0,
      cornerFades: 0,
      fades: fades ?? 0,
    };
  });

  it("draws the segments and the range it drew with each read afresh and each segment made, bit for bit", () => {
    const coverage = surveyed([0, 16, 16], [0, 17, 16], [0, 16, 17]);
    const marks = [2_000, 9_000, 40_000, 250_000].flatMap((heightM) => {
      const { view, camera, cameraM } = viewOf(coverage, heightM);
      const drawn = new DrawnGround(
        { bakeAt: (key) => bakes.get(patchKeyString(key)) ?? null, rimRevision: 0, revision: 0 },
        { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null },
      );
      // Two frames: the second reads the kept rim where its stamps stand.
      return [0, 1].map(() => {
        drawn.begin();
        for (const instance of instances) {
          drawn.push(instance);
        }
        drawn.end(cameraM, [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)]);
        return surveyEdgeMark(
          {
            ...view,
            heightAtM: (d) => drawn.rimHeightAtM(d),
            ground: {
              heightAtM: (d) => drawn.groundHeightAtM(d),
              ceilingM: 400,
              rimSource: { key: drawn, revision: drawn.revision, stamps: drawn },
            },
          },
          camera,
          VIEWPORT,
          OPTIONS,
        );
      });
    });
    const bytes = marks.flatMap((mark) => [
      ...new Uint8Array(mark.line.buffer, mark.line.byteOffset, mark.line.byteLength),
      ...new Uint8Array(mark.ticks.buffer, mark.ticks.byteOffset, mark.ticks.byteLength),
      ...new Uint8Array(Float64Array.of(mark.nearestRangeM ?? -1).buffer),
    ]);
    // As the mark drew them at 469a0dad.
    expect([
      marks.map((mark) => [mark.line.length, mark.ticks.length]),
      fnv1a64(Uint8Array.from(bytes)).toString(16),
    ]).toEqual([
      [
        [24, 0],
        [24, 0],
        [0, 0],
        [0, 0],
        [660, 228],
        [660, 228],
        [828, 360],
        [828, 360],
      ],
      "41e68d29dfa5e695",
    ]);
  });
});
