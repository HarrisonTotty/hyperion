import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "./cube";
import type { GroundContact } from "./grounded";
import { childKeys, FACES, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import { levelHeightRangeM, planetGeometry, surfacePoint } from "./planet";
import { compareRequests, PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
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

function lookingDown(positionM: Vec3, tiltRad: number): Quaternion {
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
    camera: { positionM, orientation: lookingDown(positionM, tiltRad) },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight,
    tauPx: 1,
  };
}

/** Every patch down to level 2 baked, at its level's range, so that demand reaches level 3. */
const SHALLOW_BAKED: HeightRangeLookup = {
  heightRangeM: (key) => (key.level <= 2 ? levelHeightRangeM(PLANET, key.level) : undefined),
};

function demand(
  views: ViewSelectionInput[],
  grounded: GroundContact[] = [],
): readonly PatchRequest[] {
  return selectPatches({
    planet: PLANET,
    views,
    setting: "high",
    grounded,
    heightRanges: SHALLOW_BAKED,
  }).demand;
}

describe("streaming priority across views", () => {
  const camera = above(3_000);

  it("requests each patch once for two views at one pose", () => {
    const requests = demand([
      view(camera, PRIMARY_VIEW_WEIGHT),
      view(camera, SECONDARY_VIEW_WEIGHT),
    ]);
    const keys = requests.map((r) => patchKeyString(r.key));
    expect(keys.length).toBeGreaterThan(0);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("ranks a secondary view's requests below the primary's at equal error", () => {
    const primary = demand([view(camera, PRIMARY_VIEW_WEIGHT)]);
    const secondary = demand([view(camera, SECONDARY_VIEW_WEIGHT)]);
    expect(secondary.map((r) => patchKeyString(r.key))).toEqual(
      primary.map((r) => patchKeyString(r.key)),
    );
    secondary.forEach((r, n) => {
      expect(r.priority).toBeCloseTo((primary[n]?.priority ?? NaN) * SECONDARY_VIEW_WEIGHT, 9);
    });
    // Shown together, a patch only the secondary view wants comes after the primary's at equal error.
    expect(Math.max(...secondary.map((r) => r.priority))).toBeLessThan(
      Math.max(...primary.map((r) => r.priority)),
    );
  });

  it("puts forced-region patches before everything", () => {
    const contact: GroundContact = { positionM: above(5), radiusM: 20 };
    const requests = demand([view(camera, PRIMARY_VIEW_WEIGHT)], [contact]);
    const firstUnforced = requests.findIndex((r) => !r.forced);
    const lastForced = requests.findLastIndex((r) => r.forced);
    expect(lastForced).toBeGreaterThanOrEqual(0);
    expect(firstUnforced === -1 || lastForced < firstUnforced).toBe(true);
  });

  it("orders the demand by priority, ties by patchKeyString", () => {
    const requests = demand([
      view(camera, PRIMARY_VIEW_WEIGHT),
      view(above(9_000), SECONDARY_VIEW_WEIGHT, 0.3),
    ]);
    expect([...requests].toSorted(compareRequests)).toEqual(requests);
    const tied: PatchRequest[] = FACES.flatMap((f) => childKeys(rootKey(f))).map((key) => ({
      key,
      priority: 1,
      forced: false,
    }));
    const sorted = tied
      .toReversed()
      .toSorted(compareRequests)
      .map((r) => patchKeyString(r.key));
    expect(sorted).toEqual([...sorted].toSorted());
  });
});
