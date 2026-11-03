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

/** The view's px per radian: 1,920 px across 60°, and its τ′ at 1 px. */
const PX_PER_RAD = 1920 / (2 * Math.tan(Math.PI / 6));
const TOLERANCE_RAD = 1 / PX_PER_RAD;

describe("the camera's strides on the surface module's bakes (plan R14, R14.T3.c)", () => {
  it.each(WORLDS)(
    "draw every block of $name's patches within τ′ of the stated error at its every vertex's distance",
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
      let strided = 0;
      let worst = 0;
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
          const factor = (
            x: number,
            y: number,
            morphLevel: number,
            values: ReadonlyArray<number>,
          ): number =>
            strideMorphFactor(
              vertexStrideBandM(scale, clusterErrors, edgeBandsM, x, y, morphLevel),
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
          for (let block = 0; block < 64; block += 1) {
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
            const bands = clusterStrideBandsM(scale, clusterErrors, edgeBandsM, block);
            const s = 1 << cameraStrideLevel(bands, 3, Math.max(0, distanceOf(centre) - radius));
            strided += s > 1 ? 1 : 0;
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
                worst = Math.max(
                  worst,
                  (statedM + deviationM) / (TOLERANCE_RAD * distanceOf(stride1)),
                );
              }
            }
          }
        }
      }
      console.warn(`${world.name} strided ${strided} worst ${worst}`);
      expect(strided).toBeGreaterThan(100);
      expect(worst).toBeLessThanOrEqual(1);
    },
  );
});
