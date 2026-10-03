import { describe, expect, it } from "vitest";

import { add, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir, xyzToFaceUv, uvToSt } from "./cube";
import {
  contactHold,
  FORCED_REGION_RESIDENCY_S,
  finestPatchSizeM,
  type GroundContact,
  heldRadiusM,
  isDescending,
  morphHold,
} from "./grounded";
import {
  EDGES,
  edgeNeighbour,
  parentKey,
  type PatchKey,
  patchKeyString,
  unreachable,
} from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { type Selection, selectPatches, type ViewSelectionInput } from "./select";
import type { QualitySetting } from "../quality/qualitySetting";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const PATCH_M = finestPatchSizeM(PLANET);
const SITE: PatchKey = { face: 2, level: 19, i: 200_011, j: 300_007 };

function ground(key: PatchKey = SITE, x = 32, y = 32, heightM = 0): Vec3 {
  const [px, py, pz] = surfacePoint(WGS84_FIGURE, vertexDir(key, x, y), heightM);
  return vec3(px, py, pz);
}

const CRAFT: GroundContact = { positionM: ground(SITE, 32, 32, 10), radiusM: 20 };

/** A camera 300 m above and 400 m beside the site, looking at it. */
function viewOfSite(tauPx: number): ViewSelectionInput {
  const up = normalise(CRAFT.positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const camera = add(add(CRAFT.positionM, scale(up, 300)), scale(east, 400));
  const orientation: Quaternion = lookAlong(sub(CRAFT.positionM, camera), up);
  return {
    camera: { positionM: camera, orientation },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight: 1,
    tauPx,
  };
}

function select(
  grounded: ReadonlyArray<GroundContact>,
  setting: QualitySetting = "high",
  tauPx = TERRAIN_SETTINGS[setting].tauPx,
): Selection {
  return selectPatches({ planet: PLANET, views: [viewOfSite(tauPx)], setting, grounded });
}

/** The finest-level key whose cell holds the point `p` near the spheroid: d ∝ M⁻¹ p. */
function finestKeyAt(p: Vec3): PatchKey {
  const a = WGS84_FIGURE.equatorialRadiusM;
  const c = WGS84_FIGURE.polarRadiusM;
  const { face, u, v } = xyzToFaceUv([p.x / a, p.y / a, p.z / c]);
  const side = 2 ** PLANET.finestLevel;
  return {
    face,
    level: PLANET.finestLevel,
    i: Math.min(side - 1, Math.floor(uvToSt(u) * side)),
    j: Math.min(side - 1, Math.floor(uvToSt(v) * side)),
  };
}

function coveringLeaf(sel: Selection, key: PatchKey): PatchKey | null {
  let k: PatchKey | null = key;
  while (k !== null) {
    if (sel.patches.has(patchKeyString(k))) {
      return k;
    }
    k = parentKey(k);
  }
  return null;
}

/** The finest keys whose cells lie within `radiusM` of `centre` on the ground, about it. */
function finestKeysWithin(centre: Vec3, radiusM: number): PatchKey[] {
  const middle = finestKeyAt(centre);
  const reach = Math.ceil(radiusM / (PATCH_M * 0.6)) + 1;
  const keys: PatchKey[] = [];
  for (let di = -reach; di <= reach; di += 1) {
    for (let dj = -reach; dj <= reach; dj += 1) {
      const key = { ...middle, i: middle.i + di, j: middle.j + dj };
      const near = [0, 32, 64].some((x) =>
        [0, 32, 64].some((y) => {
          const d = sub(ground(key, x, y), centre);
          return Math.sqrt(d.x * d.x + d.y * d.y + d.z * d.z) <= radiusM;
        }),
      );
      if (near) {
        keys.push(key);
      }
    }
  }
  return keys;
}

describe("the descending test", () => {
  it("holds below 1 km while moving down, and within 30 s of contact", () => {
    expect(isDescending(900, -1)).toBe(true);
    expect(isDescending(900, 1)).toBe(false);
    expect(isDescending(5_000, -200)).toBe(true);
    expect(isDescending(5_000, -100)).toBe(false);
    expect(isDescending(0, 0)).toBe(false);
  });

  it("publishes the residency time, at most the 30 s threshold", () => {
    expect(FORCED_REGION_RESIDENCY_S).toBeGreaterThan(0);
    expect(FORCED_REGION_RESIDENCY_S).toBeLessThanOrEqual(30);
  });
});

describe("the morph hold", () => {
  it("is 0 within the held radius, 1 beyond the ramp, and continuous between", () => {
    const up = normalise(CRAFT.positionM);
    const east = normalise(vec3(-up.y, up.x, 0));
    const r = heldRadiusM(CRAFT, PATCH_M);
    const at = (d: number): number =>
      morphHold(add(CRAFT.positionM, scale(east, d)), [CRAFT], PATCH_M);
    expect(at(0)).toBe(0);
    expect(at(r - 0.01)).toBe(0);
    expect(at(r + PATCH_M + 0.01)).toBe(1);
    let previous = at(r - 1);
    for (let d = r - 1; d <= r + PATCH_M + 1; d += 0.01) {
      const k = at(d);
      expect(Math.abs(k - previous)).toBeLessThanOrEqual(0.01 / PATCH_M + 1e-9);
      previous = k;
    }
  });

  it("matches terrain.wgsl's hold, evaluated in f32, for each contact", () => {
    // `morphFactor` in lane C's terrain.wgsl (R05.T11.b): beyond = length(v − c) − r_g;
    // hold = select(select(0, 1, beyond > 0), clamp(beyond / ramp, 0, 1), ramp > 0).
    const f = Math.fround;
    const shader = (distanceM: number, heldM: number, rampM: number): number => {
      const beyond = f(f(distanceM) - f(heldM));
      if (!(f(rampM) > 0)) {
        return beyond > 0 ? 1 : 0;
      }
      return Math.min(Math.max(f(beyond / f(rampM)), 0), 1);
    };
    for (const [d, held, ramp] of [
      [0, 30, 17.7],
      [29.99, 30, 17.7],
      [30, 30, 17.7],
      [38.85, 30, 17.7],
      [47.7, 30, 17.7],
      [100, 30, 17.7],
      [29, 30, 0],
      [31, 30, 0],
    ] as const) {
      expect(Math.abs(contactHold(d, held, ramp) - shader(d, held, ramp))).toBeLessThan(1e-6);
    }
  });

  it("agrees at every shared vertex of two neighbouring patches near a body", () => {
    // Two finest patches side by side across the ramp, east of the body.
    const key = finestKeyAt(CRAFT.positionM);
    const pairs: [PatchKey, PatchKey][] = [0, 1, 2].map((d) => [
      { ...key, i: key.i + d },
      { ...key, i: key.i + d + 1 },
    ]);
    const ramp: number[] = [];
    for (const [west, east] of pairs) {
      for (let y = 0; y <= 64; y += 1) {
        // The west patch's x = 64 column is the east patch's x = 0.
        const fromWest = morphHold(ground(west, 64, y), [CRAFT], PATCH_M);
        const fromEast = morphHold(ground(east, 0, y), [CRAFT], PATCH_M);
        expect(fromWest).toBe(fromEast);
        ramp.push(fromWest);
      }
    }
    // The pairs reach into the ramp, where the hold is strictly between 0 and 1.
    expect(ramp.some((k) => k > 0 && k < 1)).toBe(true);
  });

  it("is 1 with no contact", () => {
    expect(morphHold(ground(), [], PATCH_M)).toBe(1);
  });
});

describe("selection about a grounded body", () => {
  it.each([
    ["the high setting", "high" as const, TERRAIN_SETTINGS.high.tauPx],
    ["the low setting", "low" as const, TERRAIN_SETTINGS.low.tauPx],
    ["a 4 px view tolerance", "high" as const, 4],
  ])("selects the finest level under the body and the ramp's margin on %s", (_, setting, tau) => {
    const sel = select([CRAFT], setting, tau);
    const needed = finestKeysWithin(CRAFT.positionM, heldRadiusM(CRAFT, PATCH_M) + PATCH_M);
    expect(needed.length).toBeGreaterThan(4);
    const missing = needed.filter((k) => sel.patches.get(patchKeyString(k))?.forced !== true);
    expect(missing.map(patchKeyString)).toEqual([]);
    // Without the body, the 4 px view draws coarser there.
    const free = select([], setting, tau);
    expect(coveringLeaf(free, finestKeyAt(CRAFT.positionM))?.level).toBeLessThan(
      PLANET.finestLevel,
    );
  });

  it("has the morph at 1 wherever a finest patch meets a coarser one", () => {
    const sel = select([CRAFT], "low");
    const cracks: string[] = [];
    for (const p of sel.patches.values()) {
      if (p.key.level !== PLANET.finestLevel) {
        continue;
      }
      for (const edge of EDGES) {
        const leaf = coveringLeaf(sel, edgeNeighbour(p.key, edge));
        if (leaf === null || leaf.level === p.key.level) {
          continue;
        }
        const along = (n: number): readonly [number, number] => {
          switch (edge) {
            case "UMin":
              return [0, n];
            case "UMax":
              return [64, n];
            case "VMin":
              return [n, 0];
            case "VMax":
              return [n, 64];
          }
          return unreachable(edge);
        };
        for (let n = 0; n <= 64; n += 8) {
          const [x, y] = along(n);
          for (const h of [p.bounds.minHeightM, p.bounds.maxHeightM]) {
            if (morphHold(ground(p.key, x, y, h), [CRAFT], PATCH_M) < 1) {
              cracks.push(`${patchKeyString(p.key)} ${edge}`);
            }
          }
        }
      }
    }
    expect(cracks).toEqual([]);
  });

  it("forces a straight swept path's chain of contacts with no gap between them", () => {
    const up = normalise(CRAFT.positionM);
    const east = normalise(vec3(-up.y, up.x, 0));
    const chain: GroundContact[] = Array.from({ length: 5 }, (_, n) => ({
      positionM: add(CRAFT.positionM, scale(east, n * CRAFT.radiusM)),
      radiusM: CRAFT.radiusM,
    }));
    const sel = select(chain, "low");
    const gaps: string[] = [];
    for (let d = 0; d <= 4 * CRAFT.radiusM; d += 2) {
      const key = finestKeyAt(add(CRAFT.positionM, scale(east, d)));
      if (sel.patches.get(patchKeyString(key))?.forced !== true) {
        gaps.push(`${d} m`);
      }
    }
    expect(gaps).toEqual([]);
  });

  it("selects the same with contacts in one list or from two callers concatenated", () => {
    const other: GroundContact = { positionM: ground(SITE, 0, 0, 5), radiusM: 8 };
    const once = select([CRAFT, other]);
    const twice = select([other, CRAFT]);
    expect([...twice.patches.entries()].map(([k, p]) => [k, p.forced])).toEqual(
      [...once.patches.entries()].map(([k, p]) => [k, p.forced]),
    );
  });
});
