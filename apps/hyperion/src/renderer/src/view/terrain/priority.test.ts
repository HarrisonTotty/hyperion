import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "./cube";
import type { GroundContact } from "./grounded";
import { type PatchKey, parentKey, patchKeyString } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { seededRandom } from "../../test/seededRandom";
import {
  compareRequests,
  fillsHole,
  joinDemand,
  LOOKAHEAD_DEMAND_WEIGHT,
  PRIMARY_VIEW_WEIGHT,
  SECONDARY_VIEW_WEIGHT,
} from "./priority";
import {
  type HeightRangeLookup,
  type PatchRequest,
  selectPatches,
  type ViewSelectionInput,
} from "./select";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };

function above(altitudeM: number): Vec3 {
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), altitudeM);
  return vec3(x, y, z);
}

/** Looking `tiltRad` from the nadir towards the local east. */
function looking(positionM: Vec3, tiltRad: number): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const c = Math.cos(tiltRad);
  const s = Math.sin(tiltRad);
  return lookAlong(
    vec3(-up.x * c + east.x * s, -up.y * c + east.y * s, -up.z * c + east.z * s),
    up,
  );
}

function view(positionM: Vec3, weight: number, tiltRad = 1.0): ViewSelectionInput {
  return {
    camera: { positionM, orientation: looking(positionM, tiltRad) },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight,
    tauPx: 1,
  };
}

/** Every patch down to `depth` baked, at ±100 m, so that demand reaches the level below. */
function bakedTo(depth: number): HeightRangeLookup {
  return { heightRangeM: (key) => (key.level <= depth ? [-100, 100] : undefined) };
}

function demand(
  views: ReadonlyArray<ViewSelectionInput>,
  grounded: ReadonlyArray<GroundContact> = [],
  depth = 8,
): readonly PatchRequest[] {
  return selectPatches({
    planet: PLANET,
    views,
    setting: "high",
    grounded,
    heightRanges: bakedTo(depth),
  }).demand;
}

function byKey(requests: readonly PatchRequest[]): Map<string, PatchRequest> {
  return new Map(requests.map((r) => [patchKeyString(r.key), r]));
}

const KEY = (i: number): PatchKey => ({ face: 0, level: 5, i, j: 0 });

describe("the request order", () => {
  it("puts a forced request before an unforced one of higher priority", () => {
    const forced = { key: KEY(1), priority: 0.1, forced: true };
    const unforced = { key: KEY(0), priority: 9, forced: false };
    expect([unforced, forced].toSorted(compareRequests)).toEqual([forced, unforced]);
  });

  it("puts the higher priority first", () => {
    const low = { key: KEY(0), priority: 1, forced: false };
    const high = { key: KEY(1), priority: 2, forced: false };
    expect([low, high].toSorted(compareRequests)).toEqual([high, low]);
  });

  it("breaks a tie by patchKeyString", () => {
    const a = { key: KEY(10), priority: 1, forced: false };
    const b = { key: KEY(2), priority: 1, forced: false };
    // "0/5/10/0" sorts before "0/5/2/0".
    expect([b, a].toSorted(compareRequests)).toEqual([a, b]);
  });
});

/** A request's place before its time to need: forced or not, then its priority. */
function rankOf(r: PatchRequest): string {
  return `${r.forced ? 0 : 1}/${r.priority}`;
}

describe("the time to need's tie-break (R10.T14.b)", () => {
  it("puts the sooner needed first among equal priorities, before the key, absent as 0", () => {
    const later = { key: KEY(0), priority: 1, forced: false, needS: 0.8 };
    const sooner = { key: KEY(1), priority: 1, forced: false, needS: 0.2 };
    const now = { key: KEY(2), priority: 1, forced: false };
    expect([later, sooner, now].toSorted(compareRequests)).toEqual([now, sooner, later]);
  });

  it("leaves the order among different priorities, forced first, as it is without it", () => {
    const random = seededRandom(24);
    const requests: PatchRequest[] = Array.from({ length: 400 }, (_, n) => ({
      key: KEY(n),
      // Few priorities, so that most requests tie with others.
      priority: Math.floor(random() * 6),
      forced: random() < 0.1,
      needS: Math.floor(random() * 4) / 2,
    }));
    const withNeed = requests.toSorted(compareRequests);
    const without = requests
      .map(({ key, priority, forced }) => ({ key, priority, forced }))
      .toSorted(compareRequests);
    expect(withNeed.map(rankOf)).toEqual(without.map(rankOf));
    // Within each rank, the sooner needed first.
    const later = withNeed.filter((r, n) => {
      const next = withNeed[n + 1];
      return next !== undefined && rankOf(next) === rankOf(r) && (next.needS ?? 0) < (r.needS ?? 0);
    });
    expect(later).toEqual([]);
  });
});

describe("the lookahead's demand, joined (R10.T14.b)", () => {
  it("asks each patch once, at the larger priority, forced from either, by the request order", () => {
    const own = [
      { key: KEY(0), priority: 4, forced: false },
      { key: KEY(1), priority: 1, forced: false },
    ];
    const ahead = [
      { key: KEY(1), priority: 3, forced: false },
      { key: KEY(2), priority: 2, forced: true },
      { key: KEY(0), priority: 2, forced: false },
    ];
    expect(joinDemand(own, ahead, (key) => key.i / 10)).toEqual([
      { key: KEY(2), priority: 2, forced: true, needS: 0.2 },
      { key: KEY(0), priority: 4, forced: false, needS: 0 },
      { key: KEY(1), priority: 3, forced: false, needS: 0.1 },
    ]);
  });

  it("adds nothing at the view's own pose: the lookahead's views ask at half their weight", () => {
    const own = demand([view(above(3_000), PRIMARY_VIEW_WEIGHT)]);
    const ahead = demand([view(above(3_000), PRIMARY_VIEW_WEIGHT * LOOKAHEAD_DEMAND_WEIGHT)]);
    expect(LOOKAHEAD_DEMAND_WEIGHT).toBe(0.5);
    expect(joinDemand(own, ahead, () => 0)).toEqual(
      own.map(({ key, priority, forced }) => ({ key, priority, forced, needS: 0 })),
    );
  });
});

describe("streaming priority across views", () => {
  const low = above(3_000);

  it("adds nothing for a second view at the same pose", () => {
    const one = demand([view(low, PRIMARY_VIEW_WEIGHT)]);
    const two = demand([view(low, PRIMARY_VIEW_WEIGHT), view(low, SECONDARY_VIEW_WEIGHT)]);
    expect(one.length).toBeGreaterThan(0);
    expect(two).toEqual(one);
  });

  it("ranks a patch only a secondary view wants at the secondary's weight", () => {
    // Both views at one pose; the primary's tolerance is three times the secondary's, so it sees
    // every patch the secondary does but wants the last splits at only a third of its excess,
    // which at the primary's weight of 1 would outrank the secondary's 0.25.
    const primary = { ...view(low, PRIMARY_VIEW_WEIGHT), tauPx: 3 };
    const secondary = view(low, SECONDARY_VIEW_WEIGHT);
    const both = byKey(demand([primary, secondary], [], 12));
    const alone = byKey(demand([secondary], [], 12));
    const primaryAlone = byKey(demand([primary], [], 12));
    const onlySecondary = [...alone.keys()].filter((k) => !primaryAlone.has(k) && both.has(k));
    expect(onlySecondary.length).toBeGreaterThan(0);
    for (const k of onlySecondary) {
      expect(both.get(k)?.priority).toBe(alone.get(k)?.priority);
    }
  });

  it("ranks a secondary view's request below the primary's at equal error", () => {
    const primary = byKey(demand([view(low, PRIMARY_VIEW_WEIGHT)]));
    const secondary = byKey(demand([view(low, SECONDARY_VIEW_WEIGHT)]));
    for (const [k, r] of secondary) {
      expect(r.priority).toBe((primary.get(k)?.priority ?? NaN) * SECONDARY_VIEW_WEIGHT);
    }
  });

  it("puts forced-region patches before everything, even one no view sees", () => {
    const contact: GroundContact = { positionM: above(5), radiusM: 20 };
    // The camera looks up at the sky: no view sees the ground under the craft.
    const requests = demand([view(low, PRIMARY_VIEW_WEIGHT, Math.PI)], [contact]);
    const forced = requests.filter((r) => r.forced);
    expect(forced.length).toBeGreaterThan(0);
    const lastForced = requests.findLastIndex((r) => r.forced);
    expect(requests.slice(0, lastForced + 1).every((r) => r.forced)).toBe(true);
  });

  it("orders the unforced demand by priority, highest first", () => {
    const requests = demand([
      view(above(400_000), PRIMARY_VIEW_WEIGHT, 0),
      view(low, SECONDARY_VIEW_WEIGHT),
    ]).filter((r) => !r.forced);
    expect(requests.length).toBeGreaterThan(1);
    for (let n = 1; n < requests.length; n += 1) {
      expect(requests[n]?.priority ?? NaN).toBeLessThanOrEqual(requests[n - 1]?.priority ?? NaN);
    }
  });
});

describe("a hole in a pass's drawn set (R10.T14.b)", () => {
  /** Face 0's patches baked to level 6, at ±100 m, and nothing of the other faces. */
  const faceZero: HeightRangeLookup = {
    heightRangeM: (key) => (key.face === 0 && key.level <= 6 ? [-100, 100] : undefined),
  };

  /** Whether a patch over `key`, its parent or coarser, is baked. */
  function bakedOver(key: PatchKey): boolean {
    for (let up = parentKey(key); up !== null; up = parentKey(up)) {
      if (faceZero.heightRangeM(up) !== undefined) {
        return true;
      }
    }
    return false;
  }

  it("is a request with nothing baked over it, a root's, wherever the demand is not forced", () => {
    // A view of a hemisphere from 20,000 km and one over the grounded craft at 3 km.
    const contact: GroundContact = { positionM: above(5), radiusM: 20 };
    const requests = selectPatches({
      planet: PLANET,
      views: [view(above(2e7), PRIMARY_VIEW_WEIGHT, 0), view(above(3_000), SECONDARY_VIEW_WEIGHT)],
      setting: "high",
      grounded: [contact],
      heightRanges: faceZero,
    }).demand;
    expect([
      requests.some((r) => fillsHole(r)),
      requests.some((r) => !r.forced && !fillsHole(r)),
      requests.some((r) => r.forced),
    ]).toEqual([true, true, true]);
    for (const r of requests) {
      expect(fillsHole(r)).toBe(!r.forced && !bakedOver(r.key));
    }
  });
});
