/**
 * The hand-over's brightness on a generated world (plan R10, R10.T10.e; Design notes 8 and 13), on
 * the real module: the scripted stand-in for the task's check by hand. The Moon-like fixture's
 * surveys (`hyperion_surface`'s `tests/golden/worker/moon_like.golden`, every block, eight cells of
 * level 6) are held by a worker's fields through its own answers; its class map (R10.T10.d) gives
 * the disc's twin its shares, and patches over the surveyed cells, baked from the root face to one
 * level finer than the field, give the terrain's twin its weights. At 30° and 90° of phase, the
 * sun and the camera far off in the plane of the survey's mean normal, each twin's brightness, I/F
 * weighted by μ over a 16 × 16 grid of points a texel on the spheroid's normal, is averaged per
 * class-map texel and over the survey, and the terrain's is set against the disc's in stops.
 *
 * @remarks
 * The fixture's surveyed ground is one law: every material it names there resolves to A 0.20 at
 * L(0) 1 on the Regolith row, so the twins agree exactly. The second case gives the one material
 * the bakes weigh but the map does not (palette pair (0, 0), up to 16% of a level-7 vertex's
 * weight) a law of its own, half as bright under Lambert's law, on both sides: the brightness the
 * map's band-level-0 weights would miss against the bakes' (R10.T10.d's deferred "Band level 0's
 * agreement is unmeasured").
 */

import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { moonLikeFixture } from "../../test/moonLikeFixture";
import { testPatienceMs } from "../../test/patience";
import { add, cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import type { PhotometricLaw } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { discReflectance } from "../bodies/discShading";
import { classMapSharesAt } from "../bodies/discSurface";
import type { Rgb } from "../photometry/toneCurve";
import {
  CLASS_MAP_FACE_TEXELS,
  CLASS_MAP_LEVEL,
  classMapLawsOf,
  classMapTexelsOf,
} from "./classMap";
import { fieldCellAt } from "./coverage";
import { faceUvToXyz, stToUv } from "./cube";
import type { Face, PatchKey } from "./patchKey";
import {
  classPhotometryOf,
  PHASE_LAW,
  TERRAIN_CLASSES,
  terrainReflectance,
  terrainShares,
  terrainTexelLaws,
  type TexelLaw,
  texelWeights,
} from "./terrainMaterial";
import { answerRequest, type HeightModule, WorkerFields } from "./workers/heightBake";
import {
  type BakedPatch,
  type BakeSettings,
  type ClassMapAnswer,
  type HeightWorkerRequest,
  PHOTOMETRY_STRIDE,
  type TerrainLayers,
} from "./workers/messages";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const module: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

/** The fixture's body, seed and every block, in its order. */
const FIXTURE = moonLikeFixture("all");

/**
 * The Moon-like world's c ÷ a (`hyperion_surface`'s `testing/worlds.rs`: a flattening of 0.0012),
 * for each law's figure scale on both sides.
 */
const POLAR_OVER_EQUATORIAL = 1 - 0.0012;

const SETTINGS: BakeSettings = {
  vertexPath: "face-differences",
  normals: "mesh",
  source: { kind: "body", body: FIXTURE.body, detailSeed: FIXTURE.seed, weights: "baked" },
};

/**
 * The patch levels compared: the root face, where the quadtree takes the ground over from the
 * mesh regime (Design note 13), to one finer than the field.
 */
const LEVELS = [0, 2, 4, 6, 7] as const;

/** Points along a texel's side. */
const GRID = 16;

/** The phases compared, degrees. */
const PHASES_DEG = [30, 90] as const;

/** A third of a stop. */
const THIRD_STOP = 1 / 3;

/** The fields holding every block. */
function heldFields(): WorkerFields {
  const fields = new WorkerFields(module);
  FIXTURE.blocks.forEach((block, n) => {
    const request: HeightWorkerRequest =
      n === 0
        ? { kind: "field-init", id: n, body: FIXTURE.body, block, form: "payload" }
        : { kind: "field-chunk", id: n, body: FIXTURE.body, block };
    const { reply } = answerRequest(module, request, fields);
    if (reply.kind !== "field-held") {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
  });
  return fields;
}

/** A point of a face at its (s, t) cell coordinates, as a unit direction. */
function directionAt(face: Face, s: number, t: number): Vec3 {
  const [x, y, z] = faceUvToXyz(face, stToUv(s), stToUv(t));
  return normalise(vec3(x, y, z));
}

/** The spheroid's outward normal over the unit direction d (R05's M d, the pole z). */
function normalOver(d: Vec3): Vec3 {
  return normalise(vec3(d.x, d.y, d.z / POLAR_OVER_EQUATORIAL));
}

/** One region's brightness sums by twin, per channel. */
interface Sums {
  readonly disc: [number, number, number];
  readonly terrain: [number, number, number];
}

function emptySums(): Sums {
  return { disc: [0, 0, 0], terrain: [0, 0, 0] };
}

/** The terrain's mean over the disc's, in stops, per channel. */
function stops(sums: Sums): Rgb {
  const { disc, terrain } = sums;
  return [
    Math.log2(terrain[0] / disc[0]),
    Math.log2(terrain[1] / disc[1]),
    Math.log2(terrain[2] / disc[2]),
  ];
}

/** A palette pair's photometry replaced, `[B, V, R, law, L(0)]`, on both sides. */
interface Override {
  readonly code: number;
  readonly entry: number;
  readonly photometry: readonly [number, number, number, number, number];
}

/** `photometry` with each slot whose pair is `override`'s replaced, two bytes a slot in `pairs`. */
function overridden(
  photometry: Float32Array,
  pairs: Uint8Array,
  override: Override | null,
): Float32Array {
  if (override === null) {
    return photometry;
  }
  const out = photometry.slice();
  for (let k = 0; 2 * k + 1 < pairs.length; k += 1) {
    if (pairs[2 * k] === override.code && pairs[2 * k + 1] === override.entry) {
      out.set(override.photometry, PHOTOMETRY_STRIDE * k);
    }
  }
  return out;
}

/** A bake's laws as the terrain draws them, and which of its slots have one. */
interface DrawnBake {
  readonly laws: ReadonlyArray<TexelLaw | null>;
  readonly named: ReadonlyArray<boolean>;
}

/** The worst change in stops over the texels and over the survey, per phase and level. */
interface Comparison {
  readonly worstTexelStops: number;
  readonly worstSurveyStops: number;
  /** By phase then level: each texel's worst channel, and the survey's. */
  readonly rows: ReadonlyArray<string>;
}

describe("the hand-over's brightness on the Moon-like world (R10.T10.e, scripted)", () => {
  let map: ClassMapAnswer;
  let bakes: Map<string, BakedPatch>;

  // The hook's work is the test's subject, and fixed: the module's start, the fixture's every
  // block held, its class map asked for, and every patch over the surveyed texels baked at each of
  // the five levels. It took 5.6 s on the quiet 16-thread desktop, and 11.4 s in another lane's run
  // on the 8-thread laptop beside a 4-thread reference trace (both 2026-10-21). On that laptop
  // beside the same trace, run with `patience.test.ts` and `exomolLines.test.ts` at 4 workers
  // (those three files, not the suite; 2026-10-21), it took 7.8 s (M2, 1-minute load 4.5 to 4.7)
  // and, with a second such run beside it, 10.1 s and 9.9 s (M3, load 4.7 to 5.1), the first past
  // vitest's 10 s hook limit. Its limit is the harness's patience, 180 s unless
  // `HYPERION_TEST_PATIENCE_SECS` sets it, 23 times M2: the work is fixed and the machines it runs
  // on are not, so the bound is the one a slower or busier machine raises, not a figure rounded
  // from the machines it was measured on. The tests keep the 15 s default.
  beforeAll(() => {
    initSync({ module: wasmBytes() });
    const fields = heldFields();
    const { reply } = answerRequest(
      module,
      { kind: "class-map", id: 1_000, body: FIXTURE.body, detailSeed: FIXTURE.seed },
      fields,
    );
    if (reply.kind !== "class-map" || reply.map === null) {
      throw new Error(`no class map: ${JSON.stringify(reply)}`);
    }
    map = reply.map;
    // Every patch at each level over the surveyed texels.
    const keys = new Map<string, PatchKey>();
    for (const index of map.texels) {
      const cell = fieldCellAt(CLASS_MAP_LEVEL, index);
      for (const level of LEVELS) {
        const shift = CLASS_MAP_LEVEL - level;
        const span = shift >= 0 ? 1 : 2 ** -shift;
        for (let di = 0; di < span; di += 1) {
          for (let dj = 0; dj < span; dj += 1) {
            const key: PatchKey =
              shift >= 0
                ? { face: cell.face, level, i: cell.i >> shift, j: cell.j >> shift }
                : { face: cell.face, level, i: span * cell.i + di, j: span * cell.j + dj };
            keys.set(JSON.stringify(key), key);
          }
        }
      }
    }
    bakes = new Map();
    [...keys.values()].forEach((key, id) => {
      const { reply: baked } = answerRequest(
        module,
        { kind: "bake", id, key, generation: 1, settings: SETTINGS },
        fields,
      );
      if (baked.kind !== "baked") {
        throw new Error(`${JSON.stringify(key)} was not baked: ${JSON.stringify(baked)}`);
      }
      bakes.set(JSON.stringify(key), baked.bake);
    });
  }, testPatienceMs());

  /** Each twin over the survey, with `override` on both sides. */
  function compare(override: Override | null): Comparison {
    const photometry = overridden(map.photometry, map.classes, override);
    const texels = classMapTexelsOf(map);
    const laws = classMapLawsOf({ ...map, photometry });
    if (texels === null) {
      throw new Error("the map has classes");
    }
    // `elsewhere` at the survey's own p, as R10.T10.a's scale makes the classes' p the body's (the
    // fixture's header holds no scale, so c is 1), on Mercury's template, as the photometry draws
    // an airless rock.
    const classes = map.classes.length / 2;
    const p: [number, number, number] = [0, 0, 0];
    map.texels.forEach((_, t) => {
      laws.forEach((law, k) => {
        const w = (map.weights[t * classes + k] ?? 0) / 255 / map.texels.length;
        const l = law?.lommelSeeligerShare ?? 0;
        for (const c of [0, 1, 2] as const) {
          p[c] += w * (law?.a[c] ?? 0) * (l + (2 / 3) * (1 - l));
        }
      });
    });
    const elsewhere: PhotometricLaw = lawFor(p, [0.4798, 0.4798, 0.4798], "mercury");
    const discLaws = [elsewhere, ...laws.map((law) => law ?? elsewhere)];
    const n = CLASS_MAP_FACE_TEXELS;
    const centres = [...map.texels].map((index) => {
      const cell = fieldCellAt(CLASS_MAP_LEVEL, index);
      return directionAt(cell.face, (cell.i + 0.5) / n, (cell.j + 0.5) / n);
    });
    const mean = normalise(centres.reduce((sum, d) => add(sum, d), vec3(0, 0, 0)));
    const tangent = normalise(cross(mean, Math.abs(mean.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0)));
    // Each bake's laws as the terrain draws them, made once.
    const drawn = new Map<string, DrawnBake>();
    const drawnOf = (key: string, layers: TerrainLayers): DrawnBake => {
      let known = drawn.get(key);
      if (known === undefined) {
        const slots = overridden(layers.photometry, layers.palette, override);
        known = {
          laws: terrainTexelLaws(slots, elsewhere, POLAR_OVER_EQUATORIAL),
          named: Array.from(
            { length: TERRAIN_CLASSES },
            (_, k) => classPhotometryOf(slots, k) !== null,
          ),
        };
        drawn.set(key, known);
      }
      return known;
    };
    const rows: string[] = [];
    let worstTexelStops = 0;
    let worstSurveyStops = 0;
    for (const alphaDeg of PHASES_DEG) {
      const half = (alphaDeg * Math.PI) / 360;
      const toSun = add(scale(mean, Math.cos(half)), scale(tangent, Math.sin(half)));
      const toCamera = add(scale(mean, Math.cos(half)), scale(tangent, -Math.sin(half)));
      const alpha = 2 * half;
      for (const level of LEVELS) {
        const side = 2 ** level;
        const survey = emptySums();
        let texelWorst = 0;
        for (const index of map.texels) {
          const cell = fieldCellAt(CLASS_MAP_LEVEL, index);
          const sums = emptySums();
          for (let a = 0; a < GRID; a += 1) {
            for (let b = 0; b < GRID; b += 1) {
              const s = (cell.i + (a + 0.5) / GRID) / n;
              const t = (cell.j + (b + 0.5) / GRID) / n;
              const d = directionAt(cell.face, s, t);
              const normal = normalOver(d);
              const mu0 = dot(normal, toSun);
              const mu = dot(normal, toCamera);
              const disc = discReflectance(
                discLaws,
                POLAR_OVER_EQUATORIAL,
                classMapSharesAt(texels, [d.x, d.y, d.z]),
                mu0,
                mu,
                alpha,
              );
              const pi = Math.min(Math.floor(s * side), side - 1);
              const pj = Math.min(Math.floor(t * side), side - 1);
              const key = JSON.stringify({ face: cell.face, level, i: pi, j: pj });
              const layers = bakes.get(key)?.layers;
              if (layers === null || layers === undefined) {
                throw new Error(`no layers at level ${String(level)}`);
              }
              const { laws: terrainLaws, named } = drawnOf(key, layers);
              const terrain = terrainReflectance(
                terrainLaws,
                terrainShares(texelWeights(layers.weights, s * side - pi, t * side - pj), named),
                mu0,
                mu,
                alpha,
              );
              for (const c of [0, 1, 2] as const) {
                sums.disc[c] += disc[c] * mu;
                sums.terrain[c] += terrain[c] * mu;
                survey.disc[c] += disc[c] * mu;
                survey.terrain[c] += terrain[c] * mu;
              }
            }
          }
          texelWorst = Math.max(texelWorst, ...stops(sums).map(Math.abs));
        }
        const surveyWorst = Math.max(...stops(survey).map(Math.abs));
        worstTexelStops = Math.max(worstTexelStops, texelWorst);
        worstSurveyStops = Math.max(worstSurveyStops, surveyWorst);
        rows.push(
          `${String(alphaDeg)} deg, level ${String(level)}: texels ${texelWorst.toExponential(2)}, survey ${surveyWorst.toExponential(2)}`,
        );
      }
    }
    return { worstTexelStops, worstSurveyStops, rows };
  }

  it("holds the map's eight surveyed texels, one law over them all", () => {
    expect(map.texels).toHaveLength(8);
    const named = classMapLawsOf(map).filter((law) => law !== null);
    expect(named.map((law) => [law.a, law.lommelSeeligerShare])).toEqual([
      [[Math.fround(0.2), Math.fround(0.2), Math.fround(0.2)], 1],
    ]);
  });

  it("changes each texel's and the survey's mean brightness by under a third of a stop at 30° and 90°, at every level", () => {
    const { worstTexelStops, worstSurveyStops } = compare(null);
    // One law over the surveyed ground: the twins agree to rounding.
    expect(worstTexelStops).toBeLessThan(1e-9);
    expect(worstSurveyStops).toBeLessThan(1e-9);
  });

  it("keeps under a third of a stop where the material the map leaves out takes a law half as bright", () => {
    const { worstTexelStops, worstSurveyStops, rows } = compare({
      code: 0,
      entry: 0,
      photometry: [0.1, 0.1, 0.1, PHASE_LAW.lambert, 0],
    });
    expect(rows).toHaveLength(PHASES_DEG.length * LEVELS.length);
    expect(worstTexelStops).toBeGreaterThan(0);
    expect(worstTexelStops).toBeLessThan(THIRD_STOP);
    expect(worstSurveyStops).toBeLessThan(THIRD_STOP);
  });
});
