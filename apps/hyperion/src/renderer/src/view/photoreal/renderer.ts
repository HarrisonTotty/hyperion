/**
 * One photorealistic view's frame (plan R07, T8.a; Design note 8): the passes of
 * {@link photorealisticPasses} that are built, in their order, into the view's HDR scene target,
 * then bloom and the tone-mapping pass onto the canvas, and the symbology over it.
 *
 * @remarks
 * Into the scene target, pre-exposed: R06's band, then its baked star cube (each at infinity,
 * additive, keeping the meter class), the star sprites through R06's `POINT SPRITES HDR` (stars,
 * and host discs under three pixels), then the mesh bodies' figures, opaque with depth, as the
 * `bodies` pass where the frame has any (T9), then the painter's sequence of Design note 2: host
 * discs (R06's pass, meter class 0), disc bodies, mesh bodies' limbs and point bodies, back to
 * front by power. Then the bloom
 * chain over the light above the display's range, the tone-mapping pass with each host disc's glare
 * source, encoded and dithered in the pass (`encoding: "in-pass"`), and a second canvas pass that
 * loads it for the cased symbology (`colourLoad: "load"`, decisions-r06-r07 item 6). Each frame
 * that carries a meter takes the scene target's exposure histogram under it (R07.T12), read back
 * for the view's `AutoExposure` ({@link PhotorealRenderer.takeHistogram}); a frame of a view that
 * meters nothing, an instrument's, takes none (R07.T19.c). R08's, R10's and R11's passes join
 * when they exist. Its resources are made asynchronously, the pipelines compiled
 * before the first frame, and again after a device loss; until then {@link
 * PhotorealRenderer.render} draws nothing and says so, and the view keeps its wireframe.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import {
  type BodyFramePlan,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../bodies/draw";
import type { LitRegime, ScreenCircle } from "../bodies/regime";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import type { ViewRole } from "../camera/state";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  RenderView,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import { DISC_ANNULI_HIGH, DISC_ANNULI_LOW } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH, PLANETSHINE_SOURCES_LOW } from "../lighting/planetshine";
import type { PlacedLight } from "../lighting/hostLights";
import { bloomKernel, bloomThreshold, levelWeight } from "../post/bloom";
import {
  BloomChain,
  fullScreenTriangle,
  GLARE_SOURCE_BYTES,
  packGlareSources,
} from "../post/bloomChain";
import { BLUE_NOISE_SIDE, blueNoiseTile } from "../post/blueNoise";
import { type GlareSource, glareSpreadTerms } from "../post/glare";
import { HISTOGRAM_KERNEL, type Histogram, HistogramReader } from "../post/histogram";
import type { MeterMode } from "../post/meter";
import { TONEMAP_MATERIAL, TONEMAP_PASS, tonemapDraw } from "../post/tonemap";
import type { QualitySetting } from "../quality/qualitySetting";
import { DEFAULT_EYE_OBSERVER } from "../sky/eye";
import { toHalfArray } from "../sky/half";
import { SKY_SPRITE_HDR_MATERIAL } from "../sky/spriteHdr";
import type { SpriteRecord } from "../wireframe/drawList";
import { WIREFRAME_MESHES } from "../wireframe/submit";
import { PHOTOREAL_PASS_LABELS, SKY_PASS_LABEL } from "./passes";
import { createSceneTarget } from "./sceneTarget";

/** What one photorealistic frame draws. */
export interface PhotorealFrame {
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
  readonly role: ViewRole;
  readonly setting: QualitySetting;
  /** The exposure scale the target is pre-exposed with (`exposureScale`), 1 ÷ (cd/m²). */
  readonly exposureScale: number;
  /** R06's draws at infinity, in order: the band, then the baked cube. */
  readonly sky: ReadonlyArray<DrawItem>;
  /** The star sprites' records (stars, and host discs under three pixels), at depth 0. */
  readonly starSprites: ReadonlyArray<SpriteRecord>;
  /** R06's host-disc draws by star (`DiscFrame.draws`), placed at their painter entries. */
  readonly hostDraws: ReadonlyMap<number, ReadonlyArray<DrawItem>>;
  /** R06's glare sources (`HostDiscLayer.glareSources`). */
  readonly glareSources: ReadonlyArray<GlareSource>;
  /** The lights at their stars' centres in this frame (`placeLights`). */
  readonly lights: ReadonlyArray<PlacedLight>;
  readonly bodies: ReadonlyArray<LitBodyInput>;
  /**
   * The footprints of the view's other geometry that writes depth (R10's terrain, lit craft), over
   * which a disc is drawn as a mesh (Design note 2); none in a view today.
   */
  readonly depthWriters: ReadonlyArray<ScreenCircle>;
  /** Each body's regime on the previous frame, for the hysteresis. */
  readonly previousRegimes: ReadonlyMap<BodyIdHex, LitRegime>;
  /** The symbology's canvas pass, drawn loading the tone-mapped image, or `null`. */
  readonly overlay: FrameSubmission | null;
  /**
   * The operator's meter, whose weights the frame's histogram takes (Design note 10), or `null` for
   * a view that is not the exposure's source, whose frame takes no histogram (Design note 11).
   */
  readonly meter: MeterMode | null;
}

/** Where a {@link PhotorealRenderer} stands. */
export type PhotorealStatus = "idle" | "making" | "ready" | "failed";

/** The glare sources the buffer holds before it grows. */
const GLARE_CAPACITY = 8;

/** The smallest sprite buffer made, bytes; it grows by doubling. */
const MIN_SPRITE_BYTES = 4_096;

/** The renderer's GPU objects on one device. */
interface Resources {
  readonly target: RenderTarget;
  readonly bloom: BloomChain;
  readonly sprite: MaterialHandle;
  readonly quad: MeshHandle;
  readonly tonemap: MaterialHandle;
  readonly triangle: MeshHandle;
  readonly blueNoise: TextureHandle;
  /** The scene target's exposure histogram, read back one to three frames late (R07.T12). */
  readonly histograms: HistogramReader;
  glare: BufferHandle;
  glareCapacity: number;
  stars: BufferHandle;
  size: ViewSize;
  /** The bloom kernel's key: role, setting and the pixel's angle. */
  kernelKey: string;
}

/** The pixel's angle at the view's centre, rad. */
function radPerPx(camera: ProjectionCamera, viewport: Viewport): number {
  return (2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx;
}

/** A view's photorealistic frames on one engine (plan R07, T8.a). */
export class PhotorealRenderer {
  readonly #engine: RenderEngine;
  readonly #name: string;
  readonly #bodies: LitBodyRenderer;
  #resources: Resources | null = null;
  #making: Promise<void> | null = null;
  #status: PhotorealStatus = "idle";
  /** The latest histogram read back and not yet taken. */
  #histogram: Histogram | undefined;
  /** Bumped by each restore, so that a making from before it is abandoned. */
  #generation = 0;
  #disposed = false;
  readonly #unsubscribe: () => void;

  constructor(engine: RenderEngine, viewName: string) {
    this.#engine = engine;
    this.#name = viewName;
    // The bodies' renderer makes its own handles again after a loss, so it lives as long as this.
    this.#bodies = new LitBodyRenderer(engine, SKY_SPRITE_HDR_MATERIAL);
    this.#unsubscribe = engine.onRestored(() => {
      // The handles died with the device; a making in flight is abandoned, and the next frame
      // makes them again.
      this.#generation += 1;
      this.#resources?.histograms.dispose();
      this.#resources = null;
      this.#histogram = undefined;
      this.#making = null;
      this.#status = "idle";
    });
  }

  /**
   * Where the renderer stands: `idle` before a frame asks for it, `making` while its pipelines
   * compile, `ready` once it draws, `failed` where they could not be made (until a restore).
   */
  get status(): PhotorealStatus {
    return this.#status;
  }

  /** Starts making the resources for a view of `size`, once; resolves when they are made. */
  prepare(
    size: ViewSize,
    setting: QualitySetting,
    role: ViewRole,
    camera: ProjectionCamera,
  ): Promise<void> {
    if (this.#making === null) {
      const generation = this.#generation;
      this.#status = "making";
      this.#making = this.#make(size, setting, role, camera, generation).catch((error: unknown) => {
        if (generation === this.#generation && !this.#disposed) {
          this.#status = "failed";
          console.error(`the photorealistic view ${this.#name} could not be made:`, error);
        }
      });
    }
    return this.#making;
  }

  async #make(
    size: ViewSize,
    setting: QualitySetting,
    role: ViewRole,
    camera: ProjectionCamera,
    generation: number,
  ): Promise<void> {
    const engine = this.#engine;
    const triangle = fullScreenTriangle(engine, `${this.#name} tonemap triangle`);
    const quad = engine.createMesh({ ...WIREFRAME_MESHES.quad, name: `${this.#name} sprite quad` });
    const radPx = radPerPx(camera, { widthPx: size.widthPx, heightPx: size.heightPx });
    const making = BloomChain.create(
      engine,
      this.#name,
      size,
      bloomKernel(setting, role, radPx, DEFAULT_EYE_OBSERVER),
    );
    let tonemap: MaterialHandle;
    let sprite: MaterialHandle;
    let kernel: ComputeHandle;
    try {
      [tonemap, sprite, kernel] = await Promise.all([
        engine.createMaterialAsync(TONEMAP_MATERIAL, ["canvas-in-pass"], [triangle]),
        engine.createMaterialAsync(SKY_SPRITE_HDR_MATERIAL, ["rgba16float"], [quad]),
        engine.createComputeAsync(HISTOGRAM_KERNEL),
      ]);
    } catch (error: unknown) {
      // A chain made before the refusal is released with it.
      void making
        .then((chain) => chain.dispose())
        .catch((cause: unknown) => {
          console.error(`the photorealistic view ${this.#name}'s bloom chain failed too:`, cause);
        });
      throw error;
    }
    const bloom = await making;
    // A restore or a dispose while this was compiling abandons it: its handles are the lost
    // device's, or nobody's.
    if (this.#disposed || generation !== this.#generation) {
      bloom.dispose();
      return;
    }
    const blueNoise = engine.createTexture({
      name: `${this.#name} blue noise`,
      size: [BLUE_NOISE_SIDE, BLUE_NOISE_SIDE],
      dimension: "2d",
      format: "r16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    });
    engine.writeTexture(
      blueNoise,
      { x: 0, y: 0 },
      { width: BLUE_NOISE_SIDE, height: BLUE_NOISE_SIDE },
      toHalfArray(blueNoiseTile()),
    );
    this.#resources = {
      target: createSceneTarget(engine, this.#name, size),
      bloom,
      sprite,
      quad,
      tonemap,
      triangle,
      blueNoise,
      histograms: new HistogramReader(engine, kernel, this.#name, (histogram) => {
        if (generation === this.#generation) {
          this.#histogram = histogram;
        }
      }),
      glare: this.#glareBuffer(GLARE_CAPACITY),
      glareCapacity: GLARE_CAPACITY,
      stars: this.#spriteBuffer(MIN_SPRITE_BYTES),
      size,
      kernelKey: `${role} ${setting} ${String(radPx)}`,
    };
    this.#status = "ready";
  }

  #glareBuffer(capacity: number): BufferHandle {
    return this.#engine.createBuffer({
      name: `${this.#name} glare sources`,
      bytes: capacity * GLARE_SOURCE_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
  }

  #spriteBuffer(bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name: `${this.#name} star sprites`,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
  }

  /** Follows the view's size and its bloom kernel's inputs. */
  #follow(resources: Resources, frame: PhotorealFrame): void {
    const size = { widthPx: frame.viewport.widthPx, heightPx: frame.viewport.heightPx };
    if (size.widthPx !== resources.size.widthPx || size.heightPx !== resources.size.heightPx) {
      resources.target.resize(size);
      resources.bloom.resize(size);
      resources.size = size;
    }
    const radPx = radPerPx(frame.camera, frame.viewport);
    const key = `${frame.role} ${frame.setting} ${String(radPx)}`;
    if (key !== resources.kernelKey) {
      resources.bloom.setKernel(
        bloomKernel(frame.setting, frame.role, radPx, DEFAULT_EYE_OBSERVER),
      );
      resources.kernelKey = key;
    }
  }

  /** The star sprites' draw, its buffer grown by doubling, or `null` with none. */
  #starDraw(resources: Resources, records: ReadonlyArray<SpriteRecord>): DrawItem | null {
    if (records.length === 0) {
      return null;
    }
    const rows = new Float32Array(records.length * 8);
    records.forEach((record, i) => {
      rows.set(record, i * 8);
    });
    if (rows.byteLength > resources.stars.bytes) {
      let bytes = resources.stars.bytes;
      while (bytes < rows.byteLength) {
        bytes *= 2;
      }
      this.#engine.releaseBuffer(resources.stars);
      resources.stars = this.#spriteBuffer(bytes);
    }
    this.#engine.writeBuffer(resources.stars, 0, rows);
    return {
      mesh: resources.quad,
      material: resources.sprite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
      instanceCount: records.length,
      storageBuffers: { sprites: resources.stars },
    };
  }

  /**
   * Draws one frame into `view`.
   *
   * @returns The frame's lit-body plan (its regimes are the next frame's `previousRegimes`), or
   *   `null` while the resources are being made, when nothing was drawn.
   */
  render(view: RenderView, frame: PhotorealFrame): BodyFramePlan | null {
    const resources = this.#resources;
    if (resources === null) {
      if (this.#status !== "failed") {
        void this.prepare(frame.viewport, frame.setting, frame.role, frame.camera);
      }
      return null;
    }
    this.#follow(resources, frame);
    const { camera, viewport } = frame;
    const projection = perspectiveReversedInfinite(
      camera.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    );
    const viewRotation = viewRotation4(camera.orientation);
    const plan = planLitBodies(
      frame.bodies,
      frame.lights,
      {
        camera,
        viewport,
        exposureScale: frame.exposureScale,
        annuli: frame.setting === "low" ? DISC_ANNULI_LOW : DISC_ANNULI_HIGH,
        planetshine: frame.setting === "low" ? PLANETSHINE_SOURCES_LOW : PLANETSHINE_SOURCES_HIGH,
        depthWriters: frame.depthWriters,
        setting: frame.setting,
      },
      frame.previousRegimes,
    );
    const stars = this.#starDraw(resources, frame.starSprites);
    // R06's sky at infinity, timed as its own pass, then the painter's sequence over it.
    resources.target.render({
      label: SKY_PASS_LABEL,
      viewRotation,
      projection,
      draws: [...frame.sky, ...(stars === null ? [] : [stars])],
      postProcesses: [],
    });
    // The mesh bodies' figures, with depth, before the sequence; no pass where there are none.
    const meshes = this.#bodies.meshDraws(plan);
    if (meshes.length > 0) {
      resources.target.render({
        label: PHOTOREAL_PASS_LABELS.bodies,
        viewRotation,
        projection,
        draws: meshes,
        postProcesses: [],
        colourLoad: "load",
      });
    }
    resources.target.render({
      label: PHOTOREAL_PASS_LABELS.discs,
      viewRotation,
      projection,
      draws: this.#bodies.draws(plan, frame.hostDraws),
      postProcesses: [],
      colourLoad: "load",
    });
    if (frame.meter !== null) {
      resources.histograms.measure({
        hdrColour: resources.target.colour,
        size: resources.size,
        mode: frame.meter,
        stride: frame.setting === "low" ? 2 : 1,
        preExposure: frame.exposureScale,
      });
    }
    // The pre-exposure is this frame's own exposure, so the pass's exposure over it is 1.
    const threshold = bloomThreshold(frame.exposureScale, frame.exposureScale);
    resources.bloom.run(resources.target.colour, threshold);
    if (frame.glareSources.length > resources.glareCapacity) {
      this.#engine.releaseBuffer(resources.glare);
      resources.glareCapacity = frame.glareSources.length;
      resources.glare = this.#glareBuffer(resources.glareCapacity);
    }
    const terms = glareSpreadTerms(frame.role, DEFAULT_EYE_OBSERVER);
    this.#engine.writeBuffer(
      resources.glare,
      0,
      packGlareSources(frame.glareSources, frame.exposureScale, terms),
    );
    view.render({
      label: TONEMAP_PASS,
      viewRotation,
      projection,
      draws: [
        tonemapDraw({
          mesh: resources.triangle,
          material: resources.tonemap,
          hdrColour: resources.target.colour,
          bloomUp: resources.bloom.levelOne,
          blueNoise: resources.blueNoise,
          glareSources: resources.glare,
          uniforms: {
            exposure: 1,
            threshold,
            levelZeroWeight: levelWeight(resources.bloom.kernel, 0),
            levelOneWeight: resources.bloom.levelOneWeight,
            sourceCount: frame.glareSources.length,
            terms,
            dither: true,
          },
        }),
      ],
      postProcesses: [],
      encoding: "in-pass",
    });
    if (frame.overlay !== null) {
      view.render({ ...frame.overlay, colourLoad: "load" });
    }
    return plan;
  }

  /**
   * The latest histogram read back since the last call, for the view's `AutoExposure`, or
   * `undefined` where none has arrived.
   */
  takeHistogram(): Histogram | undefined {
    const histogram = this.#histogram;
    this.#histogram = undefined;
    return histogram;
  }

  /** Stops following restores and releases the view's targets, buffers and textures. */
  dispose(): void {
    this.#disposed = true;
    this.#unsubscribe();
    this.#bodies.dispose();
    const resources = this.#resources;
    if (resources !== null) {
      resources.histograms.dispose();
      resources.target.dispose();
      resources.bloom.dispose();
      this.#engine.releaseBuffer(resources.glare);
      this.#engine.releaseBuffer(resources.stars);
      this.#engine.releaseTexture(resources.blueNoise);
      this.#resources = null;
    }
  }
}
