/**
 * The decorative scatter's kernels, material and templates, compiled once a terrain session on a
 * setting that draws scatter (plan R11, R11.T6.a; R10.T14.d's rule that a hand-over compiles
 * nothing), shared by every body's lit pass.
 *
 * @remarks
 * - **Made only where drawn** ({@link ScatterKernelSet.forSetting}): the low setting, a density of
 *   0 and the geometry's `scatter` `off` make none, so that no scatter pipeline is created there
 *   (R11.T6.b).
 * - **After the terrain's own.** On the first device its compile starts once `after` settles (the
 *   session's material set's), so that the terrain's materials, which a hand-over's first frame
 *   waits for, are never queued behind the pebbles'. After a restore it compiles at once, beside
 *   the set's own compiles again (a lean: the set gives no promise of a restore's).
 * - **What it holds**: the placement on the session's lit vertex path, the particle path's instance
 *   cull, the pebbles' material on that path and the templates' mesh it was compiled against.
 * - **A device loss.** At the restore the lost handles are dropped and all of it is made again. A
 *   compile belongs to the device it was scheduled on: one scheduled before a restore and not yet
 *   started makes nothing, one settling after a restore holds the lost engine's handles, which
 *   `ResilientEngine` drops, and one settling after the set is let go is released at once. It never
 *   rejects: a failure is reported (but a lost device's `EngineUnavailable`) and settles
 *   {@link ScatterKernelSet.ready} `false`, when no pass draws pebbles.
 */

import { INSTANCE_CULL_KERNEL } from "../../clusters/instanceCull";
import type { ParticleTemplates } from "../../clusters/particles";
import { EngineUnavailable } from "../../engine/resilientEngine";
import type { ComputeHandle, MaterialHandle, MeshHandle, RenderEngine } from "../../engine/types";
import type { TerrainVertexPath } from "../../quality/qualitySetting";
import type { R11Ladder } from "../decoration/decorationSetting";
import type { GeometrySettings } from "../../quality/qualitySetting";
import { PEBBLE_MATERIALS, pebbleTemplates } from "./pebbleMaterial";
import { scatterDensity } from "./pebbleLaw";
import { PEBBLE_KERNELS } from "./pebblePlace";

/** The compiled handles a scatter pass draws with. */
export interface ScatterKernels {
  /** The vertex path they were made for: the lit layout's. */
  readonly path: TerrainVertexPath;
  readonly place: ComputeHandle;
  readonly cull: ComputeHandle;
  readonly material: MaterialHandle;
  /** The templates' mesh, which the material was compiled against. */
  readonly mesh: MeshHandle;
  readonly templates: ParticleTemplates;
}

/** Where a scatter pass finds its handles: `null` until they are compiled, or where none can be. */
export interface ScatterKernelSource {
  readonly kernels: ScatterKernels | null;
}

/** What the set asks of the engine. */
export type ScatterKernelEngine = Pick<
  RenderEngine,
  | "createComputeAsync"
  | "releaseCompute"
  | "createMaterialAsync"
  | "releaseMaterial"
  | "createMesh"
  | "releaseMesh"
  | "onRestored"
>;

/**
 * Whether a setting's ladder draws decorative scatter: decoration on, a density above 0, and the
 * geometry's `scatter` not `off` (the Mac pair's B side).
 */
export function scatterDrawn(
  ladder: Pick<R11Ladder, "decoration" | "scatterDensity">,
  geometry: Pick<GeometrySettings, "scatter">,
): boolean {
  return (
    ladder.decoration && scatterDensity(ladder.scatterDensity) > 0 && geometry.scatter !== "off"
  );
}

/** The scatter's handles, compiled once a session (see the module's remarks). */
export class ScatterKernelSet implements ScatterKernelSource {
  readonly path: TerrainVertexPath;
  readonly #engine: ScatterKernelEngine;
  readonly #templates = pebbleTemplates();
  readonly #offRestored: () => void;
  #kernels: ScatterKernels | null = null;
  #epoch = 0;
  #disposed = false;
  #ready: Promise<boolean>;

  /**
   * The set of `engine`'s handles on `path` where `ladder` and `geometry` draw scatter, else
   * `null`, its compile after `after`.
   */
  static forSetting(
    ladder: Pick<R11Ladder, "decoration" | "scatterDensity">,
    geometry: Pick<GeometrySettings, "scatter">,
    path: TerrainVertexPath,
    engine: ScatterKernelEngine,
    after: Promise<unknown> = Promise.resolve(),
  ): ScatterKernelSet | null {
    return scatterDrawn(ladder, geometry) ? new ScatterKernelSet(engine, path, after) : null;
  }

  /** Starts compiling `path`'s handles on `engine` once `after` settles, and again at each restore. */
  constructor(
    engine: ScatterKernelEngine,
    path: TerrainVertexPath,
    after: Promise<unknown> = Promise.resolve(),
  ) {
    this.#engine = engine;
    this.path = path;
    // The first device's compile, which a restore before `after` settles leaves to its own.
    const epoch = this.#epoch;
    this.#ready = after.then(
      async () => this.#compiled(epoch),
      async () => this.#compiled(epoch),
    );
    this.#offRestored = engine.onRestored(() => {
      // The lost device's handles went with it: `ResilientEngine` drops their release.
      this.#kernels = null;
      this.#epoch += 1;
      this.#ready = this.#compiled(this.#epoch);
    });
  }

  /** The handles, or `null` until they are compiled, after a failed compile and once let go. */
  get kernels(): ScatterKernels | null {
    return this.#kernels;
  }

  /** Settles `true` once the current device's handles are compiled, `false` where they are not. */
  get ready(): Promise<boolean> {
    return this.#ready;
  }

  /** The compile scheduled on the device of `epoch`, its failure reported: it never rejects. */
  async #compiled(epoch: number): Promise<boolean> {
    try {
      return await this.#compile(epoch);
    } catch (error: unknown) {
      if (!(error instanceof EngineUnavailable)) {
        console.error("the decorative scatter's kernels could not be compiled:", error);
      }
      return false;
    }
  }

  async #compile(epoch: number): Promise<boolean> {
    // Let go, or a restore since it was scheduled, whose own compile owns the new device.
    if (this.#disposed || epoch !== this.#epoch) {
      return false;
    }
    const engine = this.#engine;
    const mesh = engine.createMesh(this.#templates.mesh);
    // Each ask in an async function, so that one that throws at once is a rejection like the rest.
    const [place, cull, material] = await Promise.allSettled([
      (async () => engine.createComputeAsync(PEBBLE_KERNELS[this.path]))(),
      (async () => engine.createComputeAsync(INSTANCE_CULL_KERNEL))(),
      (async () =>
        engine.createMaterialAsync(PEBBLE_MATERIALS[this.path], ["rgba16float"], [mesh]))(),
    ]);
    if (
      this.#alive() &&
      epoch === this.#epoch &&
      place.status === "fulfilled" &&
      cull.status === "fulfilled" &&
      material.status === "fulfilled"
    ) {
      this.#release();
      this.#kernels = {
        path: this.path,
        place: place.value,
        cull: cull.value,
        material: material.value,
        mesh,
        templates: this.#templates,
      };
      return true;
    }
    // A compile of the lost device's, settled after the restore, holds the lost engine's handles,
    // which `ResilientEngine` drops at the loss: none is released.
    if (epoch === this.#epoch) {
      for (const outcome of [place, cull]) {
        if (outcome.status === "fulfilled") {
          engine.releaseCompute(outcome.value);
        }
      }
      if (material.status === "fulfilled") {
        engine.releaseMaterial(material.value);
      }
      engine.releaseMesh(mesh);
      for (const outcome of [place, cull, material]) {
        if (
          outcome.status === "rejected" &&
          this.#alive() &&
          !(outcome.reason instanceof EngineUnavailable)
        ) {
          console.error("the decorative scatter could not compile: it draws none", outcome.reason);
        }
      }
    }
    return false;
  }

  /** Whether it is still held: a compile settling after its release keeps nothing. */
  #alive(): boolean {
    return !this.#disposed;
  }

  /** Releases the handles held, if any. */
  #release(): void {
    const kernels = this.#kernels;
    this.#kernels = null;
    if (kernels !== null) {
      this.#engine.releaseCompute(kernels.place);
      this.#engine.releaseCompute(kernels.cull);
      this.#engine.releaseMaterial(kernels.material);
      this.#engine.releaseMesh(kernels.mesh);
    }
  }

  /** Releases its handles and stops following restores; idempotent. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    this.#release();
  }
}
