/**
 * The camera's strides on the real surface module's bakes (plan R14, R14.T3.c's drawn-error test):
 * over wasm bakes of R10.T4.b's sample worlds (the worker's Moon-like and Earth-like fixture fields,
 * `tests/golden/worker/`), as the GPU places them, each block drawn at the stride the cull's twin
 * gives it from cameras about each patch, through the stride morph's twin, at each bake's shared
 * bands from its blocks' measured errors (its edges its own blocks', with no patch beside), each
 * vertex at its own CDLOD k: the drawn error, the level's
 * stated error plus the drawn surface's distance from the stride-1 surface, is within τ′ at the
 * distance of every vertex it is measured at, the stride-1 vertices, where a block's triangles'
 * difference is largest. The cameras stand at random about each patch, half at 0.2 to 40 of its
 * sides and half across its level's CDLOD band, not at R05's descent poses, which select their own
 * patches; Mars-like has no field fixture.
 */

import { beforeAll, describe, expect, it } from "vitest";

import earthLike from "../../../../../../../crates/hyperion-surface/tests/golden/worker/earth_like.golden?raw";
import moonLike from "../../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";
import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { Vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import { gpuPlacement } from "../../test/strideOracle";
import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";
import { blockBytes } from "../scenes/terrainGolden";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry } from "../terrain/planet";
import { selectionErrorM } from "../terrain/select";
import { StrideEdges } from "../terrain/strideEdges";
import { MORPH_START_FRACTION } from "../terrain/terrainPass";
import { answerRequest, type HeightModule, WorkerFields } from "../terrain/workers/heightBake";
import type { BakedPatch } from "../terrain/workers/messages";
import {
  cameraStrideLevel,
  clusterStrideBandsM,
  heldEdgeBandsM,
  largestStrideErrorsM,
  patchStrideScale,
  strideMorphFactor,
  stridePlace,
  vertexStrideBandM,
} from "./strides";
import { TERRAIN_SUB_PATCH_MASK, terrainMaskCluster } from "./terrain";
import { subPatchWord } from "../terrain/gpu/uniforms";

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const MODULE: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

/** A worker fixture's field: its body, detail seed and blocks in the order posted. */
interface FixtureField {
  readonly body: BodyIdHex;
  readonly seed: DetailSeedHex;
  readonly blocks: ReadonlyArray<string>;
}

/** A world the test bakes, and the patches it bakes of it. */
interface World {
  readonly name: string;
  readonly field: FixtureField;
  readonly keys: ReadonlyArray<PatchKey>;
}

/** The `body`, `seed` and `block` lines of a worker fixture (`worker/golden.rs`' format). */
function fixtureField(text: string): FixtureField {
  let body = "";
  let seed = "";
  const blocks: string[] = [];
  for (const line of text.split("\n")) {
    const [head, ...rest] = line.split(" ");
    if (head === "body") {
      body = rest[0] ?? "";
    } else if (head === "seed") {
      seed = rest[0] ?? "";
    } else if (head === "block") {
      blocks.push(rest[2] ?? "");
    }
  }
  return { body, seed, blocks };
}

const WORLDS: ReadonlyArray<World> = [
  {
    name: "the Moon-like world",
    field: fixtureField(moonLike),
    keys: [
      { face: 1, level: 5, i: 11, j: 11 },
      { face: 1, level: 7, i: 47, j: 45 },
      { face: 1, level: 8, i: 90, j: 90 },
    ],
  },
  {
    name: "the Earth-like world",
    field: fixtureField(earthLike),
    keys: [
      { face: 0, level: 13, i: 14, j: 10 },
      { face: 0, level: 16, i: 112, j: 83 },
      { face: 0, level: 19, i: 900, j: 669 },
    ],
  },
];

let fields: WorkerFields;

/** Each world's level table as its field's last block left it (`FieldBlockAnswer.levels`). */
const levelTables = new Map<string, Float64Array>();

beforeAll(() => {
  initSync({ module: wasmBytes() });
  fields = new WorkerFields(MODULE);
  for (const { field } of WORLDS) {
    field.blocks.forEach((hex, n) => {
      const block = blockBytes(hex).slice().buffer;
      const took =
        n === 0
          ? fields.init(field.body, block, "payload", "skipped")
          : fields.chunk(field.body, block);
      if (took.kind !== "taken") {
        throw new Error(`${field.body}'s block ${String(n)} was refused: ${took.message}`);
      }
      if (took.block !== null) {
        levelTables.set(field.body, took.block.levels);
      }
    });
  }
});

/** `key` of `field`'s body as the height worker answers a lit bake of it, with its offsets. */
function lit(field: FixtureField, key: PatchKey): BakedPatch {
  const reply = answerRequest(
    MODULE,
    {
      kind: "bake",
      id: 1,
      key,
      generation: 1,
      settings: {
        vertexPath: "baked-offsets",
        normals: "mesh",
        source: {
          kind: "body",
          body: field.body,
          detailSeed: field.seed,
          weights: "baked",
          horizon: "skipped",
        },
      },
    },
    fields,
  ).reply;
  if (reply.kind !== "baked") {
    throw new Error(`${JSON.stringify(key)} was not baked: ${reply.kind}`);
  }
  return reply.bake;
}

/**
 * The masks each patch's blocks are drawn as too (plan R14, Design note 19; R14.T3.c's stand-ins):
 * one level down at each of its four parts, two down at four parts and three down at four.
 */
const MASKS: ReadonlyArray<number> = [
  [1, 0, 0],
  [1, 1, 0],
  [1, 0, 1],
  [1, 1, 1],
  [2, 0, 0],
  [2, 1, 2],
  [2, 3, 1],
  [2, 2, 3],
  [3, 0, 0],
  [3, 3, 4],
  [3, 7, 2],
  [3, 5, 7],
].map(([levels = 1, i = 0, j = 0]) => (TERRAIN_SUB_PATCH_MASK | subPatchWord(levels, i, j)) >>> 0);

/**
 * Sub-patches on their own grids each patch is drawn as too, (levels, i, j) (R14.T3.c's stand-ins):
 * one level down, a mask's levels where the masks are not drawn (low's), and four, stride 8's.
 */
const OWN_GRIDS: ReadonlyArray<readonly [number, number, number]> = [
  [1, 1, 0],
  [2, 2, 3],
  [4, 9, 6],
];

/**
 * The largest share of τ′ an own grid's strided surface may stray from its stride-1 surface: the
 * CDLOD k's alone, 1.4 × 10⁻⁶ and 1.0 × 10⁻⁷ on the two worlds' bakes (2026-11-05).
 */
const OWN_GRID_SHARE = 1e-4;

/** The view's px per radian: 1,920 px across 60°, and its τ′ at 1 px. */
const PX_PER_RAD = 1920 / (2 * Math.tan(Math.PI / 6));
const TOLERANCE_RAD = 1 / PX_PER_RAD;

describe("the camera's strides on the surface module's bakes (plan R14, R14.T3.c)", () => {
  it.each(WORLDS)(
    "draw every block of $name's patches within τ′ of the stated error at its every vertex's distance, and of their stand-ins' masks by their sets' bands",
    { timeout: 120_000 },
    (world) => {
      const figure: BodyFigure | null = fields.figure(world.field.body);
      if (figure === null) {
        throw new Error(`the module holds no datum of ${world.name}`);
      }
      // The stated error without the level table: the chord's sagitta alone, so that strides come
      // nearer and the test is the stricter.
      const planet = planetGeometry(figure, null);
      // The field's own stated errors, at which the CDLOD bands are drawn.
      const stated = planetGeometry(figure, levelTables.get(world.field.body) ?? null);
      const bakes = world.keys.map((key) => lit(world.field, key));
      const random = seededRandom(0x3c7 + world.keys.length);
      // The blocks drawn at a stride above 1, and the drawn error's largest share of τ′.
      const whole = { strided: 0, worst: 0 };
      const masks = { strided: 0, worst: 0 };
      for (const bake of bakes) {
        const place = gpuPlacement({ ...bake, figure });
        if (bake.clusterErrors === undefined) {
          throw new Error("a lit bake is measured");
        }
        const { clusterErrors } = bake;
        const scale = patchStrideScale(planet, bake.key.level, TOLERANCE_RAD);
        const edges = new StrideEdges(1);
        edges.update(
          [{ key: bake.key, level: bake.key.level, measured: { scale, clusterErrors } }],
          () => largestStrideErrorsM(clusterErrors),
        );
        const edgeBandsM = heldEdgeBandsM(edges.edgesOf(0));
        const statedM = selectionErrorM(planet, bake.key.level);
        // The level's CDLOD band at 1 px (`terrainPass.ts`' `morphRangeM`), m.
        const level = bake.key.level;
        const morphEndM = selectionErrorM(stated, level - 1) * PX_PER_RAD;
        const ownM = selectionErrorM(stated, level) * PX_PER_RAD;
        const morphStartM = ownM + MORPH_START_FRACTION * (morphEndM - ownM);
        // The patch's side, about which the cameras stand at 0.2 to 40 sides, the origin's.
        const a = place(0, 0, 0, 0);
        const b = place(64, 64, 0, 0);
        const sideM = Math.hypot(b.x - a.x, b.y - a.y, b.z - a.z) / Math.SQRT2;
        for (let n = 0; n < 16; n += 1) {
          // Half the cameras across the level's CDLOD band, where each vertex's k is its own.
          const r =
            n % 2 === 0
              ? sideM * 0.2 * 200 ** random()
              : morphStartM + (morphEndM - morphStartM) * (1.2 * random() - 0.1);
          const [u, v, w] = [random() - 0.5, random() - 0.5, random() - 0.5];
          const len = Math.hypot(u, v, w);
          const camera = { x: (r * u) / len, y: (r * v) / len, z: (r * w) / len };
          const distanceOf = (p: ReadonlyArray<number>): number =>
            Math.hypot((p[0] ?? 0) - camera.x, (p[1] ?? 0) - camera.y, (p[2] ?? 0) - camera.z);
          // Each vertex at its own CDLOD k, from its own distance through its level's band at the
          // field's stated errors (R05 Design note 6), as `morphFactor` gives it.
          const own = (x: number, y: number): number[] => {
            const at0 = place(x, y, 0, 0);
            const d = Math.hypot(at0.x - camera.x, at0.y - camera.y, at0.z - camera.z);
            const k = Math.min(1, Math.max(0, (d - morphStartM) / (morphEndM - morphStartM)));
            const p = place(x, y, 0, k);
            return [p.x, p.y, p.z];
          };
          // The whole patch, and each of its masks (R14.T3.c's stand-ins) by its set's bands.
          for (const subWord of [0, ...MASKS]) {
            const factor = (
              x: number,
              y: number,
              morphLevel: number,
              values: ReadonlyArray<number>,
            ): number =>
              strideMorphFactor(
                vertexStrideBandM(scale, clusterErrors, edgeBandsM, x, y, morphLevel, subWord),
                distanceOf(values),
              );
            const morphed = new Map<number, number[]>();
            const at = (x: number, y: number): number[] => {
              let held = morphed.get(65 * y + x);
              if (held === undefined) {
                held = stridePlace(x, y, own, factor);
                morphed.set(65 * y + x, held);
              }
              return held;
            };
            const kind = subWord === 0 ? whole : masks;
            for (let block = 0; block < 64; block += 1) {
              if (terrainMaskCluster(subWord, block) === "none") {
                continue;
              }
              const bx = 8 * (block % 8);
              const by = 8 * Math.floor(block / 8);
              // The block's sphere about its own vertices, which holds every morphed one.
              const vertices: number[][] = [];
              for (let ly = 0; ly <= 8; ly += 1) {
                for (let lx = 0; lx <= 8; lx += 1) {
                  vertices.push(own(bx + lx, by + ly));
                }
              }
              const centre = [0, 1, 2].map(
                (c) => vertices.reduce((sum, p) => sum + (p[c] ?? 0), 0) / vertices.length,
              );
              const radius = Math.max(
                ...vertices.map((p) =>
                  Math.hypot(
                    (p[0] ?? 0) - (centre[0] ?? 0),
                    (p[1] ?? 0) - (centre[1] ?? 0),
                    (p[2] ?? 0) - (centre[2] ?? 0),
                  ),
                ),
              );
              const bands = clusterStrideBandsM(scale, clusterErrors, edgeBandsM, block, subWord);
              const s = 1 << cameraStrideLevel(bands, 3, Math.max(0, distanceOf(centre) - radius));
              kind.strided += s > 1 ? 1 : 0;
              for (let ly = 0; ly <= 8; ly += 1) {
                for (let lx = 0; lx <= 8; lx += 1) {
                  const qx = Math.min(8 - s, Math.floor(lx / s) * s);
                  const qy = Math.min(8 - s, Math.floor(ly / s) * s);
                  const fx = (lx - qx) / s;
                  const fy = (ly - qy) / s;
                  const p0 = at(bx + qx, by + qy);
                  const p2 = at(bx + qx + s, by + qy + s);
                  const p1 = fx >= fy ? at(bx + qx + s, by + qy) : at(bx + qx, by + qy + s);
                  const w0 = 1 - Math.max(fx, fy);
                  const w1 = Math.abs(fx - fy);
                  const w2 = Math.min(fx, fy);
                  const drawn = [0, 1, 2].map(
                    (c) => w0 * (p0[c] ?? 0) + w1 * (p1[c] ?? 0) + w2 * (p2[c] ?? 0),
                  );
                  const stride1 = own(bx + lx, by + ly);
                  const deviationM = Math.hypot(
                    (drawn[0] ?? 0) - (stride1[0] ?? 0),
                    (drawn[1] ?? 0) - (stride1[1] ?? 0),
                    (drawn[2] ?? 0) - (stride1[2] ?? 0),
                  );
                  kind.worst = Math.max(
                    kind.worst,
                    (statedM + deviationM) / (TOLERANCE_RAD * distanceOf(stride1)),
                  );
                }
              }
            }
          }
        }
      }
      expect([whole.strided > 100, masks.strided > 50]).toEqual([true, true]);
      expect(whole.worst).toBeLessThanOrEqual(1);
      expect(masks.worst).toBeLessThanOrEqual(1);
    },
  );

  it.each(WORLDS)(
    "draw a sub-patch of $name's patches on its own grid at stride 2^levels up to 8 as at stride 1, but for the CDLOD k between its vertices (R14.T3.c's stand-ins)",
    { timeout: 120_000 },
    (world) => {
      const figure: BodyFigure | null = fields.figure(world.field.body);
      if (figure === null) {
        throw new Error(`the module holds no datum of ${world.name}`);
      }
      const stated = planetGeometry(figure, levelTables.get(world.field.body) ?? null);
      const random = seededRandom(0x5b9 + world.keys.length);
      // The largest distance of the drawn surface from stride 1's, a share of τ′ at the vertex.
      let worst = 0;
      for (const key of world.keys) {
        const bake = lit(world.field, key);
        const place = gpuPlacement({ ...bake, figure });
        const level = key.level;
        const morphEndM = selectionErrorM(stated, level - 1) * PX_PER_RAD;
        const ownM = selectionErrorM(stated, level) * PX_PER_RAD;
        const morphStartM = ownM + MORPH_START_FRACTION * (morphEndM - ownM);
        for (const [levels, i, j] of OWN_GRIDS) {
          // Its grid vertex (x, y) at (part × 64 + (x, y)) ÷ 2^levels of the patch's grid, on the
          // patch's triangle there, each corner at the vertex's own k (`surfacePointAt`).
          const stride = 2 ** Math.min(3, levels);
          for (let n = 0; n < 4; n += 1) {
            const r =
              n % 2 === 0
                ? morphStartM * 0.05 * 40 ** random()
                : morphStartM + (morphEndM - morphStartM) * (1.2 * random() - 0.1);
            const [u, v, w] = [random() - 0.5, random() - 0.5, random() - 0.5];
            const len = Math.hypot(u, v, w);
            const camera = { x: (r * u) / len, y: (r * v) / len, z: (r * w) / len };
            const distanceOf = (p: Vec3): number =>
              Math.hypot(p.x - camera.x, p.y - camera.y, p.z - camera.z);
            const vertex = (x: number, y: number): Vec3 => {
              const gx = (i * 64 + x) / 2 ** levels;
              const gy = (j * 64 + y) / 2 ** levels;
              const qx = Math.min(63, Math.floor(gx));
              const qy = Math.min(63, Math.floor(gy));
              const fx = gx - qx;
              const fy = gy - qy;
              const corners: ReadonlyArray<readonly [number, number, number]> = [
                [qx, qy, 1 - Math.max(fx, fy)],
                fx >= fy ? [qx + 1, qy, fx - fy] : [qx, qy + 1, fy - fx],
                [qx + 1, qy + 1, Math.min(fx, fy)],
              ];
              const at = (k: number): Vec3 =>
                corners.reduce(
                  (sum, [cx, cy, weight]) => {
                    const p = place(cx, cy, 0, k);
                    return {
                      x: sum.x + weight * p.x,
                      y: sum.y + weight * p.y,
                      z: sum.z + weight * p.z,
                    };
                  },
                  { x: 0, y: 0, z: 0 },
                );
              const d = distanceOf(at(0));
              return at(Math.min(1, Math.max(0, (d - morphStartM) / (morphEndM - morphStartM))));
            };
            for (let y = 0; y <= 64; y += 1) {
              for (let x = 0; x <= 64; x += 1) {
                const qx = Math.min(64 - stride, Math.floor(x / stride) * stride);
                const qy = Math.min(64 - stride, Math.floor(y / stride) * stride);
                const fx = (x - qx) / stride;
                const fy = (y - qy) / stride;
                const p0 = vertex(qx, qy);
                const p2 = vertex(qx + stride, qy + stride);
                const p1 = fx >= fy ? vertex(qx + stride, qy) : vertex(qx, qy + stride);
                const w0 = 1 - Math.max(fx, fy);
                const w1 = Math.abs(fx - fy);
                const w2 = Math.min(fx, fy);
                const one = vertex(x, y);
                const deviationM = Math.hypot(
                  w0 * p0.x + w1 * p1.x + w2 * p2.x - one.x,
                  w0 * p0.y + w1 * p1.y + w2 * p2.y - one.y,
                  w0 * p0.z + w1 * p1.z + w2 * p2.z - one.z,
                );
                worst = Math.max(worst, deviationM / (TOLERANCE_RAD * distanceOf(one)));
              }
            }
          }
        }
      }
      expect(worst).toBeLessThanOrEqual(OWN_GRID_SHARE);
    },
  );
});
