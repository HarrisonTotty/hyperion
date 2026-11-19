/**
 * The rings' slabs on a view (plan R11, R11.T9.c): their materials, mesh and tables, each ring's
 * profile texture, and each frame's records and draws.
 *
 * @remarks
 * Made in the photorealistic renderer's making with its other pipelines, in the tasks after its
 * first frame (`PhotorealRenderer.prepare`), or, where nothing has made them, at the first frame
 * with a ring in view, off the frame: until its materials are made, no ring is drawn. Its GPU
 * memory is the `rings` category's (Design note 18): the shadowing table's two textures, 39 kB,
 * made once, and a ring's profile, 590 kB, made at the first frame that draws it and let go
 * {@link PROFILE_KEPT_FRAMES} frames after the last.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { type Vec3 } from "../../geometry/vec3";
import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
} from "../engine/types";
import type { ViewRing } from "../scene/model";
import { sphereOutsideView } from "../wireframe/submit";
import { ringProfile } from "./ringProfile";
import {
  packRingSlabRecords,
  RING_PART_CODES,
  RING_PROFILE_ROWS,
  RING_PROFILE_TEXELS,
  RING_SCENE_DEPTH_TEXTURE,
  RING_SLAB_MATERIALS,
  type RingOccluderCandidate,
  ringOccludersOf,
  type RingPart,
  ringProfileTexels,
  ringSlabMesh,
  type RingSlabRecord,
  ringSlabRecordOf,
} from "./ringSlab";
import {
  SHADOWING_RATIO_TEXTURE_SIZE,
  SHADOWING_TEXTURE_SIZE,
  shadowingTexels,
} from "./shadowingLookup";

/** What the slabs make and release their resources with. */
export type RingSlabEngine = Pick<
  RenderEngine,
  | "createMaterialAsync"
  | "createMesh"
  | "createBuffer"
  | "createTexture"
  | "writeBuffer"
  | "writeTexture"
  | "releaseBuffer"
  | "releaseTexture"
  | "releaseMaterial"
  | "releaseMesh"
>;

/** Frames a ring's profile is kept after the last that drew it: 600, ten seconds at 60 Hz. */
export const PROFILE_KEPT_FRAMES = 600;

/** The records' buffer's least size, bytes. */
const MIN_RECORD_BYTES = 4_096;

/** What one frame of the slabs is drawn from. */
export interface RingSlabFrameInput {
  /** The scene's rings (`ViewScene.rings`). */
  readonly rings: ReadonlyArray<ViewRing>;
  /** The frame's lit bodies, each ring's planet among them. */
  readonly bodies: ReadonlyArray<LitBodyInput>;
  /** The frame's bodies' plan: their regimes, and their stars (`BodyFramePlan.sources`). */
  readonly plan: Pick<BodyFramePlan, "regimes" | "sources">;
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
  readonly exposureScale: number;
  /** The eclipse term's annuli, the setting's `discAnnuli`. */
  readonly annuli: number;
  /**
   * The pixel's angle each ring's particles close to are instanced at this frame (R11.T9.d;
   * `RingParticles`), by its ring, whose share of its optical depth its slab gives up; none for a
   * ring absent from it.
   */
  readonly instancedPixelRad?: ReadonlyMap<ViewRing, number>;
}

/** A ring the frame draws: its record's index, its planet and its planet's equatorial radius, m. */
export interface DrawnRing {
  readonly index: number;
  readonly body: BodyIdHex;
  readonly radiusM: number;
}

/** One frame's slabs: the rings it draws and each one's draws. */
export interface RingSlabFrame {
  readonly rings: ReadonlyArray<DrawnRing>;
  /**
   * The draw of ring `index`'s fragments of `part`, `shellOverA` its planet's air shell's radius ÷ a
   * where the part is not `all`: by the `sampled` material reading `sceneDepth` where it is given,
   * into a target that holds no depth, else by the `attached` one.
   */
  draw(
    index: number,
    part: RingPart,
    shellOverA: number,
    sceneDepth: TextureHandle | null,
  ): DrawItem;
}

interface SlabResources {
  readonly attached: MaterialHandle;
  readonly sampled: MaterialHandle;
  readonly mesh: MeshHandle;
  readonly shadowing: TextureHandle;
  readonly ratios: TextureHandle;
  records: BufferHandle;
}

/** A ring's profile's key: what its profile is made from, so that equal rings share one. */
function profileKey(ring: ViewRing): string {
  return JSON.stringify([
    ring.ringKind,
    ring.material,
    ring.innerRadiusM,
    ring.outerRadiusM,
    ring.opticalDepth,
    ring.gapRadiiM,
  ]);
}

/** The rings' slabs on one view. */
export class RingSlabs {
  readonly #engine: RingSlabEngine;
  readonly #name: string;
  #resources: SlabResources | null = null;
  #making: Promise<void> | null = null;
  #failed = false;
  /** Bumped by each restore, so that a making from before it is abandoned. */
  #generation = 0;
  #disposed = false;
  #frame = 0;
  readonly #profiles = new Map<string, { readonly texture: TextureHandle; lastFrame: number }>();

  /** The slabs of the view `name`, made with `engine` at their first frame with a ring. */
  constructor(engine: RingSlabEngine, name: string) {
    this.#engine = engine;
    this.#name = name;
  }

  /**
   * Starts making the materials, mesh and tables, once a device; resolves when they are made, or
   * when their making failed, which is reported once and leaves the rings undrawn until a restore.
   */
  prepare(): Promise<void> {
    if (this.#making === null) {
      const generation = this.#generation;
      this.#making = this.#make(generation).catch((error: unknown) => {
        if (generation === this.#generation && !this.#disposed) {
          this.#failed = true;
          console.error(`the rings of the view ${this.#name} could not be made:`, error);
        }
      });
    }
    return this.#making;
  }

  async #make(generation: number): Promise<void> {
    const engine = this.#engine;
    const mesh = engine.createMesh(ringSlabMesh(`${this.#name}:ring annulus`));
    // Both compiles settle before either is kept or let go, so that neither outlives a refusal.
    const [attached, sampled] = await Promise.allSettled([
      engine.createMaterialAsync(RING_SLAB_MATERIALS.attached, ["rgba16float"], [mesh]),
      engine.createMaterialAsync(RING_SLAB_MATERIALS.sampled, ["rgba16float"], [mesh]),
    ]);
    if (attached.status === "rejected" || sampled.status === "rejected") {
      for (const made of [attached, sampled]) {
        if (made.status === "fulfilled") {
          engine.releaseMaterial(made.value);
        }
      }
      engine.releaseMesh(mesh);
      const refused = attached.status === "rejected" ? attached : sampled;
      throw new Error(`the rings' slab materials of ${this.#name} could not be made`, {
        cause: refused.status === "rejected" ? refused.reason : undefined,
      });
    }
    if (this.#disposed || generation !== this.#generation) {
      engine.releaseMaterial(attached.value);
      engine.releaseMaterial(sampled.value);
      engine.releaseMesh(mesh);
      return;
    }
    const texels = shadowingTexels();
    const [vNodes, muNodes, layers] = SHADOWING_TEXTURE_SIZE;
    const shadowing = engine.createTexture({
      name: `${this.#name}:ring shadowing`,
      size: { width: vNodes, height: muNodes, depthOrArrayLayers: layers },
      dimension: "3d",
      format: "rgba16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "rings",
    });
    engine.writeTexture(
      shadowing,
      { x: 0, y: 0, z: 0 },
      { width: vNodes, height: muNodes, depthOrArrayLayers: layers },
      texels.quantities,
    );
    const [ratioMu, ratioTau] = SHADOWING_RATIO_TEXTURE_SIZE;
    const ratios = engine.createTexture({
      name: `${this.#name}:ring ratios`,
      size: { width: ratioMu, height: ratioTau },
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "rings",
    });
    engine.writeTexture(
      ratios,
      { x: 0, y: 0 },
      { width: ratioMu, height: ratioTau },
      texels.ratios,
    );
    this.#resources = {
      attached: attached.value,
      sampled: sampled.value,
      mesh,
      shadowing,
      ratios,
      records: this.#recordBuffer(MIN_RECORD_BYTES),
    };
  }

  #recordBuffer(bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name: `${this.#name}:ring records`,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "rings",
    });
  }

  /** A ring's profile texture, made at its first frame and kept while it is drawn. */
  #profile(ring: ViewRing): TextureHandle {
    const key = profileKey(ring);
    const held = this.#profiles.get(key);
    if (held !== undefined) {
      held.lastFrame = this.#frame;
      return held.texture;
    }
    const texture = this.#engine.createTexture({
      name: `${this.#name}:ring profile`,
      size: { width: RING_PROFILE_TEXELS, height: RING_PROFILE_ROWS },
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "rings",
    });
    this.#engine.writeTexture(
      texture,
      { x: 0, y: 0 },
      { width: RING_PROFILE_TEXELS, height: RING_PROFILE_ROWS },
      ringProfileTexels(ringProfile(ring)),
    );
    this.#profiles.set(key, { texture, lastFrame: this.#frame });
    return texture;
  }

  /**
   * The frame's slabs: each ring whose planet is drawn larger than a point (a disc, a mesh or
   * terrain) and whose bounding sphere is in view, its record uploaded; `null` where none is, or
   * while the slabs are not made, whose making the first such frame starts. Called every frame,
   * with no ring too, so that a profile no frame draws is let go {@link PROFILE_KEPT_FRAMES} frames
   * on.
   */
  frame(input: RingSlabFrameInput): RingSlabFrame | null {
    this.#frame += 1;
    this.#dropUnused();
    if (input.rings.length === 0) {
      return null;
    }
    const resources = this.#resources;
    if (resources === null) {
      if (!this.#failed) {
        void this.prepare();
      }
      return null;
    }
    const planets = new Map(input.bodies.map((body) => [body.id, body]));
    const discs: RingOccluderCandidate[] = input.bodies.flatMap((body) =>
      input.plan.regimes.get(body.id) === "disc"
        ? [{ id: body.id, centreM: body.centreM, radiusM: body.figure.equatorialRadiusM }]
        : [],
    );
    const drawn: Array<{
      readonly ring: DrawnRing;
      readonly record: RingSlabRecord;
      readonly profile: TextureHandle;
      readonly centreM: Vec3;
    }> = [];
    for (const ring of input.rings) {
      const planet = planets.get(ring.body);
      const regime = input.plan.regimes.get(ring.body);
      const stars = input.plan.sources.get(ring.body)?.stars;
      if (
        planet === undefined ||
        regime === undefined ||
        regime === "point" ||
        stars === undefined ||
        sphereOutsideView(planet.centreM, ring.outerRadiusM, input.camera, input.viewport)
      ) {
        continue;
      }
      const record = ringSlabRecordOf(
        ring,
        planet,
        stars,
        ringOccludersOf(ring.body, planet.centreM, ring.outerRadiusM, discs),
        {
          annuli: input.annuli,
          exposureScale: input.exposureScale,
          instancedPixelRad: input.instancedPixelRad?.get(ring) ?? 0,
        },
      );
      drawn.push({
        ring: { index: drawn.length, body: ring.body, radiusM: planet.figure.equatorialRadiusM },
        record,
        profile: this.#profile(ring),
        centreM: planet.centreM,
      });
    }
    if (drawn.length === 0) {
      return null;
    }
    const data = packRingSlabRecords(drawn.map((each) => each.record));
    if (data.byteLength > resources.records.bytes) {
      this.#engine.releaseBuffer(resources.records);
      resources.records = this.#recordBuffer(
        Math.max(2 * resources.records.bytes, data.byteLength),
      );
    }
    this.#engine.writeBuffer(resources.records, 0, data);
    const records = resources.records;
    return {
      rings: drawn.map((each) => each.ring),
      draw: (index, part, shellOverA, sceneDepth): DrawItem => {
        const each = drawn[index];
        if (each === undefined) {
          throw new RangeError(`the frame draws no ring ${String(index)}`);
        }
        return {
          mesh: resources.mesh,
          material: sceneDepth === null ? resources.attached : resources.sampled,
          offsetFromCameraM: new Float32Array([each.centreM.x, each.centreM.y, each.centreM.z]),
          uniforms: {
            ring: new Float32Array([index]),
            part: new Float32Array([RING_PART_CODES[part]]),
            shellOverA: new Float32Array([shellOverA]),
            radiusM: new Float32Array([each.ring.radiusM]),
          },
          textures: {
            ringProfile: each.profile,
            ringShadowing: resources.shadowing,
            ringRatios: resources.ratios,
            ...(sceneDepth === null ? {} : { [RING_SCENE_DEPTH_TEXTURE]: sceneDepth }),
          },
          storageBuffers: { rings: records },
        };
      },
    };
  }

  /** Lets go of the profiles no frame has drawn for {@link PROFILE_KEPT_FRAMES}. */
  #dropUnused(): void {
    for (const [key, held] of this.#profiles) {
      if (this.#frame - held.lastFrame > PROFILE_KEPT_FRAMES) {
        this.#engine.releaseTexture(held.texture);
        this.#profiles.delete(key);
      }
    }
  }

  /** Forgets the handles a lost device took with it; the next frame with a ring makes them again. */
  restored(): void {
    this.#generation += 1;
    this.#resources = null;
    this.#making = null;
    this.#failed = false;
    this.#profiles.clear();
  }

  /** Releases everything it made; a making in flight is released as it ends. */
  dispose(): void {
    this.#disposed = true;
    const resources = this.#resources;
    this.#resources = null;
    if (resources !== null) {
      const engine = this.#engine;
      engine.releaseMaterial(resources.attached);
      engine.releaseMaterial(resources.sampled);
      engine.releaseMesh(resources.mesh);
      engine.releaseTexture(resources.shadowing);
      engine.releaseTexture(resources.ratios);
      engine.releaseBuffer(resources.records);
    }
    for (const held of this.#profiles.values()) {
      this.#engine.releaseTexture(held.texture);
    }
    this.#profiles.clear();
  }
}
