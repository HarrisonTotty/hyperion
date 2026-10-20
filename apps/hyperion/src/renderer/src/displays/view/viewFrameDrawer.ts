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
import type { ViewBudget } from "../../view/budget/viewBudget";
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
import type { TerrainAnnunciation } from "../../view/terrain/annunciation";
import type { ClassMapSource } from "../../view/terrain/classMap";
import { TerrainFailureGuard } from "../../view/terrain/failureGuard";
import {
  LiveFrameTerrain,
  surveyEdgeViewsOf,
  type ViewTerrainInput,
  type ViewTerrainSession,
  type WireframeTerrainFrame,
  type WireframeTerrainInput,
  wireframeTerrainOf,
} from "../../view/terrain/frameTerrain";
import type { HandoverReliefSource } from "../../view/terrain/handover";
import type { TerrainViewReport } from "../../view/terrain/session";
import {
  buildWireframeDrawList,
  type DrawAnchor,
  type DrawnLineOfSight,
  type DrawnSurveyEdge,
  type SpriteStar,
  viewStrokesAt,
} from "../../view/wireframe/drawList";
import { NO_WIREFRAME_TERRAIN, WireframeRenderer } from "../../view/wireframe/submit";
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
  /**
   * The class maps its lit bodies' discs shade with (plan R10, R10.T10.d), made on the drawer's
   * engine: the terrain session's (R10.T16.a), every view's alike; absent, every disc is uniform.
   */
  readonly classMaps?: ClassMapSource | undefined;
  /**
   * Its lit bodies' reliefs, by which a body hands over to terrain (plan R10, R10.T10.e): the
   * terrain session's (R10.T16.a), every view's alike; absent, none hands over.
   */
  readonly reliefs?: HandoverReliefSource | undefined;
  /**
   * The view's terrain this frame (plan R10, R10.T16.c): the terrain session, the view's ID in it
   * and the frame's time, from which the drawer makes its photorealistic frame's `FrameTerrain`
   * (`LiveFrameTerrain`: the terrain of the bodies it hands over, on the drawer's engine), its
   * wireframe frame's terrain (`wireframeTerrainOf`, R10.T11) and its draw list's survey edges, in
   * either style (`surveyEdgeViewsOf`); absent, a body handed over is drawn as a mesh body is, the
   * wireframe draws no terrain and no survey edge is marked.
   */
  readonly terrain?: ViewTerrainInput | undefined;
  /**
   * Whether the view's terrain readout reads a point on its line of sight this frame, which its
   * tripod marks at the picture's centre (plan R10, R10.T12): `true` while the readout stands with a
   * point, over surveyed ground or not; `false` where it has none, at `NO GROUND`. Absent for a view
   * that states no readout, which draws no tripod: an instrument (decision-r10-t12-leans §3), or a
   * view under `TERRAIN NOT AVAILABLE` (§5). The drawer keeps the tripod's give-way from frame to
   * frame while it is given, `false` or `true`.
   */
  readonly lineOfSightPoint?: boolean | undefined;
}

/** What a frame drew. */
export interface ViewFrameDrawn {
  /** The marks where the frame drew them, device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** The style it was drawn in: the wireframe while the photorealistic pipelines compile. */
  readonly drawnStyle: RenderStyle;
  /**
   * The terrain's annunciation of the bodies the frame hands over, for the view's label block
   * (`labelStatements(run, terrain)`, plan R10, R10.T16.c), in either style (the wireframe's from
   * R10.T11); `null` with none.
   */
  readonly terrain: TerrainAnnunciation | null;
  /**
   * The contour interval the frame's wireframe terrain is drawn at, m, for the view's label block's
   * `CONTOUR INTERVAL` line (plan R10, R10.T11); `null` for a frame that draws no contours.
   */
  readonly contourIntervalM: number | null;
  /**
   * The bodies' survey edges the frame marked, each with the range to its nearest point seen
   * (`WireframeDrawList.surveyEdges`), for the view's list notes (plan R10, R10.T16.c;
   * decision-r10-t16-review §1); none without a terrain session.
   */
  readonly surveyEdges: ReadonlyArray<DrawnSurveyEdge>;
  /**
   * The line-of-sight mark the frame drew (`WireframeDrawList.lineOfSight`, plan R10, R10.T12): its
   * box where it is drawn, which the view's mark labels stand clear of; `null` where the frame was
   * given no line of sight.
   */
  readonly lineOfSight: DrawnLineOfSight | null;
}

/**
 * The size a view presents its frames at, device px: its stage's laid-out size times its
 * device-pixel ratio, rounded, at least 1 on each axis; its canvas's, which its wireframe draws at
 * and its photorealistic frame is presented at.
 */
export function presentedViewport(size: ElementSize): ViewSize {
  const ratio = size.devicePixelRatio;
  return {
    widthPx: Math.max(1, Math.round(size.widthPx * ratio)),
    heightPx: Math.max(1, Math.round(size.heightPx * ratio)),
  };
}

/**
 * What a view draws this frame, for the terrain session (plan R10, R10.T16.a): its run's scene and
 * pose, its field of view, its presented size, its budget's style and streaming priority, the
 * style it last drew in, the setting and its drawer's last plan's regimes; `null` while its stage
 * is not laid out, when it draws nothing.
 *
 * @param regimes - Its drawer's last photorealistic plan's regimes ({@link ViewFrameDrawer.regimes}).
 * @param streamPriority - Its budget's terrain streaming priority (`ViewBudget.streamPriority`).
 * @param drawnStyle - The style its last frame was drawn in ({@link ViewFrameDrawn.drawnStyle}), by
 *   which the session's passes take the views that hand a body over (the batch review's S8).
 */
export function terrainReportOf(
  inputs: ViewFrameInputs,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  streamPriority: ViewBudget["streamPriority"],
  drawnStyle: RenderStyle,
): TerrainViewReport | null {
  const { run, size } = inputs;
  if (size === null) {
    return null;
  }
  return {
    scene: run.scene,
    pose: runPose(run),
    fovXRad: (run.camera.fovDeg * Math.PI) / 180,
    presentedPx: presentedViewport(size),
    style: inputs.style,
    drawnStyle,
    setting: inputs.setting,
    streamPriority,
    regimes,
  };
}

/**
 * The style a view's frame is drawn in, as near as it is known before the draw (plan R10, R10.T16;
 * the batch review's S8): photorealistic where the view may draw so and its photorealistic
 * pipelines are made, and the wireframe otherwise, its stand-in while they are made, after a device
 * loss, or for good where they failed. A frame refused within its draw (a loss meanwhile) is drawn
 * in the wireframe all the same, which its {@link ViewFrameDrawn.drawnStyle} says.
 */
export function frameStyleOf(
  inputs: Pick<ViewFrameInputs, "style" | "availability">,
  status: PhotorealStatus,
): RenderStyle {
  return inputs.style === "photorealistic" &&
    inputs.availability.photorealistic &&
    status === "ready"
    ? "photorealistic"
    : "wireframe";
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
  /**
   * The guards of the survey edges (S5) and of the wireframe's terrain (R10.T11), R10.T16.e's: a
   * failure said once until the call succeeds again, and the session's terrain faulted once it has
   * failed on for 250 ms; made anew for each session (`#guardsFor`).
   */
  #edgesGuard = new TerrainFailureGuard();
  #wireGuard = new TerrainFailureGuard();
  /** The session the guards guard. */
  #guarded: ViewTerrainSession | null = null;
  /** The contour interval the view's wireframe terrain last held, its hysteresis's (R10.T11). */
  #contourIntervalM: number | null = null;
  #regimes: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
  /**
   * Whether the view's tripod gave way in its last frame drawn (plan R10, R10.T12), the give-way's
   * hysteresis, which its next frame's draw list starts from (`LineOfSightOptions.yielded`); `false`
   * before its first, and once a frame is drawn with no line of sight.
   */
  #lineOfSightYielded = false;
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
   * Each lit body's regime in its last photorealistic plan (R07's, with T10.e's `terrain`), the
   * hysteresis's previous frame, by which the terrain session takes the bodies it hands over
   * (plan R10, R10.T16.a's report); none before such a frame. A frame drawn in the wireframe plans
   * no regimes and leaves them as they were.
   */
  get regimes(): ReadonlyMap<BodyIdHex, LitRegime> {
    return this.#regimes;
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
    const viewport = presentedViewport(size);
    if (this.#sized?.widthPx !== viewport.widthPx || this.#sized.heightPx !== viewport.heightPx) {
      this.#view.resize(viewport);
      this.#sized = viewport;
    }
    const camera = { pose: runPose(run), fovXRad: (run.camera.fovDeg * Math.PI) / 180 };
    const ev100 = controlEv100(inputs.exposure);
    // The style this frame is drawn in, as near as it is known before the draw: photorealistic
    // where it may draw so and its pipelines are made (the batch review's S8).
    const triesPhotoreal =
      inputs.style === "photorealistic" &&
      inputs.availability.photorealistic &&
      this.#photoreal.status !== "failed";
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
        // The surveys' edges, in both styles (R10.T16.c), in the style the frame draws.
        surveyEdges:
          inputs.terrain === undefined
            ? []
            : this.#surveyEdges(
                inputs.terrain,
                run.scene,
                camera,
                viewport,
                frameStyleOf(inputs, this.#photoreal.status),
              ),
        // The tripod at the picture's centre where the readout has a point, and its give-way's
        // state from the frame before (R10.T12).
        lineOfSight:
          inputs.lineOfSightPoint === undefined
            ? null
            : { hasPoint: inputs.lineOfSightPoint, yielded: this.#lineOfSightYielded },
      },
    );
    this.#lineOfSightYielded = list.lineOfSight?.yielded ?? false;
    const cube = this.#cubeFor(inputs.sky, inputs.setting);
    const exposed = exposureScale(ev100);
    let drawn = false;
    let terrain: TerrainAnnunciation | null = null;
    if (triesPhotoreal) {
      const photoreal = this.#drawPhotoreal(inputs, tokens, viewport, camera, list, cube, exposed);
      drawn = photoreal.drawn;
      terrain = photoreal.terrain;
    }
    let contourIntervalM: number | null = null;
    if (!drawn) {
      // The bodies this view hands over to the wireframe's passes (R10.T11).
      const wire =
        inputs.terrain === undefined
          ? null
          : this.#wireTerrain(inputs.terrain, {
              view: inputs.terrain.view,
              scene: run.scene,
              camera,
              viewport,
              strokeScale: list.strokeScale,
              lowSetting: inputs.setting === "low",
              tokens,
              occluderSlopePx: list.occluderSlopePx,
              nowMs: inputs.terrain.nowMs,
              previousIntervalM: this.#contourIntervalM,
            });
      if (wire !== null) {
        terrain = wire.annunciation;
        contourIntervalM = wire.intervalM;
        this.#contourIntervalM = wire.heldIntervalM;
      }
      this.#wireframe.render(
        this.#view,
        list,
        camera,
        viewport,
        cube === null ? [] : [this.#cubes.draw(cube, "display", exposed)],
        wire ?? NO_WIREFRAME_TERRAIN,
      );
    }
    return {
      anchors: list.anchors,
      drawnStyle: drawn ? "photorealistic" : "wireframe",
      terrain,
      contourIntervalM,
      surveyEdges: list.surveyEdges,
      lineOfSight: list.lineOfSight,
    };
  }

  /**
   * The wireframe's terrain this frame (`wireframeTerrainOf`, plan R10, R10.T11), or none where it
   * cannot be drawn: a loss (`EngineUnavailable`) passes in silence, and anything else, a bug, is
   * reported once until it is drawn again, so that the view draws on, as the survey edges do; once
   * it has failed for `ANNUNCIATION_ONSET_MS`, the session's terrain is faulted (R10.T16.e).
   */
  #wireTerrain(
    terrain: ViewTerrainInput,
    input: WireframeTerrainInput,
  ): WireframeTerrainFrame | null {
    const session = terrain.source;
    return this.#guardsFor(session).wire.run(
      () => wireframeTerrainOf(session, input),
      null,
      terrain.nowMs,
      {
        describe: `the wireframe terrain of ${this.#name} could not be drawn:`,
        onFault: () => {
          session.failComputation();
        },
      },
    );
  }

  /** The guards of `session`'s calls, made anew when the session changes (R10.T16.e). */
  #guardsFor(session: ViewTerrainSession): {
    readonly edges: TerrainFailureGuard;
    readonly wire: TerrainFailureGuard;
  } {
    if (session !== this.#guarded) {
      this.#guarded = session;
      this.#edgesGuard = new TerrainFailureGuard();
      this.#wireGuard = new TerrainFailureGuard();
    }
    return { edges: this.#edgesGuard, wire: this.#wireGuard };
  }

  #drawPhotoreal(
    inputs: ViewFrameInputs,
    tokens: ColourTokens,
    viewport: ViewSize,
    camera: { readonly pose: CameraPose; readonly fovXRad: number },
    list: ReturnType<typeof buildWireframeDrawList>,
    cube: BakedCube | null,
    exposed: number,
  ): { readonly drawn: boolean; readonly terrain: TerrainAnnunciation | null } {
    const { sky, run } = inputs;
    // The view's own terrain of the session's passes, for this frame alone (R10.T16.c).
    const terrain =
      inputs.terrain === undefined
        ? undefined
        : new LiveFrameTerrain(inputs.terrain.source, {
            view: inputs.terrain.view,
            camera: { orientation: camera.pose.orientation, fovXRad: camera.fovXRad },
            presentedPx: viewport,
            exposureScale: exposed,
            occluderSlopePx: list.occluderSlopePx,
            nowMs: inputs.terrain.nowMs,
          });
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
          classMaps: inputs.classMaps,
          reliefs: inputs.reliefs,
          terrain,
          // The marks cased over the tone-mapped image at the canvas's resolution (R07.T16.a), and
          // the craft on their silhouettes (R07.T16.e).
          overlay: overlaySubmission(this.#wireframe, list, tokens, camera, viewport),
        }),
      );
      if (plan === null) {
        return { drawn: false, terrain: null };
      }
      this.#regimes = plan.regimes;
      this.#frameFailed = false;
      return { drawn: true, terrain: terrain?.annunciation ?? null };
    } catch (error: unknown) {
      // A creation refused between a device loss and its restore: this frame draws the
      // wireframe, and the next tries again; said once until a frame draws or a restore.
      if (!this.#frameFailed) {
        console.error(`the photorealistic frame of ${this.#name} could not be drawn:`, error);
        this.#frameFailed = true;
      }
      return { drawn: false, terrain: null };
    }
  }

  /**
   * The survey edges the frame marks (`surveyEdgeViewsOf`), or none where they cannot be made: a
   * loss (`EngineUnavailable`) passes in silence, and anything else, a bug, is reported once until
   * they are made again, so that the view draws on (the batch review's S5); once they have failed
   * for `ANNUNCIATION_ONSET_MS`, the session's terrain is faulted (`TerrainFailureGuard`;
   * R10.T16.e), since the guide requires the edge wherever surveyed terrain meets unsurveyed ground.
   */
  #surveyEdges(
    terrain: ViewTerrainInput,
    scene: ViewScene,
    camera: { readonly pose: CameraPose; readonly fovXRad: number },
    viewport: ViewSize,
    style: RenderStyle,
  ): ReturnType<typeof surveyEdgeViewsOf> {
    const session = terrain.source;
    return this.#guardsFor(session).edges.run(
      () => surveyEdgeViewsOf(session, terrain.view, scene, camera, viewport, style),
      [],
      terrain.nowMs,
      {
        describe: `the survey edges of ${this.#name} could not be marked:`,
        onFault: () => {
          session.failComputation();
        },
      },
    );
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
