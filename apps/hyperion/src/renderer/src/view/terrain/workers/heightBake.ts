/**
 * What a height worker answers, as a pure function of the loaded surface module and a request
 * (plan R05, T10.b, Design note 11).
 *
 * @remarks
 * The worker (`height.worker.ts`) holds no logic of its own: it loads the module and hands each
 * message here, so a Node-environment test drives the same answers from the module's bytes, as
 * R04's `wasm/handleRequest.ts` does for the probe worker. This file never imports
 * `generated/surface/` (only workers and tests may, `surfaceImports.test.ts`); the module's
 * exports arrive through {@link HeightModule}, whose enum values the test pins to the generated
 * glue's.
 *
 * The bake's layout is `hyperion_surface`'s `src/wasm.rs` module documentation: heights and morph
 * targets interleaved per vertex, offsets q₀ then q₁ per vertex on `baked-offsets`, and octahedral
 * normal pairs, which are packed here into half floats (`rg16float` on the GPU, Design note 5).
 * Each getter copies its array out of the module's linear memory into a new typed array whose
 * buffer the worker transfers; the module's own memory cannot be transferred.
 */

import { patchKeyString, type PatchKey, unreachable } from "../patchKey";
import { vec3 } from "../../../geometry/vec3";
import type {
  BakedPatch,
  BakeSettings,
  HeightWorkerReply,
  HeightWorkerRequest,
  TestPlanetRidges,
} from "./messages";
import { bakeTransferables } from "./messages";

/** The version of the test planet this client's code was written against, the surface crate's
 * `TEST_PLANET_VERSION` (pinned to it by `heightWasm.test.ts`). A module that bakes another
 * version is stale: its heights are not the ones the client's tests and goldens describe. */
export const EXPECTED_TEST_PLANET_VERSION = 1;

/** The module's `VertexPath` enum values (`src/wasm.rs`'s `JsVertexPath`). */
export const WASM_VERTEX_PATH = { "baked-offsets": 0, "face-differences": 1 } as const;

/** The module's `NormalScale` enum values (`src/wasm.rs`'s `JsNormalScale`). */
export const WASM_NORMAL_SCALE = { mesh: 0, double: 1 } as const;

/** The module's `Ridges` enum values (`src/wasm.rs`'s `JsRidges`). */
export const WASM_RIDGES = { off: 0, on: 1 } as const satisfies Record<TestPlanetRidges, number>;

/** The module's `BakedPatch`: getters that copy each array out, and its own memory to free. */
export interface WasmBakedPatch {
  heights(): Float32Array;
  offsets(): Float32Array | undefined;
  normals(): Float32Array;
  origin(): Float64Array;
  heightRangeM(): Float32Array;
  readonly boundingRadiusM: number;
  readonly originHeightM: number;
  readonly skirtDepthM: number;
  /** Releases the bake's memory inside the module. */
  free(): void;
}

/** The part of the surface module a height worker calls. */
export interface HeightModule {
  readonly bakePatch: (
    face: number,
    level: number,
    i: number,
    j: number,
    vertexPath: number,
    normals: number,
    ridges: number,
    skirtM: number,
  ) => WasmBakedPatch;
  readonly testPlanetVersion: () => number;
}

/** Why a loaded module cannot serve: it bakes another test planet than this client's. */
export function staleModuleMessage(module: HeightModule): string | null {
  const version = module.testPlanetVersion();
  return version === EXPECTED_TEST_PLANET_VERSION
    ? null
    : `the surface module bakes test planet version ${String(version)}, ` +
        `but this client expects ${String(EXPECTED_TEST_PLANET_VERSION)}`;
}

/** The octahedral pairs `pairs`, rounded to half floats (to nearest, ties to even). */
export function packNormals(pairs: Float32Array): Float16Array {
  return Float16Array.from(pairs);
}

/**
 * Bakes `key` with `settings` (no extra skirt margin) and copies the result out of the module.
 *
 * @throws Whatever the module throws for a key out of range, after freeing nothing it allocated.
 */
export function bakeKey(
  module: HeightModule,
  key: PatchKey,
  generation: number,
  settings: BakeSettings,
): BakedPatch {
  const baked = module.bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    WASM_VERTEX_PATH[settings.vertexPath],
    WASM_NORMAL_SCALE[settings.normals],
    WASM_RIDGES[settings.ridges],
    0,
  );
  try {
    const [ox, oy, oz] = baked.origin();
    const [low, high] = baked.heightRangeM();
    if (ox === undefined || oy === undefined || oz === undefined) {
      throw new Error("the module's bake has no three-component origin");
    }
    if (low === undefined || high === undefined) {
      throw new Error("the module's bake has no two-ended height range");
    }
    return {
      key,
      generation,
      originM: vec3(ox, oy, oz),
      heights: baked.heights(),
      offsets: baked.offsets() ?? null,
      normals: packNormals(baked.normals()),
      heightRangeM: [low, high],
      boundingRadiusM: baked.boundingRadiusM,
      originHeightM: baked.originHeightM,
      skirtDepthM: baked.skirtDepthM,
    };
  } finally {
    baked.free();
  }
}

/** A worker's answer and the buffers it transfers with it. */
export interface HeightAnswer {
  readonly reply: HeightWorkerReply;
  readonly transfer: Transferable[];
}

/**
 * Answers `request` from the loaded `module`.
 *
 * @remarks
 * A bake the module refuses (a `JsError`, for a key out of range) is answered `bake-failed`, so
 * that the pool gives up that key alone. A trap (`WebAssembly.RuntimeError`, a Rust panic) is a
 * bug that may leave the instance broken, so it is rethrown: the worker raises it as its error and
 * the pool replaces the worker. A coarse field is handed to `holdField`, which keeps it for the
 * worker's life (the previous one replaced), and acknowledged: the test planet does not read it,
 * but each worker must hold its copy so that the spike's memory runs measure the field Design
 * notes 11 and 21 count. R09's height function copies it into the module instead.
 */
export function answerRequest(
  module: HeightModule,
  request: HeightWorkerRequest,
  holdField: (bytes: ArrayBuffer) => void,
): HeightAnswer {
  switch (request.kind) {
    case "bake": {
      try {
        const bake = bakeKey(module, request.key, request.generation, request.settings);
        return {
          reply: { kind: "baked", id: request.id, bake },
          transfer: bakeTransferables(bake),
        };
      } catch (error: unknown) {
        if (error instanceof WebAssembly.RuntimeError) {
          throw error;
        }
        const message = error instanceof Error ? error.message : String(error);
        return {
          reply: {
            kind: "bake-failed",
            id: request.id,
            message: `patch ${patchKeyString(request.key)}: ${message}`,
          },
          transfer: [],
        };
      }
    }
    case "field":
      holdField(request.bytes);
      return { reply: { kind: "field-loaded", id: request.id }, transfer: [] };
  }
  return unreachable(request);
}
