/**
 * Terrain's cluster records on real bakes (plan R14, R14.T1.b's tests, built by T1.f): a block's
 * sphere holds every vertex of its block, morphed or not, and a strip's every foot, on the real
 * module's bakes of R10.T4.b's sample worlds (the Earth-like and Moon-like worker fixtures, the two
 * whose fields the repository carries) and on R05.T4.b's golden vertices, placed by the native
 * build (`vertex_f32.golden`).
 */

import { beforeAll, describe, expect, it } from "vitest";

import earthFixture from "../../../../../../../crates/hyperion-surface/tests/golden/worker/earth_like.golden?raw";
import vertexGolden from "../../../../../../../crates/hyperion-surface/tests/golden/vertex_f32.golden?raw";
import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import type { Vec3 } from "../../geometry/vec3";
import { worstOutsideClusters } from "../../test/clusterBounds";
import { bytesOf, moonLikeFixture, type WorkerFixture } from "../../test/moonLikeFixture";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir, vertexSpacing } from "../terrain/cube";
import { type Face, parentKey, type PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry, surfacePoint } from "../terrain/planet";
import { answerRequest, type HeightModule, WorkerFields } from "../terrain/workers/heightBake";
import type { BakedPatch } from "../terrain/workers/messages";
import { CLUSTER_RECORD_BYTES } from "./record";
import { TERRAIN_PAGE_CLUSTERS, writeTerrainClusterRecords } from "./terrain";

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

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

/** The Earth-like worker fixture's body, seed, blocks and its one patch, as `fieldsWasm.test.ts` reads it. */
function earthLikeFixture(): WorkerFixture & { readonly patch: PatchKey } {
  let body = "";
  let seed = "";
  let patch: PatchKey | null = null;
  const blocks: ArrayBuffer[] = [];
  for (const line of earthFixture.split("\n").slice(1)) {
    const [head = "", ...rest] = line.split(" ");
    if (head === "body") {
      body = rest[0] ?? "";
    } else if (head === "seed") {
      seed = rest[0] ?? "";
    } else if (head === "block") {
      blocks.push(bytesOf(rest[2] ?? ""));
    } else if (head === "patch") {
      const [, face, level, i, j] = rest;
      patch = { face: faceOf(face), level: Number(level), i: Number(i), j: Number(j) };
    }
  }
  if (patch === null) {
    throw new Error("the Earth-like fixture names no patch");
  }
  return { body, seed, blocks, patch };
}

/** A face index of a fixture, checked. */
function faceOf(word: string | undefined): Face {
  const n = Number(word);
  if (n !== 0 && n !== 1 && n !== 2 && n !== 3 && n !== 4 && n !== 5) {
    throw new Error(`a fixture names no face, ${String(word)}`);
  }
  return n;
}

/** The Moon-like fixture's patches (its `patch` lines' keys). */
const MOON_PATCHES: ReadonlyArray<PatchKey> = [
  { face: 1, level: 8, i: 90, j: 90 },
  { face: 1, level: 7, i: 47, j: 45 },
  { face: 1, level: 5, i: 11, j: 11 },
  { face: 1, level: 2, i: 1, j: 1 },
];

/**
 * A worker's fields holding every block of `fixture`, through the worker's answers, and the body's
 * level table as the last block left it.
 */
function heldFields(fixture: WorkerFixture): {
  readonly fields: WorkerFields;
  readonly levels: ArrayLike<number>;
} {
  const fields = new WorkerFields(module);
  let levels: ArrayLike<number> = [];
  fixture.blocks.forEach((block, n) => {
    const { reply } = answerRequest(
      module,
      n === 0
        ? {
            kind: "field-init",
            id: n,
            body: fixture.body,
            block,
            form: "payload",
            sigma: "surveyed",
          }
        : { kind: "field-chunk", id: n, body: fixture.body, block },
      fields,
    );
    if (reply.kind !== "field-held") {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
    levels = reply.block?.levels ?? levels;
  });
  return { fields, levels };
}

/** `key` and its ancestors to `levels` coarser. */
function withAncestors(key: PatchKey, levels: number): PatchKey[] {
  const keys = [key];
  let at: PatchKey | null = key;
  for (let n = 0; n < levels; n += 1) {
    at = at === null ? null : parentKey(at);
    if (at !== null) {
      keys.push(at);
    }
  }
  return keys;
}

/** `fixture`'s bake of `key` on the face-differences path, through the worker's answers. */
function bakeOf(fields: WorkerFields, fixture: WorkerFixture, key: PatchKey): BakedPatch {
  const { reply } = answerRequest(
    module,
    {
      kind: "bake",
      id: 1,
      key,
      generation: 1,
      settings: {
        vertexPath: "face-differences",
        normals: "mesh",
        source: {
          kind: "body",
          body: fixture.body,
          detailSeed: fixture.seed,
          weights: "skipped",
        },
      },
    },
    fields,
  );
  if (reply.kind !== "baked") {
    throw new Error(`the patch was not baked: ${JSON.stringify(reply)}`);
  }
  return reply.bake;
}

/** The worst distance outside its clusters of any vertex of `bake`, on `figure`. */
function worstOfBake(bake: BakedPatch, figure: BodyFigure): number {
  const own = new Float64Array(65 * 65);
  const morph = new Float64Array(65 * 65);
  for (let n = 0; n < 65 * 65; n += 1) {
    own[n] = bake.heights[2 * n] ?? Number.NaN;
    morph[n] = bake.heights[2 * n + 1] ?? Number.NaN;
  }
  return worstOutsideClusters({
    figure,
    key: bake.key,
    originM: bake.originM,
    originHeightM: bake.originHeightM,
    own,
    morph,
    heightRangeM: bake.heightRangeM,
    skirtDepthM: bake.skirtDepthM,
  });
}

/** How far `bake`'s origin lies from where `figure` puts it, m. */
function originMissM(bake: BakedPatch, figure: BodyFigure): number {
  const [x, y, z] = surfacePoint(figure, vertexDir(bake.key, 32, 32), bake.originHeightM);
  return Math.hypot(x - bake.originM.x, y - bake.originM.y, z - bake.originM.z);
}

/**
 * The body's figure the bakes were made on: its equatorial radius from the level table's largest
 * vertex spacing at level 0 (`vertexSpacing(a, 0)`, as the worker's `level_table` writes it), its
 * polar radius the one that puts `bake`'s origin where the bake has it, found by bisection.
 */
function figureOf(levels: ArrayLike<number>, bake: BakedPatch): BodyFigure {
  const a = (levels[3] ?? Number.NaN) / vertexSpacing(1, 0).maxM;
  const z = Math.abs(bake.originM.z);
  let low = 0.8 * a;
  let high = a;
  for (let n = 0; n < 80; n += 1) {
    const c = (low + high) / 2;
    const [, , pz] = surfacePoint(
      { equatorialRadiusM: a, polarRadiusM: c, pole: null },
      vertexDir(bake.key, 32, 32),
      bake.originHeightM,
    );
    if (Math.abs(pz) < z) {
      low = c;
    } else {
      high = c;
    }
  }
  return { equatorialRadiusM: a, polarRadiusM: (low + high) / 2, pole: null };
}

describe("terrain's cluster records on the real module's bakes (plan R14, R14.T1.b)", () => {
  const earth = earthLikeFixture();
  it.each([
    ["the Moon-like world", moonLikeFixture("all"), MOON_PATCHES],
    ["the Earth-like world", earth, [earth.patch]],
  ] as const)(
    "hold every vertex of %s's patches and their ancestors, morphed and as skirt feet",
    (_name, fixture, roots) => {
      const { fields, levels } = heldFields(fixture);
      const keys = roots.flatMap((key) => withAncestors(key, 3));
      const bakes = keys.map((key) => bakeOf(fields, fixture, key));
      const [first] = bakes;
      if (first === undefined) {
        throw new Error("no patch was baked");
      }
      // One figure puts every bake's origin where the bake has it: the one the bakes were made on.
      const figure = figureOf(levels, first);
      expect(Math.max(...bakes.map((each) => originMissM(each, figure)))).toBeLessThan(1e-3);
      expect(bakes.map((each) => worstOfBake(each, figure) < 0)).toEqual(bakes.map(() => true));
    },
  );
});

/** The `f32` value of `0x`-prefixed IEEE 754 bits. */
function f32Of(hex: string | undefined): number {
  const u = new Uint32Array([Number.parseInt(hex ?? "", 16)]);
  return new Float32Array(u.buffer)[0] ?? Number.NaN;
}

/** One golden vertex: its grid place and its own and morphed places about its patch's origin. */
interface GoldenVertex {
  readonly x: number;
  readonly y: number;
  readonly p: Vec3;
  readonly q: Vec3;
}

/** The golden's patches, each its key and its vertices. */
function goldenPatches(): Array<{ readonly key: PatchKey; readonly vertices: GoldenVertex[] }> {
  const out: Array<{ key: PatchKey; vertices: GoldenVertex[] }> = [];
  for (const line of vertexGolden.split("\n")) {
    const words = line.trim().split(/\s+/);
    if (words[0] === "patch") {
      const [, , face, level, i, j] = words;
      out.push({
        key: { face: faceOf(face), level: Number(level), i: Number(i), j: Number(j) },
        vertices: [],
      });
    } else if (words[0] === "vertex") {
      const vec = (at: number): Vec3 => ({
        x: f32Of(words[at]),
        y: f32Of(words[at + 1]),
        z: f32Of(words[at + 2]),
      });
      out
        .at(-1)
        ?.vertices.push({ x: Number(words[1]), y: Number(words[2]), p: vec(7), q: vec(10) });
    }
  }
  return out;
}

/** `tests/vertex_golden.rs`' fixture height at (x, y) of `key` at `level`, m (`f64`). */
function goldenHeightM(key: PatchKey, x: number, y: number, level: number): number {
  const [px, py, pz] = surfacePoint(WGS84_FIGURE, vertexDir(key, x, y), 0);
  const a = WGS84_FIGURE.equatorialRadiusM;
  return 100 + level + (16_000 * (px * py + 0.3 * pz * pz)) / (a * a);
}

describe("terrain's cluster records on R05.T4.b's golden vertices (plan R14, R14.T1.b)", () => {
  it("hold every golden vertex, at its own height and morphed, inside each block that draws it", () => {
    const outside: string[] = [];
    let checked = 0;
    for (const { key, vertices } of goldenPatches()) {
      // The fixture's heights over the whole grid, own and the parent level's, for the bake's range.
      const heights: number[] = [];
      for (let y = 0; y <= 64; y += 1) {
        for (let x = 0; x <= 64; x += 1) {
          heights.push(goldenHeightM(key, x, y, key.level));
          heights.push(goldenHeightM(key, x, y, Math.max(0, key.level - 1)));
        }
      }
      const h0M = goldenHeightM(key, 32, 32, key.level);
      const [ox, oy, oz] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), h0M);
      const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
      writeTerrainClusterRecords(
        planetGeometry(WGS84_FIGURE, null),
        {
          key,
          originM: { x: ox, y: oy, z: oz },
          heightRangeM: [Math.min(...heights), Math.max(...heights)],
          skirtDepthM: 1,
        },
        records,
        0,
      );
      for (const { x, y, p, q } of vertices) {
        // Every block whose 9 × 9 vertices hold (x, y): one inside, two on a seam, four at a corner.
        for (const by of [Math.floor(y / 8) - (y % 8 === 0 ? 1 : 0), Math.floor(y / 8)]) {
          for (const bx of [Math.floor(x / 8) - (x % 8 === 0 ? 1 : 0), Math.floor(x / 8)]) {
            if (bx < 0 || bx > 7 || by < 0 || by > 7) {
              continue;
            }
            const at = (8 * by + bx) * CLUSTER_RECORD_BYTES;
            const c = {
              x: records.getFloat32(at, true),
              y: records.getFloat32(at + 4, true),
              z: records.getFloat32(at + 8, true),
            };
            const r = records.getFloat32(at + 12, true);
            for (const place of [p, q]) {
              checked += 1;
              const d = Math.hypot(place.x - c.x, place.y - c.y, place.z - c.z) - r;
              if (!(d < 0)) {
                outside.push(`${String(key.level)} (${String(x)}, ${String(y)}) by ${String(d)} m`);
              }
            }
          }
        }
      }
    }
    expect(outside).toEqual([]);
    expect(checked).toBeGreaterThan(1_000);
  });
});
