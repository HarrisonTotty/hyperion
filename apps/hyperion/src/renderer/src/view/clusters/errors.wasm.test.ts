/**
 * The cluster path's measured errors on the real surface module's bakes (plan R14, R14.T3.a; T3.b's
 * brute-force test): over wasm bakes of R10.T4.b's sample worlds (the worker's Moon-like and
 * Earth-like fixture fields, `tests/golden/worker/`) and of the test planet, on both vertex paths,
 * e_s is at least the distance between each cluster's stride-s and stride-1 meshes at every
 * stride-1 vertex, both channels at k = 0, ½ and 1, as the GPU places them.
 *
 * @remarks
 * The worker's fixtures carry no figure, so each sample world is a sphere of its radius, as
 * `levelBoundsCounts.ts` takes them (`fixtures/level-bounds.json`): e_s and the oracle place the
 * bake's heights and offsets on the same figure, which is all the bound is about.
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
import { gpuPlacement, strideDistanceM } from "../../test/strideOracle";
import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";
import type { TerrainVertexPath } from "../quality/terrainKinds";
import { blockBytes } from "../scenes/terrainGolden";
import { TEST_PLANET_FIGURE } from "../spike/testPlanetFigure";
import levelBounds from "../terrain/fixtures/level-bounds.json" with { type: "json" };
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry } from "../terrain/planet";
import { answerRequest, type HeightModule, WorkerFields } from "../terrain/workers/heightBake";
import type { BakedPatch, BakeSettings } from "../terrain/workers/messages";
import { clusterErrorM, clusterErrors } from "./errors";
import { TERRAIN_BLOCKS, TERRAIN_CLUSTERS_PER_PAGE } from "./terrain";

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

/** A sample world's sphere, its radius `level-bounds.json`'s. */
function sphereOf(name: string): BodyFigure {
  const world = levelBounds.worlds.find((w) => w.name === name);
  if (world === undefined) {
    throw new Error(`level-bounds.json has no ${name}`);
  }
  return { equatorialRadiusM: world.radius_m, polarRadiusM: world.radius_m, pole: null };
}

/** A world the test bakes, and the patches it bakes of it. */
interface World {
  readonly name: string;
  readonly figure: BodyFigure;
  readonly field: FixtureField | null;
  readonly keys: ReadonlyArray<PatchKey>;
}

const WORLDS: ReadonlyArray<World> = [
  {
    name: "the test planet",
    figure: TEST_PLANET_FIGURE,
    field: null,
    keys: [
      { face: 2, level: 0, i: 0, j: 0 },
      { face: 4, level: 9, i: 300, j: 211 },
      { face: 1, level: 15, i: 20_000, j: 9_000 },
    ],
  },
  {
    // The fixture's own patches: inside its survey, astride its edge, and coarse over it.
    name: "the Moon-like world",
    figure: sphereOf("moon_like"),
    field: fixtureField(moonLike),
    keys: [
      { face: 1, level: 2, i: 1, j: 1 },
      { face: 1, level: 5, i: 11, j: 11 },
      { face: 1, level: 7, i: 47, j: 45 },
      { face: 1, level: 8, i: 90, j: 90 },
    ],
  },
  {
    // The fixture's finest-level patch of three classes, and three of its ancestors in its survey.
    name: "the Earth-like world",
    figure: sphereOf("earth_like"),
    field: fixtureField(earthLike),
    keys: [
      { face: 0, level: 10, i: 1, j: 1 },
      { face: 0, level: 13, i: 14, j: 10 },
      { face: 0, level: 16, i: 112, j: 83 },
      { face: 0, level: 19, i: 900, j: 669 },
    ],
  },
];

let fields: WorkerFields;

beforeAll(() => {
  initSync({ module: wasmBytes() });
  fields = new WorkerFields(MODULE);
  for (const { field } of WORLDS) {
    field?.blocks.forEach((hex, n) => {
      const block = blockBytes(hex).slice().buffer;
      const took =
        n === 0
          ? fields.init(field.body, block, "payload", "skipped")
          : fields.chunk(field.body, block);
      if (took.kind !== "taken") {
        throw new Error(`${field.body}'s block ${String(n)} was refused: ${took.message}`);
      }
    });
  }
});

/** `key` of `world` baked by the module on `vertexPath`. */
function baked(world: World, key: PatchKey, vertexPath: TerrainVertexPath): BakedPatch {
  const { field } = world;
  const settings: BakeSettings = {
    vertexPath,
    normals: "mesh",
    source:
      field === null
        ? { kind: "test-planet", ridges: "on" }
        : {
            kind: "body",
            body: field.body,
            detailSeed: field.seed,
            weights: "skipped",
            horizon: "skipped",
            normals: "skipped",
          },
  };
  const reply = answerRequest(
    MODULE,
    { kind: "bake", id: 1, key, generation: 1, settings },
    fields,
  ).reply;
  if (reply.kind !== "baked") {
    throw new Error(`${JSON.stringify(key)} of ${world.name} was not baked: ${reply.kind}`);
  }
  return reply.bake;
}

/** The largest amount by which a cluster's drawn stride distance passes its e_s, metres. */
function worstExcessM(bake: BakedPatch, figure: BodyFigure): number {
  const errors = clusterErrors(bake, bake.key, planetGeometry(figure, null));
  const place = gpuPlacement({ ...bake, figure });
  let worst = -Infinity;
  for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    for (const stride of [2, 4, 8] as const) {
      for (const k of [0, 0.5, 1]) {
        const lattice = cluster < TERRAIN_BLOCKS ? 1 : 4;
        worst = Math.max(
          worst,
          strideDistanceM(place, cluster, stride, k, lattice) -
            clusterErrorM(errors, cluster, stride),
        );
      }
    }
  }
  return worst;
}

describe("the clusters' measured errors on the surface module's bakes (plan R14, R14.T3.a)", () => {
  it.each(WORLDS)(
    "bound every stride's drawn distance on $name's patches, on both vertex paths",
    { timeout: 120_000 },
    (world) => {
      for (const key of world.keys) {
        for (const vertexPath of ["baked-offsets", "face-differences"] as const) {
          expect(worstExcessM(baked(world, key, vertexPath), world.figure)).toBeLessThanOrEqual(0);
        }
      }
    },
  );

  it("measure more at a coarser stride on real relief, and something at every stride", () => {
    const world = WORLDS[1];
    const key = world?.keys[3];
    if (world === undefined || key === undefined) {
      throw new Error("the Moon-like world's inside patch is missing");
    }
    const errors = clusterErrors(
      baked(world, key, "baked-offsets"),
      key,
      planetGeometry(world.figure, null),
    );
    const mean = (stride: 2 | 4 | 8): number => {
      let sum = 0;
      for (let cluster = 0; cluster < TERRAIN_BLOCKS; cluster += 1) {
        sum += clusterErrorM(errors, cluster, stride);
      }
      return sum / TERRAIN_BLOCKS;
    };
    expect(mean(2)).toBeGreaterThan(0);
    expect(mean(4)).toBeGreaterThan(mean(2));
    expect(mean(8)).toBeGreaterThan(mean(4));
  });
});
