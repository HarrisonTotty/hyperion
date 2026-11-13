/**
 * Selection's counting options (plan R10, R10.T14.b; decision-r10-t14b-measure §2 and §5): the
 * all-round view, culled by its horizon alone, and the diagnostic occluder's height. Neither moves
 * a selection made without them; `select.test.ts`' digests hold that bit for bit.
 */

import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { TERRAIN_SETTINGS, WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { relativeBounds } from "./bounds";
import {
  COUNT_MODEL_SITE,
  patchCountModel,
  REFERENCE_PX_PER_RAD,
  WGS84_EQUATORIAL_RADIUS_M,
} from "./cacheBudget";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { PATCH_QUADS, vertexDir, vertexSpacing } from "./cube";
import { MAX_LEVEL, parentKey, type PatchKey, patchKeyString } from "./patchKey";
import { lowestHeightM, planetGeometry, surfacePoint } from "./planet";
import {
  type HeightRangeLookup,
  type Selection,
  type SelectionInput,
  selectPatches,
  type ViewSelectionInput,
} from "./select";

/** `select.test.ts`' planet: WGS 84's figure under R05's golden level table, without ridges. */
const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const HD = { widthPx: 1920, heightPx: 1080 } as const;
const FOV_X_RAD = Math.PI / 3;

/** The point `altitudeM` above the count model's site, `select.test.ts`' own, body-fixed metres. */
function above(altitudeM: number): Vec3 {
  const centre = PATCH_QUADS / 2;
  const [x, y, z] = surfacePoint(
    WGS84_FIGURE,
    vertexDir(COUNT_MODEL_SITE, centre, centre),
    altitudeM,
  );
  return vec3(x, y, z);
}

/** The local east at `positionM`, square to its up. */
function eastOf(positionM: Vec3): Vec3 {
  return normalise(vec3(-positionM.y, positionM.x, 0));
}

/** A camera at `positionM` looking down at the ground, tilted `tiltRad` towards the east. */
function lookingDown(positionM: Vec3, tiltRad = 0): Quaternion {
  const up = normalise(positionM);
  const east = eastOf(positionM);
  const c = Math.cos(tiltRad);
  const s = Math.sin(tiltRad);
  const forward = vec3(-up.x * c + east.x * s, -up.y * c + east.y * s, -up.z * c + east.z * s);
  return lookAlong(forward, up);
}

/** A camera at `positionM` looking straight up, away from the planet, east at the top. */
function lookingUp(positionM: Vec3): Quaternion {
  return lookAlong(normalise(positionM), eastOf(positionM));
}

function view(
  positionM: Vec3,
  orientation: Quaternion,
  overrides: Partial<ViewSelectionInput> = {},
): ViewSelectionInput {
  return {
    camera: { positionM, orientation },
    fovXRad: FOV_X_RAD,
    viewport: HD,
    weight: 1,
    tauPx: 1,
    ...overrides,
  };
}

/** The selection of `views` on the planet, with no budget unless `extra` sets one. */
function select(
  views: ReadonlyArray<ViewSelectionInput>,
  extra: Partial<SelectionInput> = {},
): Selection {
  return selectPatches({ planet: PLANET, views, setting: "high", grounded: [], ...extra });
}

/** The patches view `bit` sees. */
function seenBy(sel: Selection, bit: number): number {
  return [...sel.patches.values()].filter((p) => (p.views & (1 << bit)) !== 0).length;
}

/** Everything a selection returns but its bounds' objects, in order. */
function outcome(sel: Selection): unknown {
  return {
    patches: [...sel.patches.entries()].map(([k, p]) => [k, p.views, p.seen, p.forced]),
    demand: sel.demand.map((r) => [patchKeyString(r.key), r.priority, r.forced]),
    limited: sel.limited,
    limitExcess: sel.limitExcess,
    hiddenBaked: sel.hiddenBaked.map(patchKeyString),
    nearestSurveyByViewM: sel.nearestSurveyByViewM,
  };
}

/** The selected leaf covering `key`'s area, at its level or coarser, or `null`. */
function coveringLeaf(sel: Selection, key: PatchKey): PatchKey | null {
  for (let k: PatchKey | null = key; k !== null; k = parentKey(k)) {
    if (sel.patches.has(patchKeyString(k))) {
      return k;
    }
  }
  return null;
}

/** Every selected leaf of `sel` and every patch above one, by `patchKeyString`. */
function leavesAndAbove(sel: Selection): Set<string> {
  const keys = new Set<string>();
  for (const p of sel.patches.values()) {
    for (let k: PatchKey | null = p.key; k !== null; k = parentKey(k)) {
      keys.add(patchKeyString(k));
    }
  }
  return keys;
}

/** A lookup in which every patch down to `depth` is baked at ±100 m. */
function bakedToDepth(depth: number): HeightRangeLookup {
  return { heightRangeM: (key) => (key.level <= depth ? [-100, 100] : undefined) };
}

const LOW = above(1_500);
const ORBIT = above(400_000);

/** The poses the counts compare at: low and tilted, and from orbit looking down. */
const POSES: ReadonlyArray<{ readonly name: string; readonly view: ViewSelectionInput }> = [
  { name: "1.5 km up, tilted 1.2 rad", view: view(LOW, lookingDown(LOW, 1.2)) },
  { name: "400 km up, looking down", view: view(ORBIT, lookingDown(ORBIT)) },
];

/** A selection that reads every part of the input: two views, bakes, a budget and a contact. */
function busyInput(views: ReadonlyArray<ViewSelectionInput>): SelectionInput {
  return {
    planet: PLANET,
    views,
    setting: "high",
    grounded: [{ positionM: above(5), radiusM: 20 }],
    heightRanges: bakedToDepth(7),
    maxPatches: 300,
  };
}

describe("the all-round view (R10.T14.b; decision-r10-t14b-measure §2)", () => {
  it("selects as with no all-round key when every view's is false", () => {
    const low = lookingDown(LOW, 1.2);
    const orbit = lookingDown(ORBIT, 0.3);
    const without = selectPatches(busyInput([view(LOW, low), view(ORBIT, orbit)]));
    const off = selectPatches(
      busyInput([view(LOW, low, { allRound: false }), view(ORBIT, orbit, { allRound: false })]),
    );
    expect(without.limited).toBe(true);
    expect(outcome(off)).toEqual(outcome(without));
  });

  it.each(POSES)(
    "marks every patch the frustum view sees as seen all round too, in one selection, $name",
    ({ view: v }) => {
      const sel = select([v, { ...v, allRound: true }]);
      const frustumOnly = [...sel.patches.values()].filter((p) => (p.views & 0b11) === 0b01);
      expect(frustumOnly.map((p) => patchKeyString(p.key))).toEqual([]);
      expect(seenBy(sel, 1)).toBeGreaterThan(seenBy(sel, 0));
    },
  );

  it.each(POSES)(
    "counts more patches all round than in the frustum, alone, and holds each frustum patch or finer, $name",
    ({ view: v }) => {
      const frustum = select([v]);
      const allRound = select([{ ...v, allRound: true }]);
      expect(seenBy(allRound, 0)).toBeGreaterThan(seenBy(frustum, 0));
      const reached = leavesAndAbove(allRound);
      const lost = [...frustum.patches.values()]
        .filter((p) => (p.views & 1) !== 0 && !reached.has(patchKeyString(p.key)))
        .map((p) => patchKeyString(p.key));
      expect(lost).toEqual([]);
    },
  );

  it("sees a patch behind the camera, above the horizon, that the frustum view does not", () => {
    const v = view(ORBIT, lookingUp(ORBIT));
    const sel = select([v, { ...v, allRound: true }]);
    const below = coveringLeaf(sel, COUNT_MODEL_SITE);
    if (below === null) {
      throw new Error("the all-round view sees the ground under the camera");
    }
    const patch = sel.patches.get(patchKeyString(below));
    if (patch === undefined) {
      throw new Error("a covering leaf is selected");
    }
    const rel = relativeBounds(patch.bounds, v.camera.positionM);
    const frustum = frustumOf({
      orientation: v.camera.orientation,
      fovXRad: FOV_X_RAD,
      viewport: HD,
    });
    expect({
      views: patch.views,
      inFrustum: inFrustum(rel, frustum),
      aboveHorizon: aboveHorizon(rel, horizonCone(PLANET, v.camera.positionM)),
      inFrustumSelection: coveringLeaf(select([v]), COUNT_MODEL_SITE),
    }).toEqual({ views: 0b10, inFrustum: false, aboveHorizon: true, inFrustumSelection: null });
  });

  it("ignores an all-round view's frustum margin", () => {
    const v = view(LOW, lookingDown(LOW, 1.2), { allRound: true });
    expect(outcome(select([{ ...v, frustumMarginRad: 0.2 }]))).toEqual(outcome(select([v])));
  });
});

/** The count model's columns: a setting's τ and viewport, as `cacheBudget.test.ts` takes them. */
const COLUMNS = [
  { name: "high", tauPx: TERRAIN_SETTINGS.high.tauPx, viewport: HD },
  { name: "low", tauPx: TERRAIN_SETTINGS.low.tauPx, viewport: { widthPx: 1280, heightPx: 720 } },
  { name: "wire", tauPx: WIREFRAME_TERRAIN_TAU_PX, viewport: HD },
] as const;

/**
 * The count model's premise planet's level table, for `selectPatches`: `cacheBudget.test.ts`' and
 * `terrainWireSelection.test.ts`' `countModelTable()`, copied (neither exports it): ε_n =
 * k₀ eₙ ÷ P₀ at k₀ = 5, no relief.
 */
function countModelTable(): Float64Array {
  const table = new Float64Array(4 * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const spacing = vertexSpacing(WGS84_EQUATORIAL_RADIUS_M, level);
    table[4 * level] = (5 * PATCH_QUADS * spacing.meanM) / REFERENCE_PX_PER_RAD;
    table[4 * level + 3] = spacing.maxM;
  }
  return table;
}

const PREMISE_PLANET = planetGeometry(
  {
    equatorialRadiusM: WGS84_EQUATORIAL_RADIUS_M,
    polarRadiusM: WGS84_EQUATORIAL_RADIUS_M,
    pole: null,
  },
  countModelTable(),
);

describe("the all-round view against the count model on its premise planet", () => {
  const CASES = COLUMNS.flatMap((column) =>
    [1_000, 10_000, 100_000].map((heightM) => ({ column, heightM })),
  );

  it.each(CASES)(
    "sees within [0.99, 1.02] of the model's all-round count on the boxes, $column.name at $heightM m",
    ({ column, heightM }) => {
      const model = patchCountModel({ heightM, fovXRad: FOV_X_RAD }, column.tauPx, column.viewport);
      const [peak] = model.peaks;
      if (peak === undefined) {
        throw new Error("the model has a peak for each heading");
      }
      const selection = selectPatches({
        planet: PREMISE_PLANET,
        views: [
          {
            camera: { positionM: model.positionM, orientation: peak.orientation },
            fovXRad: FOV_X_RAD,
            viewport: column.viewport,
            weight: 1,
            tauPx: column.tauPx,
            allRound: true,
          },
        ],
        setting: "high",
        grounded: [],
      });
      const ratio = [...selection.patches.values()].filter((p) => p.seen).length / model.allRound;
      expect(ratio).toBeGreaterThanOrEqual(0.99);
      expect(ratio).toBeLessThanOrEqual(1.02);
    },
  );
});

/** The planet's own occluder height, `horizonCone`'s: the level table's lowest. */
const LOWEST_M = lowestHeightM(PLANET);

describe("the diagnostic occluder (R10.T14.b's A/B; decision-r10-t14b-measure §5)", () => {
  it("selects bit for bit as by default at the level table's own lowest height", () => {
    const views = [
      view(LOW, lookingDown(LOW, 1.2)),
      view(ORBIT, lookingDown(ORBIT), { allRound: true }),
    ];
    const byDefault = selectPatches(busyInput(views));
    const atLowest = selectPatches({ ...busyInput(views), occluderHeightM: LOWEST_M });
    expect(byDefault.limited).toBe(true);
    expect(outcome(atLowest)).toEqual(outcome(byDefault));
  });

  const OCCLUDER_POSES: ReadonlyArray<{
    readonly name: string;
    readonly view: ViewSelectionInput;
  }> = [
    { name: "1.5 km up, near the horizon", view: view(LOW, lookingDown(LOW, 1.5)) },
    { name: "1.5 km up, all round", view: view(LOW, lookingDown(LOW), { allRound: true }) },
    { name: "400 km up, all round", view: view(ORBIT, lookingDown(ORBIT), { allRound: true }) },
  ];

  it.each(OCCLUDER_POSES)(
    "never sees more with a shallower occluder, nor fewer with a deeper one, $name",
    ({ view: v }) => {
      const seenAt = (occluderHeightM: number): number =>
        seenBy(select([v], { occluderHeightM }), 0);
      const byDefault = seenBy(select([v]), 0);
      const counts = {
        deeper: seenAt(LOWEST_M - 50_000),
        datum: seenAt(0),
        higher: seenAt(10_000),
      };
      expect(counts.deeper).toBeGreaterThanOrEqual(byDefault);
      expect(counts.datum).toBeLessThanOrEqual(byDefault);
      expect(counts.higher).toBeLessThanOrEqual(counts.datum);
    },
  );

  it("sees fewer all round from orbit with the occluder at the datum than at the table's lowest", () => {
    const v = view(ORBIT, lookingDown(ORBIT), { allRound: true });
    expect(seenBy(select([v], { occluderHeightM: 0 }), 0)).toBeLessThan(seenBy(select([v]), 0));
  });

  it.each([
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
    -WGS84_FIGURE.polarRadiusM,
    -2 * WGS84_FIGURE.polarRadiusM,
  ])("refuses an occluder height of %s m with a RangeError naming it", (occluderHeightM) => {
    const refused = (): Selection => select([view(LOW, lookingDown(LOW))], { occluderHeightM });
    expect(refused).toThrow(RangeError);
    expect(refused).toThrow(/occluderHeightM/);
  });

  it("takes an occluder height of either sign, down to just above minus the polar radius", () => {
    const v = view(ORBIT, lookingDown(ORBIT));
    for (const occluderHeightM of [10_000, -10_000, 1 - WGS84_FIGURE.polarRadiusM]) {
      expect(() => select([v], { occluderHeightM })).not.toThrow();
    }
  });
});
