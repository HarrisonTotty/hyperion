/**
 * A lit terrain pass's decorative scatter (plan R11, R11.T6.a; Design notes 2, 7 and 18): its
 * pebbles' placement, their cull and their draws through the particle path (plan R14, R14.T5.d),
 * on the high setting alone.
 *
 * @remarks
 * - **Made only where drawn.** A terrain pass makes one only where its session gives it the
 *   scatter's handles (`ScatterKernelSet`, made only where the setting draws decorative scatter,
 *   `scatterDrawn`): the low setting, a density of 0 and the geometry's `scatter` `off` make no
 *   kernel, material, mesh or buffer of it (R11.T6.b). It compiles nothing: until the session's set
 *   is compiled, and wherever it is refused, it draws nothing.
 * - **A frame** ({@link ScatterPass.frame}): the count zeroed, one dispatch of the placement over
 *   the frame's entries (`pebblePlace.ts`), labelled {@link SCATTER_PASS_LABEL}, then the particle
 *   path's instance cull against the view's camera ({@link SCATTER_CULL_LABEL}), both submitted
 *   before the draws they feed; the draws are one a template, the pebbles' material over the lit
 *   terrain draw's bindings with the path's buffers and the patches' records beside, drawn after the
 *   terrain's draws that never discard, in the bodies pass.
 * - **Capacity**: {@link PEBBLE_CAPACITY} times the density, rounded up, so that a lower density
 *   holds less; the instances and count are `scatter`, the path's lists and arguments
 *   `geometry-clusters` (32 MB and 16 MB at a density of 1, one pass, four templates).
 * - **No restore record.** A device's loss ends it with the terrain pass's device; the pass makes
 *   another on the restored device, and the session's set compiles its handles again.
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { BucketDrawBase } from "../../clusters/buckets";
import type { CameraCullPass } from "../../clusters/cullPass";
import { ParticlePath } from "../../clusters/particlePath";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import type { BufferHandle, DrawItem, MaterialHandle, RenderEngine } from "../../engine/types";
import { pebbleTemplates, SCATTER_PATCHES_BUFFER } from "./pebbleMaterial";
import { scatterDensity } from "./pebbleLaw";
import {
  MAX_PEBBLE_ENTRIES,
  MAX_SCATTER_PATCHES,
  PEBBLE_ENTRY_BYTES,
  PEBBLE_OCTAVES_BYTES,
  PEBBLE_PARAMS_BYTES,
  PEBBLE_PATH_BUFFER,
  SCATTER_PASS_LABEL,
  SCATTER_PATCH_BYTES,
  type ScatterPatchRecord,
  packPebbleEntries,
  packPebbleOctaves,
  packPebbleParams,
  packScatterPatches,
  pebbleWorkgroups,
  scatterEntries,
} from "./pebblePlace";
import {
  MAX_SCATTER_BODIES,
  packScatterBoxes,
  SCATTER_BOX_BYTES,
  type ScatterBox,
} from "./scatterCull";
import type { ScatterKernelSource } from "./scatterKernels";

export { scatterDrawn } from "./scatterKernels";

/** The most pebbles a pass holds at a density of 1: 2²⁰, four templates' lists within 2²⁴. */
export const PEBBLE_CAPACITY = 1_048_576;

/** The instance cull's label over the pebbles. */
export const SCATTER_CULL_LABEL = "scatter cull";

/** The part of the engine a scatter pass uses. */
export type ScatterPassEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "releaseBuffer" | "dispatch"
>;

/** The terrain's buffers the placement reads, the lit pass's device's. */
export interface ScatterGround {
  /** The slots' heights, on `face-differences`, or their baked offsets, on `baked-offsets`. */
  readonly vertices: BufferHandle;
  readonly slots: BufferHandle;
  readonly classLayers: BufferHandle;
}

/** One view-frame of a body's scatter. */
export interface ScatterFrameInput {
  /** The drawn patches it scatters over, nearest the camera first. */
  readonly patches: ReadonlyArray<ScatterPatchRecord>;
  /** Each patch's nearest point's distance from the camera, m. */
  readonly nearestM: ReadonlyArray<number>;
  /** Each octave's lattice level on the body (`pebbleLatticeLevels`). */
  readonly levels: ReadonlyArray<number>;
  /** The rows of the body's rotation, body-fixed axes into the passes' frame. */
  readonly rotationRows: readonly [Vec3, Vec3, Vec3];
  /** The view's px per rad (`decorationPxPerRad`). */
  readonly pxPerRad: number;
  /** The body's decoration seed (`decorationSeed`). */
  readonly seed: number;
  /** The bodies' boxes within 0.2 m of the ground, the passes' frame, nearest first. */
  readonly bodies: ReadonlyArray<ScatterBox>;
  /** The view's camera as the instance cull's pass. */
  readonly cull: CameraCullPass;
  /** The view axis in the passes' frame. */
  readonly viewAxis: Vec3;
  readonly ground: ScatterGround;
  /** The lit terrain draw whose bindings the pebbles' draws take. */
  readonly base: BucketDrawBase;
}

/** What one view-frame's scatter drew. */
export interface ScatterFrame {
  readonly draws: ReadonlyArray<DrawItem>;
  /** The patches and entries it dispatched over: 0 where it drew nothing. */
  readonly patches: number;
  readonly entries: number;
}

/** A frame that draws no scatter. */
const NO_SCATTER: ScatterFrame = { draws: [], patches: 0, entries: 0 };

/** A lit terrain pass's decorative scatter (see the module's remarks). */
export class ScatterPass {
  readonly path: ParticlePath;
  readonly density: number;
  readonly #engine: ScatterPassEngine;
  readonly #source: ScatterKernelSource;
  readonly #entries: BufferHandle;
  readonly #patches: BufferHandle;
  readonly #params = new ArrayBuffer(PEBBLE_PARAMS_BYTES);
  readonly #octaves = new Float32Array(PEBBLE_OCTAVES_BYTES / 4);
  readonly #bodies = new Float32Array((MAX_SCATTER_BODIES * SCATTER_BOX_BYTES) / 4);
  readonly #entryWords = new Uint32Array((MAX_PEBBLE_ENTRIES * PEBBLE_ENTRY_BYTES) / 4);
  readonly #patchBytes = new ArrayBuffer(MAX_SCATTER_PATCHES * SCATTER_PATCH_BYTES);
  readonly #bases = new WeakMap<BucketDrawBase, BucketDrawBase>();
  #disposed = false;

  /**
   * Makes its path and buffers over `source`'s handles, which it compiles none of.
   *
   * @param density - The setting's `scatterDensity`, above 0.
   * @throws RangeError for a density not above 0 (no scatter is made at 0).
   */
  constructor(
    engine: ScatterPassEngine,
    name: string,
    source: ScatterKernelSource,
    density: number,
  ) {
    const d = scatterDensity(density);
    if (!(d > 0)) {
      throw new RangeError(`no scatter is made at a density of ${String(density)}`);
    }
    this.#engine = engine;
    this.#source = source;
    this.density = d;
    this.path = new ParticlePath(
      engine,
      `${name} pebbles`,
      pebbleTemplates(),
      Math.ceil(PEBBLE_CAPACITY * d),
      1,
      "scatter",
    );
    this.#entries = engine.createBuffer({
      name: `${name} pebble entries`,
      bytes: this.#entryWords.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "scatter",
    });
    this.#patches = engine.createBuffer({
      name: `${name} pebble patches`,
      bytes: this.#patchBytes.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "scatter",
    });
    packPebbleOctaves(this.#octaves);
  }

  /** Whether its handles are compiled, so that its frames draw. */
  get ready(): boolean {
    return !this.#disposed && this.#source.kernels !== null;
  }

  /**
   * One view-frame's scatter (see the module's remarks): its placement's and cull's dispatches, and
   * the pebbles' draws; nothing before its handles are compiled, with no entry to place, or once
   * disposed.
   */
  frame(input: ScatterFrameInput): ScatterFrame {
    const kernels = this.#source.kernels;
    if (this.#disposed || kernels === null) {
      return NO_SCATTER;
    }
    const patches = input.patches.slice(0, MAX_SCATTER_PATCHES);
    const entries = scatterEntries(
      patches.map((p, n) => ({ key: p.key, nearestM: input.nearestM[n] ?? 0 })),
      input.levels,
      input.pxPerRad,
    );
    if (entries.length === 0) {
      return NO_SCATTER;
    }
    const used = (entries.at(-1)?.patch ?? 0) + 1;
    packScatterPatches(patches.slice(0, used), this.#patchBytes);
    packPebbleEntries(entries, this.#entryWords);
    const bodies = packScatterBoxes(input.bodies, this.#bodies);
    packPebbleParams(
      {
        rotationRows: input.rotationRows,
        pxPerRad: input.pxPerRad,
        density: this.density,
        entries: entries.length,
        capacity: this.path.capacity,
        seed: input.seed,
        bodies,
      },
      this.#params,
    );
    const engine = this.#engine;
    engine.writeBuffer(
      this.#patches,
      0,
      new Uint8Array(this.#patchBytes, 0, used * SCATTER_PATCH_BYTES),
    );
    engine.writeBuffer(this.#entries, 0, this.#entryWords.subarray(0, 4 * entries.length));
    this.path.writeCount(0);
    engine.dispatch(
      kernels.place,
      {
        uniforms: {
          params: new Uint32Array(this.#params),
          octaves: this.#octaves,
          bodies: this.#bodies,
        },
        buffers: {
          entries: this.#entries,
          patches: this.#patches,
          slots: input.ground.slots,
          class_layers: input.ground.classLayers,
          [PEBBLE_PATH_BUFFER[kernels.path]]: input.ground.vertices,
          instances: this.path.instances,
          instance_count: this.path.count,
        },
        sampled: {},
        storage: {},
      },
      pebbleWorkgroups(entries.length),
      SCATTER_PASS_LABEL,
    );
    this.path.cull(
      kernels.cull,
      { passes: [input.cull], viewAxis: input.viewAxis },
      SCATTER_CULL_LABEL,
    );
    return {
      draws: this.path.drawItems(0, kernels.mesh, this.#baseOf(input.base, kernels.material)),
      patches: used,
      entries: entries.length,
    };
  }

  /** The pebbles' draw base over the lit draw `lit`: its bindings, the material, the patches. */
  #baseOf(lit: BucketDrawBase, material: MaterialHandle): BucketDrawBase {
    let base = this.#bases.get(lit);
    if (base === undefined || base.material !== material) {
      base = {
        material,
        offsetFromCameraM: lit.offsetFromCameraM,
        uniforms: lit.uniforms,
        textures: lit.textures,
        storageBuffers: { ...lit.storageBuffers, [SCATTER_PATCHES_BUFFER]: this.#patches },
      };
      this.#bases.set(lit, base);
    }
    return base;
  }

  /** Releases its path and buffers; nothing more is placed or drawn. The handles are the set's. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.path.dispose();
    this.#engine.releaseBuffer(this.#entries);
    this.#engine.releaseBuffer(this.#patches);
  }
}
