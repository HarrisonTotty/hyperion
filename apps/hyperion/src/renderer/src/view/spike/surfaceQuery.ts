/**
 * The spike's questions to the test planet, answered by its own module in a worker (plan R05,
 * T13.b): the level table that selection's `PlanetGeometry` needs, the surface's height at a
 * direction, and a true upper bound of the surface over a set of patches.
 *
 * @remarks
 * The render thread cannot compile WebAssembly (R04.T10.a). The answers are measured once, before the descent starts, so that the scripted path stays a
 * pure function of the seed and these numbers (the orchestrator's ruling, 2026-10-03): the landing
 * site's height and the highest ground under the low part of the track.
 *
 * The height at a direction is R05.T4.c's collision interpolant, the module's `surfaceHeightM`:
 * the finest level's mesh, which is the ground the craft touches and the terrain drawn under it.
 *
 * The bound over patches is each patch's highest vertex plus the level bound ε_n of its level:
 * every vertex of level n lies on the level's mesh, whose distance from the finest surface is at
 * most ε_n (Design note 15), so no finest-level point of the patch lies above it.
 */

import { WASM_NORMAL_SCALE, WASM_RIDGES, WASM_VERTEX_PATH } from "../terrain/workers/heightBake";
import { type Xyz, uvToSt, xyzToFaceUv } from "../terrain/cube";
import { type PatchKey, patchKeyString, unreachable } from "../terrain/patchKey";
import { LEVEL_TABLE_STRIDE } from "../terrain/planet";
import type { TestPlanetRidges } from "../terrain/workers/messages";

/** The surface module's exports the queries use. */
export interface SurfaceQueryModule {
  readonly levelTable: (ridges: number) => Float64Array;
  readonly bakePatch: (
    face: number,
    level: number,
    i: number,
    j: number,
    vertexPath: number,
    normals: number,
    ridges: number,
    skirtM: number,
  ) => { heightRangeM(): Float32Array; free(): void };
  /** R05.T4.c's collision interpolant at a body-fixed direction, metres above the datum. */
  readonly surfaceHeightM: (x: number, y: number, z: number, ridges: number) => number;
}

/** A question to the query worker. */
export type SurfaceQueryRequest =
  | { readonly kind: "level-table"; readonly id: number; readonly ridges: TestPlanetRidges }
  /** The surface's height at each direction (body-fixed, x, y and z in turn). */
  | {
      readonly kind: "height";
      readonly id: number;
      readonly ridges: TestPlanetRidges;
      readonly dirs: Float64Array;
    }
  /** A true upper bound of the finest surface over the patches. */
  | {
      readonly kind: "max-height";
      readonly id: number;
      readonly ridges: TestPlanetRidges;
      readonly keys: ReadonlyArray<PatchKey>;
    };

/** The worker's answer to a request of the same `id`. */
export type SurfaceQueryReply =
  | { readonly kind: "level-table"; readonly id: number; readonly table: Float64Array }
  | {
      readonly kind: "height";
      readonly id: number;
      readonly heightsM: Float64Array;
    }
  | { readonly kind: "max-height"; readonly id: number; readonly maxM: number }
  | { readonly kind: "failed"; readonly id: number; readonly message: string };

/** The key of the level-`level` patch whose area holds the direction `dir`, on its own face. */
export function patchKeyAt(dir: Xyz, level: number): PatchKey {
  const { face, u, v } = xyzToFaceUv(dir);
  const side = 2 ** level;
  const cell = (c: number): number => Math.min(side - 1, Math.max(0, Math.floor(uvToSt(c) * side)));
  return { face, level, i: cell(u), j: cell(v) };
}

/** The highest baked vertex of `key`, metres above the datum. */
function patchMaximumM(
  module: SurfaceQueryModule,
  key: PatchKey,
  ridges: TestPlanetRidges,
): number {
  const bake = module.bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    WASM_VERTEX_PATH["face-differences"],
    WASM_NORMAL_SCALE.mesh,
    WASM_RIDGES[ridges],
    0,
  );
  try {
    const high = bake.heightRangeM()[1];
    if (high === undefined || !Number.isFinite(high)) {
      throw new Error(`patch ${patchKeyString(key)} has no finite height range`);
    }
    return high;
  } finally {
    bake.free();
  }
}

function heights(
  module: SurfaceQueryModule,
  { dirs, ridges }: Extract<SurfaceQueryRequest, { kind: "height" }>,
): Float64Array {
  const heightsM = new Float64Array(dirs.length / 3);
  for (let n = 0; n < heightsM.length; n += 1) {
    heightsM[n] = module.surfaceHeightM(
      dirs[3 * n] ?? 0,
      dirs[3 * n + 1] ?? 0,
      dirs[3 * n + 2] ?? 0,
      WASM_RIDGES[ridges],
    );
  }
  return heightsM;
}

/** Answers one request with `module`. */
export function answerSurfaceQuery(
  module: SurfaceQueryModule,
  request: SurfaceQueryRequest,
): SurfaceQueryReply {
  try {
    switch (request.kind) {
      case "level-table":
        return {
          kind: "level-table",
          id: request.id,
          table: module.levelTable(WASM_RIDGES[request.ridges]),
        };
      case "height":
        return {
          kind: "height",
          id: request.id,
          heightsM: heights(module, request),
        };
      case "max-height": {
        const table = module.levelTable(WASM_RIDGES[request.ridges]);
        let maxM = Number.NEGATIVE_INFINITY;
        for (const key of request.keys) {
          const epsilonM = table[LEVEL_TABLE_STRIDE * key.level];
          if (epsilonM === undefined) {
            throw new Error(`the level table has no level ${key.level}`);
          }
          maxM = Math.max(maxM, patchMaximumM(module, key, request.ridges) + epsilonM);
        }
        return { kind: "max-height", id: request.id, maxM };
      }
    }
  } catch (error: unknown) {
    if (error instanceof WebAssembly.RuntimeError) {
      throw error;
    }
    return {
      kind: "failed",
      id: request.id,
      message: error instanceof Error ? error.message : String(error),
    };
  }
  return unreachable(request);
}

/** What the client needs of a worker: posting, and its answers and errors. */
export interface SurfaceQueryWorker {
  postMessage(message: SurfaceQueryRequest, transfer: Transferable[]): void;
  addEventListener(type: "message", cb: (event: MessageEvent<SurfaceQueryReply>) => void): void;
  addEventListener(type: "error", cb: (event: ErrorEvent) => void): void;
  terminate(): void;
}

type Pending = (reply: SurfaceQueryReply) => void;

/**
 * The render thread's side of the query worker: each question a promise of its answer.
 *
 * @remarks
 * A worker error fails every question outstanding and every later one.
 */
export class SurfaceQuery {
  readonly #worker: SurfaceQueryWorker;
  readonly #ridges: TestPlanetRidges;
  readonly #pending = new Map<number, Pending>();
  #nextId = 1;
  #failure: Error | null = null;

  constructor(worker: SurfaceQueryWorker, ridges: TestPlanetRidges) {
    this.#worker = worker;
    this.#ridges = ridges;
    worker.addEventListener("message", (event) => {
      const answer = this.#pending.get(event.data.id);
      this.#pending.delete(event.data.id);
      answer?.(event.data);
    });
    worker.addEventListener("error", (event) => {
      this.#fail(new Error(`the surface query worker failed: ${event.message}`));
    });
  }

  /** The module's level table for the ridges given at construction. */
  async levelTable(): Promise<Float64Array> {
    const reply = await this.#ask({ kind: "level-table", id: 0, ridges: this.#ridges });
    if (reply.kind !== "level-table") {
      throw unexpected(reply);
    }
    return reply.table;
  }

  /**
   * The surface's height at the body-fixed direction `dir`, metres above the datum: R05.T4.c's
   * collision interpolant.
   */
  async heightM(dir: Xyz): Promise<number> {
    const reply = await this.#ask({
      kind: "height",
      id: 0,
      ridges: this.#ridges,
      dirs: Float64Array.from(dir),
    });
    if (reply.kind !== "height") {
      throw unexpected(reply);
    }
    const heightM = reply.heightsM[0];
    if (heightM === undefined) {
      throw new Error("the surface query answered no height");
    }
    return heightM;
  }

  /** A true upper bound of the finest surface over `keys`, metres above the datum. */
  async maxHeightM(keys: ReadonlyArray<PatchKey>): Promise<number> {
    const reply = await this.#ask({ kind: "max-height", id: 0, ridges: this.#ridges, keys });
    if (reply.kind !== "max-height") {
      throw unexpected(reply);
    }
    return reply.maxM;
  }

  /** Stops the worker; questions outstanding fail. */
  dispose(): void {
    this.#fail(new Error("the surface query was disposed"));
    this.#worker.terminate();
  }

  #ask(request: SurfaceQueryRequest): Promise<SurfaceQueryReply> {
    if (this.#failure !== null) {
      return Promise.reject(this.#failure);
    }
    const id = this.#nextId;
    this.#nextId += 1;
    return new Promise((resolve) => {
      this.#pending.set(id, resolve);
      this.#worker.postMessage({ ...request, id }, []);
    });
  }

  #fail(error: Error): void {
    this.#failure = error;
    for (const answer of this.#pending.values()) {
      answer({ kind: "failed", id: 0, message: error.message });
    }
    this.#pending.clear();
  }
}

function unexpected(reply: SurfaceQueryReply): Error {
  return new Error(
    reply.kind === "failed"
      ? `the surface query failed: ${reply.message}`
      : `the surface query answered ${reply.kind} out of turn`,
  );
}
