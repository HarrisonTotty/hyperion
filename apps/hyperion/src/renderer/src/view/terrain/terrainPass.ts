/**
 * The terrain pass in a view (plan R05, R05.T11.c, Design notes 6, 7, 10, 17 and 23): each frame,
 * selection (on its own cadence), the patch cache, the height-worker pool and the draw set drive
 * the one instanced draw of T11.a's resources with T11.b's material.
 *
 * @remarks
 * Selection is pure and costly, so the pass re-runs it only when its inputs move enough
 * (decision-r05-patch-demand.md, 4d): on a change of setting, viewport, field of view, contacts or
 * baked height ranges; an orientation change of more than one pixel's angle; or a camera move of
 * more than {@link RESELECT_FRACTION} of the distance to the nearest selected patch that is not at
 * the finest level. It selects at τ ÷ (1 + {@link RESELECT_FRACTION}), so that the drawn error
 * stays within τ between runs, and with a budget of half the cache's slots (4b). The draw set is
 * resolved when selection runs, in place by one `DrawSetResolver` per cache, and the instance and
 * contact records are written every frame into buffers made once.
 *
 * Positions follow R02's differencing: patch origins and contacts are body-fixed `f64`, rotated
 * into the body's non-rotating axes, less the camera in those axes, and narrowed once into the
 * records; the view's rotation is the camera's orientation in the body's axes.
 */

import type { Quaternion } from "../camera/pose";
import { conjugate, multiply, quaternionFromRows } from "../camera/quaternion";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type { DrawItem, MaterialHandle, RenderEngine, ViewSize } from "../engine/types";
import type { BufferHandle } from "../engine/types";
import { type QualitySetting, TERRAIN_SETTINGS } from "../quality/qualitySetting";
import type { Vec3 } from "../../geometry/vec3";
import {
  type TerrainAnnunciation,
  TerrainAnnunciationDebounce,
  type TerrainConditions,
  terrainConditions,
} from "./annunciation";
import { distanceToBoxM, relativeBounds } from "./bounds";
import { type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { terrainMaterialSpec } from "./gpu/material";
import { type TerrainLayout, TerrainResources } from "./gpu/resources";
import { ContactRecords, InstanceRecords } from "./gpu/uniforms";
import { MAX_LEVEL, type PatchKey, patchKeyString } from "./patchKey";
import type { PlanetGeometry } from "./planet";
import { finestPatchSizeM, type GroundContact, heldRadiusM, morphRampM } from "./grounded";
import {
  type PatchRequest,
  type Selection,
  type SelectionInput,
  selectionErrorM,
  selectPatches,
  type ViewSelectionInput,
} from "./select";
import type { BakedPatch, BakeSettings, TestPlanetRidges } from "./workers/messages";

/** The fraction of the nearest selected patch's distance the camera may move between selections. */
export const RESELECT_FRACTION = 0.1;

/**
 * How far through a level's distance band the CDLOD morph to its parent begins (Design note 6): a
 * hand value, the morph taking the band's outer 30% (Strugar 2009 morphs over an outer part of
 * each range and leaves its size free). T18 may revisit it.
 */
export const MORPH_START_FRACTION = 0.7;

/**
 * The extra skirt margin the height workers bake with, metres: `heightBake.ts`'s `bakeKey` passes
 * none, so selection's inherited ranges reach the skirts' bottoms by the bound and the step alone.
 */
export const WORKER_SKIRT_MARGIN_M = 0;

/** The visual albedo the spike shades with, a hand value (Design note 17). */
export const SPIKE_ALBEDO = 0.15;

/** The height pool the pass drives: T10's `HeightWorkerPool`, or a test's fake. */
export interface TerrainPool {
  reprioritise(demand: ReadonlyArray<PatchRequest>): void;
  onBaked(cb: (bake: BakedPatch) => void): () => void;
  terminate(): void;
}

/** Makes the pass's pool for the layout's bake settings; called again after a device rebuild. */
export type TerrainPoolFactory = (bake: BakeSettings) => TerrainPool;

/** The view the pass draws, in the body's non-rotating axes. */
export interface TerrainView {
  /** The body's rotation, body-fixed axes into its non-rotating axes. */
  readonly rotation: Rotation3;
  /** The camera less the body's centre, in the body's non-rotating axes, metres (`f64`). */
  readonly cameraM: Vec3;
  /** The camera's orientation in the body's non-rotating axes. */
  readonly orientation: Quaternion;
  readonly fovXRad: number;
  /** The presented size in device pixels. */
  readonly viewport: ViewSize;
}

/** One frame's inputs. */
export interface TerrainFrameInput {
  readonly view: TerrainView;
  /** Grounded and descending bodies, body-fixed (Design note 9). */
  readonly grounded: ReadonlyArray<GroundContact>;
  /** The unit direction towards the sun, body-fixed. */
  readonly sunDirectionBodyFixed: Vec3;
  /** The sun's illuminance per Rec. 709 channel at the body, lux (`solar.ts`, scaled by 1 ÷ d²). */
  readonly sunIlluminanceLx: readonly [number, number, number];
  /** The frame's pre-exposure (R02's `exposureScale` of its EV100). */
  readonly exposureScale: number;
  /** A monotonic time for the annunciation's debounce, milliseconds. */
  readonly nowMs: number;
}

/** One frame's outcome. */
export interface TerrainFrame {
  /** The instanced draw, or `null` before the material is ready or with nothing to draw. */
  readonly draw: DrawItem | null;
  /**
   * The pass's one draw set, rewritten in place when selection next runs: read it before the next
   * {@link TerrainPass.frame} call, or copy it.
   */
  readonly drawSet: DrawSet;
  readonly selection: Selection;
  /** Whether this frame ran selection. */
  readonly reselected: boolean;
  readonly conditions: TerrainConditions;
  /** The debounced label line (Design note 23), for `labelStatements`. */
  readonly annunciation: TerrainAnnunciation | null;
}

/** The pass's options. */
export interface TerrainPassOptions {
  readonly engine: RenderEngine;
  readonly setting: QualitySetting;
  readonly planet: PlanetGeometry;
  readonly ridges: TestPlanetRidges;
  readonly createPool: TerrainPoolFactory;
  /**
   * Whether each selection is recorded as a `terrain.select` `performance.measure` span, for the
   * descent spike's trace (decision-r05-patch-demand.md, 4d). Off by default: each span is an
   * entry the browser keeps, so only the spike's measurement runs pay for it.
   */
  readonly measureSelection?: boolean;
  /** Called with each patch the cache stores and the GPU holds, for the spike's tallies. */
  readonly onResident?: (key: PatchKey) => void;
}

/** The `performance.measure` name of one selection, when the pass measures it. */
export const SELECT_MEASURE = "terrain.select";

/** The state made per device: resources, cache and pool. */
interface Device {
  readonly resources: TerrainResources;
  readonly cache: PatchCache;
  /** Resolves the cache's draw set in place, allocating nothing after warm-up (one per cache). */
  readonly resolver: DrawSetResolver;
  readonly pool: TerrainPool;
  readonly offBaked: () => void;
  readonly instances: InstanceRecords;
  material: MaterialHandle | null;
  /** The draw, made once the material is ready; its uniforms are rewritten in place each frame. */
  draw: DrawItem | null;
}

/** The inputs of the last selection, to decide whether to select again, and what it gave. */
interface Selected {
  readonly selection: Selection;
  readonly drawSet: DrawSet;
  readonly conditions: TerrainConditions;
  /** The contacts that reach a drawn patch, body-fixed. */
  readonly nearContacts: ReadonlyArray<GroundContact>;
  readonly cameraBodyFixedM: Vec3;
  readonly orientationBodyFixed: Quaternion;
  readonly fovXRad: number;
  readonly viewport: ViewSize;
  /** A copy of the contacts given, so that a caller's array changed in place is still seen. */
  readonly grounded: ReadonlyArray<GroundContact>;
  readonly rangesVersion: number;
  /**
   * The distance to the box of the nearest selected patch not at the finest level, metres, at
   * least one finest patch: a box with a wide height range can contain the camera.
   */
  readonly nearestM: number;
}

/** The body-fixed axes into the non-rotating ones as `mat4x4f`, column-major. */
function rotationMat4(rotation: Rotation3, out: Float32Array): void {
  const [r0, r1, r2] = rotation.rows;
  out.set([r0.x, r1.x, r2.x, 0, r0.y, r1.y, r2.y, 0, r0.z, r1.z, r2.z, 0, 0, 0, 0, 1]);
}

function distance(a: Vec3, b: Vec3): number {
  return Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);
}

function sameContacts(a: ReadonlyArray<GroundContact>, b: ReadonlyArray<GroundContact>): boolean {
  return (
    a.length === b.length &&
    a.every((c, i) => {
      const d = b[i];
      return (
        d !== undefined &&
        c.radiusM === d.radiusM &&
        c.positionM.x === d.positionM.x &&
        c.positionM.y === d.positionM.y &&
        c.positionM.z === d.positionM.z
      );
    })
  );
}

/**
 * The CDLOD morph band of `level` in a view, metres: the morph begins at {@link
 * MORPH_START_FRACTION} of the way from the distance where the level meets the view's τ to the
 * distance where its parent does, and is complete there. Level 0 has no parent and no band; the
 * finest level, whose own error is zero, has [0.7 d₍ₙ₋₁₎, d₍ₙ₋₁₎].
 */
export function morphRangeM(
  planet: PlanetGeometry,
  level: number,
  view: ViewSelectionInput,
): readonly [number, number] {
  if (level === 0) {
    return [0, 0];
  }
  // Metres of distance per metre of error at which the error subtends τ (Design note 7).
  const distancePerErrorM = view.viewport.widthPx / (2 * Math.tan(view.fovXRad / 2) * view.tauPx);
  const own = selectionErrorM(planet, level) * distancePerErrorM;
  const parent = selectionErrorM(planet, level - 1) * distancePerErrorM;
  return [own + MORPH_START_FRACTION * (parent - own), parent];
}

/** The terrain pass of one view. */
export class TerrainPass {
  readonly #engine: RenderEngine;
  readonly #setting: QualitySetting;
  readonly #planet: PlanetGeometry;
  readonly #ridges: TestPlanetRidges;
  readonly #createPool: TerrainPoolFactory;
  readonly #measureSelection: boolean;
  readonly #onResident: ((key: PatchKey) => void) | null;
  readonly #contacts = new ContactRecords();
  readonly #debounce = new TerrainAnnunciationDebounce();
  readonly #offRestored: () => void;
  readonly #uniforms = {
    bodyRotation: new Float32Array(16),
    sunDirection: new Float32Array(4),
    sunRadiance: new Float32Array(4),
    atlas: new Float32Array(4),
  };
  /** Each level's morph band, start then end, for the last selection's view at the setting's τ. */
  readonly #morph = new Float64Array(2 * (MAX_LEVEL + 1));
  #rangesVersion = 0;
  #device: Device;
  #selected: Selected | null = null;
  #demandDirty = true;
  #disposed = false;

  constructor(options: TerrainPassOptions) {
    this.#engine = options.engine;
    this.#setting = options.setting;
    this.#planet = options.planet;
    this.#ridges = options.ridges;
    this.#createPool = options.createPool;
    this.#measureSelection = options.measureSelection ?? false;
    this.#onResident = options.onResident ?? null;
    this.#device = this.#makeDevice();
    this.#offRestored = this.#device.resources.onRebuilt(() => {
      this.#rebuild();
    });
  }

  /** The current device's layout. */
  get layout(): TerrainLayout {
    return this.#device.resources.layout;
  }

  /** The selection budget: half the cache's slots (decision-r05-patch-demand.md, 4b). */
  get maxPatches(): number {
    return Math.floor(this.layout.slots.slotCount / 2);
  }

  /** Compiles the material ahead of the first frame; frames draw nothing until it resolves. */
  async ready(): Promise<void> {
    const device = this.#device;
    const material = await this.#engine.createMaterialAsync(
      terrainMaterialSpec(device.resources.layout.vertexPath),
      ["rgba16float"],
      [device.resources.mesh],
    );
    if (device === this.#device && !this.#disposed) {
      device.material = material;
      device.draw = this.#drawItem(device.resources, material);
    }
  }

  /**
   * Runs one frame: selection when due, the demand to the pool, the draw set, and the records.
   *
   * @throws RangeError if more contacts are given than the contact buffer holds.
   */
  frame(input: TerrainFrameInput): TerrainFrame {
    const device = this.#device;
    const view = this.#selectionView(input.view);
    let reselected = false;
    if (this.#due(view, input)) {
      this.#select(view, input);
      reselected = true;
    }
    const selected = this.#selected;
    if (selected === null) {
      throw new Error("selection did not run");
    }
    const { selection, drawSet, conditions } = selected;
    if (reselected || this.#demandDirty) {
      device.pool.reprioritise(
        selection.demand.filter((r) => !device.cache.has(patchKeyString(r.key))),
      );
      this.#demandDirty = false;
    }
    const annunciation = this.#debounce.update(conditions, input.nowMs);
    const draw = this.#write(device, drawSet, selected.nearContacts, input);
    return { draw, drawSet, selection, reselected, conditions, annunciation };
  }

  /** Stops the pool and following restores. The engine frees the GPU objects with itself. */
  dispose(): void {
    this.#disposed = true;
    this.#offRestored();
    this.#device.offBaked();
    this.#device.pool.terminate();
    this.#device.resources.dispose();
  }

  #makeDevice(resources?: TerrainResources): Device {
    const made =
      resources ??
      new TerrainResources(this.#engine, TERRAIN_SETTINGS[this.#setting], this.#planet.figure);
    const { layout } = made;
    const cache = new PatchCache(layout.slots);
    const pool = this.#createPool({
      vertexPath: layout.vertexPath,
      normals: TERRAIN_SETTINGS[this.#setting].normals,
      ridges: this.#ridges,
    });
    const device: Device = {
      resources: made,
      cache,
      resolver: new DrawSetResolver(cache),
      pool,
      offBaked: pool.onBaked((bake) => {
        this.#store(device, bake);
      }),
      instances: new InstanceRecords(layout.slots.slotCount),
      material: null,
      draw: null,
    };
    return device;
  }

  /** After a device loss: the same resources object, re-made; a new cache, pool and material. */
  #rebuild(): void {
    const old = this.#device;
    old.offBaked();
    old.pool.terminate();
    this.#device = this.#makeDevice(old.resources);
    this.#rangesVersion += 1;
    this.#selected = null;
    void this.ready().catch((error: unknown) => {
      console.error("the terrain material failed after a restore", error);
    });
  }

  #store(device: Device, bake: BakedPatch): void {
    if (device !== this.#device) {
      return;
    }
    const keyString = patchKeyString(bake.key);
    const stored = device.cache.insert(bake);
    if (stored.kind !== "stored") {
      return;
    }
    const upload = device.resources.upload({ slot: stored.slot, ...bake });
    if (upload.kind !== "uploaded") {
      device.cache.remove(keyString);
      return;
    }
    this.#rangesVersion += 1;
    this.#demandDirty = true;
    this.#onResident?.(bake.key);
  }

  /** The primary view as selection takes it: body-fixed, at τ ÷ (1 + m). */
  #selectionView(view: TerrainView): ViewSelectionInput {
    const qBody = quaternionFromRows(view.rotation.rows);
    return {
      camera: {
        positionM: rotateToBodyFixed(view.rotation, view.cameraM),
        orientation: multiply(conjugate(qBody), view.orientation),
      },
      fovXRad: view.fovXRad,
      viewport: view.viewport,
      weight: 1,
      tauPx: TERRAIN_SETTINGS[this.#setting].tauPx / (1 + RESELECT_FRACTION),
    };
  }

  #due(view: ViewSelectionInput, input: TerrainFrameInput): boolean {
    const last = this.#selected;
    if (last === null) {
      return true;
    }
    if (
      last.fovXRad !== view.fovXRad ||
      last.viewport.widthPx !== view.viewport.widthPx ||
      last.viewport.heightPx !== view.viewport.heightPx ||
      last.rangesVersion !== this.#rangesVersion ||
      !sameContacts(last.grounded, input.grounded)
    ) {
      return true;
    }
    // The whole rotation, roll included: 2 acos |q · q′| is the angle between two orientations.
    const q = view.camera.orientation;
    const p = last.orientationBodyFixed;
    const dot = Math.abs(q.w * p.w + q.x * p.x + q.y * p.y + q.z * p.z);
    const pixelRad = view.fovXRad / view.viewport.widthPx;
    if (2 * Math.acos(Math.min(dot, 1)) > pixelRad) {
      return true;
    }
    return (
      distance(view.camera.positionM, last.cameraBodyFixedM) > RESELECT_FRACTION * last.nearestM
    );
  }

  #select(view: ViewSelectionInput, input: TerrainFrameInput): void {
    const selectionInput: SelectionInput = {
      planet: this.#planet,
      views: [view],
      setting: this.#setting,
      grounded: input.grounded,
      maxPatches: this.maxPatches,
      heightRanges: this.#device.cache,
      skirtMarginM: WORKER_SKIRT_MARGIN_M,
    };
    const startMs = this.#measureSelection ? performance.now() : 0;
    const selection = selectPatches(selectionInput);
    if (this.#measureSelection) {
      performance.measure(SELECT_MEASURE, { start: startMs, end: performance.now() });
    }
    const patchM = finestPatchSizeM(this.#planet);
    let nearestM = Number.POSITIVE_INFINITY;
    for (const patch of selection.patches.values()) {
      if (patch.key.level < this.#planet.finestLevel) {
        // The box, as selection measures a patch's distance: a coarse patch's bounding sphere
        // often holds the camera, which would clamp the rule to one finest patch (lane B).
        nearestM = Math.min(
          nearestM,
          distanceToBoxM(relativeBounds(patch.bounds, view.camera.positionM)),
        );
      }
    }
    const morphView = { ...view, tauPx: TERRAIN_SETTINGS[this.#setting].tauPx };
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      const [start, end] = morphRangeM(this.#planet, level, morphView);
      this.#morph[2 * level] = start;
      this.#morph[2 * level + 1] = end;
    }
    const device = this.#device;
    // The same object every selection, rewritten in place; unseen forced patches are pinned by
    // `retain` but not drawn.
    const drawSet = device.resolver.resolve(selection);
    device.cache.retain(selection, drawSet);
    const limited = terrainConditions(drawSet, selection, selection);
    const conditions: TerrainConditions = {
      streaming: limited.streaming,
      // The low setting is coarser than the high one wherever terrain is drawn (Design note 26).
      detailLimited: limited.detailLimited || (this.#setting === "low" && drawSet.count > 0),
    };
    const grounded = input.grounded.map((c) => ({
      positionM: { ...c.positionM },
      radiusM: c.radiusM,
    }));
    const nearContacts = grounded.filter((contact) => {
      const reach = heldRadiusM(contact, patchM) + morphRampM(patchM);
      return drawSet.patches.some(
        ({ patch }) => distance(contact.positionM, patch.originM) <= patch.boundingRadiusM + reach,
      );
    });
    this.#selected = {
      selection,
      drawSet,
      conditions,
      nearContacts,
      cameraBodyFixedM: view.camera.positionM,
      orientationBodyFixed: view.camera.orientation,
      fovXRad: view.fovXRad,
      viewport: view.viewport,
      grounded,
      rangesVersion: this.#rangesVersion,
      nearestM: Math.max(nearestM, patchM),
    };
  }

  /** Writes the frame's records and returns the draw, or `null`. */
  #write(
    device: Device,
    drawSet: DrawSet,
    contacts: ReadonlyArray<GroundContact>,
    input: TerrainFrameInput,
  ): DrawItem | null {
    const { resources, instances } = device;
    const { cameraM } = input.view;
    const [r0, r1, r2] = input.view.rotation.rows;
    const morph = this.#morph;
    instances.clear();
    // `patches` holds exactly the `count` drawn patches (`DrawSetResolver`).
    for (const { patch } of drawSet.patches) {
      // R · origin − camera, component by component, so that a frame makes no vector a patch.
      const o = patch.originM;
      const level = patch.key.level;
      instances.pushXyz(
        patch.slot,
        r0.x * o.x + r0.y * o.y + r0.z * o.z - cameraM.x,
        r1.x * o.x + r1.y * o.y + r1.z * o.z - cameraM.y,
        r2.x * o.x + r2.y * o.y + r2.z * o.z - cameraM.z,
        morph[2 * level] ?? 0,
        morph[2 * level + 1] ?? 0,
      );
    }
    this.#contacts.clear();
    const patchM = finestPatchSizeM(this.#planet);
    for (const contact of contacts) {
      const c = contact.positionM;
      this.#contacts.pushXyz(
        r0.x * c.x + r0.y * c.y + r0.z * c.z - cameraM.x,
        r1.x * c.x + r1.y * c.y + r1.z * c.z - cameraM.y,
        r2.x * c.x + r2.y * c.y + r2.z * c.z - cameraM.z,
        heldRadiusM(contact, patchM),
        morphRampM(patchM),
      );
    }
    resources.writeFrame(instances, this.#contacts);
    if (device.draw === null || instances.count === 0) {
      return null;
    }
    const u = this.#uniforms;
    rotationMat4(input.view.rotation, u.bodyRotation);
    const sun = input.sunDirectionBodyFixed;
    u.sunDirection[0] = sun.x;
    u.sunDirection[1] = sun.y;
    u.sunDirection[2] = sun.z;
    const k = (SPIKE_ALBEDO / Math.PI) * input.exposureScale;
    const [red, green, blue] = input.sunIlluminanceLx;
    u.sunRadiance[0] = red * k;
    u.sunRadiance[1] = green * k;
    u.sunRadiance[2] = blue * k;
    return device.draw;
  }

  /** The draw of `resources` with `material`, its uniforms this pass's arrays. */
  #drawItem(resources: TerrainResources, material: MaterialHandle): DrawItem {
    const heights = resources.field("heights");
    if (heights === null) {
      throw new Error("the terrain layout has no heights buffer");
    }
    const storageBuffers: Record<string, BufferHandle> = {
      heights,
      slots: resources.slotRecords,
      instances: resources.instances,
      contacts: resources.contacts,
    };
    const offsets = resources.field("offsets");
    if (offsets !== null) {
      storageBuffers["offsets"] = offsets;
    }
    const { atlas } = resources.layout;
    this.#uniforms.atlas.set([
      atlas.columns,
      atlas.tilesPerLayer,
      atlas.tileTexels,
      atlas.samplesPerSide,
    ]);
    return {
      mesh: resources.mesh,
      material,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: this.#uniforms,
      textures: { normals: resources.normals },
      storageBuffers,
      indirect: { buffer: resources.indirect, offsetBytes: 0 },
    };
  }
}

const ZERO_OFFSET = new Float32Array(3);
