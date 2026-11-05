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
 * - **What is compiled.** {@link sessionMaterials}: the lit material, its interior patches' form
 *   (plan R14, R14.T1.a) and its depth material on the lit layout's path, on the high setting the
 *   shadow cascades' caster beside them (R10.T9.b), and the wireframe's on the wireframe layout's,
 *   each as `terrainLayout` resolves the setting's own path against the device's limits
 *   (`baked-offsets` falling back to `face-differences` where the offsets exceed a binding), into
 *   the targets its pass draws into ({@link terrainMaterialOf}). Each is compiled against a mesh of
 *   the patches' own vertex layout (`terrainMeshSpec`), made for the compile and released after it:
 *   the engine keys a material's pipelines by the mesh's layout, the target's format and its depth,
 *   so the pipelines serve every pass's mesh of that layout.
 * - **Two at a time, first use first** (plan R14's Mac measurements; a lean, 2026-11-03). The
 *   twelve of the high setting, sixteen pipelines, were asked at once: twelve of the twenty
 *   compiles in flight on the GPU process's thread pool in a session's first seconds (pod,
 *   SwiftShader, 2026-11-03), which Metal's compiler takes as wide. So the set compiles
 *   {@link TERRAIN_COMPILES_IN_FLIGHT} at once, the rest waiting in
 *   the setting's order of first use ({@link sessionMaterials}): the lit material first, which a
 *   pass's first frame waits for, then the forms the setting's geometry draws, then their fallbacks
 *   and the wireframe's. A material a pass asks that the set does not hold (an emissive form,
 *   another path, a variant) waits ahead of the setting's, in the order asked, and a pass's ask of
 *   the material its first frame waits for (`lit`, `wire`) goes first of all. The materials' still
 *   forms (plan R10, R10.T16.i; `stillMaterialSpec`) follow them all, in the same order, and no
 *   pass's ask hurries one: a frame draws a still form only once its instances mix no heights,
 *   after the material it stands for has drawn the session's first frames, whose ground rises out
 *   of the datum. Until a form is made
 *   the pass draws by the one it falls back on, and until a caster is, with no shadow (w = 0), as
 *   where one cannot be made. Each pipeline of a compile then waits its turn in the device's one
 *   budget (`engine/webgpu/compileBudget.ts`), beside every other compile of the page, so that the
 *   set's two keep their order there.
 * - **A path not compiled** (a pass on another setting, say) is compiled at its first ask and kept
 *   for the next; a compile that fails is forgotten, so that the next ask tries again.
 * - **A device loss.** The handles die with the device; at the restore the set forgets them and
 *   compiles the setting's materials again, the caster with the new device's `depth-clip-control`
 *   (decision-r10-t9b-design D1), before any pass is told: its listener is added when the
 *   session is made, before any pass's resources add theirs, and the engine calls its listeners in
 *   the order they were added (`ResilientEngine`'s restore). A compile that ends after a restore or
 *   after the set is let go is released at once, and rejects; one still waiting then is refused,
 *   never started.
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
import { stillMaterialSpec } from "./gpu/material";
import { fittedTerrainLayout, type TerrainLimits, terrainMeshSpec } from "./gpu/resources";
import type { TerrainShading } from "./slotLayout";
import {
  terrainClusterDepthMaterialSpec,
  terrainClusterIdMaterialSpec,
  terrainClusterShadowMaterialSpec,
  terrainDepthMaterialSpec,
  terrainFinDepthMaterialSpec,
  terrainFinShadowMaterialSpec,
  terrainLitEmissiveInteriorMaterialSpec,
  terrainLitEmissiveMaterialSpec,
  terrainLitClusterMaterialSpec,
  terrainLitFinMaterialSpec,
  terrainLitInteriorMaterialSpec,
  terrainLitMaterialSpec,
  terrainShadowMaterialSpec,
} from "./terrainMaterial";
import {
  terrainWireClusterMaterialSpec,
  terrainWireMaterialSpec,
  wireframeTerrainSettings,
} from "./terrainWire";

/**
 * A material of the terrain session's passes: R10's lit material (`lit`), its emissive form for a
 * body above R08's visibility cut (`lit-emissive`, R08.T18.a), compiled at its first ask, the two
 * without the survey discard for the interior patches (`lit-interior` and `lit-emissive-interior`,
 * plan R14, R14.T1.a), its depth material for the symbology's pass (`depth`, R10.T16.c), the
 * wireframe's (`wire`, R10.T11), the lit terrain as a caster into the high setting's shadow
 * cascades (`shadow`, R10.T9.b), its interior patches as casters on the cluster path
 * (`shadow-cluster`, plan R14, R14.T1.b), their depth for the symbology's pass on it
 * (`depth-cluster`, plan R14, R14.T1.e), their lit form and its emissive form on it
 * (`lit-cluster`, `lit-emissive-cluster`, plan R14, R14.T2.a), the wireframe's interior patches
 * on it (`wire-cluster`, plan R14, R14.T4.c), and the edge patches' may-discard clusters' lit form,
 * its emissive form, depth and caster on it (`lit-fin`, `lit-emissive-fin`, `depth-fin`,
 * `shadow-fin`, plan R14, R14.T1.g), and the visibility buffer's ID raster of the interior patches
 * on it, de-indexed or by `primitive_index` (`id-cluster`, `id-cluster-primitive`, plan R14,
 * R14.T2.c).
 */
export type TerrainMaterialKind =
  | "lit"
  | "lit-emissive"
  | "lit-interior"
  | "lit-emissive-interior"
  | "lit-cluster"
  | "lit-emissive-cluster"
  | "lit-fin"
  | "lit-emissive-fin"
  | "depth"
  | "depth-cluster"
  | "depth-fin"
  | "wire"
  | "wire-cluster"
  | "shadow"
  | "shadow-cluster"
  | "shadow-fin"
  | "id-cluster"
  | "id-cluster-primitive";

/** A material of the terrain and a vertex path it is compiled on. */
export interface TerrainMaterialKey {
  readonly kind: TerrainMaterialKind;
  readonly path: TerrainVertexPath;
  /** Its still form (plan R10, R10.T16.i; {@link terrainMaterialOf}); absent for the material. */
  readonly still?: true;
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
 * What a material of `kind` on `path` is compiled as, or its still form (`still`; plan R10,
 * R10.T16.i, `stillMaterialSpec`), drawn on a frame whose instances mix no heights.
 *
 * @remarks
 * The lit material draws into the photorealistic scene target (`rgba16float`); its depth material
 * and the wireframe's into the canvas, the symbology's pass and the wireframe's frame; the caster
 * into the shadow cascades' layers (`depth-only`), drawing the lit mesh, with `unclippedDepth`
 * where the device offers `depth-clip-control` (decision-r10-t9b-design D1); the ID raster into
 * the visibility buffer's `r32uint` (plan R14, R14.T2.c).
 *
 * @param capabilities - The device's, read at each making, so that a caster made again after a
 *   restore onto another adapter takes that adapter's `depth-clip-control`.
 */
export function terrainMaterialOf(
  kind: TerrainMaterialKind,
  path: TerrainVertexPath,
  capabilities: Pick<GpuCapabilities, "depthClipControl">,
  still = false,
): TerrainMaterialMaking {
  let made: TerrainMaterialMaking;
  switch (kind) {
    case "lit":
      made = { spec: terrainLitMaterialSpec(path), targets: ["rgba16float"], shading: "lit" };
      break;
    case "lit-emissive":
      made = {
        spec: terrainLitEmissiveMaterialSpec(path),
        targets: ["rgba16float"],
        shading: "lit",
      };
      break;
    case "lit-interior":
      made = {
        spec: terrainLitInteriorMaterialSpec(path),
        targets: ["rgba16float"],
        shading: "lit",
      };
      break;
    case "lit-emissive-interior":
      made = {
        spec: terrainLitEmissiveInteriorMaterialSpec(path),
        targets: ["rgba16float"],
        shading: "lit",
      };
      break;
    case "lit-cluster":
    case "lit-emissive-cluster":
      made = {
        spec: terrainLitClusterMaterialSpec(path, kind === "lit-emissive-cluster"),
        targets: ["rgba16float"],
        shading: "lit",
      };
      break;
    case "lit-fin":
    case "lit-emissive-fin":
      made = {
        spec: terrainLitFinMaterialSpec(path, kind === "lit-emissive-fin"),
        targets: ["rgba16float"],
        shading: "lit",
      };
      break;
    case "depth":
      made = { spec: terrainDepthMaterialSpec(path), targets: ["canvas"], shading: "lit" };
      break;
    case "depth-fin":
      made = { spec: terrainFinDepthMaterialSpec(path), targets: ["canvas"], shading: "lit" };
      break;
    case "depth-cluster":
      made = { spec: terrainClusterDepthMaterialSpec(path), targets: ["canvas"], shading: "lit" };
      break;
    case "wire":
      made = { spec: terrainWireMaterialSpec(path), targets: ["canvas"], shading: "wire" };
      break;
    case "wire-cluster":
      made = { spec: terrainWireClusterMaterialSpec(path), targets: ["canvas"], shading: "wire" };
      break;
    case "shadow":
      made = {
        spec: terrainShadowMaterialSpec(path, capabilities.depthClipControl),
        targets: ["depth-only"],
        shading: "lit",
      };
      break;
    case "shadow-cluster":
      made = {
        spec: terrainClusterShadowMaterialSpec(path, capabilities.depthClipControl),
        targets: ["depth-only"],
        shading: "lit",
      };
      break;
    case "shadow-fin":
      made = {
        spec: terrainFinShadowMaterialSpec(path, capabilities.depthClipControl),
        targets: ["depth-only"],
        shading: "lit",
      };
      break;
    case "id-cluster":
    case "id-cluster-primitive":
      made = {
        spec: terrainClusterIdMaterialSpec(path, kind === "id-cluster-primitive"),
        targets: ["r32uint"],
        shading: "lit",
      };
      break;
  }
  return still ? { ...made, spec: stillMaterialSpec(made.spec) } : made;
}

/**
 * The materials a terrain session's passes draw with on `setting`, on a device of `limits`, in the
 * order the setting's passes first draw them, which the set compiles them in: on the lit layout's
 * vertex path, the lit material, which a lit pass's first frame waits for; its interior patches'
 * form on the cluster path (plan R14, R14.T2.a) and the edge patches' fin form (R14.T1.g), which the
 * setting's geometry draws them by; on the high setting the shadow cascades' casters on that path,
 * the interior patches' cluster caster (R14.T1.b) and its fin form (R10.T9.b; the low setting has no
 * cascades); the overlay's depth on that path, the interior patches' cluster depth (R14.T1.e) and
 * its fin form; then the forms the pass falls back on while those are made or where they cannot be,
 * the interior patches' own mesh (R14.T1.a), the caster and the depth material by the lit mesh; and
 * last, on the wireframe layout's path, the wireframe's and its interior patches' on the cluster path
 * (plan R14, R14.T4.c), which an instrument's view draws; then each one's still form, in the same
 * order (plan R10, R10.T16.i), which a frame draws only once its instances mix no heights. Each path
 * is as `terrainLayout` resolves it; none of a layout the device cannot hold
 * (`fittedTerrainLayout`), of which no pass can be made either.
 */
export function sessionMaterials(
  setting: QualitySetting,
  limits: TerrainLimits,
): ReadonlyArray<TerrainMaterialKey> {
  const keys: TerrainMaterialKey[] = [];
  const lit = fittedTerrainLayout(TERRAIN_SETTINGS[setting], limits, "lit");
  if (lit !== null) {
    const path = lit.vertexPath;
    const cascades = setting === "high";
    keys.push(
      { kind: "lit", path },
      { kind: "lit-cluster", path },
      { kind: "lit-fin", path },
      ...(cascades
        ? ([
            { kind: "shadow-cluster", path },
            { kind: "shadow-fin", path },
          ] as const)
        : []),
      { kind: "depth-cluster", path },
      { kind: "depth-fin", path },
      { kind: "lit-interior", path },
      ...(cascades ? ([{ kind: "shadow", path }] as const) : []),
      { kind: "depth", path },
    );
  }
  const wire = fittedTerrainLayout(wireframeTerrainSettings(setting), limits, "wire");
  if (wire !== null) {
    keys.push(
      { kind: "wire", path: wire.vertexPath },
      { kind: "wire-cluster", path: wire.vertexPath },
    );
  }
  const stills: TerrainMaterialKey[] = [];
  for (const { kind, path } of keys) {
    stills.push({ kind, path, still: true });
  }
  return [...keys, ...stills];
}

/** What the terrain's passes ask of a session's materials. */
export interface TerrainMaterialSource {
  /**
   * The material of `kind` on `path`, or its still form (`still`; {@link terrainMaterialOf}), once
   * compiled: the one handle every pass that asks shares, which the source owns and no pass
   * releases.
   *
   * @throws EngineUnavailable (rejects) while the device is lost, and for a compile that a restore
   * overtook; an `Error` where the device refuses the compile or the source has been let go.
   */
  material(
    kind: TerrainMaterialKind,
    path: TerrainVertexPath,
    still?: boolean,
  ): Promise<MaterialHandle>;
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

/**
 * The compiles a set runs at once, the rest waiting their turn (a lean, 2026-11-03; see the module's
 * remarks): two, so that a session's start asks the GPU process for two compiles at a time, not
 * twelve.
 */
export const TERRAIN_COMPILES_IN_FLIGHT = 2;

/** The kinds a pass's first frame waits for (`TerrainPass.ready`): an ask of one waiting goes first. */
const FIRST_FRAME_KINDS: ReadonlySet<TerrainMaterialKind> = new Set(["lit", "wire"]);

/** A material asked of the set whose compile waits for its turn. */
interface Waiting {
  readonly key: string;
  /** Starts its compile, which takes one of the {@link TERRAIN_COMPILES_IN_FLIGHT}. */
  readonly start: () => void;
  /** Refuses it, unstarted: at a restore, or when the set is let go. */
  readonly refuse: (error: unknown) => void;
}

/** One material asked of the set, on the device it was asked on. */
interface Entry {
  readonly promise: Promise<MaterialHandle>;
  /** The handle, once compiled; `null` before. */
  handle: MaterialHandle | null;
}

/** The key of a material, or of its still form, in the set's map. */
function keyOf(kind: TerrainMaterialKind, path: TerrainVertexPath, still: boolean): string {
  return `${kind}|${path}${still ? "|still" : ""}`;
}

/** A terrain session's materials (see the module's remarks). */
export class TerrainMaterialSet implements TerrainMaterialSource {
  readonly #engine: TerrainMaterialEngine;
  readonly #setting: QualitySetting;
  readonly #entries = new Map<string, Entry>();
  readonly #offRestored: () => void;
  /** The asks of passes waiting for a compile, in the order asked, each before the setting's. */
  readonly #asked: Waiting[] = [];
  /** The setting's materials waiting for a compile, in its order of first use. */
  readonly #queued: Waiting[] = [];
  /** The compiles running: at most {@link TERRAIN_COMPILES_IN_FLIGHT}, a lost device's among them. */
  #inFlight = 0;
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
   * Starts compiling `setting`'s materials ({@link sessionMaterials}) on `engine`'s device, in
   * their order, {@link TERRAIN_COMPILES_IN_FLIGHT} at a time, and again after each restore.
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

  material(
    kind: TerrainMaterialKind,
    path: TerrainVertexPath,
    still = false,
  ): Promise<MaterialHandle> {
    return this.#ask(kind, path, "pass", still);
  }

  /**
   * `kind` on `path`, or its still form: its entry's promise where it has one, else a new entry
   * whose compile waits its turn, a pass's ask ahead of the setting's (see the module's remarks).
   */
  #ask(
    kind: TerrainMaterialKind,
    path: TerrainVertexPath,
    by: "pass" | "setting",
    still: boolean,
  ): Promise<MaterialHandle> {
    if (this.#disposed) {
      return Promise.reject(new Error("the terrain's materials were let go"));
    }
    const key = keyOf(kind, path, still);
    // A first frame waits for the material, never for its still form.
    const first = !still && FIRST_FRAME_KINDS.has(kind);
    const held = this.#entries.get(key);
    if (held !== undefined) {
      if (by === "pass" && first) {
        this.#hurry(key);
      }
      return held.promise;
    }
    const epoch = this.#epoch;
    let entry: Entry | null = null;
    const compiled = new Promise<MaterialHandle>((resolve, reject) => {
      const waiting: Waiting = {
        key,
        start: () => {
          this.#inFlight += 1;
          void this.#compile(kind, path, still)
            .then(resolve, reject)
            .finally(() => {
              this.#inFlight -= 1;
              this.#startWaiting();
            })
            .catch((error: unknown) => {
              // The compile's own outcome went to its entry; only the queue's step can land here.
              console.error("the terrain's material queue could not go on:", error);
            });
        },
        refuse: reject,
      };
      if (by === "setting") {
        this.#queued.push(waiting);
      } else if (first) {
        this.#asked.unshift(waiting);
      } else {
        this.#asked.push(waiting);
      }
    });
    const promise = compiled.then(
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
    this.#startWaiting();
    return promise;
  }

  /** Moves `key`'s compile, where it still waits, to the head of the queue. */
  #hurry(key: string): void {
    for (const queue of [this.#asked, this.#queued]) {
      const at = queue.findIndex((each) => each.key === key);
      if (at >= 0) {
        const [waiting] = queue.splice(at, 1);
        if (waiting !== undefined) {
          this.#asked.unshift(waiting);
        }
        return;
      }
    }
  }

  /** Starts the waiting compiles, the passes' asks first, while fewer than the cap run. */
  #startWaiting(): void {
    while (this.#inFlight < TERRAIN_COMPILES_IN_FLIGHT) {
      const next = this.#asked.shift() ?? this.#queued.shift();
      if (next === undefined) {
        return;
      }
      next.start();
    }
  }

  /** Refuses every compile still waiting with `error`, unstarted. */
  #refuseWaiting(error: unknown): void {
    const waiting = [...this.#asked, ...this.#queued];
    this.#asked.length = 0;
    this.#queued.length = 0;
    for (const each of waiting) {
      each.refuse(error);
    }
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
    this.#refuseWaiting(new Error("the terrain's materials were let go"));
    this.#releaseAll();
  }

  /** Compiles every material of the setting; settles with how many compiled, never rejecting. */
  async #compileSetting(): Promise<number> {
    const settled = await Promise.allSettled(
      this.keys.map(({ kind, path, still }) => this.#ask(kind, path, "setting", still === true)),
    );
    // A failure is the pass's to report when it asks (`TerrainPass.ready`), as before the set.
    return settled.filter((each) => each.status === "fulfilled").length;
  }

  /**
   * Compiles `kind` on `path`, or its still form, against a mesh of its pass's layout, made and
   * released for it.
   */
  async #compile(
    kind: TerrainMaterialKind,
    path: TerrainVertexPath,
    still: boolean,
  ): Promise<MaterialHandle> {
    const { spec, targets, shading } = terrainMaterialOf(
      kind,
      path,
      this.#engine.capabilities,
      still,
    );
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
    // Asked on the lost device: refused as a compile a restore overtook is.
    this.#refuseWaiting(new EngineUnavailable("createMaterialAsync"));
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
