/**
 * Collision agrees with what is drawn, with rocks (plan R11, T4.c; R10.T15.a's test with T4.a's
 * rocks): the drawn surface, R05's finest mesh from the worker's bake and the patches' rock meshes
 * from the procedural builder, against the module's `surfaceHeightWithRocksM`, over the fixture
 * fields of `hyperion_surface`'s `tests/golden/worker/`.
 *
 * @remarks
 * The points are the Rust goldens' rule (`scatter/golden.rs`' `collision_<world>`) over the
 * middle 3 × 3 of a 5 × 5 block of finest patches the fixture surveys: each rock's axis, two points
 * inside its outline and one past it, then points drawn over the patches. The sampled worlds of the
 * Rust goldens are the crate's own fields, which the client has no payload of, so the points are
 * drawn here by the same rule on the client's fixtures (T4.c as built).
 *
 * For each point the test finds its foot d on the datum and intersects the line M·d + t ν(d) with
 * the drawn surface, every vertex relative to the middle patch's origin: the terrain's triangle by
 * R05's diagonal rule, from the bake's `f32` offsets q₀ (the `BakedOffsets` path), and every rock
 * triangle of the block's meshes, each vertex its grid coordinates times its page's step in `f32`
 * about the page's origin, as `rockPage.wgsl` forms it; the drawn height is the highest. It is
 * held to the module's height with rocks:
 *
 * - where no rock stands, within {@link BOUND_M}, R10.T15.a's bound;
 * - where a rock stands, from {@link BOUND_M} above it down to the rock's chordal tolerance below
 *   it, along the line, so over the cosine of the rock's slope there, with the grid's half step and
 *   the module's flat frame's bound ({@link contactBoundM}); points where the rock is steeper than
 *   {@link MIN_COSINE} are counted, not compared, since there the line runs along the surface.
 */

import { beforeAll, describe, expect, it } from "vitest";

import earthFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/earth_like.golden?raw";
import moonFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";
import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../../generated/surface/hyperion_surface_bg.wasm?inline";
import { add, cross, dot, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { meshPagesOf } from "../../../test/rockFixtures";
import { seededRandom } from "../../../test/seededRandom";
import {
  directFieldsOf,
  finestAbout,
  parseWorkerFixture,
  type WorkerFixture,
  workerFieldsOf,
} from "../../../test/workerFieldFixtures";
import {
  CLUSTER_TABLE_BYTES,
  pageClusterCorners,
  PAGE_VERTEX_BYTES,
} from "../../clusters/pageFormat";
import { faceUvToDir, finestLevel, stToUv, uvToSt, xyzToFaceUv } from "../../terrain/cube";
import type { PatchKey } from "../../terrain/patchKey";
import { type BodyFigure, spheroidNormal, spheroidPoint } from "../../terrain/planet";
import { PATCH_VERTICES_PER_SIDE } from "../../terrain/slotLayout";
import { answerRequest, type HeightModule } from "../../terrain/workers/heightBake";
import type { BakeSettings } from "../../terrain/workers/messages";
import { type Rock, type RockLayer, rocksOf } from "./rockLayer";
import { rockToleranceM } from "./rockMesh";
import { buildRockPages, placeInPatch } from "./rockPages";

/**
 * The bound where no rock stands, metres: R10.T15.a's, the height texture's `f32` step at ±20 km,
 * 1.95 mm, and R05.T4.b's formation error, under 1 mm.
 */
const BOUND_M = 0.003;

/**
 * The least cosine of a rock's slope at which a point on it is compared: 0.25, a slope of 76°.
 * Steeper, the line through the point runs nearly along the rock's side, where the mesh's chordal
 * depth is many times its own along the line.
 */
const MIN_COSINE = 0.25;

/** The points drawn over the middle patches beside the rocks' own, a world. */
const DRAWN_POINTS = 400;

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

/** The settings of the high setting's bake: baked offsets, no weights, horizon or normals. */
function settingsOf(fixture: WorkerFixture): BakeSettings {
  return {
    vertexPath: "baked-offsets",
    normals: "mesh",
    source: {
      kind: "body",
      body: fixture.body,
      detailSeed: fixture.seed,
      weights: "skipped",
      horizon: "skipped",
      normals: "skipped",
    },
  };
}

/** The patch of `level` holding the unit direction `dir`, as the surface crate's `PatchKey::containing`. */
function patchContaining(level: number, dir: Vec3): PatchKey {
  const { face, u, v } = xyzToFaceUv([dir.x, dir.y, dir.z]);
  const side = 2 ** level;
  const cell = (st: number): number => Math.min(side - 1, Math.max(0, Math.floor(st * side)));
  return { face, level, i: cell(uvToSt(u)), j: cell(uvToSt(v)) };
}

function keyName(key: PatchKey): string {
  return `${String(key.face)}/${String(key.level)}/${String(key.i)}/${String(key.j)}`;
}

function datumPointOf(figure: BodyFigure, dir: Vec3): Vec3 {
  const [x, y, z] = spheroidPoint(figure, [dir.x, dir.y, dir.z]);
  return vec3(x, y, z);
}

function normalOf(figure: BodyFigure, dir: Vec3): Vec3 {
  const [x, y, z] = spheroidNormal(figure, [dir.x, dir.y, dir.z]);
  return vec3(x, y, z);
}

/**
 * The foot on the datum of the body-fixed point `p`: the unit direction d whose datum point's
 * normal passes through `p`, by fixed-point steps from p's own, each a few orders closer.
 */
function datumFoot(figure: BodyFigure, p: Vec3): Vec3 {
  const { equatorialRadiusM: a, polarRadiusM: c } = figure;
  let d = normalise(vec3(p.x / a, p.y / a, p.z / c));
  for (let step = 0; step < 8; step += 1) {
    const nu = normalOf(figure, d);
    const q = sub(p, scale(nu, dot(sub(p, datumPointOf(figure, d)), nu)));
    d = normalise(vec3(q.x / a, q.y / a, q.z / c));
  }
  return d;
}

/**
 * The parameter t at which the line `from` + t `along` meets the triangle (`a`, `b`, `c`), or
 * `null` where it misses it: Möller and Trumbore's test, on the whole line.
 */
function lineMeetsTriangle(from: Vec3, along: Vec3, a: Vec3, b: Vec3, c: Vec3): number | null {
  const e1 = sub(b, a);
  const e2 = sub(c, a);
  const h = cross(along, e2);
  const det = dot(e1, h);
  if (Math.abs(det) < 1e-18) {
    return null;
  }
  const s = sub(from, a);
  const u = dot(s, h) / det;
  if (u < 0 || u > 1) {
    return null;
  }
  const q = cross(s, e1);
  const v = dot(along, q) / det;
  if (v < 0 || u + v > 1) {
    return null;
  }
  return dot(e2, q) / det;
}

/** A rock's drawn triangles, nine coordinates a triangle about the reference origin, and its sphere. */
interface DrawnRock {
  readonly rock: Rock;
  readonly centreM: Vec3;
  readonly radiusM: number;
  readonly cornersM: Float64Array;
  /** Half the diagonal of its pages' grid step, m. */
  readonly halfStepM: number;
}

/** A baked patch as it is drawn: its terrain's offsets and its rocks, about the reference origin. */
interface DrawnPatch {
  readonly key: PatchKey;
  /** Its origin less the reference origin, m. */
  readonly shiftM: Vec3;
  readonly offsets: Float32Array;
  readonly rocks: ReadonlyArray<Rock>;
}

/** Vertex (`x`, `y`)'s q₀ of `patch`, about the reference origin. */
function terrainVertex(patch: DrawnPatch, x: number, y: number): Vec3 {
  const at = 6 * (PATCH_VERTICES_PER_SIDE * y + x);
  const q = (k: number): number => patch.offsets[at + k] ?? Number.NaN;
  return add(patch.shiftM, vec3(q(0), q(1), q(2)));
}

/**
 * The drawn terrain's height along the line `from` + t `along` whose foot is `foot`: its meeting
 * with the plane of the quad's half R05's rule picks (`hyperion_surface::patch::collision`).
 */
function drawnTerrainT(patch: DrawnPatch, foot: Vec3, from: Vec3, along: Vec3): number {
  const [px, py] = placeInPatch(patch.key, foot);
  const quads = PATCH_VERTICES_PER_SIDE - 1;
  const x = Math.min(quads - 1, Math.max(0, Math.floor(quads * px)));
  const y = Math.min(quads - 1, Math.max(0, Math.floor(quads * py)));
  const [fa, fb] = [quads * px - x, quads * py - y];
  const corners =
    fa < fb
      ? [
          terrainVertex(patch, x, y),
          terrainVertex(patch, x + 1, y + 1),
          terrainVertex(patch, x, y + 1),
        ]
      : [
          terrainVertex(patch, x, y),
          terrainVertex(patch, x + 1, y),
          terrainVertex(patch, x + 1, y + 1),
        ];
  const [a = vec3(0, 0, 0), b = a, c = a] = corners;
  const n = cross(sub(b, a), sub(c, a));
  return dot(n, sub(a, from)) / dot(n, along);
}

/** The highest meeting of the line with `rock`'s drawn triangles, or `null` where it misses them. */
function drawnRockT(rock: DrawnRock, from: Vec3, along: Vec3): number | null {
  const offset = sub(rock.centreM, from);
  const past = sub(offset, scale(along, dot(offset, along)));
  if (dot(past, past) > rock.radiusM * rock.radiusM) {
    return null;
  }
  const corner = (k: number): Vec3 =>
    vec3(rock.cornersM[k] ?? 0, rock.cornersM[k + 1] ?? 0, rock.cornersM[k + 2] ?? 0);
  let highest: number | null = null;
  for (let k = 0; k < rock.cornersM.length; k += 9) {
    const t = lineMeetsTriangle(from, along, corner(k), corner(k + 3), corner(k + 6));
    if (t !== null && (highest === null || t > highest)) {
      highest = t;
    }
  }
  return highest;
}

/** `rock`'s top along the datum's normal at `foot`, m above the datum, in its flat frame (`shape.rs`' `top_at_m`), and the cosine of its slope there. */
function analyticTop(
  figure: BodyFigure,
  rock: Rock,
  foot: Vec3,
): { readonly topM: number; readonly cosine: number } | null {
  const q = add(datumPointOf(figure, foot), scale(normalOf(figure, foot), rock.groundHeightM));
  const delta = sub(q, rock.baseCentreM);
  const [ea, eb, up] = rock.axes;
  const [x, y, z] = [dot(delta, ea), dot(delta, eb), dot(delta, up)];
  const [a, b, c] = rock.semiAxesM;
  const n = rock.exponent;
  const s = (Math.abs(x) / a) ** n + (Math.abs(y) / b) ** n;
  if (s >= 1) {
    return null;
  }
  const top = c * (1 - s) ** (1 / n);
  const g = [
    (Math.sign(x) * (Math.abs(x) / a) ** (n - 1)) / a,
    (Math.sign(y) * (Math.abs(y) / b) ** (n - 1)) / b,
    (top / c) ** (n - 1) / c,
  ] as const;
  return { topM: rock.groundHeightM + top - z, cosine: g[2] / Math.hypot(...g) };
}

/**
 * The module's flat frame's bound on a rock's top, m (`shape.rs`' `contact_bound`): 2 c α, α the
 * rock's reach over the datum's least radius of curvature.
 */
function contactBoundM(figure: BodyFigure, rock: Rock): number {
  const { equatorialRadiusM: e, polarRadiusM: p } = figure;
  const rho = Math.min(e, p) ** 2 / Math.max(e, p);
  const [a, b, c] = rock.semiAxesM;
  return (2 * c * Math.hypot(a, b)) / rho;
}

/** The drawn rocks of `patch`'s mesh, every vertex about the reference origin. */
function drawnRocksOf(patch: DrawnPatch, originM: Vec3, layer: RockLayer): DrawnRock[] {
  const layerRocks = patch.rocks;
  const built = buildRockPages({ key: patch.key, originM, layer });
  if (built === null) {
    return [];
  }
  const corners: number[][] = layerRocks.map(() => []);
  const steps: number[] = layerRocks.map(() => 0);
  for (const page of meshPagesOf(built.file)) {
    const view = new DataView(page.body);
    for (let c = 0; c < page.clusters; c += 1) {
      const first = view.getUint32(page.sections.table + CLUSTER_TABLE_BYTES * c, true);
      const n = view.getUint32(page.sections.vertices + PAGE_VERTEX_BYTES * first + 12, true);
      const into = corners[n];
      if (into === undefined) {
        throw new Error(`a cluster names rock ${String(n)} of ${String(layerRocks.length)}`);
      }
      steps[n] = Math.max(steps[n] ?? 0, page.stepM);
      const grid = pageClusterCorners(page, c);
      for (let k = 0; k < grid.length; k += 3) {
        into.push(
          patch.shiftM.x + page.originM.x + Math.fround((grid[k] ?? 0) * page.stepM),
          patch.shiftM.y + page.originM.y + Math.fround((grid[k + 1] ?? 0) * page.stepM),
          patch.shiftM.z + page.originM.z + Math.fround((grid[k + 2] ?? 0) * page.stepM),
        );
      }
    }
  }
  return layerRocks.map((rock, n) => {
    const [a, b, c] = rock.semiAxesM;
    return {
      rock,
      centreM: add(patch.shiftM, sub(rock.baseCentreM, originM)),
      radiusM: Math.hypot(a, b, c),
      cornersM: Float64Array.from(corners[n] ?? []),
      halfStepM: (Math.sqrt(3) / 2) * (steps[n] ?? 0),
    };
  });
}

/** A world's 5 × 5 block, baked and drawn. */
interface DrawnBlock {
  readonly figure: BodyFigure;
  readonly finest: number;
  readonly referenceM: Vec3;
  readonly patches: ReadonlyMap<string, DrawnPatch>;
  readonly middle: ReadonlyArray<DrawnPatch>;
  readonly rocks: ReadonlyArray<DrawnRock>;
}

function drawnBlock(fixture: WorkerFixture, figure: BodyFigure): DrawnBlock {
  const fields = workerFieldsOf(module, fixture);
  const finest = finestLevel(figure.equatorialRadiusM);
  let referenceM: Vec3 | null = null;
  const patches = new Map<string, DrawnPatch>();
  const middle: DrawnPatch[] = [];
  const rocks: DrawnRock[] = [];
  // The middle patch first, whose origin is the reference.
  const steps = [12, ...Array.from({ length: 25 }, (_, s) => s).filter((s) => s !== 12)];
  for (const step of steps) {
    const key = finestAbout(fixture, finest, step);
    const { reply } = answerRequest(
      module,
      { kind: "bake", id: 100 + step, key, generation: 1, settings: settingsOf(fixture) },
      fields,
    );
    if (reply.kind !== "baked") {
      throw new Error(`patch ${keyName(key)} was not baked: ${reply.kind}`);
    }
    const { originM: o, offsets, layers } = reply.bake;
    const originM = vec3(o.x, o.y, o.z);
    const rockLayer = layers?.rocks;
    if (offsets === null || rockLayer === undefined) {
      throw new Error("a finest patch's bake on the baked-offsets path carries offsets and rocks");
    }
    referenceM ??= originM;
    const patch: DrawnPatch = {
      key,
      shiftM: sub(originM, referenceM),
      offsets,
      rocks: rocksOf(rockLayer),
    };
    patches.set(keyName(key), patch);
    const [di, dj] = [(step % 5) - 2, Math.floor(step / 5) - 2];
    if (Math.abs(di) <= 1 && Math.abs(dj) <= 1) {
      middle.push(patch);
    }
    rocks.push(...drawnRocksOf(patch, originM, rockLayer));
  }
  fields.free();
  if (referenceM === null) {
    throw new Error("the block holds its middle patch");
  }
  return { figure, finest, referenceM, patches, middle, rocks };
}

/** The Rust goldens' points over the middle patches (see the module's remarks), body-fixed m. */
function pointsOver(block: DrawnBlock, seed: number): Vec3[] {
  const points: Vec3[] = [];
  for (const patch of block.middle) {
    for (const rock of patch.rocks) {
      const o = rock.baseCentreM;
      const up = normalOf(block.figure, rock.dir);
      const t1 = normalise(vec3(-up.y, up.x, 0));
      const t2 = cross(up, t1);
      const r = Math.hypot(rock.semiAxesM[0], rock.semiAxesM[1]);
      points.push(o);
      for (const [f1, f2] of [
        [0.3, 0.1],
        [-0.2, 0.35],
        [1.25, 0],
      ] as const) {
        points.push(add(o, add(scale(t1, f1 * r), scale(t2, f2 * r))));
      }
    }
  }
  const random = seededRandom(seed);
  for (let n = 0; n < DRAWN_POINTS; n += 1) {
    const patch = block.middle[n % block.middle.length];
    if (patch === undefined) {
      throw new Error("the block has middle patches");
    }
    const side = 2 ** patch.key.level;
    const [s, t] = [(patch.key.i + random()) / side, (patch.key.j + random()) / side];
    const dir = faceUvToDir(patch.key.face, stToUv(s), stToUv(t));
    const from = sub(datumPointOf(block.figure, dir), block.referenceM);
    const ground = drawnTerrainT(patch, dir, from, normalOf(block.figure, dir));
    points.push(
      add(
        datumPointOf(block.figure, dir),
        scale(normalOf(block.figure, dir), ground + 5 * random() - 2),
      ),
    );
  }
  return points;
}

/** What one point's comparison found. */
interface Compared {
  readonly kind: "bare" | "rock" | "steep" | "elsewhere";
  /** The module's height less the drawn one, m. */
  readonly gapM: number;
  /** Where it fails, why. */
  readonly failure: string | null;
  /** On a rock, whether the terrain alone, without the rocks' meshes, would fail: the control. */
  readonly failsBare: boolean;
}

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

describe("collision agrees with what is drawn, with rocks (R11.T4.c)", () => {
  it.each([
    ["the Moon-like world", moonFixture, 1],
    ["the Earth-like world", earthFixture, 2],
  ])(
    "on %s, the drawn ground and rocks meet the module's height within their bounds",
    { timeout: 120_000 },
    (_, text, seed) => {
      const fixture = parseWorkerFixture(text);
      const { direct, args, seed: detailSeed } = directFieldsOf(() => new SurfaceFields(), fixture);
      const [a, c] = direct.figureM(...args) ?? [];
      if (a === undefined || c === undefined) {
        throw new Error("the fixture's field holds a figure");
      }
      const figure: BodyFigure = { equatorialRadiusM: a, polarRadiusM: c, pole: null };
      const block = drawnBlock(fixture, figure);
      const middle = new Set(block.middle.map((patch) => keyName(patch.key)));
      const compared: Compared[] = [];
      for (const p of pointsOver(block, seed)) {
        const foot = datumFoot(figure, p);
        const key = patchContaining(block.finest, foot);
        const patch = block.patches.get(keyName(key));
        if (patch === undefined || !middle.has(keyName(key))) {
          compared.push({ kind: "elsewhere", gapM: 0, failure: null, failsBare: false });
          continue;
        }
        const nu = normalOf(figure, foot);
        const from = sub(datumPointOf(figure, foot), block.referenceM);
        const terrain = drawnTerrainT(patch, foot, from, nu);
        let drawn = terrain;
        for (const rock of block.rocks) {
          const t = drawnRockT(rock, from, nu);
          if (t !== null) {
            drawn = Math.max(drawn, t);
          }
        }
        const withRocks = direct.surfaceHeightWithRocksM(...args, detailSeed, p.x, p.y, p.z);
        if (withRocks === undefined) {
          throw new Error("the surveyed ground's height is known");
        }
        const gapM = withRocks - drawn;
        let standing: { readonly rock: DrawnRock; readonly cosine: number } | null = null;
        for (const rock of block.rocks) {
          const top = analyticTop(figure, rock.rock, foot);
          if (top !== null && Math.abs(top.topM - withRocks) <= 1e-6) {
            standing = { rock, cosine: top.cosine };
          }
        }
        const at = `${keyName(key)} (${p.x}, ${p.y}, ${p.z})`;
        if (standing === null) {
          compared.push({
            kind: "bare",
            gapM,
            failure: Math.abs(gapM) <= BOUND_M ? null : `bare ground at ${at}: ${gapM} m`,
            failsBare: false,
          });
        } else if (standing.cosine < MIN_COSINE) {
          compared.push({ kind: "steep", gapM, failure: null, failsBare: false });
        } else {
          const { rock, cosine } = standing;
          const below = (rockToleranceM(rock.rock) + rock.halfStepM) / cosine;
          const above = rock.halfStepM / cosine + BOUND_M;
          const reach = below + contactBoundM(figure, rock.rock) + BOUND_M;
          compared.push({
            kind: "rock",
            gapM,
            failure:
              gapM >= -above && gapM <= reach
                ? null
                : `a rock at ${at}: ${gapM} m, from ${-above} to ${reach} m (cos ${cosine})`,
            failsBare: withRocks - terrain > reach,
          });
        }
      }
      direct.free();
      expect(compared.flatMap(({ failure }) => (failure === null ? [] : [failure]))).toEqual([]);
      const count = (kind: Compared["kind"]): number =>
        compared.filter((point) => point.kind === kind).length;
      expect(count("bare")).toBeGreaterThan(DRAWN_POINTS / 2);
      expect(count("rock")).toBeGreaterThan(10);
      expect(count("steep")).toBeLessThan(count("rock"));
      // The control: without the rocks' meshes, most points on a rock are out of their bound.
      expect(compared.filter(({ failsBare }) => failsBare).length).toBeGreaterThan(
        count("rock") / 2,
      );
    },
  );
});
