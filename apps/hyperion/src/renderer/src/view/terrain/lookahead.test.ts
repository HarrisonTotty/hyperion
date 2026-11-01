import { describe, expect, it } from "vitest";

import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "./cube";
import {
  aheadPositionM,
  LOOKAHEAD_COST_P95_MS,
  LOOKAHEAD_MAX_CLOSING_FRACTION,
  LOOKAHEAD_MAX_S,
  LOOKAHEAD_MIN_S,
  lookaheadSeconds,
  movesFast,
  slowSelection,
  timeToNeedS,
} from "./lookahead";
import type { PatchKey } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { RESELECT_MOVE_FRACTION } from "./selectionTolerance";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
// About 0.6 km wide, so that its neighbours along its row are reached within 2 s at 1 km/s.
const SITE: PatchKey = { face: 0, level: 14, i: 4_000, j: 8_000 };

function above(altitudeM: number, key: PatchKey = SITE): Vec3 {
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), altitudeM);
  return vec3(x, y, z);
}

/** The local east at `positionM`, along the ground. */
function eastAt(positionM: Vec3): Vec3 {
  const up = normalise(positionM);
  return normalise(vec3(-up.y, up.x, 0));
}

describe("the lookahead's L", () => {
  it("is the median of the round trips, the mean of the middle two for an even count", () => {
    expect(lookaheadSeconds([0.9, 0.3, 1.5])).toBe(0.9);
    expect(lookaheadSeconds([0.4, 1.6, 0.8, 1.2])).toBe(1);
  });

  it("holds within 0.25–2 s, and takes the least before any bake has landed", () => {
    expect(lookaheadSeconds([])).toBe(LOOKAHEAD_MIN_S);
    expect(lookaheadSeconds([0.01, 0.05, 0.1])).toBe(LOOKAHEAD_MIN_S);
    expect(lookaheadSeconds([5, 80, 6])).toBe(LOOKAHEAD_MAX_S);
    expect([LOOKAHEAD_MIN_S, LOOKAHEAD_MAX_S]).toEqual([0.25, 2]);
  });
});

/** Whether a view at `speedMps` moves fast, 1 km from its nearest ground. */
function movesFastAt(speedMps: number): boolean {
  return movesFast({ velocityMps: vec3(0, speedMps, 0), nearestM: 1_000 });
}

/** 64 steps' selection times, ms: `slow` of them at 2.5 ms, the rest at 0.5. */
function stepsWithSlow(slow: number): number[] {
  return Array.from({ length: 64 }, (_, n) => (n < slow ? 2.5 : 0.5));
}

describe("a view that moves fast", () => {
  it("is one faster than m ÷ (1 + m) of its nearest ground a second", () => {
    expect(movesFastAt(RESELECT_MOVE_FRACTION * 1_000 * 1.01)).toBe(true);
    expect(movesFastAt(RESELECT_MOVE_FRACTION * 1_000 * 0.99)).toBe(false);
    expect(movesFastAt(0)).toBe(false);
  });

  it("is never one with no nearest ground known", () => {
    expect(movesFast({ velocityMps: vec3(1e6, 0, 0), nearestM: Number.POSITIVE_INFINITY })).toBe(
      false,
    );
  });
});

describe("the lookahead's pose", () => {
  const p = above(2_000);

  it("is v × L on along the ground", () => {
    const v = scale(eastAt(p), 1_500);
    const ahead = aheadPositionM(p, { velocityMps: v, nearestM: 2_000 }, 0.5);
    expect(norm(sub(ahead, add(p, scale(v, 0.5))))).toBe(0);
  });

  it("is v × L on rising, and on closing by less than half the nearest ground", () => {
    const up = normalise(p);
    const rising = scale(up, 400);
    expect(aheadPositionM(p, { velocityMps: rising, nearestM: 2_000 }, 2)).toEqual(
      add(p, scale(rising, 2)),
    );
    const closing = scale(up, -400);
    expect(aheadPositionM(p, { velocityMps: closing, nearestM: 2_000 }, 2)).toEqual(
      add(p, scale(closing, 2)),
    );
  });

  it("closes on the body by at most half its nearest ground, its motion along the ground kept", () => {
    const up = normalise(p);
    const east = eastAt(p);
    const v = add(scale(up, -3_000), scale(east, 1_000));
    const ahead = aheadPositionM(p, { velocityMps: v, nearestM: 2_000 }, 1);
    const step = sub(ahead, p);
    const radialM = step.x * up.x + step.y * up.y + step.z * up.z;
    const eastM = step.x * east.x + step.y * east.y + step.z * east.z;
    expect(radialM).toBeCloseTo(-LOOKAHEAD_MAX_CLOSING_FRACTION * 2_000, 6);
    expect(eastM).toBeCloseTo(1_000, 6);
  });
});

describe("the time to need", () => {
  const p = above(2_000);
  /** SITE's neighbour `n` patches along its row; the camera moves towards `n` > 0. */
  const along = (n: number): PatchKey => ({ ...SITE, i: SITE.i + n });
  const forward = normalise(sub(above(0, along(1)), above(0)));
  const camera = { positionM: p, velocityMps: scale(forward, 1_000) };

  it("is sooner for a patch nearer along the motion, 0 at or behind the camera", () => {
    const near = timeToNeedS(PLANET, along(1), [camera], 2);
    const far = timeToNeedS(PLANET, along(3), [camera], 2);
    expect(near).toBeGreaterThan(0);
    expect(far).toBeGreaterThan(near);
    expect(timeToNeedS(PLANET, along(-3), [camera], 2)).toBe(0);
  });

  it("is L for a patch beyond the lookahead, and 0 with no moving view", () => {
    expect(timeToNeedS(PLANET, along(3), [camera], 0.01)).toBe(0.01);
    expect(timeToNeedS(PLANET, along(3), [], 2)).toBe(0);
  });

  it("is the least over the moving views: the first to reach the patch", () => {
    const faster = { positionM: p, velocityMps: scale(forward, 4_000) };
    const both = timeToNeedS(PLANET, along(3), [camera, faster], 2);
    expect(both).toBe(timeToNeedS(PLANET, along(3), [faster], 2));
    expect(both).toBeLessThan(timeToNeedS(PLANET, along(3), [camera], 2));
  });
});

describe("slow selection", () => {
  it("is a p95 of the steps' selection times above 2 ms", () => {
    // The p95 of 64 is the 61st smallest: three of them above 2 ms leave it at 0.5 ms, four raise it.
    expect(slowSelection(stepsWithSlow(3))).toBe(false);
    expect(slowSelection(stepsWithSlow(4))).toBe(true);
    expect(LOOKAHEAD_COST_P95_MS).toBe(2);
  });

  it("is never so with no step timed", () => {
    expect(slowSelection([])).toBe(false);
  });
});
