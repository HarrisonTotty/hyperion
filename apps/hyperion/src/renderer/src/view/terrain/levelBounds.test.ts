/**
 * Selection by the real bound (plan R10, R10.T4.b): the selection bound under either rule, and
 * `selectPatches` over R09's six sample worlds' level bounds as R10.T4.a measured them
 * (`fixtures/level-bounds.json`), with `signoff-12.md` item 30's rule applied to their counts over
 * R05's scripted descent.
 *
 * @remarks
 * Each world's level table is composed from the fixture as the worker's `level_table` makes it
 * (`crates/hyperion-surface/src/worker.rs`): the cells' lowest and highest elevation, widened by a
 * micrometre and a part in 10¹², plus the most the level's and its parent's contributions can add,
 * stretched to the datum; the stated bound; the largest vertex spacing on the radius. Its σ is the
 * fixture's `sigma_m`, the largest `omitted_rms` over up to 16,384 of the field's cells, the
 * worker's σ over a whole surveyed field but for that sample. Levels past the complete level take
 * its row (bound and σ 0). Each world is a sphere of its radius: the fixture holds no flattening.
 * No patch is baked, so each box takes its level's subtree range, as an unbaked patch's does.
 */

import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import {
  type QualitySetting,
  SETTINGS,
  WIREFRAME_TERRAIN_SLOTS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../quality/qualitySetting";
import { RECORD_SEED } from "../spike/demandRecord";
import { DescentProfile, landingSiteOf } from "../spike/descentProfile";
import { distanceToBoxM, type PatchBounds, patchBounds, relativeBounds } from "./bounds";
import { vertexDir, vertexSpacing } from "./cube";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import fixture from "./fixtures/level-bounds.json" with { type: "json" };
import type { GroundContact } from "./grounded";
import {
  LEVEL_SIGMA_LENGTH,
  SELECTION_RULE,
  selectionBound,
  type SelectionRule,
  SIGMA_MULTIPLE,
} from "./levelBounds";
import {
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  MAX_LEVEL,
  parentKey,
  type PatchKey,
  patchKeyString,
} from "./patchKey";
import {
  type BodyFigure,
  LEVEL_TABLE_STRIDE,
  levelBoundM,
  type PlanetGeometry,
  planetGeometry,
  selectionBoundM,
  subtreeHeightRangeM,
  surfacePoint,
} from "./planet";
import {
  chordSagittaM,
  type HeightRangeLookup,
  type Selection,
  selectionErrorM,
  selectPatches,
  screenSpaceErrorPx,
  type ViewSelectionInput,
} from "./select";
import { terrainSlotLayout } from "./slotLayout";

const RULES: ReadonlyArray<SelectionRule> = ["hard", "statistical"];

/** The presented view every style is counted on: 1920 × 1080, 60° across (Design note 23). */
const HD = { widthPx: 1920, heightPx: 1080 } as const;
const FOV_X_RAD = Math.PI / 3;

/** Each style's τ, px: the high setting's, the low setting's and the wireframe's. */
const STYLE_TAUS: ReadonlyArray<number> = [
  SETTINGS.high.terrain.tauPx,
  SETTINGS.low.terrain.tauPx,
  WIREFRAME_TERRAIN_TAU_PX,
];

/** A style as its terrain pass selects: its setting, its τ and its budget, half its slots. */
interface Style {
  readonly setting: QualitySetting;
  readonly tauPx: number;
  readonly maxPatches: number;
}

/** The lit terrain on each setting, and the wireframe's (R05.T7.c's three tolerances). */
const STYLES: ReadonlyArray<Style> = [
  ...(["high", "low"] as const).map((setting) => ({
    setting,
    tauPx: SETTINGS[setting].terrain.tauPx,
    maxPatches: Math.floor(terrainSlotLayout(SETTINGS[setting].terrain, "lit").slotCount / 2),
  })),
  {
    setting: "high",
    tauPx: WIREFRAME_TERRAIN_TAU_PX,
    maxPatches: Math.floor(WIREFRAME_TERRAIN_SLOTS / 2),
  },
];

type FixtureWorld = (typeof fixture.worlds)[number];

/** A sample world as selection reads it, with T4.a's per-patch counts. */
interface SampleWorld {
  readonly name: string;
  readonly figure: BodyFigure;
  readonly finestLevel: number;
  readonly completeLevel: number;
  readonly table: Float64Array;
  readonly sigmaM: Float64Array;
  /** T4.a's patches sampled over every level, and those whose maximum passed 4σ_body. */
  readonly patches: number;
  readonly pastBodySigma: number;
}

/** The worker's slack on a held elevation, metres: a micrometre and a part in 10¹². */
function slackM(x: number): number {
  return 1e-6 + 1e-12 * Math.abs(x);
}

function sampleWorld(world: FixtureWorld): SampleWorld {
  const a = world.radius_m;
  const complete = world.complete_level;
  const row = (level: number): FixtureWorld["levels"][number] => {
    const found = world.levels[Math.min(level, complete)];
    if (found === undefined) {
      throw new Error(`${world.name} has no level ${String(level)}`);
    }
    return found;
  };
  const baseLowM = world.elevation_lowest_m - slackM(world.elevation_lowest_m);
  const baseHighM = world.elevation_highest_m + slackM(world.elevation_highest_m);
  const table = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
  const sigmaM = new Float64Array(LEVEL_SIGMA_LENGTH);
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const own = row(level);
    const parent = row(Math.max(0, level - 1));
    const lowM = Math.min(own.contribution_low_m, parent.contribution_low_m);
    const highM = Math.max(own.contribution_high_m, parent.contribution_high_m);
    table.set(
      [
        own.bound_m,
        Math.min(baseLowM + lowM, 0),
        Math.max(baseHighM + highM, 0),
        vertexSpacing(a, level).maxM,
      ],
      LEVEL_TABLE_STRIDE * level,
    );
    sigmaM[level] = own.sigma_m;
  }
  let patches = 0;
  let pastBodySigma = 0;
  for (const measured of world.measured) {
    patches += measured.patches;
    pastBodySigma += measured.patches_over_4sigma_body;
  }
  return {
    name: world.name,
    figure: { equatorialRadiusM: a, polarRadiusM: a, pole: null },
    finestLevel: world.finest_level,
    completeLevel: complete,
    table,
    sigmaM,
    patches,
    pastBodySigma,
  };
}

const WORLDS: ReadonlyArray<SampleWorld> = fixture.worlds.map(sampleWorld);

/**
 * The worlds the unbudgeted tests select on: all but the locked Earth-like, whose bounds are the
 * Earth-like's to a few per cent and whose selections, among the dearest, add nothing to them.
 */
const UNBUDGETED_WORLDS: ReadonlyArray<SampleWorld> = WORLDS.filter(
  (world) => world.name !== "locked_earth_like",
);

function planetOf(world: SampleWorld, rule: SelectionRule): PlanetGeometry {
  return planetGeometry(world.figure, world.table, world.sigmaM, rule);
}

/**
 * A view an eighth of {@link HD}'s width, for the tests that select with no budget: ρ goes as the
 * width over τ, so what they test holds alike, and on the hard bound, which is 13 to 200 times
 * loose (R10.T4.a), an unbudgeted selection runs to some 50,000 patches from orbit on the Earths
 * at a quarter of the width, and past 16,384 at the whole (R10.T4.b's record).
 */
const EIGHTH = { widthPx: 240, heightPx: 135 } as const;

/** One view of the descent's pose at `tS`, at tolerance `tauPx`, on `viewport`. */
function descentView(
  profile: DescentProfile,
  tS: number,
  tauPx: number,
  viewport: ViewSelectionInput["viewport"] = HD,
): ViewSelectionInput {
  const pose = profile.poseAt(tS);
  return {
    camera: { positionM: pose.positionM, orientation: pose.orientation },
    fovXRad: FOV_X_RAD,
    viewport,
    weight: 1,
    tauPx,
  };
}

function profileOf(world: SampleWorld): DescentProfile {
  return new DescentProfile(world.figure, landingSiteOf(RECORD_SEED));
}

/**
 * The limit of the unbudgeted tests, ms. Their selections, CPU-bound, run to some 12,000 patches on
 * the hard bound: the dearest took 4.5 s in the terrain directory's run at 4 workers (M2) and 5.0 s
 * with a second run beside it (M3), on the desktop, 2026-10-26. Ten times M2, rounded up to 60 s
 * (the config's rule).
 */
const UNBUDGETED_LIMIT_MS = 60_000;

/** The descent's poses the unbudgeted tests look from: the arc at about 300 km and 170 km. */
const LOOK_TIMES_S: ReadonlyArray<number> = [300, 600];

/**
 * ρ of `level`'s selection error at the nearest point of `bounds` from `view`, px: a patch's
 * unbaked box, as selection builds it from its level's subtree range.
 */
function rhoPx(
  planet: PlanetGeometry,
  level: number,
  bounds: PatchBounds,
  view: ViewSelectionInput,
): number {
  const box = relativeBounds(bounds, view.camera.positionM);
  return screenSpaceErrorPx(selectionErrorM(planet, level), distanceToBoxM(box), view);
}

/** Whether `view` sees the patch of `bounds`, by its frustum and horizon. */
function sees(planet: PlanetGeometry, bounds: PatchBounds, view: ViewSelectionInput): boolean {
  const rel = relativeBounds(bounds, view.camera.positionM);
  const frustum = frustumOf({
    orientation: view.camera.orientation,
    fovXRad: view.fovXRad,
    viewport: view.viewport,
  });
  return inFrustum(rel, frustum) && aboveHorizon(rel, horizonCone(planet, view.camera.positionM));
}

/** `key`'s ancestor at `level`, at most its own. */
function ancestorAt(key: PatchKey, level: number): PatchKey {
  const shift = key.level - level;
  return {
    face: key.face,
    level,
    i: Math.floor(key.i / 2 ** shift),
    j: Math.floor(key.j / 2 ** shift),
  };
}

/**
 * For each level L, the keys at L of the selected patches' ancestors at L that hold a selected
 * patch at L + 2 or finer: a patch at L beside one of them is split by the 2:1 balance alone.
 */
function deepAncestors(selection: Selection): ReadonlyArray<ReadonlySet<string>> {
  const deep = Array.from({ length: MAX_LEVEL + 1 }, () => new Set<string>());
  for (const p of selection.patches.values()) {
    for (let level = 0; level + 2 <= p.key.level; level += 1) {
      deep[level]?.add(patchKeyString(ancestorAt(p.key, level)));
    }
  }
  return deep;
}

/** Whether the 2:1 balance alone splits `key`: an edge or corner neighbour holds a patch two levels finer. */
function balanceSplits(deep: ReadonlyArray<ReadonlySet<string>>, key: PatchKey): boolean {
  const near = deep[key.level];
  if (near === undefined) {
    return false;
  }
  const neighbours = [
    ...EDGES.map((edge) => edgeNeighbour(key, edge)),
    ...cornerNeighbours(key).filter((k): k is PatchKey => k !== null),
  ];
  return neighbours.some((k) => near.has(patchKeyString(k)));
}

describe("the selection bound (R10.T4.b)", () => {
  it("is the stated bound under the hard rule, and min(ε, 4σ) under the statistical one", () => {
    expect(selectionBound(100, 10, "hard")).toBe(100);
    expect(selectionBound(100, 10, "statistical")).toBe(40);
    expect(selectionBound(100, 30, "statistical")).toBe(100);
    expect(SIGMA_MULTIPLE).toBe(4);
  });

  it("keeps the stated bound where σ is zero, under either rule", () => {
    expect(RULES.map((rule) => selectionBound(7, 0, rule))).toEqual([7, 7]);
  });

  it("is under the rule in force unless a rule is given", () => {
    expect(selectionBound(100, 10)).toBe(selectionBound(100, 10, SELECTION_RULE));
  });

  it("charges selection the rule's bound and leaves the stated one to every bounding volume", () => {
    const world = WORLDS[0];
    if (world === undefined) {
      throw new Error("the fixture has no world");
    }
    const hard = planetOf(world, "hard");
    const statistical = planetOf(world, "statistical");
    const unruled = planetGeometry(world.figure, world.table);
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      const epsilon = world.table[LEVEL_TABLE_STRIDE * level] ?? Number.NaN;
      const sigma = world.sigmaM[level] ?? Number.NaN;
      expect([
        selectionBoundM(hard, level),
        selectionBoundM(statistical, level),
        selectionBoundM(unruled, level),
        levelBoundM(statistical, level),
      ]).toEqual([epsilon, sigma > 0 ? Math.min(epsilon, 4 * sigma) : epsilon, epsilon, epsilon]);
    }
    expect(statistical.rule).toBe("statistical");
    expect(unruled.rule).toBe(SELECTION_RULE);
    expect(subtreeHeightRangeM(statistical, 3)).toEqual(subtreeHeightRangeM(hard, 3));
  });

  it("is what selection's error charges a level, beside the chords' sagitta", () => {
    const world = WORLDS[1];
    if (world === undefined) {
      throw new Error("the fixture has a second world");
    }
    const lower: number[] = [];
    for (const rule of RULES) {
      const planet = planetOf(world, rule);
      for (let level = 0; level < planet.finestLevel; level += 1) {
        expect(selectionErrorM(planet, level)).toBe(
          selectionBoundM(planet, level) + chordSagittaM(planet, level),
        );
        if (rule === "statistical" && selectionBoundM(planet, level) < levelBoundM(planet, level)) {
          lower.push(level);
        }
      }
    }
    // min(ε, 4σ) is below ε at every level that omits anything: the rule reaches the error.
    expect(lower).toEqual(Array.from({ length: world.completeLevel }, (_, level) => level));
  });

  it("charges no bound with no table, σ or not", () => {
    const world = WORLDS[0];
    if (world === undefined) {
      throw new Error("the fixture has no world");
    }
    for (const sigma of [null, world.sigmaM]) {
      expect(selectionBoundM(planetGeometry(world.figure, null, sigma, "statistical"), 0)).toBe(0);
    }
  });

  it("refuses a σ by level of the wrong length, not finite, or negative", () => {
    const world = WORLDS[0];
    if (world === undefined) {
      throw new Error("the fixture has no world");
    }
    for (const sigma of [
      new Float64Array(24),
      Float64Array.from({ length: 25 }, (_, n) => (n === 3 ? Number.NaN : 1)),
      Float64Array.from({ length: 25 }, (_, n) => (n === 3 ? -1 : 1)),
    ]) {
      expect(() => planetGeometry(world.figure, world.table, sigma)).toThrow(/σ by level/);
    }
  });
});

describe("selection over the sample worlds' bounds (T4.a's fixture)", () => {
  it("holds six worlds whose bound and σ are positive exactly below the complete level", () => {
    expect(WORLDS.map((w) => w.name)).toEqual([
      "earth_like",
      "mars_like",
      "moon_like",
      "ceres_like",
      "europa_like",
      "locked_earth_like",
    ]);
    for (const world of WORLDS) {
      const planet = planetOf(world, "hard");
      expect(planet.finestLevel).toBe(world.finestLevel);
      for (let level = 0; level <= MAX_LEVEL; level += 1) {
        const positive = level < world.completeLevel;
        expect([levelBoundM(planet, level) > 0, (world.sigmaM[level] ?? 0) > 0]).toEqual([
          positive,
          positive,
        ]);
      }
    }
  });

  it.each(RULES.flatMap((rule) => STYLE_TAUS.map((tauPx) => ({ rule, tauPx }))))(
    "selects a level exactly where its selection error subtends at most τ: $rule rule, $tauPx px",
    ({ rule, tauPx }) => {
      const over: string[] = [];
      const unwanted: string[] = [];
      let patches = 0;
      for (const world of UNBUDGETED_WORLDS) {
        const planet = planetOf(world, rule);
        const profile = profileOf(world);
        for (const tS of LOOK_TIMES_S) {
          const view = descentView(profile, tS, tauPx, EIGHTH);
          const selection = selectPatches({ planet, views: [view], setting: "high", grounded: [] });
          patches += selection.patches.size;
          const deep = deepAncestors(selection);
          // Siblings share a parent: each parent's ρ once.
          const parents = new Map<string, number>();
          for (const p of selection.patches.values()) {
            const at = `${world.name} ${String(tS)} s ${patchKeyString(p.key)}`;
            if (p.key.level < planet.finestLevel && sees(planet, p.bounds, view)) {
              const rho = rhoPx(planet, p.key.level, p.bounds, view);
              if (rho > tauPx) {
                over.push(`${at}: ${String(rho)} px`);
              }
            }
            const parent = parentKey(p.key);
            if (parent === null) {
              continue;
            }
            const name = patchKeyString(parent);
            let rho = parents.get(name);
            if (rho === undefined) {
              const box = patchBounds(planet, parent, subtreeHeightRangeM(planet, parent.level));
              rho = rhoPx(planet, parent.level, box, view);
              parents.set(name, rho);
            }
            if (!(rho > tauPx) && !balanceSplits(deep, parent)) {
              unwanted.push(`${at}: its parent at ${String(rho)} px`);
            }
          }
        }
      }
      expect(patches).toBeGreaterThan(100);
      expect({ over, unwanted }).toEqual({ over: [], unwanted: [] });
    },
    UNBUDGETED_LIMIT_MS,
  );

  // Its error is never larger, so it splits only what the hard rule splits. A leaf of its own can
  // still lie where the hard rule draws nothing: a patch seen whose children, split, are all
  // below the horizon.
  it(
    "never selects a finer level under the statistical rule than under the hard one",
    () => {
      const finer: string[] = [];
      for (const world of UNBUDGETED_WORLDS) {
        const profile = profileOf(world);
        for (const tS of LOOK_TIMES_S) {
          const [hard, statistical] = RULES.map((rule) =>
            selectPatches({
              planet: planetOf(world, rule),
              views: [descentView(profile, tS, 1, EIGHTH)],
              setting: "high",
              grounded: [],
            }),
          );
          if (hard === undefined || statistical === undefined) {
            throw new Error("two rules, two selections");
          }
          for (const p of statistical.patches.values()) {
            for (let k = parentKey(p.key); k !== null; k = parentKey(k)) {
              if (hard.patches.has(patchKeyString(k))) {
                finer.push(`${world.name} ${String(tS)} s ${patchKeyString(p.key)}`);
              }
            }
          }
        }
      }
      expect(finer).toEqual([]);
    },
    UNBUDGETED_LIMIT_MS,
  );

  it(
    "gives the same selection for views in any order, called twice or after other calls",
    () => {
      for (const rule of RULES) {
        const world = WORLDS[1];
        if (world === undefined) {
          throw new Error("the fixture has a second world");
        }
        const planet = planetOf(world, rule);
        const profile = profileOf(world);
        const a = descentView(profile, 600, 1, EIGHTH);
        const b = { ...descentView(profile, 300, 1, EIGHTH), weight: 0.25 };
        const select = (views: ViewSelectionInput[]): string[] => [
          ...selectPatches({ planet, views, setting: "high", grounded: [] }).patches.keys(),
        ];
        const first = select([a, b]);
        const again = select([a, b]);
        const swapped = select([b, a]);
        // Calls on another planet and from another pose in between change nothing.
        selectPatches({
          planet: planetOf(world, "hard"),
          views: [a],
          setting: "low",
          grounded: [],
        });
        select([descentView(profile, 0, 4, EIGHTH)]);
        const after = select([a, b]);
        expect(first.length).toBeGreaterThan(50);
        expect({ again, swapped, after }).toEqual({ again: first, swapped: first, after: first });
      }
    },
    UNBUDGETED_LIMIT_MS,
  );

  it("draws the ground under a contact at the finest level on every world, setting and style", () => {
    const flat: HeightRangeLookup = { heightRangeM: () => [-1, 1] };
    const missed: string[] = [];
    for (const world of WORLDS) {
      const finest = world.finestLevel;
      const half = 2 ** (finest - 1);
      const site: PatchKey = { face: 0, level: finest, i: half + 1, j: half + 3 };
      const ground = vertexDir(site, 32, 32);
      const [cx, cy, cz] = surfacePoint(world.figure, ground, 0);
      const contact: GroundContact = { positionM: vec3(cx, cy, cz), radiusM: 20 };
      const [x, y, z] = surfacePoint(world.figure, ground, 2_000);
      const positionM = vec3(x, y, z);
      for (const rule of RULES) {
        for (const { setting, tauPx, maxPatches } of STYLES) {
          const selection = selectPatches({
            planet: planetOf(world, rule),
            views: [
              {
                camera: { positionM, orientation: lookingDown(positionM) },
                fovXRad: FOV_X_RAD,
                viewport: HD,
                weight: 1,
                tauPx,
              },
            ],
            setting,
            grounded: [contact],
            heightRanges: flat,
            maxPatches,
          });
          if (coveringLevel(selection, site) !== finest) {
            missed.push(`${world.name} ${rule} ${setting} ${String(tauPx)} px`);
          }
        }
      }
    }
    expect(missed).toEqual([]);
  });
});

/** The level of the selected patch covering `key`'s place, or −1 where none does. */
function coveringLevel(selection: Selection, key: PatchKey): number {
  for (let k: PatchKey | null = key; k !== null; k = parentKey(k)) {
    if (selection.patches.has(patchKeyString(k))) {
      return k.level;
    }
  }
  return -1;
}

/** A camera at `positionM` looking straight down. */
function lookingDown(positionM: Vec3): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  return lookAlong(vec3(-up.x, -up.y, -up.z), east);
}

describe("signoff-12.md item 30's rule on T4.a's figures", () => {
  // The statistical rule needs both conditions on every world: over R05's scripted descent at
  // τ = 1 px it selects at most two thirds of the hard rule's patches (R10.T4.b's record counts
  // them: 1.9 to 19 times fewer, at least), and T4.a's per-patch maxima pass 4σ in at most one patch
  // in three. The second, from the fixture alone, fails, which settles the rule; a fixture on which
  // it held would fail this test, for the counts to be taken again and the rule applied anew.
  it("keeps the hard bound, the per-patch maxima passing 4σ in more than one patch in three", () => {
    // Four of the six at generator version 23: the Mars-, Moon-, Ceres- and Europa-like worlds.
    const past = WORLDS.filter((world) => world.pastBodySigma > world.patches / 3);
    expect(past.length).toBeGreaterThan(0);
    expect(SELECTION_RULE).toBe("hard");
  });
});
