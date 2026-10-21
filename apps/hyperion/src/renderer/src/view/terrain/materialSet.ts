/**
 * The terrain's materials, compiled once a terrain session, when it is made (plan R10, R10.T14.d;
 * decision-r10-t14a §1, from decision-r10-t16-leans §2, amended by decision-r10-t14d-stall): R10's
 * lit material, its depth material for the symbology's pass and the wireframe's, each on the vertex
 * path its setting's passes take, so that the terrain draws on a body's hand-over's frame.
 *
 * @remarks
 * - **Why.** A pass compiled its materials at its making, the body's first hand-over, through
 *   `createRenderPipelineAsync` on Dawn's workers: the page never stalled for them, but the terrain
 *   was first drawn 2,483 ms after the hand-over on a cold driver cache (low, R10.T14.d's trace;
 *   the hand-over's stall of the page was R07's `bodies:smoothMesh`, which `LitBodyRenderer` now
 *   prepares). Compiled when the session is made, the materials are ready before any view hands a
 *   body over, and every pass of the session draws with the same handles.
 * - **What is compiled.** {@link sessionMaterials}: the lit material and its depth material on the
 *   lit layout's path, on the high setting the shadow cascades' caster beside them (R10.T9.b), and
 *   the wireframe's on the wireframe layout's, each as `terrainLayout`
 *   resolves the setting's own path against the device's limits (`baked-offsets` falling back to
 *   `face-differences` where the offsets exceed a binding), into the targets its pass draws into
 *   ({@link terrainMaterialOf}). Each is compiled against a mesh of the patches' own vertex layout
 *   (`terrainMeshSpec`), made for the compile and released after it: the engine keys a material's
 *   pipelines by the mesh's layout, the target's format and its depth, so the pipelines serve every
 *   pass's mesh of that layout.
 * - **A path not compiled** (a pass on another setting, say) is compiled at its first ask and kept
 *   for the next; a compile that fails is forgotten, so that the next ask tries again.
 * - **A device loss.** The handles die with the device; at the restore the set forgets them and
 *   compiles the setting's materials again, the caster with the new device's `depth-clip-control`
 *   (decision-r10-t9b-design D1), before any pass is told: its listener is added when the
 *   session is made, before any pass's resources add theirs, and the engine calls its listeners in
 *   the order they were added (`ResilientEngine`'s restore). A compile that ends after a restore or
 *   after the set is let go is released at once, and rejects.
 * - **Ownership.** The set owns every handle it compiles and releases them when it is let go
 *   ({@link TerrainMaterialSet.dispose}); a pass never releases one.
 */

import { EngineUnavailable } from "../engine/resilientEngine";
import type { GpuCapabilities } from "../engine/platform";
import type {
  MaterialHandle,
  RenderEngine,
  RenderTargetFormat,
  WgslMaterialSpec,
} from "../engine/types";
import {
  type QualitySetting,
  TERRAIN_SETTINGS,
  type TerrainVertexPath,
} from "../quality/qualitySetting";
import { fittedTerrainLayout, type TerrainLimits, terrainMeshSpec } from "./gpu/resources";
import type { TerrainShading } from "./slotLayout";
import {
  terrainDepthMaterialSpec,
  terrainLitMaterialSpec,
  terrainShadowMaterialSpec,
} from "./terrainMaterial";
import { terrainWireMaterialSpec, wireframeTerrainSettings } from "./terrainWire";

/**
 * A material of the terrain session's passes: R10's lit material (`lit`), its depth material for
 * the symbology's pass (`depth`, R10.T16.c), the wireframe's (`wire`, R10.T11), and the lit
 * terrain as a caster into the high setting's shadow cascades (`shadow`, R10.T9.b).
 */
export type TerrainMaterialKind = "lit" | "depth" | "wire" | "shadow";

/** A material of the terrain and a vertex path it is compiled on. */
export interface TerrainMaterialKey {
  readonly kind: TerrainMaterialKind;
  readonly path: TerrainVertexPath;
}

/** What a terrain material is compiled as ({@link terrainMaterialOf}). */
export interface TerrainMaterialMaking {
  readonly spec: WgslMaterialSpec;
  /** The targets its pass draws it into. */
  readonly targets: ReadonlyArray<RenderTargetFormat>;
  /** The shading whose mesh it draws (`terrainMeshSpec`). */
  readonly shading: TerrainShading;
}

/**
 * What a material of `kind` on `path` is compiled as.
 *
 * @remarks
 * The lit material draws into the photorealistic scene target (`rgba16float`); its depth material
 * and the wireframe's into the canvas, the symbology's pass and the wireframe's frame; the caster
 * into the shadow cascades' layers (`depth-only`), drawing the lit mesh, with `unclippedDepth`
 * where the device offers `depth-clip-control` (decision-r10-t9b-design D1).
 *
 * @param capabilities - The device's, read at each making, so that a caster made again after a
 *   restore onto another adapter takes that adapter's `depth-clip-control`.
 */
export function terrainMaterialOf(
  kind: TerrainMaterialKind,
  path: TerrainVertexPath,
  capabilities: Pick<GpuCapabilities, "depthClipControl">,
): TerrainMaterialMaking {
  let made: TerrainMaterialMaking;
  switch (kind) {
    case "lit":
      made = { spec: terrainLitMaterialSpec(path), targets: ["rgba16float"], shading: "lit" };
      break;
    case "depth":
      made = { spec: terrainDepthMaterialSpec(path), targets: ["canvas"], shading: "lit" };
      break;
    case "wire":
      made = { spec: terrainWireMaterialSpec(path), targets: ["canvas"], shading: "wire" };
      break;
    case "shadow":
      made = {
        spec: terrainShadowMaterialSpec(path, capabilities.depthClipControl),
        targets: ["depth-only"],
        shading: "lit",
      };
      break;
  }
  return made;
}

/**
 * The materials a terrain session's passes draw with on `setting`, on a device of `limits`: the lit
 * material and its depth material on the lit layout's vertex path, on the high setting the shadow
 * cascades' caster on that path too (R10.T9.b; the low setting has no cascades), and the
 * wireframe's on the wireframe layout's, as `terrainLayout` resolves each; none of a layout the
 * device cannot hold (`fittedTerrainLayout`), of which no pass can be made either.
 */
export function sessionMaterials(
  setting: QualitySetting,
  limits: TerrainLimits,
): ReadonlyArray<TerrainMaterialKey> {
  const keys: TerrainMaterialKey[] = [];
  const lit = fittedTerrainLayout(TERRAIN_SETTINGS[setting], limits, "lit");
  if (lit !== null) {
    keys.push({ kind: "lit", path: lit.vertexPath }, { kind: "depth", path: lit.vertexPath });
    if (setting === "high") {
      keys.push({ kind: "shadow", path: lit.vertexPath });
    }
  }
  const wire = fittedTerrainLayout(wireframeTerrainSettings(setting), limits, "wire");
  if (wire !== null) {
    keys.push({ kind: "wire", path: wire.vertexPath });
  }
  return keys;
}

/** What the terrain's passes ask of a session's materials. */
export interface TerrainMaterialSource {
  /**
   * The material of `kind` on `path`, once compiled: the one handle every pass that asks shares,
   * which the source owns and no pass releases.
   *
   * @throws EngineUnavailable (rejects) while the device is lost, and for a compile that a restore
   * overtook; an `Error` where the device refuses the compile or the source has been let go.
   */
  material(kind: TerrainMaterialKind, path: TerrainVertexPath): Promise<MaterialHandle>;
}

/** The part of the engine the set uses. */
export type TerrainMaterialEngine = Pick<
  RenderEngine,
  | "capabilities"
  | "createMesh"
  | "releaseMesh"
  | "createMaterialAsync"
  | "releaseMaterial"
  | "onRestored"
>;

/** The name of the mesh each compile is made against, released once the compile ends. */
export const COMPILE_MESH_NAME = "terrain compile";

/** One material asked of the set, on the device it was asked on. */
interface Entry {
  readonly promise: Promise<MaterialHandle>;
  /** The handle, once compiled; `null` before. */
  handle: MaterialHandle | null;
}

/** The key of a material in the set's map. */
function keyOf(kind: TerrainMaterialKind, path: TerrainVertexPath): string {
  return `${kind}|${path}`;
}

/** A terrain session's materials (see the module's remarks). */
export class TerrainMaterialSet implements TerrainMaterialSource {
  readonly #engine: TerrainMaterialEngine;
  readonly #setting: QualitySetting;
  readonly #entries = new Map<string, Entry>();
  readonly #offRestored: () => void;
  /** Rises at each restore, so that a compile asked on a lost device is told from a new one's. */
  #epoch = 0;
  #disposed = false;
  /**
   * Settles once the setting's materials on the first device have compiled, or failed to: its
   * value is how many compiled.
   */
  readonly compiled: Promise<number>;
  /** When that compile began, as the set was made, `performance.now()` ms. */
  readonly startedAtMs: number;

  /**
   * Starts compiling `setting`'s materials ({@link sessionMaterials}) on `engine`'s device at once,
   * and again after each restore.
   */
  constructor(engine: TerrainMaterialEngine, setting: QualitySetting) {
    this.#engine = engine;
    this.#setting = setting;
    this.#offRestored = engine.onRestored(() => {
      this.#restored();
    });
    this.startedAtMs = globalThis.performance.now();
    this.compiled = this.#compileSetting();
  }

  /** The materials it compiles on its setting on the current device ({@link sessionMaterials}). */
  get keys(): ReadonlyArray<TerrainMaterialKey> {
    return sessionMaterials(this.#setting, this.#engine.capabilities);
  }

  material(kind: TerrainMaterialKind, path: TerrainVertexPath): Promise<MaterialHandle> {
    if (this.#disposed) {
      return Promise.reject(new Error("the terrain's materials were let go"));
    }
    const key = keyOf(kind, path);
    const held = this.#entries.get(key);
    if (held !== undefined) {
      return held.promise;
    }
    const epoch = this.#epoch;
    let entry: Entry | null = null;
    const promise = this.#compile(kind, path).then(
      (handle) => {
        if (this.#disposed || epoch !== this.#epoch) {
          // Asked on a device a restore replaced, or of a set let go meanwhile: no pass draws it.
          this.#engine.releaseMaterial(handle);
          throw this.#disposed
            ? new Error("the terrain's materials were let go")
            : new EngineUnavailable("createMaterialAsync");
        }
        if (entry !== null) {
          entry.handle = handle;
        }
        return handle;
      },
      (error: unknown) => {
        if (entry !== null && this.#entries.get(key) === entry) {
          this.#entries.delete(key);
        }
        throw error;
      },
    );
    entry = { promise, handle: null };
    this.#entries.set(key, entry);
    return promise;
  }

  /**
   * Releases every material it holds and stops following restores; a compile still running is
   * released as it ends. Every pass of the session is let go first.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    this.#releaseAll();
  }

  /** Compiles every material of the setting; settles with how many compiled, never rejecting. */
  async #compileSetting(): Promise<number> {
    const settled = await Promise.allSettled(
      this.keys.map(({ kind, path }) => this.material(kind, path)),
    );
    // A failure is the pass's to report when it asks (`TerrainPass.ready`), as before the set.
    return settled.filter((each) => each.status === "fulfilled").length;
  }

  /** Compiles `kind` on `path` against a mesh of its pass's layout, made and released for it. */
  async #compile(kind: TerrainMaterialKind, path: TerrainVertexPath): Promise<MaterialHandle> {
    const { spec, targets, shading } = terrainMaterialOf(kind, path, this.#engine.capabilities);
    const mesh = this.#engine.createMesh({ ...terrainMeshSpec(shading), name: COMPILE_MESH_NAME });
    try {
      return await this.#engine.createMaterialAsync(spec, targets, [mesh]);
    } finally {
      this.#engine.releaseMesh(mesh);
    }
  }

  /**
   * After a device loss: the lost device's handles forgotten, and the setting compiled again, its
   * entries in place before this listener returns, so that a pass's resources rebuilt by a later
   * listener ask for the new device's.
   */
  #restored(): void {
    this.#epoch += 1;
    try {
      this.#releaseAll();
    } finally {
      void this.#compileSetting().catch((error: unknown) => {
        console.error("the terrain's materials could not be compiled again:", error);
      });
    }
  }

  /** Releases every compiled handle and forgets every entry; one release's throw stops none. */
  #releaseAll(): void {
    const held = [...this.#entries.values()];
    this.#entries.clear();
    for (const entry of held) {
      if (entry.handle !== null) {
        try {
          // After a loss its release is dropped, since it died with its device.
          this.#engine.releaseMaterial(entry.handle);
        } catch (error: unknown) {
          console.error(
            `the terrain's material ${entry.handle.name} could not be released:`,
            error,
          );
        }
      }
    }
  }
}
