/**
 * One view's frames on its canvas, in either style (plan R07, T19): the wireframe through R02's
 * renderer, or the photorealistic frame through R07's, with R06's sky, its band and host-disc
 * layers and its baked cube. Every view of `VIEW` draws through it, the primary and each
 * instrument alike (R07.T19.c), so that what one view's frame draws every view's does; only the
 * exposure's source, the primary, gives a meter, and its frames alone take the histogram that its
 * `AutoExposure` reads (R07.T8.a; Design note 11). Each frame draws at the quality setting `VIEW`
 * is given (R07.T17): R02's wireframe, R06's cube and R07's photorealistic frame at its forms, the
 * scene target at most its `terrain.renderHeightPx` rows times the budget's scale.
 *
 * @remarks
 * Every handle is made again after a device loss: the photorealistic renderer itself, R06's layers
 * in `onRestored`, and the cube through the device's shared cache, from which each view acquires
 * the sky's cube under its own name, so that one sky is baked once. A drawer asked for during a
 * loss is made at the restore ({@link makeViewFrameDrawer}).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type { ElementSize } from "../../lib/useElementSize";
import type { ColourTokens } from "../../spatial/paint";
import type { LitRegime } from "../../view/bodies/regime";
import type { CameraPose } from "../../view/camera/pose";
import type { CameraTarget, RenderStyle } from "../../view/camera/state";
import type { StyleAvailability } from "../../view/engine/platform";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { RenderEngine, RenderView, ViewSize } from "../../view/engine/types";
import { controlEv100, type ExposureControl, exposureScale } from "../../view/photometry/exposure";
import {
  internalViewport,
  renderViewport,
  spritesAtScale,
} from "../../view/photoreal/internalScale";
import { overlaySubmission } from "../../view/photoreal/overlay";
import { PhotorealRenderer, type PhotorealStatus } from "../../view/photoreal/renderer";
import type { Histogram } from "../../view/post/histogram";
import type { MeterMode } from "../../view/post/meter";
import { type QualitySetting, SETTINGS } from "../../view/quality/qualitySetting";
import type { ViewScene, ViewStar } from "../../view/scene/model";
import { BandLayer } from "../../view/sky/band";
import type { BakedCube } from "../../view/sky/bake";
import { skyCubeCacheOf } from "../../view/sky/cache";
import { cameraFromObserverM } from "../../view/sky/camera";
import { SkyCubeLayer } from "../../view/sky/cubeLayer";
import { HostDiscLayer } from "../../view/sky/disc";
import { skySpriteStars } from "../../view/sky/sprites";
import {
  buildWireframeDrawList,
  type DrawAnchor,
  type SpriteStar,
  viewStrokesAt,
} from "../../view/wireframe/drawList";
import { WireframeRenderer } from "../../view/wireframe/submit";
import { photorealFrame } from "./photorealFrame";
import { bakeInputOf, type DrawnSky } from "./useViewSky";
import { runPose, type ViewRun } from "./viewRun";

/** What one frame of a view draws from. */
export interface ViewFrameInputs {
  readonly run: ViewRun;
  /** The view's stage, as laid out, or `null` before it is. */
  readonly size: ElementSize | null;
  /** The colour tokens, read from the canvas, or `null` while it is not connected. */
  readonly tokens: ColourTokens | null;
  /**
   * The exposure it draws at: the primary's own, for the primary its `AutoExposure`'s applied
   * control and for an instrument the display's control (Design note 11).
   */
  readonly exposure: ExposureControl;
  /** The interim stars (R02.T16), drawn until the sky arrives. */
  readonly stars: ReadonlyArray<ViewStar>;
  /** The sky (R06), or `null` before it arrives. */
  readonly sky: DrawnSky | null;
  readonly selection: CameraTarget | null;
  /** The style its budget draws (`ViewBudget.style`). */
  readonly style: RenderStyle;
  /**
   * The quality setting `VIEW` is given (R07.T17): the wireframe's, the sky cube's and the
   * photorealistic frame's forms, and the render resolution its scale is a fraction of.
   */
  readonly setting: QualitySetting;
  /**
   * Its budget's internal scale, a photorealistic frame's (`ViewBudget.renderScale`), of the render
   * resolution (`renderViewport`).
   */
  readonly renderScale: number;
  /** The styles it may draw: the adapter's, and for the primary the budget's permission too. */
  readonly availability: StyleAvailability;
  /**
   * The operator's meter where the view is the exposure's source, whose photorealistic frames then
   * take the histogram {@link ViewFrameDrawer.takeHistogram} hands over; `null` for a view that
   * meters nothing, an instrument (Design note 11), whose frames take none.
   */
  readonly meter: MeterMode | null;
}

/** What a frame drew. */
export interface ViewFrameDrawn {
  /** The marks where the frame drew them, device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** The style it was drawn in: the wireframe while the photorealistic pipelines compile. */
  readonly drawnStyle: RenderStyle;
}

/**
 * The sky's sprites from the camera this frame: the selection's stars placed from the camera's
 * offset from the sky's observer, in `f64` (R06 Design note 20); `null` where the camera's
 * galactic position is not known.
 */
export function skySprites(
  sky: DrawnSky,
  pose: CameraPose,
  scene: ViewScene,
): ReadonlyArray<SpriteStar> | null {
  const offset = cameraFromObserverM(pose, scene, sky.model.request.observer);
  // A scene that has lost its system's position draws the interim stars, which say so.
  return offset === null ? null : skySpriteStars(sky.model.stars, sky.selection.sprites, offset);
}

/**
 * The side, texels, of the sky cube's faces a view bakes (R06 Design note 22): the setting's, but
 * the low setting's 1,024² wherever the device cannot blend `float32`, whose bake runs on the CPU
 * (R06.T13.f).
 */
export function skyFaceSizePx(setting: QualitySetting, float32Blendable: boolean): number {
  return float32Blendable ? SETTINGS[setting].sky.faceSizePx : SETTINGS.low.sky.faceSizePx;
}

/** A view's renderers on one engine, drawing into its canvas's view. */
export class ViewFrameDrawer {
  readonly #engine: RenderEngine;
  readonly #view: RenderView;
  readonly #name: string;
  readonly #wireframe: WireframeRenderer;
  readonly #photoreal: PhotorealRenderer;
  readonly #cubes: SkyCubeLayer;
  #band: BandLayer;
  #discs: HostDiscLayer;
  #bandFor: DrawnSky | null = null;
  /** A sky whose bake failed is not baked again until another sky or a restore. */
  #failedFor: DrawnSky | null = null;
  #frameFailed = false;
  #regimes: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
  #sized: ViewSize | null = null;
  readonly #unsubscribe: () => void;

  /**
   * Draws into `view`, made by `engine` under `name`; disposes of it with itself.
   *
   * @throws `EngineUnavailable` while the engine has no device (a loss), having released what it
   *   made before; the caller keeps the view and makes the drawer again at the restore.
   */
  constructor(engine: RenderEngine, view: RenderView, name: string) {
    this.#engine = engine;
    this.#view = view;
    this.#name = name;
    const made: Array<{ dispose(): void }> = [];
    try {
      this.#wireframe = new WireframeRenderer(engine);
      made.push(this.#wireframe);
      this.#photoreal = new PhotorealRenderer(engine, name);
      made.push(this.#photoreal);
      this.#cubes = new SkyCubeLayer(engine);
      made.push(this.#cubes);
      this.#band = new BandLayer(engine);
      made.push(this.#band);
      this.#discs = new HostDiscLayer(engine);
    } catch (error: unknown) {
      for (const each of made) {
        each.dispose();
      }
      throw error;
    }
    this.#unsubscribe = engine.onRestored(() => {
      this.#band = new BandLayer(engine);
      this.#discs = new HostDiscLayer(engine);
      this.#bandFor = null;
      this.#failedFor = null;
      this.#frameFailed = false;
    });
  }

  /** The photorealistic renderer's standing (`PhotorealRenderer.status`). */
  get photorealStatus(): PhotorealStatus {
    return this.#photoreal.status;
  }

  /**
   * The latest histogram read back since the last call (`PhotorealRenderer.takeHistogram`), or
   * `undefined` where none has arrived: always, for a view whose frames carry no meter.
   */
  takeHistogram(): Histogram | undefined {
    return this.#photoreal.takeHistogram();
  }

  /** Draws one frame, or nothing (`null`) while the stage is not laid out. */
  draw(inputs: ViewFrameInputs): ViewFrameDrawn | null {
    const { run, size, tokens } = inputs;
    if (size === null || tokens === null || size.widthPx <= 0) {
      return null;
    }
    const ratio = size.devicePixelRatio;
    const viewport = {
      widthPx: Math.max(1, Math.round(size.widthPx * ratio)),
      heightPx: Math.max(1, Math.round(size.heightPx * ratio)),
    };
    if (this.#sized?.widthPx !== viewport.widthPx || this.#sized.heightPx !== viewport.heightPx) {
      this.#view.resize(viewport);
      this.#sized = viewport;
    }
    const camera = { pose: runPose(run), fovXRad: (run.camera.fovDeg * Math.PI) / 180 };
    const ev100 = controlEv100(inputs.exposure);
    const list = buildWireframeDrawList(
      { ...run.scene, stars: inputs.stars },
      camera,
      viewport,
      tokens,
      {
        // R02's wireframe at the setting: graticules at 30° only and 2,000 sprites on low.
        lowSetting: inputs.setting === "low",
        ev100,
        selection: inputs.selection,
        destination: null,
        remPx: size.remPx * ratio,
        // The guide's CSS-pixel widths at the display's ratio, never under 2 device pixels.
        ...viewStrokesAt(ratio),
        skyStars: inputs.sky === null ? null : skySprites(inputs.sky, camera.pose, run.scene),
      },
    );
    const cube = this.#cubeFor(inputs.sky, inputs.setting);
    const exposed = exposureScale(ev100);
    let drawn = false;
    if (
      inputs.style === "photorealistic" &&
      inputs.availability.photorealistic &&
      this.#photoreal.status !== "failed"
    ) {
      drawn = this.#drawPhotoreal(inputs, tokens, viewport, camera, list, cube, exposed);
    }
    if (!drawn) {
      this.#wireframe.render(
        this.#view,
        list,
        camera,
        viewport,
        cube === null ? [] : [this.#cubes.draw(cube, "display", exposed)],
      );
    }
    return { anchors: list.anchors, drawnStyle: drawn ? "photorealistic" : "wireframe" };
  }

  #drawPhotoreal(
    inputs: ViewFrameInputs,
    tokens: ColourTokens,
    viewport: ViewSize,
    camera: { readonly pose: CameraPose; readonly fovXRad: number },
    list: ReturnType<typeof buildWireframeDrawList>,
    cube: BakedCube | null,
    exposed: number,
  ): boolean {
    const { sky, run } = inputs;
    try {
      if (sky !== null && sky !== this.#bandFor) {
        this.#band.update(
          sky.model.band,
          sky.model.response.band.face_texels,
          sky.bandIlluminanceLx,
        );
        this.#bandFor = sky;
      }
      const internal = internalViewport(
        renderViewport(viewport, SETTINGS[inputs.setting].terrain.renderHeightPx),
        inputs.renderScale,
      );
      const plan = this.#photoreal.render(
        this.#view,
        photorealFrame({
          run,
          pose: camera.pose,
          viewport: internal,
          setting: inputs.setting,
          exposureScale: exposed,
          list: { ...list, sprites: spritesAtScale(list.sprites, viewport, internal) },
          sky,
          band: sky === null ? null : this.#band,
          discs: this.#discs,
          cube: cube === null ? null : this.#cubes.draw(cube, "hdr", exposed),
          previousRegimes: this.#regimes,
          meter: inputs.meter,
          // The marks cased over the tone-mapped image at the canvas's resolution (R07.T16.a), and
          // the craft on their silhouettes (R07.T16.e).
          overlay: overlaySubmission(this.#wireframe, list, tokens, camera, viewport),
        }),
      );
      if (plan === null) {
        return false;
      }
      this.#regimes = plan.regimes;
      this.#frameFailed = false;
      return true;
    } catch (error: unknown) {
      // A creation refused between a device loss and its restore: this frame draws the
      // wireframe, and the next tries again; said once until a frame draws or a restore.
      if (!this.#frameFailed) {
        console.error(`the photorealistic frame of ${this.#name} could not be drawn:`, error);
        this.#frameFailed = true;
      }
      return false;
    }
  }

  #cubeFor(sky: DrawnSky | null, setting: QualitySetting): BakedCube | null {
    const cache = skyCubeCacheOf(this.#engine);
    if (sky === null || sky === this.#failedFor) {
      cache.release(this.#name);
      return null;
    }
    try {
      return cache.acquire(this.#name, {
        stars: sky.model.stars,
        baked: sky.selection.baked,
        // Read at the bake: a restore may have brought a device without float32-blendable.
        bakeInput: () =>
          bakeInputOf(sky, skyFaceSizePx(setting, this.#engine.capabilities.float32Blendable)),
      });
    } catch (error: unknown) {
      // A bake that fails (a lost device) leaves the sprites; tried again on another sky.
      console.error("the sky's cube could not be baked:", error);
      this.#failedFor = sky;
      cache.release(this.#name);
      return null;
    }
  }

  /** Releases the renderers, the layers, the view's hold on the cube, and the view itself. */
  dispose(): void {
    this.#unsubscribe();
    this.#wireframe.dispose();
    this.#photoreal.dispose();
    this.#band.dispose();
    skyCubeCacheOf(this.#engine).release(this.#name);
    this.#cubes.dispose();
    this.#view.dispose();
  }
}

/**
 * Makes `view`'s drawer, at once or, where the engine has no device (a loss), once its restore has
 * told every listener, and hands it to `onMade`.
 *
 * @remarks
 * A drawer made inside the restore's dispatch would subscribe its renderers' own listeners to that
 * dispatch, which would then make their handles a second time and drop the first; so it is made in
 * a microtask after it.
 *
 * @returns Its release: stops waiting for a restore, and disposes of the drawer with the view, or
 *   of the view alone where no drawer was made.
 * @throws Whatever the drawer's first making throws but `EngineUnavailable`, having disposed of
 *   the view.
 */
export function makeViewFrameDrawer(
  engine: RenderEngine,
  view: RenderView,
  name: string,
  onMade: (drawer: ViewFrameDrawer) => void,
): () => void {
  let drawer: ViewFrameDrawer | null = null;
  let released = false;
  const make = (): void => {
    try {
      drawer = new ViewFrameDrawer(engine, view, name);
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        // Asked for during a loss: the view waits, and the drawer is made at the restore.
        return;
      }
      throw error;
    }
    onMade(drawer);
  };
  try {
    make();
  } catch (error: unknown) {
    // No release reaches the caller, so the view goes with the failure.
    view.dispose();
    throw error;
  }
  const unsubscribe = engine.onRestored(() => {
    if (drawer !== null) {
      return;
    }
    queueMicrotask(() => {
      if (released || drawer !== null) {
        return;
      }
      try {
        make();
      } catch (error: unknown) {
        console.error(`view ${name}'s drawer could not be made after the restore:`, error);
      }
    });
  });
  return () => {
    released = true;
    unsubscribe();
    if (drawer === null) {
      view.dispose();
    } else {
      drawer.dispose();
    }
  };
}
