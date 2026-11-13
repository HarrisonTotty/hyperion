/**
 * The wireframe's terrain selection (plan R10, R10.T11): "the wireframe's selection is within a
 * factor of two of `patchCountModel`'s ratio to the lit style, about a fifth; the grounded-body
 * rule holds in this style". R05's `selectPatches` at the wireframe's τ of 4 px
 * (`WIREFRAME_TERRAIN_TAU_PX`) against the lit style's 1 px, each on one 1920 × 1080 view of 60°
 * across looking 30° below the horizon (Design note 14's look-down), with the same budget, none, so
 * that the counts are the tolerances' alone.
 *
 * @remarks
 * Deviation: the bracket was ruled on Design note 15's table (decision-r10-view-block-room §9),
 * which stands for `patchCountModel` here, and it stays on it now the model is built (R10.T14.a's
 * `cacheBudget.ts`; decision-r10-t14a §4, decision-r10-t14b-measure §6(a)): the model's ratios and
 * the measured ones are R10.T14.a's and R10.T14.b's records'. The table's frustum counts, the high
 * setting's 1 px at 1080p against the wireframe's 4 px at 1080p, give
 * the ratio r of their middles at each height, and the wireframe's selection must lie in
 * [r ÷ 2, 2 r]. It is taken on two planets of WGS 84's figure: one built on the count model's own
 * premise, a patch accepted at k = 5 times its level's edge at 1 px (a flat level table whose bound
 * is 5 edges ÷ the view's pixels a radian), and R05's test planet, the golden level table, as
 * `select.test.ts` builds it, whose bound is what selection charges as built.
 */

import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import { PATCH_QUADS, vertexDir, vertexSpacing } from "./cube";
import { finestPatchSizeM, type GroundContact, inForcedRegion } from "./grounded";
import { MAX_LEVEL, parentKey, type PatchKey, patchKeyString } from "./patchKey";
import { patchBounds } from "./bounds";
import { type PlanetGeometry, planetGeometry, surfacePoint } from "./planet";
import {
  type HeightRangeLookup,
  type Selection,
  selectPatches,
  type ViewSelectionInput,
} from "./select";

const HD = { widthPx: 1920, heightPx: 1080 } as const;
const FOV_X_RAD = Math.PI / 3;
/** The view's pixels a radian at its centre, W ÷ (2 tan(fov ÷ 2)). */
const PX_PER_RAD = HD.widthPx / (2 * Math.tan(FOV_X_RAD / 2));
/** 30° below the horizon: 60° from the nadir. */
const LOOK_DOWN_TILT_RAD = Math.PI / 3;
/** The patch the camera stands over, as `select.test.ts`'s. */
const SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };

/**
 * A level table on the count model's premise (Design note 15): flat ground, and a bound that a
 * view of {@link PX_PER_RAD} meets at 1 px at k = 5 times the level's mean patch edge.
 */
function countModelTable(): Float64Array {
  const table = new Float64Array(4 * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const spacing = vertexSpacing(WGS84_FIGURE.equatorialRadiusM, level);
    table[4 * level] = (5 * PATCH_QUADS * spacing.meanM) / PX_PER_RAD;
    table[4 * level + 3] = spacing.maxM;
  }
  return table;
}

const COUNT_MODEL_PLANET = planetGeometry(WGS84_FIGURE, countModelTable());
const TEST_PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** The point `altitudeM` above the datum over {@link SITE}'s centre, body-fixed. */
function above(altitudeM: number): Vec3 {
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), altitudeM);
  return vec3(x, y, z);
}

/** A camera at `positionM` looking `tiltRad` from its nadir towards the east. */
function lookingDown(positionM: Vec3, tiltRad: number): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const forward = normalise(
    vec3(
      -up.x * Math.cos(tiltRad) + east.x * Math.sin(tiltRad),
      -up.y * Math.cos(tiltRad) + east.y * Math.sin(tiltRad),
      -up.z * Math.cos(tiltRad) + east.z * Math.sin(tiltRad),
    ),
  );
  return lookAlong(forward, up);
}

/** One view at `altitudeM` over the site, `tiltRad` from its nadir, at tolerance `tauPx`. */
function viewAt(altitudeM: number, tiltRad: number, tauPx: number): ViewSelectionInput {
  const positionM = above(altitudeM);
  return {
    camera: { positionM, orientation: lookingDown(positionM, tiltRad) },
    fovXRad: FOV_X_RAD,
    viewport: HD,
    weight: 1,
    tauPx,
  };
}

/** The patches the view sees, the count model's "in the frustum". */
function seenCount(selection: Selection): number {
  return [...selection.patches.values()].filter((patch) => patch.seen).length;
}

/** The ratio of the wireframe's frustum count to the lit style's on `planet` at `altitudeM`. */
function wireToLitRatio(planet: PlanetGeometry, altitudeM: number): number {
  const count = (tauPx: number): number =>
    seenCount(
      selectPatches({
        planet,
        views: [viewAt(altitudeM, LOOK_DOWN_TILT_RAD, tauPx)],
        setting: "high",
        grounded: [],
      }),
    );
  return count(WIREFRAME_TERRAIN_TAU_PX) / count(1);
}

/** A row of Design note 15's table: a camera height and the ratio of its frustum counts' middles. */
interface TableRow {
  readonly heightM: number;
  readonly ratio: number;
}

/** A row from the high setting's and the wireframe's frustum counts, each a range. */
function tableRow(
  heightM: number,
  high: readonly [number, number],
  wire: readonly [number, number],
): TableRow {
  return { heightM, ratio: (wire[0] + wire[1]) / (high[0] + high[1]) };
}

/** Design note 15's frustum counts: high, 1 px 1080p; wireframe, 4 px 1080p. */
const TABLE: ReadonlyArray<TableRow> = [
  tableRow(1_000, [200, 210], [52, 56]),
  tableRow(10_000, [144, 164], [38, 42]),
  tableRow(100_000, [92, 116], [26, 30]),
];

describe("the wireframe's selection against the lit style's (Design note 15's table)", () => {
  it.each(TABLE)(
    "selects within a factor of two of the table's ratio at $heightM m on the count model's premise",
    ({ heightM, ratio }) => {
      const selected = wireToLitRatio(COUNT_MODEL_PLANET, heightM);
      expect(selected).toBeGreaterThanOrEqual(ratio / 2);
      expect(selected).toBeLessThanOrEqual(2 * ratio);
    },
  );

  // Ruled (decision-r10-view-block-room §9): held between the τ⁻² limit and twice the table's
  // ratio, not to the table. Under a screen-space error bound a selection refined well past the
  // quadtree's coarse levels scales as τ⁻², the least the wire-to-lit ratio can be (a level cap or
  // the forced region raises it, nothing lowers it), and R05's test planet's level table accepts a
  // patch at about 15 edges at 1 px, not the count model's 5, so at 4 px it stays past the
  // granular floor and its ratio heads for 1 ÷ 16 rather than the table's quarter (measured 0.115,
  // 0.115 and 0.070 at 1, 10 and 100 km; plan 10's Risks, R10.T11's deferred findings).
  it.each(TABLE)(
    "selects between the τ⁻² limit and twice the table's ratio at $heightM m on R05's test planet",
    ({ heightM, ratio }) => {
      const selected = wireToLitRatio(TEST_PLANET, heightM);
      expect(selected).toBeGreaterThanOrEqual(1 / WIREFRAME_TERRAIN_TAU_PX ** 2);
      expect(selected).toBeLessThanOrEqual(2 * ratio);
    },
  );
});

/** Every patch baked flat, its heights within ±1 m, so that τ alone sets how deep a view refines. */
const FLAT_BAKES: HeightRangeLookup = { heightRangeM: () => [-1, 1] };

/** A body 20 m round on the ground under the camera. */
const CONTACT: GroundContact = { positionM: above(0), radiusM: 20 };

/** The camera 2 km over the contact, looking straight down at it. */
const OVER_CONTACT_M = 2_000;

function groundedSelection(tauPx: number, grounded: GroundContact[]): Selection {
  return selectPatches({
    planet: TEST_PLANET,
    views: [viewAt(OVER_CONTACT_M, 0, tauPx)],
    setting: "high",
    grounded,
    heightRanges: FLAT_BAKES,
  });
}

/** The keys of a selection's forced patches, sorted. */
function forcedKeys(selection: Selection): string[] {
  return [...selection.patches.values()]
    .filter((patch) => patch.forced)
    .map((patch) => patchKeyString(patch.key))
    .toSorted();
}

/** The level of the selected patch covering `key`'s place, or −1 where none does. */
function coveringLevel(selection: Selection, key: PatchKey): number {
  for (let k: PatchKey | null = key; k !== null; k = parentKey(k)) {
    if (selection.patches.has(patchKeyString(k))) {
      return k.level;
    }
  }
  return -1;
}

describe("the grounded-body rule in the wireframe (Design note 9)", () => {
  it("forces the same patches at the wireframe's 4 px as the lit style does at 1 px", () => {
    const wire = forcedKeys(groundedSelection(WIREFRAME_TERRAIN_TAU_PX, [CONTACT]));
    expect(wire.length).toBeGreaterThan(4);
    expect(wire).toEqual(forcedKeys(groundedSelection(1, [CONTACT])));
  });

  it("forces exactly the finest-level patches within the contact's forced radius", () => {
    const patchSizeM = finestPatchSizeM(TEST_PLANET);
    const within: string[] = [];
    for (let di = -6; di <= 6; di += 1) {
      for (let dj = -6; dj <= 6; dj += 1) {
        const key = { ...SITE, i: SITE.i + di, j: SITE.j + dj };
        if (inForcedRegion(patchBounds(TEST_PLANET, key), [CONTACT], patchSizeM)) {
          within.push(patchKeyString(key));
        }
      }
    }
    expect(forcedKeys(groundedSelection(WIREFRAME_TERRAIN_TAU_PX, [CONTACT]))).toEqual(
      within.toSorted(),
    );
  });

  it("draws the ground under the contact at the finest level, where 4 px alone stops coarser", () => {
    expect([
      coveringLevel(groundedSelection(WIREFRAME_TERRAIN_TAU_PX, []), SITE) <
        TEST_PLANET.finestLevel,
      coveringLevel(groundedSelection(WIREFRAME_TERRAIN_TAU_PX, [CONTACT]), SITE),
    ]).toEqual([true, TEST_PLANET.finestLevel]);
  });

  it("lets the view see every patch it forces under a contact in view", () => {
    const selection = groundedSelection(WIREFRAME_TERRAIN_TAU_PX, [CONTACT]);
    expect([...selection.patches.values()].filter((p) => p.forced && !p.seen)).toEqual([]);
  });
});
