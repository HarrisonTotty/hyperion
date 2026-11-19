import { describe, expect, it } from "vitest";

import { add, normalise, norm, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { goldenLevelTable, WGS84_FIGURE } from "../../../test/terrainFixtures";
import type { Quaternion } from "../../camera/pose";
import { lookAlong } from "../../camera/quaternion";
import {
  type QualitySetting,
  TERRAIN_SETTINGS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../../quality/qualitySetting";
import { uvToSt, vertexDir, xyzToFaceUv } from "../../terrain/cube";
import type { GroundContact } from "../../terrain/grounded";
import { type PatchKey, patchKeyString } from "../../terrain/patchKey";
import { planetGeometry, surfacePoint } from "../../terrain/planet";
import { type Selection, selectPatches, type ViewSelectionInput } from "../../terrain/select";
import {
  couldTouch,
  inRegion,
  LARGEST_ROCK_REACH_M,
  type RockPatchInView,
  rockPx,
  rockRule,
  rocksLimitDetail,
  rocksToDraw,
} from "./rockSelection";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const SITE: PatchKey = { face: 2, level: 19, i: 200_011, j: 300_007 };

/** The ground point of vertex (`x`, `y`) of `key` at `heightM`. */
function ground(key: PatchKey = SITE, x = 32, y = 32, heightM = 0): Vec3 {
  const [px, py, pz] = surfacePoint(WGS84_FIGURE, vertexDir(key, x, y), heightM);
  return vec3(px, py, pz);
}

/** A 20 m craft resting on the ground at the site. */
const BODY = { groundPointM: ground(), radiusM: 20 };
const CONTACT: GroundContact = { positionM: BODY.groundPointM, radiusM: BODY.radiusM };

/** A view of the site from 300 m up and 400 m east, at the style's tolerance. */
function viewOfSite(tauPx: number): ViewSelectionInput {
  const up = normalise(BODY.groundPointM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const camera = add(add(BODY.groundPointM, scale(up, 300)), scale(east, 400));
  const orientation: Quaternion = lookAlong(sub(BODY.groundPointM, camera), up);
  return {
    camera: { positionM: camera, orientation },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight: 1,
    tauPx,
  };
}

function select(setting: QualitySetting, tauPx: number): Selection {
  return selectPatches({
    planet: PLANET,
    views: [viewOfSite(tauPx)],
    setting,
    grounded: [CONTACT],
  });
}

/** The finest-level key whose cell holds the point `p` near the spheroid. */
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

/** A finest patch's bounding sphere, about its centre vertex, out past its corners. */
function sphereOf(key: PatchKey): { readonly centreM: Vec3; readonly radiusM: number } {
  const centreM = ground(key);
  const radiusM = Math.max(
    ...[
      [0, 0],
      [64, 0],
      [0, 64],
      [64, 64],
    ].map(([x = 0, y = 0]) => norm(sub(ground(key, x, y), centreM))),
  );
  return { centreM, radiusM: radiusM + 1 };
}

/** The finest patches about the site whose spheres meet the region the body could touch. */
function touchedKeys(): PatchKey[] {
  const middle = finestKeyAt(BODY.groundPointM);
  const keys: PatchKey[] = [];
  for (let di = -4; di <= 4; di += 1) {
    for (let dj = -4; dj <= 4; dj += 1) {
      const key = { ...middle, i: middle.i + di, j: middle.j + dj };
      if (inRegion(sphereOf(key), [couldTouch(BODY)])) {
        keys.push(key);
      }
    }
  }
  return keys;
}

describe("the rocks' draw rules (R11.T4.b, Design note 7)", () => {
  it("widens a body's footprint by the largest octave's reach, the crate's 1.1 × 0.7695 × 6.4 m", () => {
    expect(LARGEST_ROCK_REACH_M).toBeCloseTo(5.41728, 10);
    expect(couldTouch(BODY)).toEqual({
      positionM: BODY.groundPointM,
      radiusM: 20 + LARGEST_ROCK_REACH_M,
    });
  });

  it.each([
    ["the high setting, lit", "high" as const, TERRAIN_SETTINGS.high.tauPx],
    ["the low setting, lit", "low" as const, TERRAIN_SETTINGS.low.tauPx],
    ["the high setting, in the wireframe", "high" as const, WIREFRAME_TERRAIN_TAU_PX],
    ["the low setting, in the wireframe", "low" as const, WIREFRAME_TERRAIN_TAU_PX],
  ])(
    "selects every patch the body could touch at the finest level and draws its rocks whole on %s",
    (_, setting, tau) => {
      const touched = touchedKeys();
      expect(touched.length).toBeGreaterThan(1);
      const sel = select(setting, tau);
      const unforced = touched.filter((k) => sel.patches.get(patchKeyString(k))?.forced !== true);
      expect(unforced.map(patchKeyString)).toEqual([]);
      for (const key of touched) {
        expect(rockRule(sphereOf(key), [couldTouch(BODY)], setting)).toBe("region");
      }
    },
  );

  it("draws the rocks elsewhere on the high setting where each subtends a pixel, and none on the low", () => {
    const away = { centreM: ground({ ...SITE, i: SITE.i + 40 }), radiusM: 14 };
    expect(rockRule(away, [couldTouch(BODY)], "high")).toBe("pixel");
    expect(rockRule(away, [couldTouch(BODY)], "low")).toBeNull();
  });

  it("is a function of the patches, the bodies and the setting alone, in any order", () => {
    const keys = Array.from({ length: 40 }, (_, n) => ({ ...SITE, i: SITE.i - 20 + n }));
    const rules = (order: ReadonlyArray<PatchKey>): Map<string, string | null> =>
      new Map(
        order.map((key) => [
          patchKeyString(key),
          rockRule(sphereOf(key), [couldTouch(BODY)], "low"),
        ]),
      );
    const forwards = rules(keys);
    expect(rules(keys.toReversed())).toEqual(forwards);
    expect(rules(keys)).toEqual(forwards);
    expect([...forwards.values()].filter((rule) => rule === "region").length).toBeGreaterThan(0);
    expect([...forwards.values()].filter((rule) => rule === null).length).toBeGreaterThan(0);
  });

  it("annunciates on the low setting exactly where a patch in view outside the region holds a rock of a pixel", () => {
    const camera = add(BODY.groundPointM, scale(normalise(BODY.groundPointM), 200));
    const pxPerRad = 1920 / (2 * Math.tan(Math.PI / 6));
    const region = [couldTouch(BODY)];
    const at = (i: number): RockPatchInView => ({
      ...sphereOf({ ...SITE, i: SITE.i + i }),
      largestRockM: 0,
    });
    const outside = at(60);
    const nearestM = norm(sub(outside.centreM, camera)) - outside.radiusM;
    // The diameter that subtends exactly one pixel there.
    const onePx = nearestM / pxPerRad;
    const limited = (patch: RockPatchInView, setting: QualitySetting): boolean =>
      rocksLimitDetail([patch], region, setting, camera, pxPerRad, 1);
    expect(limited({ ...outside, largestRockM: 1.01 * onePx }, "low")).toBe(true);
    expect(limited({ ...outside, largestRockM: 0.99 * onePx }, "low")).toBe(false);
    expect(limited({ ...outside, largestRockM: 6.4 }, "high")).toBe(false);
    expect(limited({ ...at(0), largestRockM: 6.4 }, "low")).toBe(false);
    expect(limited({ ...outside, largestRockM: 0 }, "low")).toBe(false);
    expect(rockPx(1, 0, pxPerRad)).toBe(Infinity);
  });

  it("takes the finest patches a view sees drawn whole, each by its rule where its mesh is held", () => {
    const keys: PatchKey[] = [SITE, { ...SITE, i: SITE.i + 60 }, { ...SITE, i: SITE.i + 61 }];
    const drawn = (key: PatchKey, within: PatchKey | null) => ({
      keyString: patchKeyString(key),
      patch: {
        ...sphereOf(key),
        key,
        generation: 1,
        originM: sphereOf(key).centreM,
        heightRangeM: [0, 0] as const,
        boundingRadiusM: sphereOf(key).radiusM,
        slot: key.i % 100,
        keyString: patchKeyString(key),
      },
      standIn: false,
      within,
    });
    const drawSet = {
      count: 5,
      patches: [
        ...keys.map((key) => drawn(key, null)),
        drawn({ ...SITE, level: 18, i: SITE.i >> 1 }, null),
        drawn(keys[0] ?? SITE, { ...SITE, i: SITE.i + 1 }),
      ],
      views: Uint32Array.of(1, 1, 2, 1, 1),
    };
    const meshes = new Map([
      [patchKeyString(SITE), { largestRockM: 2 }],
      [patchKeyString({ ...SITE, i: SITE.i + 60 }), { largestRockM: 3 }],
    ]);
    const held = { get: (key: string) => meshes.get(key), isRefused: () => false };
    const region = [couldTouch(BODY)];
    const high = rocksToDraw(drawSet, 1, PLANET.finestLevel, held, region, "high");
    expect(high.patches).toEqual([
      { keyString: patchKeyString(SITE), terrainSlot: SITE.i % 100, rule: "region" },
      {
        keyString: patchKeyString({ ...SITE, i: SITE.i + 60 }),
        terrainSlot: (SITE.i + 60) % 100,
        rule: "pixel",
      },
    ]);
    expect(high.inView.map((p) => p.largestRockM)).toEqual([2, 3]);
    const low = rocksToDraw(drawSet, 1, PLANET.finestLevel, held, region, "low");
    expect(low.patches.map((p) => p.rule)).toEqual(["region"]);
    expect(rocksToDraw(drawSet, 2, PLANET.finestLevel, held, region, "high").patches).toEqual([]);
    expect([high.regionRefused, low.regionRefused]).toEqual([false, false]);
  });

  it("says where the pool refused the mesh of a region patch in view, and not of a patch outside it", () => {
    const key = (i: number): PatchKey => ({ ...SITE, i });
    const drawn = (k: PatchKey) => ({
      keyString: patchKeyString(k),
      patch: {
        key: k,
        generation: 1,
        originM: sphereOf(k).centreM,
        heightRangeM: [0, 0] as const,
        boundingRadiusM: sphereOf(k).radiusM,
        slot: 0,
        keyString: patchKeyString(k),
      },
      standIn: false,
      within: null,
    });
    const refusedOf = (i: number) => ({
      get: () => undefined,
      isRefused: (keyString: string) => keyString === patchKeyString(key(i)),
    });
    const region = [couldTouch(BODY)];
    const drawSet = {
      count: 2,
      patches: [drawn(SITE), drawn(key(SITE.i + 60))],
      views: Uint32Array.of(1, 1),
    };
    const rule = (i: number) =>
      rocksToDraw(drawSet, 1, PLANET.finestLevel, refusedOf(i), region, "high").regionRefused;
    expect([rule(SITE.i), rule(SITE.i + 60)]).toEqual([true, false]);
  });
});
