/**
 * The cluster path's measured errors on the real surface module's bakes (plan R14, R14.T3.a and
 * T3.b): over wasm bakes of R10.T4.b's sample worlds (the worker's Moon-like and Earth-like fixture
 * fields, `tests/golden/worker/`) and of the test planet, on both vertex paths, the errors the
 * height worker answers a lit bake with, and the test planet's formed here (the worker measures
 * no spike bake), are at least the distance between each cluster's stride-s and stride-1 meshes at
 * every stride-1 vertex, both channels at k = 0, ½ and 1, as the GPU places them; and they are the
 * main thread's own of the same bake, bit for bit.
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
import { terrainTestGolden } from "../scenes/terrainScene";
import { TEST_PLANET_FIGURE } from "../spike/testPlanetFigure";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry } from "../terrain/planet";
import {
  answerRequest,
  bakeKey,
  type HeightModule,
  WorkerFields,
} from "../terrain/workers/heightBake";
import type { BakedPatch, BakeSettings, HeightSourceSetting } from "../terrain/workers/messages";
import { clusterErrorM, clusterErrors, packClusterErrors } from "./errors";
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

/** A world the test bakes, and the patches it bakes of it. */
interface World {
  readonly name: string;
  readonly field: FixtureField | null;
  readonly keys: ReadonlyArray<PatchKey>;
}

const WORLDS: ReadonlyArray<World> = [
  {
    name: "the test planet",
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
    field: fixtureField(earthLike),
    keys: [
      { face: 0, level: 10, i: 1, j: 1 },
      { face: 0, level: 13, i: 14, j: 10 },
      { face: 0, level: 16, i: 112, j: 83 },
      { face: 0, level: 19, i: 900, j: 669 },
    ],
  },
];

/** `TERRAIN TEST`'s field, whose header states its datum. */
const TERRAIN_TEST = terrainTestGolden();

let fields: WorkerFields;

/** Posts `blocks` of `body` to the worker's fields, as a payload. */
function hold(body: BodyIdHex, blocks: ReadonlyArray<string>): void {
  blocks.forEach((hex, n) => {
    const block = blockBytes(hex).slice().buffer;
    const took =
      n === 0 ? fields.init(body, block, "payload", "skipped") : fields.chunk(body, block);
    if (took.kind !== "taken") {
      throw new Error(`${body}'s block ${String(n)} was refused: ${took.message}`);
    }
  });
}

beforeAll(() => {
  initSync({ module: wasmBytes() });
  fields = new WorkerFields(MODULE);
  for (const { field } of WORLDS) {
    if (field !== null) {
      hold(field.body, field.blocks);
    }
  }
  hold(TERRAIN_TEST.body, TERRAIN_TEST.payloads[2]?.blocksHex ?? []);
});

/** `world`'s datum: its field's as the module holds it, or the test planet's. */
function figureOf(world: World): BodyFigure {
  if (world.field === null) {
    return TEST_PLANET_FIGURE;
  }
  const figure = fields.figure(world.field.body);
  if (figure === null) {
    throw new Error(`the module holds no datum of ${world.name}`);
  }
  return figure;
}

/** `world`'s bake source, a lit style's or, where `wireframe`, the wireframe's. */
function sourceOf(world: World, wireframe = false): HeightSourceSetting {
  return world.field === null
    ? { kind: "test-planet", ridges: "on" }
    : {
        kind: "body",
        body: world.field.body,
        detailSeed: world.field.seed,
        weights: wireframe ? "skipped" : "baked",
        horizon: "skipped",
        ...(wireframe ? { normals: "skipped" } : {}),
      };
}

/** `key` of `world` as the height worker answers its bake on `vertexPath`. */
function answered(
  world: World,
  key: PatchKey,
  vertexPath: TerrainVertexPath,
  wireframe = false,
): BakedPatch {
  const settings: BakeSettings = {
    vertexPath,
    normals: "mesh",
    source: sourceOf(world, wireframe),
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

/** The module's own bake of `key` of `world` with its offsets, made here, not by the worker. */
function moduleBake(world: World, key: PatchKey): BakedPatch {
  const settings: BakeSettings = {
    vertexPath: "baked-offsets",
    normals: "mesh",
    source: sourceOf(world),
  };
  const { source } = settings;
  const bake =
    source.kind === "test-planet"
      ? bakeKey(MODULE, key, 1, settings, source.ridges)
      : fields.bake(key, 1, settings, source);
  if (bake === null) {
    throw new Error(`${JSON.stringify(key)} of ${world.name} was not baked`);
  }
  return bake;
}

/**
 * `key` of `world`'s errors as the worker answers them on `vertexPath`, or for the test planet,
 * which the worker leaves unmeasured, formed here from the module's bake.
 */
function answeredErrors(world: World, key: PatchKey, vertexPath: TerrainVertexPath): Float16Array {
  const errors =
    world.field === null
      ? packClusterErrors(
          clusterErrors(moduleBake(world, key), key, planetGeometry(figureOf(world), null)),
        )
      : answered(world, key, vertexPath).clusterErrors;
  if (errors === undefined) {
    throw new Error(`${JSON.stringify(key)} of ${world.name} was answered unmeasured`);
  }
  return errors;
}

/**
 * The largest amount by which a cluster's drawn stride distance, as the GPU places `bake` on
 * `figure`, passes `measured`, its errors as answered, metres.
 */
function worstExcessM(bake: BakedPatch, measured: Float16Array, figure: BodyFigure): number {
  const errors = Float64Array.from(measured);
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

describe("the clusters' measured errors on the surface module's bakes (plan R14, R14.T3.a, T3.b)", () => {
  it.each(WORLDS)(
    "bound every stride's drawn distance on $name's patches as the worker answers them, on both vertex paths",
    { timeout: 120_000 },
    (world) => {
      for (const key of world.keys) {
        for (const vertexPath of ["baked-offsets", "face-differences"] as const) {
          expect(
            worstExcessM(
              answered(world, key, vertexPath),
              answeredErrors(world, key, vertexPath),
              figureOf(world),
            ),
          ).toBeLessThanOrEqual(0);
        }
      }
    },
  );

  it.each(WORLDS.filter((world) => world.field !== null))(
    "answer $name's bakes with the main thread's own errors of the same bake, bit for bit, keeping the offsets only where drawn",
    (world) => {
      const key = world.keys[1];
      if (key === undefined) {
        throw new Error(`${world.name} has no second patch`);
      }
      const own = new Uint16Array(
        packClusterErrors(
          clusterErrors(moduleBake(world, key), key, planetGeometry(figureOf(world), null)),
        ).buffer,
      );
      for (const vertexPath of ["baked-offsets", "face-differences"] as const) {
        const bake = answered(world, key, vertexPath);
        expect(new Uint16Array(bake.clusterErrors?.buffer ?? new ArrayBuffer(0))).toEqual(own);
        expect(bake.offsets?.length ?? null).toBe(
          vertexPath === "baked-offsets" ? 6 * 65 * 65 : null,
        );
      }
    },
  );

  it("measure no test-planet bake, whose spike shading keeps today's path", () => {
    const world = WORLDS[0];
    const key = world?.keys[1];
    if (world === undefined || key === undefined) {
      throw new Error("the test planet's patch is missing");
    }
    expect(answered(world, key, "baked-offsets").clusterErrors).toBeUndefined();
  });

  it("measure no wireframe bake, which draws at stride 1 into slots that hold no cluster records", () => {
    const world = WORLDS[1];
    const key = world?.keys[2];
    if (world === undefined || key === undefined) {
      throw new Error("the Moon-like world's patch is missing");
    }
    const bake = answered(world, key, "face-differences", true);
    expect([bake.clusterErrors, bake.offsets]).toEqual([undefined, null]);
  });

  it("hold each field's datum as its header states it", () => {
    expect(fields.figure(TERRAIN_TEST.body)).toEqual({
      equatorialRadiusM: TERRAIN_TEST.header.equatorial_radius_m,
      polarRadiusM: TERRAIN_TEST.header.polar_radius_m,
      pole: null,
    });
    expect(fields.figure("5e5e000000000000.0fff")).toBeNull();
  });

  it("measure more at a coarser stride on real relief, and something at every stride", () => {
    const world = WORLDS[1];
    const key = world?.keys[3];
    if (world === undefined || key === undefined) {
      throw new Error("the Moon-like world's inside patch is missing");
    }
    const errors = Float64Array.from(answered(world, key, "baked-offsets").clusterErrors ?? []);
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
