/**
 * One photorealistic view's frame (plan R07, T8.a; Design note 8): the passes of
 * {@link photorealisticPasses} that are built, in their order, into the view's HDR scene target,
 * then bloom and the tone-mapping pass onto the canvas, and the symbology over it.
 *
 * @remarks
 * Into the scene target, pre-exposed: R06's band, then its baked star cube (each at infinity,
 * additive, keeping the meter class), the star sprites through R06's `POINT SPRITES HDR` (stars,
 * and host discs under three pixels); then the `disc cells` dispatch, which shades every cell of
 * the discs under 32 px at once for the draws after it to read (R07.T8.d), where the frame has
 * such a disc; then the mesh bodies' figures, opaque with depth, as the
 * `bodies` pass where the frame has any (T9), with the terrain of the bodies handed over to it
 * where the frame has a {@link FrameTerrain} (plan R10, R10.T10.e), the reference surface beneath
 * it, opaque first as Apple's GPUs ask (plan R14, R14.T2.e; its Design note 7): the terrain that
 * never discards, then the figures, which may, then the rest of the terrain; or, where the terrain
 * shades by the visibility buffer (plan R14, R14.T2.c and T2.d), its ID raster into the view's
 * visibility target with the scene's depth, then the resolve's pass, which shades the raster's
 * pixels once, then draws the figures and the rest of the terrain over them; then the painter's
 * sequence of Design note 2: host
 * discs (R06's pass, meter class 0), disc bodies, mesh bodies' limbs and point bodies, back to
 * front by power. Then the bloom
 * chain over the light above the display's range, the tone-mapping pass with each host disc's glare
 * source, encoded and dithered in the pass (`encoding: "in-pass"`), and a second canvas pass that
 * loads it for the cased symbology (`colourLoad: "load"`, decisions-r06-r07 item 6), the terrain
 * drawn into its depth first, so that nearer ground hides the marks behind it (R10.T16.c), the
 * symbology built once the plan is made, so that a body handed over has its datum give way over its
 * surveyed ground as its reference surface does (R10.T16.f; {@link PhotorealOverlay}). Each body
 * whose clouds are drawn (plan R11, R11.T7.b) has its two-dimensional layer drawn after its
 * atmosphere's composite, into the target that composite drew, as the `clouds` pass, and its
 * terrain lit beneath them where the setting's `cloudShadows` is on ({@link withCloudsOnGround}). Each frame
 * that carries a meter takes the scene target's exposure histogram under it (R07.T12), the veil of
 * its glare sources in it, read back for the view's `AutoExposure`
 * ({@link PhotorealRenderer.takeHistogram}); a frame of a view that
 * meters nothing, an instrument's, takes none (R07.T19.c). R08's, R10's and R11's passes join
 * when they exist. Its resources are made asynchronously, the pipelines compiled
 * before the first frame, and again after a device loss; until then {@link
 * PhotorealRenderer.render} draws nothing and says so, and the view keeps its wireframe.
 */
import type { AtmosphereLeversOverride } from "../../../../preload/api";
import type { BodyIdHex, UniverseTime } from "@hyperion/protocol";

import { norm } from "../../geometry/vec3";
import {
  ATMOSPHERE_COMPOSITE_PASS,
  type AtmosphereCamera,
  AtmosphereRetired,
  type AtmosphereFrameKernels,
  createAtmosphereFrameKernels,
  geodeticOf,
  HillaireAtmosphere,
  type SourceTransmittance,
  type SunState,
  tablesFigureOf,
} from "../atmosphere/hillaire";
import type { AbsorberCurve } from "../atmosphere/absorbers";
import type { DiscAir } from "../atmosphere/discReflectance";
import { gravityRatio } from "../atmosphere/oblate";
import { skySources } from "../atmosphere/sources";
import {
  createAirlessTables,
  type SurfaceLightingTables,
  surfaceLightingTablesOf,
} from "../atmosphere/surfaceLighting";
import { absorberCount, type AtmosphereTables } from "../atmosphere/tables";
import { CORRECTION_ROUTED } from "../atmosphere/thick/routing";
import {
  CLOUD_LAYER_MATERIAL,
  CLOUD_LAYER_PASS,
  type CloudBodyGpu,
  cloudLayerDraw,
  cloudLayerHeightM,
  CloudLayers,
  cloudLayerUniforms,
  cloudShellOf,
  LAYER_OCTAVES,
  layerCondensateOf,
  seasonOf,
} from "../clouds/cloudLayer";
import {
  CLOUD_COMPOSITE_MATERIAL,
  CLOUD_RESOLVE_MATERIAL,
  CLOUD_VOLUME_MATERIAL,
  CloudVolumes,
} from "../clouds/cloudVolumePass";
import type { BodyClouds, CloudCondensate } from "../clouds/condensates";
import {
  BODY_DISC_CELLS_AIR_KERNEL,
  BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL,
  BODY_DISC_CELLS_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_CELLS_PACKED_AIR_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_AIR_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_PACKED_KERNEL,
  type BodyFramePlan,
  type BodyLightSources,
  bodyLightSources,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../bodies/draw";
import type { DiscRecord } from "../bodies/discShading";
import { drawnAsMesh, type LitRegime, type ScreenCircle } from "../bodies/regime";
import {
  DEPTH_PYRAMID_KERNEL,
  DepthPyramid,
  type FrameOcclusion,
  pyramidProjectionOf,
} from "../clusters/hzb";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../camera/projection";
import type { Quaternion } from "../camera/pose";
import type { ViewRole } from "../camera/state";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { GpuTimer, GraphicsFault } from "../engine/status";
import type {
  BufferHandle,
  ColourTargetFormat,
  ComputeHandle,
  DrawItem,
  FrameSubmission,
  MaterialHandle,
  MeshHandle,
  PassTimes,
  RenderEngine,
  RenderTarget,
  RenderView,
  TexelRect,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import type { KernelPair } from "../engine/kernels";
import { lightsAt, type PlacedLight } from "../lighting/hostLights";
import { bloomKernel, bloomThreshold, levelWeight } from "../post/bloom";
import {
  BLOOM_UP_MATERIAL,
  BloomChain,
  fullScreenTriangle,
  GLARE_SOURCE_BYTES,
  packGlareSources,
} from "../post/bloomChain";
import { BLUE_NOISE_SIDE, blueNoiseTile } from "../post/blueNoise";
import { type GlareSource, glareSpreadTerms } from "../post/glare";
import { HISTOGRAM_KERNEL, type Histogram, HistogramReader } from "../post/histogram";
import type { HistogramMeter } from "../post/meter";
import { TONEMAP_MATERIAL, TONEMAP_PASS, tonemapDraw } from "../post/tonemap";
import type { BalancedWhite } from "../post/whiteBalance";
import { OVERLAY_SCENE_DEPTH_MATERIAL, overlaySceneDepthDraw, sizesAgree } from "./overlayDepth";
import {
  type GeometrySettings,
  QUALITY_SETTINGS,
  type QualitySetting,
  SETTINGS,
} from "../quality/qualitySetting";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import { type OceanSea, STANDARD_GRAVITY_MPS2 } from "../ocean/oceanBody";
import {
  drawsNearSea,
  nearSeaAnchor,
  OCEAN_GRID_CELL_PX,
  OCEAN_WAVE_ROWS,
  oceanGridMesh,
  oceanGridSize,
  packOceanWaves,
} from "../ocean/oceanNear";
import {
  OCEAN_COPY_PASS_LABEL,
  OCEAN_DEPTH_COPY_MATERIAL,
  OCEAN_NEAR_PASS_MATERIAL,
  oceanDepthCopyDraw,
  oceanNearDraw,
  oceanNearState,
  type OceanNearState,
  oceanNearUniforms,
  type OceanNearView,
} from "../ocean/oceanNearPass";
import {
  OCEAN_ECLIPSE_ROWS,
  OCEAN_FAR_PASS_MATERIAL,
  OCEAN_PASS_LABEL,
  type OceanDrawView,
  type OceanEclipse,
  OceanGlare,
  oceanDrawOptics,
  oceanPassDraw,
  oceanPassUniforms,
  oceanSourcesOfRecord,
  packOceanEclipse,
} from "../ocean/oceanPass";
import { DEFAULT_EYE_OBSERVER } from "../sky/eye";
import { toHalfArray } from "../sky/half";
import { SKY_SPRITE_HDR_MATERIAL } from "../sky/spriteHdr";
import type { SpriteRecord } from "../wireframe/drawList";
import { sphereOutsideView, type SurveyedDatumFrame, WIREFRAME_MESHES } from "../wireframe/submit";
import { type OverlayHider, overlayHidersOf, surveyedDatumsOf } from "./overlay";
import { PHOTOREAL_PASS_LABELS, SKY_PASS_LABEL } from "./passes";
import { withVisibilityIds } from "../clusters/resolve";
import {
  VISIBILITY_LATE_RASTER_LABEL,
  VISIBILITY_RASTER_LABEL,
  VISIBILITY_RESOLVE_LABEL,
  VisibilityBuffer,
} from "../clusters/visibility";
import { createSceneTarget } from "./sceneTarget";
import { log } from "../../lib/pinnedMath";
import type { MeasuredPass } from "../stills/batchController";
import type {
  StillDevice,
  StillRenderer,
  StillScene,
  StillSnapshot,
  StillSource,
} from "../stills/stillJob";
import { isStillSide, type StillRequest } from "../stills/stillLabel";
import { planTiles, type StillTile, type StillTiling, tileProjection } from "../stills/stillPlan";

/**
 * What draws the terrain of a photorealistic frame's bodies handed over to it (plan R10, R10.T10.e;
 * R10 Design note 13): the view's share of the terrain session's passes, one a body
 * (`TerrainPass`, lit by its disc record's lights), with the GPU coverage masks of their surveys
 * (`CoverageMask`).
 *
 * @remarks
 * The terrain session gives each view's frame its own (plan R10, R10.T16.c; `LiveFrameTerrain`).
 * Without one, a body handed over is drawn as a mesh body is, its figure whole. R10.T16.c changed
 * T10.e's shape, as main accepted (2026-10-19): `draws(plan, bodies)` takes the frame's lit bodies
 * beside its plan, which holds no body's centre or rotation, and `symbologyDepth()` is new.
 */
export interface FrameTerrain {
  /**
   * The GPU coverage mask of `body`'s survey (`terrain/coverage.ts`' `CoverageMask.texture`),
   * beneath whose surveyed cells its figure gives the ground to its terrain once it is handed
   * over; `null` where it has none, or where the view's terrain cannot yet draw all the ground the
   * mask would discard (R10.T16.c; decision-r10-live-path, D7).
   */
  surveyCoverage(body: BodyIdHex): TextureHandle | null;
  /**
   * Whether the mask {@link FrameTerrain.surveyCoverage} gives holds every cell surveyed, so that
   * its body's figure and limb discard every pixel and the `disc cells` pass shades none of them
   * (R07.T8.h); `false` where it gives none.
   */
  surveyedWhole(body: BodyIdHex): boolean;
  /**
   * R_occ of `body`'s terrain, m: the sphere about its centre inside every surface it can draw, its
   * polar radius and its lowest height (`terrain/cull.ts`' `horizonCone`), behind whose ground the
   * overlay's marks may lie hidden (`GeometrySettings`' `hiddenMarks`); `null` for a body with no
   * pass. Absent, as on a fake, no mark is taken as hidden.
   */
  innerRadiusM?(body: BodyIdHex): number | null;
  /**
   * The draws of the terrain of each body `plan` hands over (`LitRegime` `terrain`, a
   * `MeshBodyPlan` whose `role` is `handed-over`, its record its lights), opaque with depth, drawn
   * in the `bodies` pass after the mesh bodies' figures; none for a body with nothing resident.
   *
   * @remarks
   * On the high setting it submits the view's shadow cascades' four caster passes, for the body
   * that takes them, before it returns (plan R10, R10.T9.b; decision-r10-t9b-design D1 and D3): it
   * is called as the `bodies` pass is built, and that pass is rendered next, so that its resolve
   * times them and the terrain's draws read the layers they drew.
   *
   * With `occlusion` (plan R14, R14.T3.d), its lit clusters visible last frame alone; the rest are
   * {@link FrameTerrain.lateDraws}'.
   *
   * @param bodies - The frame's lit bodies, which the plan was made from: each handed-over body's
   *   terrain is drawn with the rotation and centre the plan drew its figure with (R10.T16.c).
   * @param occlusion - The frame's two-pass occlusion, where the geometry asks it (`occlusion`
   *   `hzb`).
   */
  draws(
    plan: BodyFramePlan,
    bodies: ReadonlyArray<LitBodyInput>,
    occlusion?: FrameOcclusion,
  ): ReadonlyArray<DrawItem>;
  /**
   * The two-pass occlusion's late draws (plan R14, R14.T3.d): the lit clusters the frame's
   * {@link FrameTerrain.draws} left to the second raster that the view's pyramid, built from the
   * first raster's depth, does not refuse; none without the occlusion. Called once, after the
   * pyramid's build, and drawn in the `bodies late` pass.
   */
  lateDraws(): ReadonlyArray<DrawItem>;
  /**
   * The terrain the frame's {@link FrameTerrain.draws} drew, as depth alone, which the symbology's
   * pass draws before its lines, so that nearer ground hides a mark behind it over the image as it
   * hides it in the image (plan R10, R10.T16.c).
   */
  symbologyDepth(): ReadonlyArray<DrawItem>;
  /**
   * Whether the symbology's pass may take the scene's depth in place of
   * {@link FrameTerrain.symbologyDepth} where the scene target's size is the canvas's (plan R14,
   * R14.T1.e; `GeometrySettings.overlayDepth` `scene-depth`).
   */
  sceneDepth(): boolean;
  /**
   * The terrain the last {@link FrameTerrain.draws} drew, by the visibility buffer (plan R14,
   * R14.T2.c; `GeometrySettings.shading` `visibility`), where every body it drew gives its own;
   * `null` otherwise, when its draws draw it forward.
   */
  visibility(): FrameVisibility | null;
  /**
   * Of the last {@link FrameTerrain.draws}, those that never discard, in their order (plan R14,
   * R14.T2.e): each body's interior patches', which the `bodies` pass draws before the mesh bodies'
   * figures, and the rest after them.
   */
  opaqueDraws(): ReadonlyArray<DrawItem>;
}

/**
 * A frame's terrain by the visibility buffer (plan R14, R14.T2.c; Design note 9), the same
 * triangles as its forward draws, each part every handed-over body's in the frame's order: the ID
 * raster's draws, into the view's visibility target with the scene's depth, then the resolve's,
 * which shade its pixels once, then the draws drawn forward over them, depth-tested against the
 * raster's depth (`TerrainVisibility`).
 */
export interface FrameVisibility {
  readonly raster: ReadonlyArray<DrawItem>;
  /**
   * Each body's resolve (plan R14, R14.T2.d), drawn first in the resolve's pass, with the view's IDs
   * bound (`clusters/resolve.ts`' `withVisibilityIds`).
   */
  readonly resolve: ReadonlyArray<DrawItem>;
  readonly forward: ReadonlyArray<DrawItem>;
}

/**
 * One body's atmosphere in a photorealistic frame (plan R08, R08.T10.b): the atmosphere cache's
 * per-planet tables, which the frame draws with and never disposes, taken afresh each frame, and
 * the body's atmosphere camera.
 */
export interface FrameAtmosphere {
  readonly id: BodyIdHex;
  /** Its per-planet tables (`AtmosphereCache.entryOf(id).tables`), its figure and gravity theirs. */
  readonly tables: AtmosphereTables;
  /**
   * Its atmosphere's camera (`bodyAtmosphereCamera`): from its drawn centre, in its body-fixed
   * axes, at the frame's own viewport.
   */
  readonly camera: AtmosphereCamera;
  /** The rotation from the frame's galactic axes into its body-fixed ones (`skySources`'). */
  readonly toBodyFixed: Quaternion;
  /**
   * What of its record its clouds read (plan R11, R11.T7.b; `AtmosphereEntry.clouds`): its cloud
   * fraction, its air's surface pressure and its decks; `null`, or absent, for a body whose clouds
   * are not drawn.
   */
  readonly clouds?: BodyClouds | null | undefined;
}

/**
 * One body's sea in a photorealistic frame (plan R11, R11.T8.b): the terrain session's
 * (`TerrainSession.oceans`), and the body's camera, from its drawn centre in its body-fixed axes,
 * which its ocean map is laid out in.
 */
export interface FrameOcean {
  readonly id: BodyIdHex;
  readonly sea: OceanSea;
  /** The body's camera (`bodyAtmosphereCamera`), its axes the body's rotation's. */
  readonly camera: AtmosphereCamera;
  /** The rotation from the frame's galactic axes into the body-fixed ones. */
  readonly toBodyFixed: Quaternion;
  /** The scene's time on the universe clock, at which its near sea's waves stand (R11.T8.c). */
  readonly time: UniverseTime;
}

/**
 * The symbology's canvas pass over a photorealistic frame (`overlaySubmission`), built by
 * {@link PhotorealRenderer.render} once it has made the frame's plan (plan R10, R10.T16.f; R10's
 * Design note 12), from the datums of the bodies the plan hands over to terrain with a coverage mask
 * (`surveyedDatumsOf`): each one's occluder sphere and own lines, its limb and graticule, discarded
 * wherever the mask that discards its reference surface holds a cell surveyed.
 *
 * @remarks
 * R10.T16.f changed R07's `PhotorealFrame.overlay`, a `FrameSubmission` built before the plan, to
 * this, and left T10.e's and T16.c's `FrameTerrain` as it was: the plan's own role holds the mask
 * and the rotation its reference surface was discarded with, so that the datum gives way on the
 * same mask, where the frame terrain's `surveyCoverage`, read again after its draws, might answer
 * otherwise.
 *
 * With `GeometrySettings`' `hiddenMarks` `culled` it is given the bodies among those whose mask
 * holds every cell (`overlayHidersOf`, its role's `whole`) and whose terrain names the sphere inside
 * all its ground (`FrameTerrain.innerRadiusM`), behind whose ground it leaves out the marks hidden
 * there (`hiddenMarksCulled`); a submission that says no draw of it tests depth (`depthTested`
 * `false`) is drawn with none of the terrain's depth before it, and one that does not say is drawn
 * with it.
 */
export type PhotorealOverlay = (
  surveyed: ReadonlyMap<string, SurveyedDatumFrame>,
  hiders: ReadonlyArray<OverlayHider>,
) => FrameSubmission & { readonly depthTested?: boolean };

/** What one photorealistic frame draws. */
export interface PhotorealFrame {
  readonly camera: ProjectionCamera;
  /** The frame's own viewport, device px: the scene target's, at the frame's internal scale. */
  readonly viewport: Viewport;
  /**
   * The size the view is presented at, device px, its canvas's, whose centre pixel the lit bodies'
   * hand-over to terrain measures (plan R10, R10.T10.g; `BodyFrameOptions.presentedPx`): the
   * frame's `viewport` where it is drawn at the presented size, larger under an internal scale.
   */
  readonly presentedPx: Viewport;
  /**
   * The size the view's terrain selects at, whose centre pixel the hand-over measures in
   * {@link PhotorealFrame.presentedPx}'s place (`BodyFrameOptions.selectionPx`; its CSS px, lean
   * 2026-11-01); absent, `presentedPx`.
   */
  readonly selectionPx?: Viewport | undefined;
  readonly role: ViewRole;
  readonly setting: QualitySetting;
  /**
   * How the frame's geometry is drawn (plan R14): a launch's `--geometry` over its setting's, which
   * the mesh bodies' figures take (R14.T4.a); {@link setting}'s where absent.
   */
  readonly geometry?: GeometrySettings | undefined;
  /**
   * The setting whose sizes the frame's air is drawn at (`TABLE_SIZES`' sky view, aerial
   * perspective and ray march): the low setting's while the view's frame-rate floor has stepped it
   * down (`BudgetedScale.airStepped`; the owner's report, 2026-11-01); {@link setting} where
   * absent.
   */
  readonly atmosphereSetting?: QualitySetting | undefined;
  /**
   * A frame profile's atmosphere levers over its setting's `SETTINGS[s].atmosphereLevers` (plan R08,
   * R08.T8.d; `--march-cover`, `--limb-rays`, `--march-sources`, `--surface-lobes`,
   * `--decked-air-surface`), which its atmospheres draw with, the A/B of each
   * lever on one build; the setting's alone where absent.
   */
  readonly atmosphereLevers?: AtmosphereLeversOverride | undefined;
  /**
   * Whether the view's frame-rate floor stands at its lowest step (`BudgetedScale.atLowestStep`),
   * when a large disc's limb pixels take `PhotorealSettings.floorLimbSamples` cells (R07.T8.i, the
   * owner's priority). Absent, `false`.
   */
  readonly floorLowest?: boolean | undefined;
  /** The exposure scale the target is pre-exposed with (`exposureScale`), 1 ÷ (cd/m²). */
  readonly exposureScale: number;
  /**
   * The scene's time on the universe clock (`ViewScene.time`), at which its bodies' clouds are
   * drawn (plan R11, R11.T7.b); absent, the clock's zero.
   */
  readonly time?: Pick<UniverseTime, "seconds" | "nanos"> | undefined;
  /**
   * The scene's time rate, simulation seconds per real second (`ViewScene.timeRate`), which fades
   * the clouds' octaves that would flicker under it (`cloudDensity.ts`' `timeRateFade`); absent, 0.
   */
  readonly timeRate?: number | undefined;
  /**
   * The projection its passes draw through in P's place (plan R11, R11.T10.b): a still's sample's,
   * `tileProjection` of the still with the sample's sub-pixel offset, which only what is projected
   * through the whole matrix takes, the figures and R06's sky; absent, as for every view's frame,
   * P of the camera and the viewport.
   */
  readonly projection?: Float32Array | undefined;
  /** R06's draws at infinity, in order: the band, then the baked cube. */
  readonly sky: ReadonlyArray<DrawItem>;
  /** The star sprites' records (stars, and host discs under three pixels), at depth 0. */
  readonly starSprites: ReadonlyArray<SpriteRecord>;
  /** R06's host-disc draws by star (`DiscFrame.draws`), placed at their painter entries. */
  readonly hostDraws: ReadonlyMap<number, ReadonlyArray<DrawItem>>;
  /** R06's glare sources (`HostDiscLayer.glareSources`). */
  readonly glareSources: ReadonlyArray<GlareSource>;
  /**
   * The white balance its tone step applies (`frameWhiteBalance`): the adaptation to its brightest
   * host star's white under `HOST`; `null`, or absent, under `D65`, whose step is skipped.
   */
  readonly whiteBalance?: BalancedWhite | null | undefined;
  /**
   * The lights at their stars' drawn centres in this frame (`placeLights`), which keep the hosts'
   * places in the painter's order and light a body without its own lighting frame.
   */
  readonly lights: ReadonlyArray<PlacedLight>;
  readonly bodies: ReadonlyArray<LitBodyInput>;
  /**
   * The footprints of the view's other geometry that writes depth (R10's terrain, lit craft), over
   * which a disc is drawn as a mesh (Design note 2); none in a view today.
   */
  readonly depthWriters: ReadonlyArray<ScreenCircle>;
  /** Each body's regime on the previous frame, for the hysteresis. */
  readonly previousRegimes: ReadonlyMap<BodyIdHex, LitRegime>;
  /**
   * The bodies drawn by their disc tables on the previous frame (`BodyFramePlan.tabled`, plan R08,
   * R08.T16.b), for the 10⁹ m boundary's hysteresis; absent, none.
   */
  readonly previousTabled?: ReadonlySet<BodyIdHex> | undefined;
  /**
   * Each body handed over on the previous frame, with its relief's subtense where it handed over
   * (`BodyFramePlan.handedOver`, plan R10, R10.T15.b.2), from which its terrain's own detail fades
   * in; absent, every fade starts at the hand-over's threshold.
   */
  readonly previousHandedOver?: ReadonlyMap<BodyIdHex, number> | undefined;
  /**
   * The terrain of the bodies handed over to it (plan R10, R10.T10.e), or absent, when a body
   * handed over is drawn as a mesh body is.
   */
  readonly terrain?: FrameTerrain | undefined;
  /**
   * The symbology's canvas pass, built after the plan and drawn loading the tone-mapped image, or
   * `null` (R10.T16.f).
   */
  readonly overlay: PhotorealOverlay | null;
  /**
   * The operator's meter, whose weights the frame's histogram takes (Design note 10), or `null` for
   * a view that is not the exposure's source, or whose meter reads none (`INCIDENT`), whose frame
   * takes no histogram (Design note 11).
   */
  readonly meter: HistogramMeter | null;
  /**
   * Whether the frame is presented: `false` draws the scene target and takes its histogram, then
   * draws nothing onto the view, which keeps the image it shows, while a cut's exposure awaits its
   * reading (`AutoExposure.holds`; fix-view-light); presented where absent.
   */
  readonly present?: boolean | undefined;
  /**
   * The atmospheres of the frame's lit bodies that have them drawn (plan R08, R08.T10.b): each
   * body's tables lighting its surface through the air, and its composite over everything opaque
   * drawn before it; none, or absent, draws no atmosphere.
   */
  readonly atmospheres?: ReadonlyArray<FrameAtmosphere> | undefined;
  /**
   * The seas of the frame's lit bodies that have them drawn (plan R11, R11.T8.b): each drawn by the
   * `ocean` pass where its body is drawn as a mesh or handed over to terrain, its glint's light past
   * the bloom threshold a glare source beside {@link PhotorealFrame.glareSources}; none, or absent,
   * draws no sea.
   */
  readonly oceans?: ReadonlyArray<FrameOcean> | undefined;
  /**
   * Takes a throw from the drawing of a body's atmosphere, other than a lost device's
   * (`decision-r08-t10b-leans.md` §2), with the view's name: the view's handle on the cache, which
   * holds the body failed and logs it (`AtmosphereView.reportDrawFailure`). Absent, the renderer
   * logs it itself.
   */
  readonly onAtmosphereFailed?:
    ((body: BodyIdHex, error: unknown, viewName: string) => void) | undefined;
}

/** Where a {@link PhotorealRenderer} stands. */
export type PhotorealStatus = "idle" | "making" | "ready" | "failed";

/** No light's curves: a sea's glare line through air whose tables hold none for its light. */
const NO_CURVES: ReadonlyArray<AbsorberCurve> = [];

/** The glare sources the buffer holds before it grows. */
const GLARE_CAPACITY = 8;

/** The smallest sprite buffer made, bytes; it grows by doubling. */
const MIN_SPRITE_BYTES = 4_096;

let blueNoiseHalves: Uint16Array | null = null;

/**
 * The blue-noise tile's texels as the texture takes them, in half precision.
 *
 * @remarks
 * The tile is a fixed pattern, the same for every renderer and every device, and takes about
 * 0.2 s to make, so it is made once and its halves kept, as `bloom.ts` keeps its levels'
 * responses. The texture write copies it; nothing writes into it.
 */
function blueNoiseTexels(): Uint16Array {
  blueNoiseHalves ??= toHalfArray(blueNoiseTile());
  return blueNoiseHalves;
}

/**
 * A form of the `disc cells` kernel: the plain one, R08.T18.a's emissive, R08.T16.b's air, or
 * R08.T18.b's both.
 */
type CellForm = "plain" | "emissive" | "air" | "emissiveAir";

/** A way of the `disc cells` kernel (`GeometrySettings.discCells`): a cell an invocation, or packed. */
type CellWay = GeometrySettings["discCells"];

/**
 * Each way's kernel of each form. The per-cell plain kernel is the renderer's own, made with its
 * other pipelines (`Resources.discCells`); every other is made at the first frame that asks for it.
 */
const CELL_FORM_KERNELS: Readonly<Record<CellWay, Readonly<Record<CellForm, KernelPair>>>> = {
  "per-cell": {
    plain: BODY_DISC_CELLS_KERNEL,
    emissive: BODY_DISC_CELLS_EMISSIVE_KERNEL,
    air: BODY_DISC_CELLS_AIR_KERNEL,
    emissiveAir: BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL,
  },
  packed: {
    plain: BODY_DISC_CELLS_PACKED_KERNEL,
    emissive: BODY_DISC_CELLS_PACKED_EMISSIVE_KERNEL,
    air: BODY_DISC_CELLS_PACKED_AIR_KERNEL,
    emissiveAir: BODY_DISC_CELLS_PACKED_EMISSIVE_AIR_KERNEL,
  },
};

/**
 * The way of the `disc cells` kernel whose forms {@link PhotorealRenderer.prepareEmissive} and its
 * kin make where they are not told: the high setting's (`GeometrySettings.discCells`), the low's
 * too.
 */
const PREPARED_CELL_WAY: CellWay = SETTINGS.high.geometry.discCells;

/**
 * A form of the `disc cells` kernel as one device's renderer asks for it: R08.T18.a's emissive,
 * R08.T16.b's air, or R08.T18.b's emissive with air.
 */
interface FormCells {
  readonly generation: number;
  /** The kernel, once made; `null` before, and where it could not be made. */
  kernel: ComputeHandle | null;
  /** Settles once it is made, or could not be; never rejects. */
  made: Promise<void>;
}

/** The form of the `disc cells` kernel a record is shaded by. */
function cellFormOf(record: DiscRecord): CellForm {
  const emits = record.emission !== undefined;
  const aired = record.air !== undefined;
  return emits && aired ? "emissiveAir" : aired ? "air" : emits ? "emissive" : "plain";
}

/** No form of one way's `disc cells` kernel asked yet. */
function noWayCells(): Record<CellForm, FormCells | null> {
  return { plain: null, emissive: null, air: null, emissiveAir: null };
}

/** No form of the `disc cells` kernel asked yet, of either way. */
function noFormCells(): Record<CellWay, Record<CellForm, FormCells | null>> {
  return { "per-cell": noWayCells(), packed: noWayCells() };
}

/** The near sea's copies of the scene before its draws (plan R11, R11.T8.c), under `ocean`. */
interface OceanCopies {
  /** The scene's depth's bits, `r32uint`. */
  readonly depthBits: RenderTarget;
  /** The scene's colour, made only with refraction. */
  colour: RenderTarget | null;
}

/** A sea drawn near this frame (R11.T8.c), its draw made once the copies are. */
interface NearSea {
  readonly view: OceanNearView;
  readonly state: OceanNearState;
  readonly ocean: FrameOcean;
  readonly tables: SurfaceLightingTables;
  readonly eclipseRows: BufferHandle;
}

/** The renderer's GPU objects on one device. */
interface Resources {
  readonly target: RenderTarget;
  readonly bloom: BloomChain;
  readonly sprite: MaterialHandle;
  readonly quad: MeshHandle;
  readonly tonemap: MaterialHandle;
  /** The scene's depth into the symbology's pass (plan R14, R14.T1.e; `overlayDepth.ts`). */
  readonly overlayDepth: MaterialHandle;
  readonly triangle: MeshHandle;
  readonly blueNoise: TextureHandle;
  /** The scene target's exposure histogram, read back one to three frames late (R07.T12). */
  readonly histograms: HistogramReader;
  /** The `disc cells` kernel, which shades the small discs' cells in parallel (R07.T8.d). */
  readonly discCells: ComputeHandle;
  /**
   * The depth pyramid's kernel (plan R14, R14.T3.d), made with the rest, run only with occlusion;
   * `null` where its compiler refused it, when the view draws in one raster.
   */
  readonly pyramidKernel: ComputeHandle | null;
  /** The atmospheres' shared per-frame kernels and composite (R08.T10.b), made with the rest. */
  readonly atmosphere: AtmosphereFrameKernels;
  /**
   * The clouds' two-dimensional layer (plan R11, R11.T7.b; `CLOUD_LAYER_MATERIAL`), made with the
   * rest; `null` where its compiler refused it, when the view draws no clouds.
   */
  readonly cloudLayer: MaterialHandle | null;
  /**
   * The cloud volume's march, resolve and composite (plan R11, R11.T7.c; `cloudVolumePass.ts`), made
   * with the rest; `null` where its compiler refused any, when the high setting draws the layer.
   */
  readonly cloudVolume: {
    readonly march: MaterialHandle;
    readonly resolve: MaterialHandle;
    readonly composite: MaterialHandle;
  } | null;
  /**
   * The `ocean` pass's far sea (plan R11, R11.T8.b), made with the rest; `null` where its compiler
   * refused it, when the frame draws no sea.
   */
  readonly ocean: MaterialHandle | null;
  /**
   * The tables a sea without its air drawn is lit through (`createAirlessTables`), made at the first
   * frame that draws one.
   */
  airless: SurfaceLightingTables | null;
  /**
   * The near sea's grid and the copy of the scene's depth it reads (plan R11, R11.T8.c), made with
   * the rest; `null` where its compiler refused either, when every sea is drawn far.
   */
  readonly oceanNear: MaterialHandle | null;
  readonly oceanDepthCopy: MaterialHandle | null;
  /**
   * The near sea's copies of the scene (R11.T8.c), under `ocean`: its depth's bits, and with
   * refraction its colour; made at the first near frame, let go at the first with none.
   */
  oceanCopies: OceanCopies | null;
  /** The near sea's grid at the frame's size and the setting's cell, and its key. */
  oceanGrid: { readonly mesh: MeshHandle; readonly key: string } | null;
  /** The one-texel stand-in for the scene's colour where the near sea refracts nothing. */
  oceanNoColour: TextureHandle | null;
  /**
   * The targets the bodies' composites draw into in turn, the first reading the scene target and
   * each after it the one before (R08.T10.b): made at the first frame that draws an atmosphere.
   */
  readonly airTargets: RenderTarget[];
  /**
   * The view's visibility target (plan R14, R14.T2.c), drawn with the scene target's depth: made at
   * the first frame whose terrain shades by it, and let go at a frame whose setting shades forward.
   */
  visibility: VisibilityBuffer | null;
  /**
   * The view's figure IDs (plan R07, the owner's Mac; `GeometrySettings.figureShading` `screen`),
   * an `r32uint` drawn with the scene target's depth: made at the first frame whose figures are
   * shaded once a pixel, released at the first drawn forward.
   */
  figureIds: VisibilityBuffer | null;
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

/**
 * The frame's lit bodies with their atmospheres' tables, through which each one's surface is lit
 * (plan R08, R08.T9.b's `surfaceLighting.wgsl`; R08.T10.b), each source's transmittance to it
 * taken the setting's way (`sourceTransmittance`, R08.T9.b's lever), and their disc tables where
 * the tables hold one (R08.T16.b), by which a body is drawn beyond the 10⁹ m boundary.
 */
function withAtmospheres(
  bodies: ReadonlyArray<LitBodyInput>,
  atmospheres: ReadonlyArray<FrameAtmosphere>,
  sourceTransmittance: SourceTransmittance,
): ReadonlyArray<LitBodyInput> {
  if (atmospheres.length === 0) {
    return bodies;
  }
  const tables = new Map(atmospheres.map((each) => [each.id, each.tables]));
  return bodies.map((body) => {
    const own = tables.get(body.id);
    if (own === undefined) {
      return body;
    }
    const air = discAirOf(own);
    return {
      ...body,
      atmosphere: surfaceLightingTablesOf(own, sourceTransmittance),
      ...(air === null ? {} : { air }),
    };
  });
}

/** A body's clouds as a frame draws them (plan R11, R11.T7.b): its deck and its GPU state. */
export interface FrameClouds {
  readonly condensate: CloudCondensate;
  readonly gpu: CloudBodyGpu;
  /**
   * Its condensate's correction texture where the frame draws its clouds as the volume (plan R11,
   * R11.T7.c; `cloudVolumePass.ts`), absent where it draws the layer.
   */
  readonly volume?: TextureHandle;
}

/** Of bodies' atmospheres, the one whose camera stands nearest its centre; `null` with none. */
function nearestOf<T extends { readonly air: FrameAtmosphere }>(
  bodies: ReadonlyArray<T>,
): T | null {
  let nearest: T | null = null;
  for (const body of bodies) {
    if (nearest === null || norm(body.air.camera.positionM) < norm(nearest.air.camera.positionM)) {
      nearest = body;
    }
  }
  return nearest;
}

/** No season: a body with no star is drawn at the equinox. */
const NO_SEASON = { subsolarLatitudeRad: 0 } as const;

/** A body's centre in its own frame, from which its brightest star's direction is taken. */
const BODY_CENTRE = { x: 0, y: 0, z: 0 } as const;

/** The universe clock's zero, for a frame that carries no time. */
const CLOCK_ZERO = { seconds: 0, nanos: 0 } as const;

/**
 * The frame's lit bodies with their clouds (plan R11, R11.T7.b; `LitBodyInput.clouds`), whose light
 * their terrain's cloudy forms take on the ground: each body `clouds` holds, where the setting's
 * `cloudShadows` is on; with it off, `bodies` as they are, so that their terrain writes neither of
 * R11's hooks.
 */
export function withCloudsOnGround(
  bodies: ReadonlyArray<LitBodyInput>,
  clouds: ReadonlyMap<BodyIdHex, FrameClouds>,
  cloudShadows: boolean,
): ReadonlyArray<LitBodyInput> {
  if (!cloudShadows || clouds.size === 0) {
    return bodies;
  }
  return bodies.map((body) => {
    const own = clouds.get(body.id);
    return own === undefined ? body : { ...body, clouds: own.gpu };
  });
}

/**
 * A body's disc air from its tables (R08.T16.b): their disc table, their bands' ln s and the body's
 * gravity, which the twin reads the bands by; `null` before the table lands.
 */
export function discAirOf(tables: AtmosphereTables): DiscAir | null {
  const disc = tables.discReflectance;
  if (disc === null) {
    return null;
  }
  return {
    table: disc.table,
    texture: disc.texture,
    lnBands: tables.plan.bands.map((band) => log(band.gravityRatio)),
    gravity: tables.body?.gravity ?? null,
  };
}

/**
 * `plan` with the sources of each body it draws larger than a point and holds no record of, as
 * `planLitBodies` builds a recorded body's (`bodyLightSources`).
 */
function withRecordlessSources(
  plan: BodyFramePlan,
  bodies: ReadonlyArray<LitBodyInput>,
  hosts: PhotorealFrame["lights"],
  options: { readonly annuli: number; readonly planetshine: number },
): BodyFramePlan {
  const recordless = bodies.filter((body) => {
    const regime = plan.regimes.get(body.id);
    return regime !== undefined && regime !== "point" && !plan.sources.has(body.id);
  });
  if (recordless.length === 0) {
    return plan;
  }
  const sources = new Map(plan.sources);
  for (const body of recordless) {
    sources.set(body.id, bodyLightSources(body, bodies, hosts, options));
  }
  return { ...plan, sources };
}

/**
 * The radius of a body's air's shell, m: the bounding sphere of its medium's top, a + H ÷ s_e
 * (`decision-r08-t10b-leans.md` §3), the larger semi-axis of the march's top shell, (a + H ÷ s_e,
 * c + H ÷ s_p) (`hillaire.ts`' `viewExactOf`), s_e and s_p its gravity's ratios at the equator and
 * the pole.
 *
 * @throws RangeError as `oblate.ts`'s `gravityRatio`, which the tables' build met first.
 */
export function airShellRadiusM(tables: AtmosphereTables): number {
  const figure = tablesFigureOf(tables);
  const gravity = tables.body?.gravity ?? null;
  const topM = tables.medium.topHeightM;
  const atEquator = gravity === null ? 1 : gravityRatio(gravity, 0);
  const atPole = gravity === null ? 1 : gravityRatio(gravity, Math.PI / 2);
  return Math.max(figure.equatorialRadiusM + topM / atEquator, figure.polarRadiusM + topM / atPole);
}

/** Whether a throw is a lost device's, which the graphics fault covers and no cache holds. */
export function lostDevice(error: unknown): boolean {
  return error instanceof EngineUnavailable || error instanceof AtmosphereRetired;
}

/** Whether a body's atmosphere camera lies inside its air: under its medium's top over the datum. */
function insideAir(air: FrameAtmosphere): boolean {
  const { heightM } = geodeticOf(air.camera.positionM, tablesFigureOf(air.tables));
  return heightM < air.tables.medium.topHeightM;
}

/**
 * The cloud volume's three materials where all were made (plan R11, R11.T7.c), else `null`, those
 * made released: the high setting then draws the layer.
 */
function volumeMaterialsOf(
  engine: Pick<RenderEngine, "releaseMaterial">,
  made: ReadonlyArray<MaterialHandle | null>,
): Resources["cloudVolume"] {
  const [march, resolve, composite] = made;
  if (
    march === undefined ||
    march === null ||
    resolve === undefined ||
    resolve === null ||
    composite === undefined ||
    composite === null
  ) {
    for (const material of made) {
      if (material !== null) {
        engine.releaseMaterial(material);
      }
    }
    return null;
  }
  return { march, resolve, composite };
}

/** `rect` held to a target of `size`, its pixels from the top left. */
function rectWithin(rect: TexelRect, size: ViewSize): TexelRect {
  const x = Math.min(rect.x, size.widthPx);
  const y = Math.min(rect.y, size.heightPx);
  return {
    x,
    y,
    width: Math.min(rect.width, size.widthPx - x),
    height: Math.min(rect.height, size.heightPx - y),
  };
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
  /**
   * Each body's atmosphere on this view, over the cache's tables (R08.T10.b), made at the first
   * frame that draws it and let go at the first that does not, with the setting it was made at and
   * the levers it was made with over that setting's (R08.T8.d).
   */
  readonly #atmospheres = new Map<
    BodyIdHex,
    {
      readonly atmosphere: HillaireAtmosphere;
      readonly setting: QualitySetting;
      readonly levers: AtmosphereLeversOverride | undefined;
    }
  >();
  /** The colour the latest frame's later passes read: its last composite's, or the scene's. */
  #drawnColour: TextureHandle | null = null;
  /** The seas' glare sources, held across frames in their bodies' axes (plan R11, R11.T8.b). */
  readonly #oceanGlare = new OceanGlare();
  /**
   * Each drawn sea's eclipse rows (plan R11, R11.T8.b; `packOceanEclipse`), under `ocean`, made at
   * its first frame and let go at the first that draws it not; forgotten with a lost device.
   */
  readonly #oceanEclipses = new Map<BodyIdHex, BufferHandle>();
  /** Each near sea's waves' rows (R11.T8.c; `packOceanWaves`), under `ocean`, as the eclipses'. */
  readonly #oceanWaves = new Map<BodyIdHex, BufferHandle>();
  /** The latest frame's glare sources: its own and its seas' (R11.T8.b). */
  #glareSources: ReadonlyArray<GlareSource> = [];
  /**
   * The clouds of the bodies this view draws them on (plan R11, R11.T7.b): the noise tile, made over
   * frames, and each body's rows and coverage; made again after a device loss.
   */
  #clouds: CloudLayers;
  /** The cloud volume of the body it draws as one (plan R11, R11.T7.c); made again after a loss. */
  #volumes: CloudVolumes;
  /**
   * The view's depth pyramid (plan R14, R14.T3.d), made at the first frame whose geometry asks the
   * two-pass occlusion and let go at the first that does not; and whether a cut came since its last
   * frame ({@link PhotorealRenderer.dropHistograms}), which drops the visibility it carries.
   */
  #pyramid: DepthPyramid | null = null;
  #cut = false;
  /**
   * The `disc cells` kernel's forms of each way: its emissive form (R08.T18.a), made at the first
   * frame with a record that carries its ground's emission, or at
   * {@link PhotorealRenderer.prepareEmissive}, and its air form (R08.T16.b), at the first with a
   * record that carries its air, or at {@link PhotorealRenderer.prepareAir}, and its form for both
   * (R08.T18.b), at the first with a record that carries both; and the packed plain kernel, at the
   * first frame whose geometry packs the cells (`discCells`); each on the generation it was asked
   * on, `kernel` `null` until made. The per-cell plain kernel is `Resources.discCells`.
   */
  #formCells: Record<CellWay, Record<CellForm, FormCells | null>> = noFormCells();
  /** Each resolve's draw with the view's IDs bound, by the draw (plan R14, R14.T2.d). */
  readonly #resolveDraws = new WeakMap<
    DrawItem,
    { readonly ids: TextureHandle; readonly draw: DrawItem }
  >();
  readonly #unsubscribe: () => void;
  /** The clock the clouds' noise tile is made by within its budget a frame, ms. */
  readonly #nowMs: () => number;

  /**
   * @param options - The clock the clouds' noise tile's making reads, ms (`performance.now` if
   *   absent; a test's own).
   */
  constructor(
    engine: RenderEngine,
    viewName: string,
    options: { readonly nowMs?: () => number } = {},
  ) {
    this.#engine = engine;
    this.#name = viewName;
    this.#nowMs = options.nowMs ?? ((): number => performance.now());
    // The bodies' renderer makes its own handles again after a loss, so it lives as long as this.
    this.#bodies = new LitBodyRenderer(engine, SKY_SPRITE_HDR_MATERIAL);
    this.#clouds = new CloudLayers(engine, viewName);
    this.#volumes = new CloudVolumes(engine, viewName);
    this.#unsubscribe = engine.onRestored(() => {
      // The handles died with the device; a making in flight is abandoned, and the next frame
      // makes them again.
      this.#generation += 1;
      this.#resources?.histograms.dispose();
      this.#resources = null;
      this.#drawnColour = null;
      // Its buffer died with the device; the next frame that occludes makes it again.
      this.#pyramid?.dispose();
      this.#pyramid = null;
      // Their shared kernels died with the device; each is made again at its next frame.
      this.#releaseAtmospheres();
      // The clouds' handles died with it too; their noise tile is made again over frames.
      this.#clouds.dispose();
      this.#clouds = new CloudLayers(engine, viewName);
      this.#volumes.dispose();
      this.#volumes = new CloudVolumes(engine, viewName);
      // The seas' eclipse and wave rows died with it too.
      this.#oceanEclipses.clear();
      this.#oceanWaves.clear();
      this.#formCells = noFormCells();
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

  /**
   * Starts making the resources for a view of `size`, once; resolves when they are made.
   *
   * @param levers - A frame profile's atmosphere levers over the settings' (R08.T8.d), whose tile
   *   cull's kernels are made up front beside the settings' own; none where absent.
   */
  prepare(
    size: ViewSize,
    setting: QualitySetting,
    role: ViewRole,
    camera: ProjectionCamera,
    levers?: AtmosphereLeversOverride,
  ): Promise<void> {
    if (this.#making === null) {
      const generation = this.#generation;
      this.#status = "making";
      this.#making = this.#make(size, setting, role, camera, generation, levers).catch(
        (error: unknown) => {
          if (generation === this.#generation && !this.#disposed) {
            this.#status = "failed";
            console.error(`the photorealistic view ${this.#name} could not be made:`, error);
          }
        },
      );
    }
    return this.#making;
  }

  async #make(
    size: ViewSize,
    setting: QualitySetting,
    role: ViewRole,
    camera: ProjectionCamera,
    generation: number,
    levers: AtmosphereLeversOverride | undefined,
  ): Promise<void> {
    const engine = this.#engine;
    const triangle = fullScreenTriangle(engine, `${this.#name} tonemap triangle`);
    const quad = engine.createMesh({ ...WIREFRAME_MESHES.quad, name: `${this.#name} sprite quad` });
    const radPx = radPerPx(camera, { widthPx: size.widthPx, heightPx: size.heightPx });
    const making = BloomChain.create(
      engine,
      this.#name,
      size,
      bloomKernel(SETTINGS[setting].photoreal.bloom, role, radPx, DEFAULT_EYE_OBSERVER),
    );
    let tonemap: MaterialHandle;
    let overlayDepth: MaterialHandle;
    let sprite: MaterialHandle;
    let kernel: ComputeHandle;
    let discCells: ComputeHandle;
    let pyramidKernel: ComputeHandle | null;
    let atmosphere: AtmosphereFrameKernels;
    let cloudLayer: MaterialHandle | null;
    let cloudVolume: ReadonlyArray<MaterialHandle | null>;
    let ocean: MaterialHandle | null;
    let oceanNear: MaterialHandle | null;
    let oceanDepthCopy: MaterialHandle | null;
    try {
      [
        tonemap,
        overlayDepth,
        sprite,
        kernel,
        discCells,
        pyramidKernel,
        atmosphere,
        cloudLayer,
        cloudVolume,
        ocean,
        oceanNear,
        oceanDepthCopy,
      ] = await Promise.all([
        engine.createMaterialAsync(TONEMAP_MATERIAL, ["canvas-in-pass"], [triangle]),
        engine.createMaterialAsync(OVERLAY_SCENE_DEPTH_MATERIAL, ["canvas"], [triangle]),
        engine.createMaterialAsync(SKY_SPRITE_HDR_MATERIAL, ["rgba16float"], [quad]),
        engine.createComputeAsync(HISTOGRAM_KERNEL),
        engine.createComputeAsync(BODY_DISC_CELLS_KERNEL),
        // A refused pyramid leaves the view its one raster: the occlusion is a lever, not the frame.
        engine.createComputeAsync(DEPTH_PYRAMID_KERNEL).catch((error: unknown) => {
          console.warn(`the photorealistic view ${this.#name} has no depth pyramid:`, error);
          return null;
        }),
        // Every world with air draws by the thick read's kernels once its correction is routed
        // (R08.T14.f), so they are made ahead of the first frame of one; and the kernel of what lies
        // behind the sky at the march's resolution, which the settings' levers take (the owner's
        // priority, 2026-11-01), with the tile cull's where a setting takes it (R08.T8.d): either
        // setting's, since the frame-rate floor draws high's air at low's. The packed limb's are made
        // in the background at the first frame that takes them, each body's form alone, its frames
        // marching per texel, the same bits, until then. The thick read's marches from the
        // sources' table are made too where a setting takes it (perf-thick-march).
        createAtmosphereFrameKernels(engine, {
          thick: QUALITY_SETTINGS.some(
            (each) =>
              ({ ...SETTINGS[each].atmosphereLevers, ...levers }).marchSourceTransmittance ===
              "table",
          )
            ? "with the table"
            : CORRECTION_ROUTED,
          behind: true,
          tiles: QUALITY_SETTINGS.some(
            (each) => ({ ...SETTINGS[each].atmosphereLevers, ...levers }).marchCover === "tiles",
          ),
        }),
        // A refused cloud layer leaves the view without clouds, as a refused pyramid leaves it
        // one raster (plan R11, R11.T7.b).
        engine
          .createMaterialAsync(CLOUD_LAYER_MATERIAL, [HDR_COLOUR_FORMAT], [triangle])
          .catch((error: unknown) => {
            if (!lostDevice(error)) {
              console.warn(`the photorealistic view ${this.#name} draws no clouds:`, error);
            }
            return null;
          }),
        // A refused volume leaves the high setting the layer (plan R11, R11.T7.c).
        Promise.all(
          [CLOUD_VOLUME_MATERIAL, CLOUD_RESOLVE_MATERIAL, CLOUD_COMPOSITE_MATERIAL].map((spec) =>
            engine
              .createMaterialAsync(spec, [HDR_COLOUR_FORMAT], [triangle])
              .catch((error: unknown) => {
                if (!lostDevice(error)) {
                  console.warn(
                    `the photorealistic view ${this.#name} draws its clouds as the layer:`,
                    error,
                  );
                }
                return null;
              }),
          ),
        ),
        // A refused sea leaves the view without one, its frame drawn on (plan R11, R11.T8.b).
        engine
          .createMaterialAsync(OCEAN_FAR_PASS_MATERIAL, ["rgba16float"], [triangle])
          .catch((error: unknown) => {
            console.warn(`the photorealistic view ${this.#name} draws no sea:`, error);
            return null;
          }),
        // A refused near sea leaves every sea drawn far (R11.T8.c).
        engine
          .createMaterialAsync(OCEAN_NEAR_PASS_MATERIAL, ["rgba16float"], [triangle])
          .catch((error: unknown) => {
            console.warn(`the photorealistic view ${this.#name} draws no near sea:`, error);
            return null;
          }),
        engine
          .createMaterialAsync(OCEAN_DEPTH_COPY_MATERIAL, ["r32uint"], [triangle])
          .catch((error: unknown) => {
            console.warn(
              `the photorealistic view ${this.#name} copies no depth for its sea:`,
              error,
            );
            return null;
          }),
      ]);
    } catch (error: unknown) {
      // A chain made before the refusal is released with it, and the meshes made for it.
      void making
        .then((chain) => chain.dispose())
        .catch((cause: unknown) => {
          console.error(`the photorealistic view ${this.#name}'s bloom chain failed too:`, cause);
        });
      engine.releaseMesh(triangle);
      engine.releaseMesh(quad);
      throw error;
    }
    const bloom = await making;
    // A restore or a dispose while this was compiling abandons it: its handles are the lost
    // device's, or nobody's, and are released (plan R11, R11.T10.b: a still's renderer is made,
    // and may be disposed, at each still).
    if (this.#disposed || generation !== this.#generation) {
      bloom.dispose();
      for (const material of [
        tonemap,
        overlayDepth,
        sprite,
        atmosphere.composite,
        cloudLayer,
        ...cloudVolume,
        ocean,
        oceanNear,
        oceanDepthCopy,
      ]) {
        if (material !== null) {
          engine.releaseMaterial(material);
        }
      }
      for (const mesh of [triangle, quad, atmosphere.triangle]) {
        engine.releaseMesh(mesh);
      }
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
      blueNoiseTexels(),
    );
    this.#resources = {
      target: createSceneTarget(engine, this.#name, size),
      bloom,
      sprite,
      quad,
      tonemap,
      overlayDepth,
      triangle,
      blueNoise,
      histograms: new HistogramReader(engine, kernel, this.#name, (histogram) => {
        if (generation === this.#generation) {
          this.#histogram = histogram;
        }
      }),
      discCells,
      pyramidKernel,
      atmosphere,
      cloudLayer,
      cloudVolume: volumeMaterialsOf(engine, cloudVolume),
      ocean,
      airless: null,
      oceanNear,
      oceanDepthCopy,
      oceanCopies: null,
      oceanGrid: null,
      oceanNoColour: null,
      airTargets: [],
      visibility: null,
      figureIds: null,
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
      for (const target of resources.airTargets) {
        target.resize(size);
      }
      resources.visibility?.resize(size);
      resources.figureIds?.resize(size);
      resources.oceanCopies?.depthBits.resize(size);
      resources.oceanCopies?.colour?.resize(size);
      resources.size = size;
    }
    const radPx = radPerPx(frame.camera, frame.viewport);
    const key = `${frame.role} ${frame.setting} ${String(radPx)}`;
    if (key !== resources.kernelKey) {
      resources.bloom.setKernel(
        bloomKernel(
          SETTINGS[frame.setting].photoreal.bloom,
          frame.role,
          radPx,
          DEFAULT_EYE_OBSERVER,
        ),
      );
      resources.kernelKey = key;
    }
  }

  /**
   * `draw`, a resolve's, with the view's IDs `ids` bound (`withVisibilityIds`), kept by the draw so
   * that a draw kept from frame to frame is too, until the IDs' texture changes.
   */
  #withIds(draw: DrawItem, ids: TextureHandle): DrawItem {
    const held = this.#resolveDraws.get(draw);
    if (held?.ids === ids) {
      return held.draw;
    }
    const bound = withVisibilityIds(draw, ids);
    this.#resolveDraws.set(draw, { ids, draw: bound });
    return bound;
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
   * @param view - The view it is presented on; `null` for a still's sample (plan R11, R11.T10.b),
   *   which draws the scene target alone, as a frame held back (`present: false`) does.
   * @returns The frame's lit-body plan (its regimes are the next frame's `previousRegimes`), or
   *   `null` while the resources are being made, when nothing was drawn.
   */
  render(view: RenderView | null, frame: PhotorealFrame): BodyFramePlan | null {
    const resources = this.#resources;
    if (resources === null) {
      if (this.#status !== "failed") {
        void this.prepare(
          frame.viewport,
          frame.setting,
          frame.role,
          frame.camera,
          frame.atmosphereLevers,
        );
      }
      return null;
    }
    this.#follow(resources, frame);
    const { camera, viewport } = frame;
    const geometry = frame.geometry ?? SETTINGS[frame.setting].geometry;
    if (this.#bodies.geometry !== geometry) {
      this.#bodies.setGeometry(geometry);
    }
    // The setting's forms of the histogram, the eclipse term and planetshine (Design note 18).
    const settings = SETTINGS[frame.setting].photoreal;
    const projection =
      frame.projection ??
      perspectiveReversedInfinite(
        camera.fovXRad,
        viewport.widthPx / viewport.heightPx,
        NEAR_PLANE_M,
      );
    const viewRotation = viewRotation4(camera.orientation);
    // Each body whose air is drawn lights its surface through it, and is drawn with depth, so that
    // its composite lays the air over its ground (R08.T10.b).
    const atmospheres = frame.atmospheres ?? [];
    // Each body whose clouds are drawn (plan R11, R11.T7.b): its rows and coverage written before
    // its terrain's draws, which take their light on the ground where the setting's `cloudShadows`
    // is on, and its layer drawn after its composite.
    const clouds = this.#cloudsOf(resources, frame, atmospheres);
    const bodies = withCloudsOnGround(
      withAtmospheres(
        frame.bodies,
        atmospheres,
        SETTINGS[frame.setting].atmosphereLevers.sourceTransmittance,
      ),
      clouds,
      SETTINGS[frame.setting].cloudShadows,
    );
    const shellRadiiM = new Map(atmospheres.map((each) => [each.id, airShellRadiusM(each.tables)]));
    // A body given its disc table is drawn by it beyond the boundary once the air forms are
    // prepared, their making begun at the first such body (R08.T16.b).
    const drawsAir = bodies.some((body) => body.air !== undefined) && this.#bodies.airReady();
    const planned = planLitBodies(
      bodies,
      frame.lights,
      {
        camera,
        viewport,
        presentedPx: frame.presentedPx,
        ...(frame.selectionPx === undefined ? {} : { selectionPx: frame.selectionPx }),
        exposureScale: frame.exposureScale,
        annuli: settings.discAnnuli,
        planetshine: settings.planetshineSources,
        depthWriters: frame.depthWriters,
        depthBodyShellRadiiM: shellRadiiM,
        setting: frame.setting,
        drawsAir,
        ...(frame.previousTabled === undefined ? {} : { previousTabled: frame.previousTabled }),
        ...(frame.previousHandedOver === undefined
          ? {}
          : { previousHandedOver: frame.previousHandedOver }),
        ...(frame.floorLowest === true ? { limbSamples: settings.floorLimbSamples } : {}),
      },
      frame.previousRegimes,
    );
    // A body drawn larger than a point with no record, the camera's local body seen from inside
    // its figure's sphere above all, takes its sources as the plan would build them, so that its
    // atmosphere's lights and sky are its surface's (R08.T10.b).
    const plan = withRecordlessSources(planned, bodies, frame.lights, {
      annuli: settings.discAnnuli,
      planetshine: settings.planetshineSources,
    });
    const stars = this.#starDraw(resources, frame.starSprites);
    // R06's sky at infinity, timed as its own pass, then the painter's sequence over it.
    resources.target.render({
      label: SKY_PASS_LABEL,
      viewRotation,
      projection,
      draws: [...frame.sky, ...(stars === null ? [] : [stars])],
      postProcesses: [],
    });
    // The small discs' cells and the larger discs' limbs' and face bands', all at once, a dispatch
    // for each set of textures, before any draw reads their sums (R07.T8.d–g); in the frame the
    // `discs` pass draws in, the scene target's. By the way the geometry asks (`discCells`): the
    // packed plain kernel once made and the per-cell one until then, whose sums are its bits.
    const way = geometry.discCells;
    this.#bodies.dispatchCells(
      plan,
      (way === "packed" ? this.#cellsFor(plan, way, "plain") : null) ?? resources.discCells,
      {
        viewRotation,
        projection,
        size: { widthPx: viewport.widthPx, heightPx: viewport.heightPx },
      },
      this.#cellsFor(plan, way, "emissive"),
      this.#cellsFor(plan, way, "air"),
      this.#cellsFor(plan, way, "emissiveAir"),
    );
    // The mesh bodies' figures, with depth, before the sequence, then the terrain of the bodies
    // handed over to it over them; no pass where there are none. On the high setting the terrain's
    // draws submit the view's shadow cascades first (R10.T9.b's `terrain shadows`), whose times
    // ride this pass's resolve: nothing is rendered between them and it. A frame whose terrain
    // draws nothing submits none, so the pass always follows them.
    const occlusion = this.#occlusionOf(resources, frame, geometry);
    // The figures shaded once a pixel where the geometry asks it and their pipelines are prepared
    // (plan R07, the owner's Mac; `figureShading` `screen`): their raster into the view's figure IDs
    // with the scene's depth where the forward draws would stand among the draws, then their
    // resolve in the next pass, first; forward elsewhere, the same bits.
    const screen =
      geometry.figureShading === "screen" &&
      plan.meshes.some(({ mesh }) => mesh.patches.length > 0) &&
      this.#bodies.screenReady(plan);
    if (!screen && resources.figureIds !== null) {
      resources.figureIds.dispose();
      resources.figureIds = null;
    }
    const figureIds = screen
      ? (resources.figureIds ??= new VisibilityBuffer(
          this.#engine,
          `${this.#name} figures`,
          resources.target,
          resources.size,
        ))
      : null;
    // Made before the terrain's, as the forward figures always are: the figures' cull among them.
    const figureDraws = this.#bodies.meshDraws(plan, figureIds === null ? "forward" : "ids");
    const figures = figureIds === null ? figureDraws : [];
    const terrainDraws = frame.terrain?.draws(plan, bodies, occlusion ?? undefined) ?? [];
    // The terrain by the visibility buffer where every body it draws gives it (plan R14, R14.T2.c
    // and T2.d), with the two-pass occlusion or without (R14.T3.d).
    const byIds = frame.terrain?.visibility() ?? null;
    const opaque = frame.terrain?.opaqueDraws() ?? [];
    // Opaque first (plan R14, R14.T2.e): the terrain that never discards, then the figures, which
    // may, then the rest of the terrain; by the visibility buffer, the raster is that opaque pass.
    const after = [...figures, ...terrainDraws.filter((draw) => !opaque.includes(draw))];
    if (byIds === null) {
      if (resources.visibility !== null && geometry.shading === "forward") {
        resources.visibility.dispose();
        resources.visibility = null;
      }
      if (figureIds === null) {
        const meshes = [...opaque, ...after];
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
      } else {
        if (opaque.length > 0) {
          resources.target.render({
            label: PHOTOREAL_PASS_LABELS.bodies,
            viewRotation,
            projection,
            draws: opaque,
            postProcesses: [],
            colourLoad: "load",
          });
        }
        this.#figureRaster(figureIds, figureDraws, viewRotation, projection);
        resources.target.render({
          label: PHOTOREAL_PASS_LABELS.figureResolve,
          viewRotation,
          projection,
          draws: [...this.#bodies.figureResolveDraws(plan, figureIds.ids), ...after],
          postProcesses: [],
          colourLoad: "load",
        });
      }
      // With the two-pass occlusion, the terrain's clusters that raster's pyramid does not refuse,
      // over it.
      const late = this.#pyramidThenLateDraws(resources, frame, occlusion);
      if (late.length > 0) {
        resources.target.render({
          label: PHOTOREAL_PASS_LABELS.bodiesLate,
          viewRotation,
          projection,
          draws: late,
          postProcesses: [],
          colourLoad: "load",
        });
      }
    } else {
      const visibility = (resources.visibility ??= new VisibilityBuffer(
        this.#engine,
        this.#name,
        resources.target,
        resources.size,
      ));
      visibility.target.render({
        label: VISIBILITY_RASTER_LABEL,
        viewRotation,
        projection,
        draws: byIds.raster,
        postProcesses: [],
      });
      // With the two-pass occlusion, the late pass's opaque clusters that the raster's pyramid does
      // not refuse, into the same IDs, loading them and the depth; the one resolve shades both.
      const late = this.#pyramidThenLateDraws(resources, frame, occlusion);
      if (late.length > 0) {
        visibility.target.render({
          label: VISIBILITY_LATE_RASTER_LABEL,
          viewRotation,
          projection,
          draws: late,
          postProcesses: [],
          colourLoad: "load",
        });
      }
      if (figureIds !== null) {
        this.#figureRaster(figureIds, figureDraws, viewRotation, projection);
      }
      resources.target.render({
        label: VISIBILITY_RESOLVE_LABEL,
        viewRotation,
        projection,
        draws: [
          ...byIds.resolve.map((draw) => this.#withIds(draw, visibility.ids)),
          ...(figureIds === null ? figures : this.#bodies.figureResolveDraws(plan, figureIds.ids)),
          ...byIds.forward,
        ],
        postProcesses: [],
        colourLoad: "load",
      });
    }
    // Each body's sea over its opaque ground, with its depth, before the painter's sequence and the
    // air composited over it (plan R11, R11.T8.b), its glint past the threshold a glare source.
    const oceanGlares = this.#drawOceans(resources, frame, plan, bodies, viewRotation, projection);
    const glareSources =
      oceanGlares.length === 0 ? frame.glareSources : [...frame.glareSources, ...oceanGlares];
    this.#glareSources = glareSources;
    resources.target.render({
      label: PHOTOREAL_PASS_LABELS.discs,
      viewRotation,
      projection,
      draws: this.#bodies.draws(plan, frame.hostDraws),
      postProcesses: [],
      colourLoad: "load",
    });
    // Each body's air over everything drawn before it, farthest first (R08.T10.b), into the air
    // targets in turn; the passes after read the last.
    const hdrColour = this.#drawAtmospheres(
      resources,
      frame,
      plan,
      { bodies, shellRadiiM, clouds },
      {
        viewRotation,
        projection,
      },
    );
    this.#drawnColour = hdrColour;
    // The glare sources are written before the histogram is dispatched, which weighs their veil
    // as the tone-mapping pass draws it.
    if (glareSources.length > resources.glareCapacity) {
      this.#engine.releaseBuffer(resources.glare);
      resources.glareCapacity = glareSources.length;
      resources.glare = this.#glareBuffer(resources.glareCapacity);
    }
    const terms = glareSpreadTerms(frame.role, DEFAULT_EYE_OBSERVER);
    this.#engine.writeBuffer(
      resources.glare,
      0,
      packGlareSources(glareSources, frame.exposureScale, terms),
    );
    if (frame.meter !== null) {
      resources.histograms.measure({
        hdrColour,
        size: resources.size,
        mode: frame.meter,
        stride: settings.histogramStride,
        preExposure: frame.exposureScale,
        glare: {
          sources: resources.glare,
          count: glareSources.length,
          viewRotation,
          projection,
          terms,
        },
      });
    }
    if (frame.present === false || view === null) {
      // Held back: no frame of a cut is shown at the exposure from before it. A still's sample is
      // presented by its own passes.
      return plan;
    }
    // The pre-exposure is this frame's own exposure, so the pass's exposure over it is 1.
    const threshold = bloomThreshold(frame.exposureScale, frame.exposureScale);
    resources.bloom.run(hdrColour, threshold);
    view.render({
      label: TONEMAP_PASS,
      viewRotation,
      projection,
      draws: [
        tonemapDraw({
          mesh: resources.triangle,
          material: resources.tonemap,
          hdrColour,
          bloomUp: resources.bloom.levelOne,
          blueNoise: resources.blueNoise,
          glareSources: resources.glare,
          uniforms: {
            exposure: 1,
            threshold,
            levelZeroWeight: levelWeight(resources.bloom.kernel, 0),
            levelOneWeight: resources.bloom.levelOneWeight,
            sourceCount: glareSources.length,
            terms,
            dither: true,
            balance: frame.whiteBalance?.rows ?? null,
          },
        }),
      ],
      postProcesses: [],
      encoding: "in-pass",
    });
    if (frame.overlay !== null) {
      // Each handed-over body's datum gives way where its reference surface did (R10.T16.f), and
      // the terrain's depth goes first, so that nearer ground hides the marks drawn after it
      // (R10.T16.c): the scene's, where its size is the canvas's and the terrain asks it (plan
      // R14, R14.T1.e), else the terrain's own; none where no mark left tests it (`hiddenMarks`).
      const surveyed = surveyedDatumsOf(plan, frame.bodies);
      const terrain = frame.terrain;
      const radiusOf = terrain?.innerRadiusM?.bind(terrain);
      const hiders =
        radiusOf !== undefined && geometry.hiddenMarks === "culled"
          ? overlayHidersOf(plan, radiusOf)
          : [];
      const { depthTested, ...overlay } = frame.overlay(surveyed, hiders);
      const sceneDepth = resources.target.depth;
      let depth: ReadonlyArray<DrawItem> = [];
      if (terrain !== undefined && depthTested !== false) {
        depth =
          terrain.sceneDepth() &&
          sceneDepth !== null &&
          sizesAgree(frame.viewport, frame.presentedPx)
            ? [overlaySceneDepthDraw(resources.triangle, resources.overlayDepth, sceneDepth)]
            : terrain.symbologyDepth();
      }
      view.render({
        ...overlay,
        draws: depth.length === 0 ? overlay.draws : [...depth, ...overlay.draws],
        colourLoad: "load",
      });
    }
    return plan;
  }

  /**
   * The latest frame's scene target, its colour and depth as the opaque passes left them, and the
   * colour its later passes read, its last composite's (R08.T10.b), for the smoke page's
   * read-backs; `null` before the first frame, or while the resources are being made again.
   */
  get frameColours(): { readonly scene: RenderTarget; readonly drawn: TextureHandle } | null {
    const resources = this.#resources;
    const drawn = this.#drawnColour;
    return resources === null || drawn === null ? null : { scene: resources.target, drawn };
  }

  /**
   * The latest frame's glare sources: its own ({@link PhotorealFrame.glareSources}) and its seas'
   * (plan R11, R11.T8.b), which a still's tone-mapping pass takes; none before the first frame.
   */
  get glareSources(): ReadonlyArray<GlareSource> {
    return this.#glareSources;
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

  /**
   * Drops the histogram arrived and those in flight, of images drawn before a cut, so that the
   * next one {@link PhotorealRenderer.takeHistogram} gives is of a frame drawn after it.
   */
  dropHistograms(): void {
    this.#histogram = undefined;
    this.#resources?.histograms.dropInFlight();
    // A cut: the next frame's two-pass occlusion keeps nothing of this one's visibility (R14.T3.d),
    // nor its clouds' volume of its history (R11.T7.c).
    this.#cut = true;
    this.#volumes.cut();
  }

  /**
   * The figures' raster, `draws` (`LitBodyRenderer.meshDraws`' `ids`), into the view's figure IDs
   * (plan R07, the owner's Mac; `figureShading` `screen`): its own pass, cleared to no figure, on the
   * scene's depth, which it tests and writes as the forward draws would at their place among the
   * draws.
   */
  #figureRaster(
    figureIds: VisibilityBuffer,
    draws: ReadonlyArray<DrawItem>,
    viewRotation: Float32Array,
    projection: Float32Array,
  ): void {
    figureIds.target.render({
      label: PHOTOREAL_PASS_LABELS.figureIds,
      viewRotation,
      projection,
      draws,
      postProcesses: [],
    });
  }

  /**
   * The frame's two-pass occlusion (plan R14, R14.T3.d), where its geometry asks it (`occlusion`
   * `hzb`), it has terrain and the pyramid's kernel was made: the view's pyramid, made at the first
   * such frame and begun at the
   * scene's size, and whether the last frame's visibility no longer holds, a cut among the reasons
   * (a dropped histogram, or a frame held back, `present: false`); else `null`, the pyramid let go.
   */
  #occlusionOf(
    resources: Resources,
    frame: PhotorealFrame,
    geometry: GeometrySettings,
  ): (FrameOcclusion & { readonly pyramid: DepthPyramid }) | null {
    const kernel = resources.pyramidKernel;
    if (geometry.occlusion !== "hzb" || frame.terrain === undefined || kernel === null) {
      this.#pyramid?.dispose();
      this.#pyramid = null;
      return null;
    }
    const pyramid = (this.#pyramid ??= new DepthPyramid(this.#engine, kernel, this.#name));
    const cut = this.#cut || frame.present === false;
    this.#cut = false;
    return { pyramid, fresh: pyramid.begin(resources.size, cut) };
  }

  /**
   * With the frame's two-pass occlusion (plan R14, R14.T3.d), the view's pyramid built from the
   * scene's depth as its first raster left it, then the terrain's late draws
   * (`FrameTerrain.lateDraws`): by the visibility buffer, the late pass's ID raster; none without
   * the occlusion.
   */
  #pyramidThenLateDraws(
    resources: Resources,
    frame: PhotorealFrame,
    occlusion: (FrameOcclusion & { readonly pyramid: DepthPyramid }) | null,
  ): ReadonlyArray<DrawItem> {
    const depth = resources.target.depth;
    if (occlusion === null || depth === null || frame.terrain === undefined) {
      return [];
    }
    occlusion.pyramid.build(depth, pyramidProjectionOf(frame.camera.fovXRad, frame.viewport));
    return frame.terrain.lateDraws();
  }

  /**
   * Draws each body's sea the frame holds (plan R11, R11.T8.b; `oceanPass.ts`) into the scene target
   * as the `ocean` pass, one draw a sea, over the opaque passes and before the `discs` pass and the
   * composites: a body drawn as a mesh or handed over (`drawnAsMesh`) whose sources the plan holds,
   * lit through its air's tables where the frame draws them and the airless ones elsewhere; no pass
   * where none is drawn, or where the material was refused.
   *
   * @remarks
   * A throw from a sea's draw, other than a lost device's, is logged and the sea left out of the
   * frame, which is drawn on.
   *
   * @returns The seas' glare sources this frame, in the scene's camera-relative axes.
   */
  #drawOceans(
    resources: Resources,
    frame: PhotorealFrame,
    plan: BodyFramePlan,
    bodies: ReadonlyArray<LitBodyInput>,
    viewRotation: Float32Array,
    projection: Float32Array,
  ): GlareSource[] {
    const material = resources.ocean;
    const oceans = frame.oceans ?? [];
    const kept = new Set<BodyIdHex>();
    const draws: DrawItem[] = [];
    const glares: GlareSource[] = [];
    // The seas drawn near (R11.T8.c), their draws made once the scene's copies are.
    const nearSeas: NearSea[] = [];
    const nearMaterial = resources.oceanNear;
    const copyMaterial = resources.oceanDepthCopy;
    const cellPx = OCEAN_GRID_CELL_PX[frame.setting];
    if (material !== null && oceans.length > 0) {
      const airOf = new Map((frame.atmospheres ?? []).map((each) => [each.id, each]));
      const byId = new Map(bodies.map((body) => [body.id, body]));
      const thresholdPreExposed = bloomThreshold(frame.exposureScale, frame.exposureScale);
      const whitecaps = SETTINGS[frame.setting].whitecaps;
      const pixelAngleRad = radPerPx(frame.camera, frame.viewport);
      for (const ocean of oceans) {
        const regime = plan.regimes.get(ocean.id);
        const sources = plan.sources.get(ocean.id);
        const lit = byId.get(ocean.id);
        if (!drawnAsMesh(regime) || sources === undefined || lit === undefined) {
          continue;
        }
        try {
          const air = airOf.get(ocean.id) ?? null;
          const tables = lit.atmosphere ?? (resources.airless ??= this.#airlessTables());
          const lightSlots =
            air !== null && absorberCount(air.tables.medium) > 0
              ? air.tables.curves.length
              : Infinity;
          // The record's lights and their eclipses where the plan holds the body's record; else
          // the sky's sources, uneclipsed.
          const record = plan.discs.find((each) => each.body === ocean.id);
          const fromRecord =
            record === undefined || lit.rotation === undefined
              ? null
              : oceanSourcesOfRecord(
                  record,
                  lit.rotation,
                  lit.figure.equatorialRadiusM,
                  lightSlots,
                );
          const suns =
            fromRecord?.suns ??
            skySources(
              sources.stars,
              sources.planetshine,
              ocean.camera,
              air === null
                ? {
                    equatorialRadiusM: ocean.sea.equatorialRadiusM,
                    polarRadiusM: ocean.sea.polarRadiusM,
                  }
                : tablesFigureOf(air.tables),
              air?.tables.medium ?? { topHeightM: 0 },
              ocean.toBodyFixed,
            ).filter((sun) => sun.curveSlot < lightSlots);
          const eclipse: OceanEclipse | null = fromRecord?.eclipse ?? null;
          const view: OceanDrawView = {
            sea: ocean.sea,
            camera: ocean.camera,
            // The air's reference gravity, else the record's surface gravity, else standard.
            gravityMps2:
              air?.tables.body?.gravity.referenceGravityMS2 ??
              ocean.sea.surfaceGravityMps2 ??
              STANDARD_GRAVITY_MPS2,
            // Its figure stands whole on the datum as a mesh, or where its terrain takes none of
            // its ground: no mask, or no terrain drawn (a still's).
            figureOnDatum:
              regime === "mesh" || lit.surveyCoverage === undefined || frame.terrain === undefined,
            suns,
            exposureScale: frame.exposureScale,
            thresholdPreExposed,
            pixelAngleRad,
            whitecaps,
            eclipse,
          };
          const rows = this.#oceanEclipseRows(ocean.id);
          this.#engine.writeBuffer(rows, 0, packOceanEclipse(eclipse));
          // Under 20 km over the sea the near sea draws it, the far draw its basins alone; its state
          // formed first, so that a throw leaves the whole sea out of the frame, its glare too.
          const near =
            nearMaterial !== null &&
            copyMaterial !== null &&
            resources.target.depth !== null &&
            drawsNearSea(oceanDrawOptics(view), ocean.camera.positionM);
          const nearView: OceanNearView | null = near
            ? {
                ...view,
                time: ocean.time,
                components: SETTINGS[frame.setting].oceanComponents,
                cellRad: cellPx * pixelAngleRad,
                anchor: nearSeaAnchor(
                  oceanDrawOptics(view),
                  ocean.sea.map.texels,
                  ocean.camera.positionM,
                  ocean.camera.orientation,
                ),
                refraction: SETTINGS[frame.setting].oceanRefraction,
              }
            : null;
          const nearState = nearView === null ? null : oceanNearState(nearView);
          draws.push(
            oceanPassDraw(
              resources.triangle,
              material,
              ocean.sea.map.texture,
              tables,
              rows,
              oceanPassUniforms(view, near),
            ),
          );
          if (nearView !== null && nearState !== null) {
            nearSeas.push({ view: nearView, state: nearState, ocean, tables, eclipseRows: rows });
          }
          kept.add(ocean.id);
          const brightest = suns[0];
          const glare = this.#oceanGlare.sourceOf(
            view,
            ocean.toBodyFixed,
            air === null || brightest === undefined
              ? null
              : {
                  medium: air.tables.medium,
                  curves: air.tables.curves[brightest.curveSlot ?? 0] ?? NO_CURVES,
                },
          );
          if (glare !== null) {
            glares.push(glare);
          }
        } catch (error: unknown) {
          if (!lostDevice(error)) {
            console.error(`the sea of ${ocean.id} could not be drawn in ${this.#name}:`, error);
          }
        }
      }
    }
    this.#oceanGlare.keepOnly(kept);
    for (const [id, rows] of this.#oceanEclipses) {
      if (!kept.has(id)) {
        this.#engine.releaseBuffer(rows);
        this.#oceanEclipses.delete(id);
      }
    }
    const nearDraws = this.#nearSeaDraws(resources, nearSeas, cellPx, viewRotation, projection);
    draws.push(...nearDraws);
    if (draws.length > 0) {
      resources.target.render({
        label: OCEAN_PASS_LABEL,
        viewRotation,
        projection,
        draws,
        postProcesses: [],
        colourLoad: "load",
      });
    }
    return glares;
  }

  /**
   * The near seas' draws (plan R11, R11.T8.c; `oceanNearPass.ts`), after the pass's copies of the
   * scene, its depth's bits and with refraction its colour, drawn first under their own label; the
   * copies, the grid and the waves' rows let go where no sea is drawn near.
   */
  #nearSeaDraws(
    resources: Resources,
    nearSeas: ReadonlyArray<NearSea>,
    cellPx: number,
    viewRotation: Float32Array,
    projection: Float32Array,
  ): DrawItem[] {
    const nearMaterial = resources.oceanNear;
    const copyMaterial = resources.oceanDepthCopy;
    const depth = resources.target.depth;
    const kept = new Set<BodyIdHex>();
    const draws: DrawItem[] = [];
    if (nearSeas.length > 0 && nearMaterial !== null && copyMaterial !== null && depth !== null) {
      const refraction = nearSeas.some((each) => each.view.refraction);
      const copies = this.#oceanCopies(resources, refraction);
      copies.depthBits.render({
        label: OCEAN_COPY_PASS_LABEL,
        viewRotation,
        projection,
        draws: [oceanDepthCopyDraw(resources.triangle, copyMaterial, depth)],
        postProcesses: [],
      });
      copies.colour?.render({
        label: OCEAN_COPY_PASS_LABEL,
        viewRotation,
        projection,
        draws: [],
        postProcesses: [],
        colourFrom: resources.target.colour,
      });
      const grid = this.#oceanGridOf(resources, cellPx);
      const sceneColour = copies.colour?.colour ?? this.#oceanNoColour(resources);
      for (const { view, state, ocean, tables, eclipseRows } of nearSeas) {
        try {
          const waveRows = this.#oceanWaveRows(ocean.id);
          this.#engine.writeBuffer(
            waveRows,
            0,
            packOceanWaves(state.waves, state.set, state.peakWavelengthM),
          );
          draws.push(
            oceanNearDraw(
              grid,
              nearMaterial,
              {
                oceanMap: ocean.sea.map.texture,
                tables,
                eclipseRows,
                waveRows,
                sceneDepthBits: copies.depthBits.colour,
                sceneColour,
              },
              oceanNearUniforms(view, state.frame),
            ),
          );
          kept.add(ocean.id);
        } catch (error: unknown) {
          if (!lostDevice(error)) {
            console.error(
              `the near sea of ${ocean.id} could not be drawn in ${this.#name}:`,
              error,
            );
          }
        }
      }
    } else {
      this.#releaseOceanCopies(resources);
    }
    for (const [id, rows] of this.#oceanWaves) {
      if (!kept.has(id)) {
        this.#engine.releaseBuffer(rows);
        this.#oceanWaves.delete(id);
      }
    }
    return draws;
  }

  /** The near sea's copies of the scene at its size, its colour's only with refraction. */
  #oceanCopies(resources: Resources, refraction: boolean): OceanCopies {
    const target = (name: string, format: ColourTargetFormat): RenderTarget =>
      this.#engine.createRenderTarget({
        name: `${this.#name}:ocean ${name}`,
        size: resources.size,
        format,
        mips: 1,
        depth: false,
        category: "ocean",
      });
    const copies = (resources.oceanCopies ??= {
      depthBits: target("depth copy", "r32uint"),
      colour: null,
    });
    if (refraction && copies.colour === null) {
      copies.colour = target("colour copy", HDR_COLOUR_FORMAT);
    } else if (!refraction && copies.colour !== null) {
      copies.colour.dispose();
      copies.colour = null;
    }
    return copies;
  }

  /** Lets go of the near sea's copies, its grid and its colour's stand-in. */
  #releaseOceanCopies(resources: Resources): void {
    resources.oceanCopies?.depthBits.dispose();
    resources.oceanCopies?.colour?.dispose();
    resources.oceanCopies = null;
    if (resources.oceanGrid !== null) {
      this.#engine.releaseMesh(resources.oceanGrid.mesh);
      resources.oceanGrid = null;
    }
    if (resources.oceanNoColour !== null) {
      this.#engine.releaseTexture(resources.oceanNoColour);
      resources.oceanNoColour = null;
    }
  }

  /** The near sea's grid at the frame's size and `cellPx`, made again where either changed. */
  #oceanGridOf(resources: Resources, cellPx: number): MeshHandle {
    const { widthPx, heightPx } = resources.size;
    const key = `${String(widthPx)} ${String(heightPx)} ${String(cellPx)}`;
    if (resources.oceanGrid?.key === key) {
      return resources.oceanGrid.mesh;
    }
    if (resources.oceanGrid !== null) {
      this.#engine.releaseMesh(resources.oceanGrid.mesh);
    }
    const { columns, rows } = oceanGridSize(widthPx, heightPx, cellPx);
    const { positions, indices } = oceanGridMesh(columns, rows);
    const mesh = this.#engine.createMesh({
      name: `${this.#name} ocean grid`,
      positions,
      indices,
      topology: "triangle-list",
      attributes: {},
    });
    resources.oceanGrid = { mesh, key };
    return mesh;
  }

  /** The one-texel stand-in for the scene's colour where the near sea refracts nothing. */
  #oceanNoColour(resources: Resources): TextureHandle {
    if (resources.oceanNoColour === null) {
      const texture = this.#engine.createTexture({
        name: `${this.#name} ocean: no colour`,
        size: [1, 1],
        dimension: "2d",
        format: "rgba16float",
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
        category: "ocean",
      });
      this.#engine.writeTexture(
        texture,
        { x: 0, y: 0 },
        { width: 1, height: 1 },
        new Uint16Array(4),
      );
      resources.oceanNoColour = texture;
    }
    return resources.oceanNoColour;
  }

  /** `body`'s near sea's waves' rows, made at its first near frame. */
  #oceanWaveRows(body: BodyIdHex): BufferHandle {
    let rows = this.#oceanWaves.get(body);
    if (rows === undefined) {
      rows = this.#engine.createBuffer({
        name: `${this.#name} ocean waves ${body}`,
        bytes: 16 * OCEAN_WAVE_ROWS,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "ocean",
      });
      this.#oceanWaves.set(body, rows);
    }
    return rows;
  }

  /** `body`'s sea's eclipse rows, made at its first frame. */
  #oceanEclipseRows(body: BodyIdHex): BufferHandle {
    let rows = this.#oceanEclipses.get(body);
    if (rows === undefined) {
      rows = this.#engine.createBuffer({
        name: `${this.#name} ocean eclipse ${body}`,
        bytes: 16 * OCEAN_ECLIPSE_ROWS,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "ocean",
      });
      this.#oceanEclipses.set(body, rows);
    }
    return rows;
  }

  /** The tables a sea without its air drawn is lit through, one texture of zeros. */
  #airlessTables(): SurfaceLightingTables {
    return createAirlessTables(this.#engine, `${this.#name} ocean: no air`);
  }

  /**
   * Draws each body's composite over the scene target, the farthest first, each into the next air
   * target over the one before (plan R08, R08.T10.b), and lets go of the atmospheres of the bodies
   * it draws none for.
   *
   * @remarks
   * A body's air is drawn where the plan holds its record, where the camera is inside its air,
   * and where its shell, a + H ÷ s_e, meets the view though its figure does not, but for a body the
   * plan draws by its disc table beyond the 10⁹ m boundary (R08.T16.b)
   * (`decision-r08-t10b-leans.md` §3), its limb's glow then still in the picture. Its sky takes the
   * sources its record was shaded from (`BodyFramePlan.sources`, Design note 7's one list), less
   * Design note 7's skips (`skySources`), a star past the light slots its tables hold left out
   * until the cache builds them for it; a body the plan holds no record of takes them as the plan
   * would (`bodyLightSources`).
   *
   * A throw from a body's atmosphere (`HillaireAtmosphere.over`, `skySources`, `drawFrame`) is item
   * 6's failure (`decision-r08-t10b-leans.md` §2): reported through
   * {@link PhotorealFrame.onAtmosphereFailed}, whose cache holds the body failed and states the
   * fault, and the frame drawn on without the body's air. A lost device's throw
   * (`EngineUnavailable`, `AtmosphereRetired`) is not reported: the graphics fault covers it.
   *
   * @returns The colour the frame's later passes read: the last composite's, or the scene target's
   *   where none was drawn.
   */
  #drawAtmospheres(
    resources: Resources,
    frame: PhotorealFrame,
    plan: BodyFramePlan,
    lit: {
      readonly bodies: ReadonlyArray<LitBodyInput>;
      readonly shellRadiiM: ReadonlyMap<BodyIdHex, number>;
      readonly clouds: ReadonlyMap<BodyIdHex, FrameClouds>;
    },
    pass: { readonly viewRotation: Float32Array; readonly projection: Float32Array },
  ): TextureHandle {
    const recorded = new Set(plan.discs.map((record) => record.body));
    const byId = new Map(lit.bodies.map((body) => [body.id, body]));
    const drawn: Array<{ readonly air: FrameAtmosphere; readonly sources: BodyLightSources }> = [];
    for (const air of frame.atmospheres ?? []) {
      // A body drawn by its disc table beyond the boundary takes none of its full passes (R08.T16.b).
      if (plan.tabled.has(air.id)) {
        continue;
      }
      const sources = plan.sources.get(air.id);
      const centreM = byId.get(air.id)?.centreM;
      const shellM = lit.shellRadiiM.get(air.id);
      const shellSeen =
        centreM !== undefined &&
        shellM !== undefined &&
        !sphereOutsideView(centreM, shellM, frame.camera, frame.viewport);
      // A body drawn by its record, one whose air's shell is in view, or the camera's own inside it.
      if (sources !== undefined && (recorded.has(air.id) || shellSeen || insideAir(air))) {
        drawn.push({ air, sources });
      }
    }
    const order = drawn.toSorted(
      (a, b) => norm(b.air.camera.positionM) - norm(a.air.camera.positionM),
    );
    const depth = resources.target.depth;
    let colour = resources.target.colour;
    const kept = new Set<BodyIdHex>();
    for (const { air, sources } of order) {
      kept.add(air.id);
      if (depth === null) {
        break;
      }
      const tables = air.tables;
      const lightSlots = absorberCount(tables.medium) > 0 ? tables.curves.length : Infinity;
      let composite: DrawItem;
      let area: TexelRect | null;
      let suns: ReadonlyArray<SunState>;
      try {
        const atmosphere = this.#atmosphereOf(
          resources,
          air,
          frame.atmosphereSetting ?? frame.setting,
          frame.atmosphereLevers,
        );
        suns = skySources(
          sources.stars,
          sources.planetshine,
          air.camera,
          tablesFigureOf(tables),
          tables.medium,
          air.toBodyFixed,
        ).filter((sun) => sun.curveSlot < lightSlots);
        composite = atmosphere.drawFrame(air.camera, suns, {
          colour,
          depth,
          nearM: NEAR_PLANE_M,
          exposureScale: frame.exposureScale,
        });
        area = atmosphere.compositeArea;
      } catch (error: unknown) {
        // The one place a body's drawing fails: item 6's failure, the cache's to hold and state.
        if (!lostDevice(error)) {
          if (frame.onAtmosphereFailed === undefined) {
            console.error(
              `the atmosphere of ${air.id} could not be drawn in ${this.#name}:`,
              error,
            );
          } else {
            frame.onAtmosphereFailed(air.id, error, this.#name);
          }
        }
        continue;
      }
      // The air target the last composite did not draw into: the first after the scene's.
      const target = this.#airTarget(resources, colour === resources.airTargets[0]?.colour ? 1 : 0);
      // Where the composite keeps the scene's light outside the air's area, it is drawn over that
      // area alone, over a copy of the colour it reads (R08.T16.a.2): not a full-screen pass.
      target.render({
        label: ATMOSPHERE_COMPOSITE_PASS,
        viewRotation: pass.viewRotation,
        projection: pass.projection,
        draws: [composite],
        postProcesses: [],
        ...(area === null ? {} : { colourFrom: colour, scissor: rectWithin(area, resources.size) }),
      });
      // Its clouds over its composite, into the target that drew it (plan R11, R11.T7.b), within
      // the air's area, where the composite keeps one: the layer lies inside the air's shell.
      const own = lit.clouds.get(air.id);
      const lighting = byId.get(air.id)?.atmosphere;
      if (own !== undefined && resources.cloudLayer !== null && lighting !== undefined) {
        const figure = tablesFigureOf(tables);
        const view = {
          camera: air.camera,
          figure,
          shell: cloudShellOf(figure, cloudLayerHeightM(own.condensate)),
          airTopM: tables.medium.topHeightM,
          suns,
          exposureScale: frame.exposureScale,
          octaves: LAYER_OCTAVES[frame.setting],
          deck: { baseM: own.condensate.baseAltitudeM, topM: own.condensate.topAltitudeM },
        };
        const scissor = area === null ? null : rectWithin(area, resources.size);
        // The volume of the body it draws as one (plan R11, R11.T7.c), else the layer.
        if (own.volume !== undefined && resources.cloudVolume !== null) {
          this.#volumes.draw(
            {
              id: air.id,
              gpu: own.gpu,
              condensate: own.condensate,
              view,
              lighting,
              sceneDepth: depth,
              blueNoise: resources.blueNoise,
              correction: own.volume,
            },
            { ...resources.cloudVolume, triangle: resources.triangle },
            target,
            resources.size,
            pass,
            scissor,
          );
          colour = target.colour;
          continue;
        }
        const uniforms = cloudLayerUniforms(view);
        target.render({
          label: CLOUD_LAYER_PASS,
          viewRotation: pass.viewRotation,
          projection: pass.projection,
          draws: [
            cloudLayerDraw(
              resources.triangle,
              resources.cloudLayer,
              own.gpu,
              depth,
              lighting,
              uniforms,
            ),
          ],
          postProcesses: [],
          colourLoad: "load",
          ...(scissor === null ? {} : { scissor }),
        });
      }
      colour = target.colour;
    }
    for (const [id, held] of this.#atmospheres) {
      if (!kept.has(id)) {
        held.atmosphere.dispose();
        this.#atmospheres.delete(id);
      }
    }
    return colour;
  }

  /**
   * Each body's clouds this frame (plan R11, R11.T7.b): for each of the frame's atmospheres whose
   * record's clouds the layer draws (`layerCondensateOf`), its rows and coverage from the view's
   * {@link CloudLayers}, at the frame's time and rate, its season the subsolar latitude of its
   * brightest star, and the setting's octaves (`LAYER_OCTAVES`); none before the noise tile is made,
   * or where the layer's material was refused. The tile's making steps on at each frame with such a
   * body, within its budget, and the bodies no frame draws are let go.
   */
  #cloudsOf(
    resources: Resources,
    frame: PhotorealFrame,
    atmospheres: ReadonlyArray<FrameAtmosphere>,
  ): ReadonlyMap<BodyIdHex, FrameClouds> {
    const drawn = new Map<BodyIdHex, FrameClouds>();
    const wanted =
      resources.cloudLayer === null
        ? []
        : atmospheres.flatMap((air) => {
            const clouds = air.clouds ?? null;
            const condensate = clouds === null ? null : layerCondensateOf(clouds);
            return clouds === null || condensate === null ? [] : [{ air, clouds, condensate }];
          });
    if (wanted.length > 0) {
      this.#clouds.step(this.#nowMs);
    }
    const centres = new Map(frame.bodies.map((body) => [body.id, body.centreM]));
    for (const { air, clouds, condensate } of wanted) {
      const centreM = centres.get(air.id);
      const brightest = centreM === undefined ? undefined : lightsAt(centreM, frame.lights, 1)[0];
      const gpu = this.#clouds.frame({
        id: air.id,
        clouds,
        condensate,
        figure: tablesFigureOf(air.tables),
        season:
          brightest === undefined
            ? NO_SEASON
            : seasonOf(air.toBodyFixed, BODY_CENTRE, brightest.toStarM),
        time: frame.time ?? CLOCK_ZERO,
        rate: frame.timeRate ?? 0,
        octaves: LAYER_OCTAVES[frame.setting],
      });
      if (gpu !== null) {
        drawn.set(air.id, { condensate, gpu });
      }
    }
    this.#clouds.keepOnly(new Set(drawn.keys()));
    // The nearest such body as the volume, where the setting's clouds are one and the frame-rate
    // floor has not stepped the air down (plan R11, R11.T7.c), once its correction is made.
    const volume =
      SETTINGS[frame.setting].clouds === "volume" &&
      (frame.atmosphereSetting ?? frame.setting) === frame.setting &&
      resources.cloudVolume !== null
        ? nearestOf(wanted.filter(({ air }) => drawn.has(air.id)))
        : null;
    if (volume === null) {
      // A frame without the volume ends its history; its corrections, 2 KiB each, are kept.
      this.#volumes.cut();
      return drawn;
    }
    const correction = this.#volumes.correction(volume.condensate, this.#nowMs);
    this.#volumes.keepOnly([volume.condensate]);
    const own = drawn.get(volume.air.id);
    if (correction !== null && own !== undefined) {
      drawn.set(volume.air.id, { ...own, volume: correction });
    }
    return drawn;
  }

  /**
   * Makes the clouds' noise tile at once, and the volume's correction of each of `atmospheres`'
   * condensates, so that a still's first sample draws its bodies' clouds as its frames would (plan
   * R11, R11.T10.b's still renderer, made at each still); a view's frames make them over frames.
   */
  prepareClouds(atmospheres: ReadonlyArray<FrameAtmosphere> = []): void {
    this.#clouds.step(this.#nowMs, Number.POSITIVE_INFINITY);
    for (const air of atmospheres) {
      const clouds = air.clouds ?? null;
      const condensate = clouds === null ? null : layerCondensateOf(clouds);
      if (condensate !== null) {
        this.#volumes.correction(condensate, this.#nowMs, Number.POSITIVE_INFINITY);
      }
    }
  }

  /**
   * A body's atmosphere on this view over `air`'s tables, made at the setting's sizes the first
   * time, or again on a new setting or levers, with `levers` over the setting's (R08.T8.d), and
   * following the cache's tables as they are remade.
   */
  #atmosphereOf(
    resources: Resources,
    air: FrameAtmosphere,
    setting: QualitySetting,
    levers: AtmosphereLeversOverride | undefined,
  ): HillaireAtmosphere {
    const held = this.#atmospheres.get(air.id);
    if (
      held !== undefined &&
      held.setting === setting &&
      held.levers?.marchCover === levers?.marchCover &&
      held.levers?.limbRays === levers?.limbRays &&
      held.levers?.marchSourceTransmittance === levers?.marchSourceTransmittance &&
      held.levers?.surfaceLobes === levers?.surfaceLobes &&
      held.levers?.deckedAirSurface === levers?.deckedAirSurface
    ) {
      held.atmosphere.setTables(air.tables);
      return held.atmosphere;
    }
    held?.atmosphere.dispose();
    const atmosphere = HillaireAtmosphere.over(
      this.#engine,
      air.tables,
      SETTINGS[setting].atmosphere,
      resources.atmosphere,
      { ...SETTINGS[setting].atmosphereLevers, ...levers },
    );
    this.#atmospheres.set(air.id, { atmosphere, setting, levers });
    return atmosphere;
  }

  /**
   * The air target `index` (0 or 1), made at the frame's size where it has not been: a colour
   * target without depth, so that a composite may read the scene target's depth as it draws.
   */
  #airTarget(resources: Resources, index: 0 | 1): RenderTarget {
    let target = resources.airTargets[index];
    while (target === undefined) {
      const made = this.#engine.createRenderTarget({
        name: `${this.#name}:air ${resources.airTargets.length}`,
        size: resources.size,
        format: HDR_COLOUR_FORMAT,
        mips: 1,
        depth: false,
        category: "render-targets",
      });
      resources.airTargets.push(made);
      target = resources.airTargets[index];
    }
    return target;
  }

  /**
   * A way's form of the `disc cells` kernel for a plan with a record that needs it (R08.T18.a's
   * emission, R08.T16.b's air; any record, for the packed plain kernel), or `null`: asked for at the
   * first such plan, off the frame, as the renderer's other kernels are made, so that the frame
   * never waits on its compile; until it is made those records take no cells and their draws sum
   * their own, but the packed plain kernel's, which the per-cell kernel stands in for. A failure is
   * reported once and leaves them so.
   */
  #cellsFor(plan: BodyFramePlan, way: CellWay, kind: CellForm): ComputeHandle | null {
    const held = this.#formCells[way][kind];
    if (held !== null && held.generation === this.#generation) {
      return held.kernel;
    }
    const needed = plan.discs.some((record) => cellFormOf(record) === kind);
    if (needed) {
      this.#askCells(way, kind);
    }
    return null;
  }

  /** Asks for a way's form of the `disc cells` kernel on this generation's device. */
  #askCells(way: CellWay, kind: CellForm): FormCells {
    const making: FormCells = {
      generation: this.#generation,
      kernel: null,
      made: Promise.resolve(),
    };
    making.made = this.#makeCells(way, kind, making);
    this.#formCells[way][kind] = making;
    return making;
  }

  /** A way's form's `disc cells` kernel as asked on this generation, asked now where it was not. */
  #cellsMaking(way: CellWay, kind: CellForm): FormCells {
    const held = this.#formCells[way][kind];
    return held !== null && held.generation === this.#generation ? held : this.#askCells(way, kind);
  }

  /**
   * Makes the lit bodies' emissive forms and the emissive `disc cells` kernel (R08.T18.a) ahead of
   * a body's first frame above the cut, as a timed run needs them made before its timed frames;
   * resolves once both are made, or could not be.
   *
   * @param way - The way of the `disc cells` kernel its frames take (`GeometrySettings.discCells`).
   */
  async prepareEmissive(way: CellWay = PREPARED_CELL_WAY): Promise<void> {
    await Promise.all([this.#bodies.prepareEmissive(), this.#cellsMaking(way, "emissive").made]);
  }

  /**
   * Makes the lit bodies' air forms and the air `disc cells` kernel (R08.T16.b) ahead of a body's
   * first frame drawn by its disc table, as a timed run needs them made before its timed frames;
   * resolves once both are made, or could not be.
   *
   * @param way - The way of the `disc cells` kernel its frames take (`GeometrySettings.discCells`).
   */
  async prepareAir(way: CellWay = PREPARED_CELL_WAY): Promise<void> {
    await Promise.all([this.#bodies.prepareAir(), this.#cellsMaking(way, "air").made]);
  }

  /**
   * Makes the lit bodies' forms for emission and air together and their `disc cells` kernel
   * (R08.T18.b) ahead of a glowing body's first frame drawn by its disc table; resolves once both are
   * made, or could not be.
   *
   * @param way - The way of the `disc cells` kernel its frames take (`GeometrySettings.discCells`).
   */
  async prepareEmissiveAir(way: CellWay = PREPARED_CELL_WAY): Promise<void> {
    await Promise.all([
      this.#bodies.prepareEmissiveAir(),
      this.#cellsMaking(way, "emissiveAir").made,
    ]);
  }

  /**
   * Makes every pipeline its lit bodies draw with, ahead of the first frame (plan R11, R11.T10.b):
   * every regime's (`LitBodyRenderer.prepareAll`), the emissive, air and emissive air forms with
   * their `disc cells` kernels where `forms` says its frames take them, and the packed plain kernel
   * where `way` packs the cells; resolves once all are made, or could not be. A still's renderer,
   * new at each still, waits for them before its first batch, so that its draws make none; a view's
   * frames never wait for them.
   *
   * @param way - The way of the `disc cells` kernel its frames take (`GeometrySettings.discCells`).
   * @param forms - Whether its frames hold a body whose ground emits (R08.T18.a) and one drawn by
   *   its air's disc table (R08.T16.b).
   */
  async prepareBodies(
    way: CellWay,
    forms: { readonly emissive: boolean; readonly air: boolean },
  ): Promise<void> {
    await Promise.all([
      this.#bodies.prepareAll(),
      ...(forms.emissive ? [this.prepareEmissive(way)] : []),
      ...(forms.air ? [this.prepareAir(way)] : []),
      ...(forms.emissive && forms.air ? [this.prepareEmissiveAir(way)] : []),
      ...(way === "packed" ? [this.#cellsMaking(way, "plain").made] : []),
    ]);
  }

  /**
   * Makes a way's form's `disc cells` kernel into `making`; a failure is reported, a loss's not,
   * and it never rejects.
   */
  async #makeCells(
    way: CellWay,
    kind: CellForm,
    making: { kernel: ComputeHandle | null },
  ): Promise<void> {
    try {
      making.kernel = await this.#engine.createComputeAsync(CELL_FORM_KERNELS[way][kind]);
    } catch (error: unknown) {
      if (!lostDevice(error)) {
        console.error(`the ${way} ${kind} disc cells of ${this.#name} could not be made:`, error);
      }
    }
  }

  /** Lets go of every body's atmosphere on this view. */
  #releaseAtmospheres(): void {
    for (const { atmosphere } of this.#atmospheres.values()) {
      atmosphere.dispose();
    }
    this.#atmospheres.clear();
  }

  /** Stops following restores and releases the view's targets, buffers and textures. */
  dispose(): void {
    this.#disposed = true;
    this.#unsubscribe();
    this.#bodies.dispose();
    this.#releaseAtmospheres();
    this.#clouds.dispose();
    this.#volumes.dispose();
    const resources = this.#resources;
    if (resources !== null) {
      resources.histograms.dispose();
      resources.target.dispose();
      for (const target of resources.airTargets) {
        target.dispose();
      }
      resources.visibility?.dispose();
      resources.figureIds?.dispose();
      // The atmospheres' composite and its triangle, made with the view's pipelines; their
      // kernels have no release.
      this.#engine.releaseMaterial(resources.atmosphere.composite);
      for (const material of [
        resources.cloudLayer,
        resources.cloudVolume?.march ?? null,
        resources.cloudVolume?.resolve ?? null,
        resources.cloudVolume?.composite ?? null,
      ]) {
        if (material !== null) {
          this.#engine.releaseMaterial(material);
        }
      }
      this.#engine.releaseMaterial(resources.overlayDepth);
      this.#engine.releaseMesh(resources.atmosphere.triangle);
      // The sea's materials, its airless tables, its rows and its copies (plan R11, R11.T8.b–c).
      for (const rows of [...this.#oceanEclipses.values(), ...this.#oceanWaves.values()]) {
        this.#engine.releaseBuffer(rows);
      }
      this.#oceanEclipses.clear();
      this.#oceanWaves.clear();
      this.#releaseOceanCopies(resources);
      for (const each of [resources.ocean, resources.oceanNear, resources.oceanDepthCopy]) {
        if (each !== null) {
          this.#engine.releaseMaterial(each);
        }
      }
      if (resources.airless !== null) {
        this.#engine.releaseTexture(resources.airless.body);
      }
      // Its own pipelines and meshes (plan R11, R11.T10.b: a still's renderer is made at each still).
      this.#engine.releaseMaterial(resources.tonemap);
      this.#engine.releaseMaterial(resources.sprite);
      this.#engine.releaseMesh(resources.triangle);
      this.#engine.releaseMesh(resources.quad);
      resources.bloom.dispose();
      this.#engine.releaseBuffer(resources.glare);
      this.#engine.releaseBuffer(resources.stars);
      this.#engine.releaseTexture(resources.blueNoise);
      this.#resources = null;
    }
    this.#pyramid?.dispose();
    this.#pyramid = null;
  }
}

/**
 * The last photorealistic frame a view drew and its plan (plan R11, R11.T10.b): what a still
 * freezes at its request.
 */
export interface DrawnPhotorealFrame {
  readonly frame: PhotorealFrame;
  readonly plan: BodyFramePlan;
}

/**
 * The jittered samples a still accumulates on each setting (plan R11, Design note 17: "sub-pixel
 * jitter accumulates at full resolution"): 8 on the high setting, the Halton (2, 3) pattern of 8
 * samples Unreal Engine 4's temporal anti-aliasing jitters its projection through (Karis 2014, "High
 * Quality Temporal Supersampling", SIGGRAPH 2014 course Advances in Real-Time Rendering in Games,
 * slides 13 and 14, checked 2026-11-18), and 4 on the low, half the samples' GPU time (lean,
 * 2026-11-18). Each sample is one unit of the job, a whole frame of the view's passes.
 */
export const STILL_SAMPLES: Readonly<Record<QualitySetting, number>> = { high: 8, low: 4 };

/** The name a still's GPU objects are made under, and its passes' labels begin with. */
const STILL_NAME = "still";

/**
 * The passes of a still's units (`<pass> <tile>`, the label a unit's cost is learnt under): each
 * jittered sample of the view's passes with its accumulation, the bloom chain over the assembled
 * image, and the tone-mapping pass into the image read back.
 */
export const STILL_UNIT_PASSES = { scene: "scene", bloom: "bloom", tonemap: "tonemap" } as const;

/**
 * The accumulation of a still's samples (plan R11, R11.T10.b): R07's bloom upsample (`bloomUp.wgsl`,
 * unchanged), its `coarse` input weighted 0, in an additive form, so that each draw adds its
 * `level`, a sample's HDR colour read texel for texel, at its weight, and the destination's alpha,
 * R07's meter class, is kept.
 */
export const STILL_ACCUMULATE_MATERIAL: WgslMaterialSpec = {
  ...BLOOM_UP_MATERIAL,
  name: "still accumulation",
  displayName: "STILL ACCUMULATION",
  blend: "additive",
};

/** The `NOT AVAILABLE:` causes of a view that gives no still now, with what lifts each. */
export const STILL_UNAVAILABLE = {
  /**
   * No view is attached: the engine is not ready, the view could not be made, or the display is
   * hidden or between scenes.
   */
  noView: "the view is not drawn",
  /** The view's last frame is its wireframe, or none. */
  noImage: "the view draws no photorealistic image",
  /**
   * The view's last frame was held back at a cut, drawn at the exposure from before it, which the
   * view never shows (`PhotorealFrame.present`).
   */
  settling: "the view's exposure is settling after a cut",
  /**
   * A body is handed over to its terrain, which a still does not draw: the terrain is drawn for
   * the view's own frames alone, and a still's own terrain view, frozen and streamed at bulk
   * priority, is not built (lean, 2026-11-18).
   */
  terrain: "terrain is not yet drawn in stills, take one farther from the ground",
  /** The view's image is over `STILL_MAX_SIDE_PX` on a side, which the main process refuses. */
  oversized: "the view is over 8192 px on a side, the largest still",
} as const;

/** A {@link STILL_UNAVAILABLE} cause. */
export type StillUnavailableCause = (typeof STILL_UNAVAILABLE)[keyof typeof STILL_UNAVAILABLE];

/**
 * The radical inverse of `index` in `base`, each coordinate of Halton's sequence (Halton 1960,
 * Numerische Mathematik 2, 84–90, from memory; the sequence as Karis 2014's slide 14 takes it).
 */
function radicalInverse(index: number, base: number): number {
  let inverse = 0;
  let digit = 1 / base;
  for (let n = index; n > 0; n = Math.floor(n / base)) {
    inverse += (n % base) * digit;
    digit /= base;
  }
  return inverse;
}

/**
 * Sample `k`'s sub-pixel offset, px right and down: the first at the pixel's centre, so that a
 * still of one sample is drawn through P itself, and each after it the Halton (2, 3) point `k`
 * less ½ on each axis, in (−½, ½): the centre and points 1 to n − 1, Karis 2014's count but not
 * Unreal Engine 4's own points.
 */
export function stillJitterPx(k: number): readonly [number, number] {
  return k === 0 ? [0, 0] : [radicalInverse(k, 2) - 0.5, radicalInverse(k, 3) - 0.5];
}

/**
 * Sample `k`'s projection: `tileProjection` of the still drawn as one tile whose origin is moved
 * by the sample's offset, so that the image moves by it; P itself, bit for bit, for the first.
 */
function sampleProjection(fovXRad: number, size: ViewSize, k: number): Float32Array {
  const [dx, dy] = stillJitterPx(k);
  const tile: StillTile = {
    column: 0,
    row: 0,
    rect: { x0Px: dx, y0Px: dy, x1Px: size.widthPx + dx, y1Px: size.heightPx + dy },
    texture: size,
    bytesPerRow: size.widthPx * 4,
  };
  return tileProjection(fovXRad, size, tile);
}

/**
 * Each unit's resolves, by R01's `passTimesFrame`, summed into the unit's GPU time once all have
 * resolved: a unit is a whole frame of passes, each render its own resolve.
 */
class UnitTimes {
  readonly #byResolve = new Map<
    number,
    { readonly label: string; remaining: number; ns: number; valid: boolean }
  >();

  /** The unit `label` drew the resolves after `fromFrame` up to and with `toFrame`. */
  note(label: string, fromFrame: number, toFrame: number): void {
    if (toFrame <= fromFrame) {
      return;
    }
    const tally = { label, remaining: toFrame - fromFrame, ns: 0, valid: true };
    for (let frame = fromFrame + 1; frame <= toFrame; frame += 1) {
      this.#byResolve.set(frame, tally);
    }
  }

  /**
   * The unit `times` completes, with its GPU time, or `null`: for a resolve of no unit, one the
   * unit has more of, or a unit a negative pass time (a reset of the timestamp counter) leaves
   * unmeasured.
   */
  take(times: PassTimes): MeasuredPass | null {
    const tally = this.#byResolve.get(times.frame);
    if (tally === undefined) {
      return null;
    }
    this.#byResolve.delete(times.frame);
    for (const pass of times.passes) {
      if (pass.ns < 0) {
        tally.valid = false;
      }
      tally.ns += pass.ns;
    }
    tally.remaining -= 1;
    return tally.remaining === 0 && tally.valid ? { label: tally.label, ns: tally.ns } : null;
  }
}

/** The identity, column-major: the still's full-screen passes read no camera. */
const STILL_IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** What a {@link PhotorealStillRenderer} is made with. */
export interface PhotorealStillOptions {
  /** The tile limit the read-back is cut at, px: the device's `maxTextureDimension2D`. */
  readonly tileLimitPx: number;
  /** The jittered samples it accumulates: {@link STILL_SAMPLES} at the frame's setting. */
  readonly samples: number;
  /** Takes each unit's GPU time as it resolves, with timestamps; never called without. */
  readonly onUnitTimes: (passes: ReadonlyArray<MeasuredPass>) => void;
  /** Called once, at its disposal. */
  readonly onDispose: () => void;
}

/** A still renderer's GPU objects, made in its `prepare()`. */
interface StillResources {
  readonly triangle: MeshHandle;
  readonly accumulate: MaterialHandle;
  readonly tonemap: MaterialHandle;
  readonly bloom: BloomChain;
  /** The samples' sum at their weights: the assembled HDR image. */
  readonly accumulation: RenderTarget;
  /** The tone-mapped image, `rgba8unorm`, read back by tiles. */
  readonly image: RenderTarget;
  readonly blueNoise: TextureHandle;
  readonly glare: BufferHandle;
}

/**
 * A still of a view's photorealistic frame (plan R11, R11.T10.b, Design note 17): the frame frozen
 * at the request drawn through its own {@link PhotorealRenderer}, sample by sample with a sub-pixel
 * jitter, each sample accumulated at full resolution into one HDR image at a fixed exposure, the
 * frame's own; bloom and glare once over that assembled image; R07's tone-mapping pass into an
 * `rgba8unorm` image; each tile of the still read back from it through R01's `readTexture`.
 *
 * @remarks
 * It draws at the frame's own size, the view's scene target's, with the frame's own plan inputs, so
 * that every screen-space footprint is the view's, and in one pass rather than tile by tile: R07's
 * disc, point-sprite and host-disc passes place their CPU-made rectangles by `frame.viewport` and
 * rebuild their rays as ndc ÷ P's diagonal, which holds for a centred projection alone, so a tile's
 * off-axis M · P would draw them elsewhere; the tiles are the read-back's (lean, 2026-11-18). A
 * sample's offset moves what is projected through the whole matrix (the figures, R06's sky); the
 * passes placed in pixels keep their own anti-aliasing. It draws no terrain
 * ({@link STILL_UNAVAILABLE}'s `terrain`), the symbology, the meter or the histogram.
 *
 * Its units are its samples (`scene 0`), then `bloom 0` and `tonemap 0`, each one submission of
 * whole passes; with timestamps each unit's GPU time is the sum of its resolves', handed to
 * {@link PhotorealStillOptions.onUnitTimes} under its label. A frame's resources taken by
 * reference (the sky's cube, an atmosphere's tables) are the view's: a cache that lets one go while
 * the still runs fails its draw.
 */
export class PhotorealStillRenderer implements StillRenderer {
  readonly tiling: StillTiling;
  readonly units: ReadonlyArray<string>;
  readonly #engine: RenderEngine;
  /** The frozen frame as each sample draws it, but its projection. */
  readonly #frame: PhotorealFrame;
  readonly #size: ViewSize;
  readonly #samples: number;
  readonly #scene: PhotorealRenderer;
  readonly #times = new UnitTimes();
  readonly #unsubscribeTimes: () => void;
  readonly #onDispose: () => void;
  /** What it made, released at its disposal, whether `prepare()` finished or not. */
  readonly #made: Array<() => void> = [];
  #resources: StillResources | null = null;
  /** An atmosphere's failure in the last sample, which fails the still. */
  #failure: { readonly error: unknown } | null = null;
  #disposed = false;

  /**
   * @param frame - The view's frame frozen at the request, drawn at its own size.
   * @throws RangeError for a frame whose size is not a still's, or a sample count under 1.
   */
  constructor(engine: RenderEngine, frame: PhotorealFrame, options: PhotorealStillOptions) {
    if (!(Number.isSafeInteger(options.samples) && options.samples >= 1)) {
      throw new RangeError("a still takes at least one sample");
    }
    this.#engine = engine;
    this.#onDispose = options.onDispose;
    this.#size = { widthPx: frame.viewport.widthPx, heightPx: frame.viewport.heightPx };
    this.#samples = options.samples;
    this.tiling = planTiles(this.#size, options.tileLimitPx, frame.setting);
    this.units = [
      ...Array.from({ length: options.samples }, () => `${STILL_UNIT_PASSES.scene} 0`),
      `${STILL_UNIT_PASSES.bloom} 0`,
      `${STILL_UNIT_PASSES.tonemap} 0`,
    ];
    // As the view drew it, its frame-rate floor's steps with it, so that every kernel its air asks
    // for is one the view's frames made, in the device's records: none is compiled in a batch.
    this.#frame = {
      ...frame,
      terrain: undefined,
      overlay: null,
      meter: null,
      present: false,
      onAtmosphereFailed: (_body, error) => {
        this.#failure ??= { error };
      },
    };
    this.#scene = new PhotorealRenderer(engine, STILL_NAME);
    this.#made.push(() => {
      this.#scene.dispose();
    });
    this.#unsubscribeTimes = engine.onPassTimes((times) => {
      const unit = this.#times.take(times);
      if (unit !== null) {
        options.onUnitTimes([unit]);
      }
    });
  }

  /** Holds `made` for the disposal, or releases it at once where that has come. */
  #hold<T>(made: T, release: (made: T) => void): T {
    if (this.#disposed) {
      release(made);
    } else {
      this.#made.push(() => {
        release(made);
      });
    }
    return made;
  }

  async prepare(): Promise<void> {
    const engine = this.#engine;
    const frame = this.#frame;
    const size = this.#size;
    const triangle = this.#hold(fullScreenTriangle(engine, `${STILL_NAME} triangle`), (mesh) => {
      engine.releaseMesh(mesh);
    });
    const releaseMaterial = (material: MaterialHandle): void => {
      engine.releaseMaterial(material);
    };
    const geometry = frame.geometry ?? SETTINGS[frame.setting].geometry;
    const [accumulate, tonemap, bloom] = await Promise.all([
      engine
        .createMaterialAsync(STILL_ACCUMULATE_MATERIAL, [HDR_COLOUR_FORMAT], [triangle])
        .then((made) => this.#hold(made, releaseMaterial)),
      engine
        .createMaterialAsync(TONEMAP_MATERIAL, ["rgba8unorm"], [triangle])
        .then((made) => this.#hold(made, releaseMaterial)),
      BloomChain.create(
        engine,
        STILL_NAME,
        size,
        bloomKernel(
          SETTINGS[frame.setting].photoreal.bloom,
          frame.role,
          radPerPx(frame.camera, size),
          DEFAULT_EYE_OBSERVER,
        ),
      ).then((made) =>
        this.#hold(made, (chain) => {
          chain.dispose();
        }),
      ),
      this.#scene.prepare(size, frame.setting, frame.role, frame.camera, frame.atmosphereLevers),
      this.#scene.prepareBodies(geometry.discCells, {
        emissive: frame.bodies.some((body) => body.thermal !== undefined),
        air:
          frame.bodies.some((body) => body.air !== undefined) ||
          (frame.atmospheres ?? []).some((each) => discAirOf(each.tables) !== null),
      }),
    ]);
    if (this.#scene.status !== "ready") {
      throw new Error("the still's photorealistic pipelines could not be made");
    }
    // Its bodies' clouds from its first sample (plan R11, R11.T7.b).
    if ((frame.atmospheres ?? []).some((each) => (each.clouds ?? null) !== null)) {
      this.#scene.prepareClouds(frame.atmospheres ?? []);
    }
    if (this.#disposed) {
      return;
    }
    const target = (name: string, format: ColourTargetFormat): RenderTarget =>
      this.#hold(
        engine.createRenderTarget({
          name: `${STILL_NAME} ${name}`,
          size,
          format,
          mips: 1,
          depth: false,
          category: "render-targets",
        }),
        (made) => {
          made.dispose();
        },
      );
    const blueNoise = this.#hold(
      engine.createTexture({
        name: `${STILL_NAME} blue noise`,
        size: [BLUE_NOISE_SIDE, BLUE_NOISE_SIDE],
        dimension: "2d",
        format: "r16float",
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
        category: "other",
      }),
      (texture) => {
        engine.releaseTexture(texture);
      },
    );
    engine.writeTexture(
      blueNoise,
      { x: 0, y: 0 },
      { width: BLUE_NOISE_SIDE, height: BLUE_NOISE_SIDE },
      blueNoiseTexels(),
    );
    const glare = this.#hold(
      engine.createBuffer({
        name: `${STILL_NAME} glare sources`,
        // Its own and at most one a sea (plan R11, R11.T8.b), written at its tone mapping.
        bytes:
          Math.max(1, frame.glareSources.length + (frame.oceans?.length ?? 0)) * GLARE_SOURCE_BYTES,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "other",
      }),
      (buffer) => {
        engine.releaseBuffer(buffer);
      },
    );
    engine.writeBuffer(
      glare,
      0,
      packGlareSources(
        frame.glareSources,
        frame.exposureScale,
        glareSpreadTerms(frame.role, DEFAULT_EYE_OBSERVER),
      ),
    );
    this.#resources = {
      triangle,
      accumulate,
      tonemap,
      bloom,
      accumulation: target("accumulation", HDR_COLOUR_FORMAT),
      image: target("image", "rgba8unorm"),
      blueNoise,
      glare,
    };
  }

  /** It draws no terrain, so it streams none: always streamed. */
  streamed(): boolean {
    return true;
  }

  submit(first: number, count: number): void {
    const resources = this.#resources;
    if (resources === null) {
      throw new Error("a still's batch was submitted before its pipelines were made");
    }
    for (let unit = first; unit < first + count; unit += 1) {
      const label = this.units[unit];
      if (label === undefined) {
        throw new RangeError(`a still has no unit ${String(unit)}`);
      }
      const fromFrame = this.#engine.passTimesFrame;
      this.#drawUnit(resources, unit);
      this.#times.note(label, fromFrame, this.#engine.passTimesFrame);
    }
  }

  /** The atmosphere's failure since the last call, or `null`, which it leaves. */
  #takeFailure(): { readonly error: unknown } | null {
    const failure = this.#failure;
    this.#failure = null;
    return failure;
  }

  /** Draws unit `unit`: a sample and its accumulation, the bloom chain, or the tone-mapping pass. */
  #drawUnit(resources: StillResources, unit: number): void {
    const frame = this.#frame;
    if (unit < this.#samples) {
      this.#takeFailure();
      const plan = this.#scene.render(null, {
        ...frame,
        projection: sampleProjection(frame.camera.fovXRad, this.#size, unit),
      });
      const failure = this.#takeFailure();
      if (failure !== null) {
        throw new Error("a still's atmosphere could not be drawn", { cause: failure.error });
      }
      const colours = this.#scene.frameColours;
      if (plan === null || colours === null) {
        throw new Error("a still's sample drew nothing");
      }
      this.#accumulate(resources, colours.drawn, unit);
    } else if (unit === this.#samples) {
      resources.bloom.run(
        resources.accumulation.colour,
        bloomThreshold(frame.exposureScale, frame.exposureScale),
      );
    } else {
      this.#toneMap(resources);
    }
  }

  /**
   * Adds sample `k`'s colour to the accumulation at the weight 1 ÷ n: the first copied in whole,
   * its meter class with it, and taken down to its weight by a draw of weight 1 ÷ n − 1 over it.
   */
  #accumulate(resources: StillResources, colour: TextureHandle, k: number): void {
    const weight = 1 / this.#samples;
    const draw = (sampleWeight: number): DrawItem => ({
      mesh: resources.triangle,
      material: resources.accumulate,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {
        levelWeight: new Float32Array([sampleWeight]),
        coarseWeight: new Float32Array([0]),
      },
      textures: { level: colour, coarse: resources.blueNoise },
    });
    const pass = {
      label: `${STILL_NAME} accumulation`,
      viewRotation: STILL_IDENTITY,
      projection: STILL_IDENTITY,
      postProcesses: [],
    };
    resources.accumulation.render(
      k === 0
        ? { ...pass, draws: this.#samples > 1 ? [draw(weight - 1)] : [], colourFrom: colour }
        : { ...pass, draws: [draw(weight)], colourLoad: "load" },
    );
  }

  /**
   * R07's tone-mapping pass over the assembled image, its bloom and its glare, into the image: the
   * frame's glare sources and its seas', as its last sample drew them (plan R11, R11.T8.b).
   */
  #toneMap(resources: StillResources): void {
    const frame = this.#frame;
    const terms = glareSpreadTerms(frame.role, DEFAULT_EYE_OBSERVER);
    const threshold = bloomThreshold(frame.exposureScale, frame.exposureScale);
    const glareSources = this.#scene.glareSources.slice(
      0,
      frame.glareSources.length + (frame.oceans?.length ?? 0),
    );
    this.#engine.writeBuffer(
      resources.glare,
      0,
      packGlareSources(glareSources, frame.exposureScale, terms),
    );
    resources.image.render({
      label: `${STILL_NAME} ${TONEMAP_PASS}`,
      viewRotation: viewRotation4(frame.camera.orientation),
      projection: sampleProjection(frame.camera.fovXRad, this.#size, 0),
      draws: [
        tonemapDraw({
          mesh: resources.triangle,
          material: resources.tonemap,
          hdrColour: resources.accumulation.colour,
          bloomUp: resources.bloom.levelOne,
          blueNoise: resources.blueNoise,
          glareSources: resources.glare,
          uniforms: {
            exposure: 1,
            threshold,
            levelZeroWeight: levelWeight(resources.bloom.kernel, 0),
            levelOneWeight: resources.bloom.levelOneWeight,
            sourceCount: glareSources.length,
            terms,
            dither: true,
            balance: frame.whiteBalance?.rows ?? null,
          },
        }),
      ],
      postProcesses: [],
      encoding: "in-pass",
    });
  }

  async readTile(tile: StillTile): Promise<Uint8Array> {
    const resources = this.#resources;
    if (resources === null) {
      throw new Error("a still's tile was read back before its pipelines were made");
    }
    const { x0Px, y0Px, x1Px, y1Px } = tile.rect;
    const widthPx = x1Px - x0Px;
    const rows = new Uint8Array(
      await this.#engine.readTexture(resources.image.colour, 0, {
        x: x0Px,
        y: y0Px,
        width: widthPx,
        height: y1Px - y0Px,
      }),
    );
    const rowBytes = widthPx * 4;
    if (rowBytes === tile.bytesPerRow) {
      return rows;
    }
    // The tile's texture is padded to its aligned rows; the columns past the still are cropped.
    const padded = new Uint8Array(tile.bytesPerRow * tile.texture.heightPx);
    for (let v = 0; v < y1Px - y0Px; v += 1) {
      padded.set(rows.subarray(v * rowBytes, (v + 1) * rowBytes), v * tile.bytesPerRow);
    }
    return padded;
  }

  /**
   * The assembled HDR image's texels, `rgba16float` rows from the top, read back once every unit is
   * done: the smoke harness's "every texel finite".
   *
   * @throws Error, as a rejection, before its pipelines are made.
   */
  readHdr(): Promise<ArrayBuffer> {
    const resources = this.#resources;
    return resources === null
      ? Promise.reject(new Error("a still's image was read back before it was made"))
      : this.#engine.readTexture(resources.accumulation.colour);
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#onDispose();
    this.#unsubscribeTimes();
    this.#resources = null;
    for (const release of this.#made.splice(0).toReversed()) {
      release();
    }
  }
}

/**
 * What a view gives the still hook ({@link PhotorealStills.attach}): its engine, its timer, vsync
 * period and setting, and at a request its last photorealistic frame and the label's inputs.
 */
export interface PhotorealStillView {
  readonly engine: RenderEngine;
  /** `GraphicsStatus.timer` now. */
  timer(): GpuTimer;
  /** T, the display's measured vsync period, ns. */
  frameIntervalNs(): number;
  /** The view's quality setting now. */
  setting(): QualitySetting;
  /** The last frame it drew where that was photorealistic, presented or held back; else `null`. */
  drawn(): DrawnPhotorealFrame | null;
  /**
   * The label's inputs as the view states them now: its time, camera, exposure, readings, setting,
   * decoration, survey and annunciations, all but the still's size.
   */
  request(): Omit<StillRequest, "size">;
}

/** A still's frozen scene, by its {@link StillScene}: the engine and timer its job runs on. */
interface FrozenStill {
  readonly engine: RenderEngine;
  readonly timer: GpuTimer;
  readonly frame: PhotorealFrame;
  /** Whether its plan draws a figure, which alone the samples' jitter moves. */
  readonly figures: boolean;
}

/** The engine and timer a still's device answers on: the job's, or the attached view's. */
interface StillBinding {
  readonly engine: RenderEngine;
  readonly timer: GpuTimer;
}

/** What {@link PhotorealStills} may be made with: the smoke harness's, its defaults otherwise. */
export interface PhotorealStillsOptions {
  /** The tile limit the read-back is cut at, px: the device's `maxTextureDimension2D` if absent. */
  readonly tileLimitPx?: number;
  /** The samples a still takes: {@link STILL_SAMPLES} at the frame's setting if absent. */
  readonly samples?: number;
  /** Resolves at the next animation frame: `requestAnimationFrame` if absent. */
  readonly nextFrame?: () => Promise<void>;
  /** A monotonic clock, ns: `performance.now()` if absent. */
  readonly nowNs?: () => number;
}

/** The vsync period a still is sized at before any view is attached, ns: 60 Hz. */
const UNATTACHED_FRAME_INTERVAL_NS = 1e9 / 60;

/** Whether `fault` is a loss a still in flight is abandoned at (Design note 17). */
function lossOf(fault: GraphicsFault): boolean {
  return fault.kind === "device-lost" || fault.kind === "gpu-process-gone";
}

/**
 * A still's device on R01's engine: its queue's `submittedWorkDone()` and empty submits, the
 * animation frame, the clock, the units' times the renderer hands it, and `onFault`'s losses; on the
 * engine and timer of the still in progress, frozen at its request, so that a view detached
 * meanwhile (its display hidden) leaves the still to run to its end; else on the attached view's.
 */
class ViewStillDevice implements StillDevice {
  readonly #bindingOf: () => StillBinding | null;
  readonly #passListeners: Set<(passes: ReadonlyArray<MeasuredPass>) => void>;
  readonly nextFrame: () => Promise<void>;
  readonly nowNs: () => number;

  constructor(
    bindingOf: () => StillBinding | null,
    passListeners: Set<(passes: ReadonlyArray<MeasuredPass>) => void>,
    options: PhotorealStillsOptions,
  ) {
    this.#bindingOf = bindingOf;
    this.#passListeners = passListeners;
    this.nextFrame = options.nextFrame ?? nextAnimationFrame;
    this.nowNs = options.nowNs ?? (() => performance.now() * 1e6);
  }

  /** The engine it answers on. */
  #engine(): RenderEngine {
    const binding = this.#bindingOf();
    if (binding === null) {
      throw new Error("no still runs and no view is attached to the still source");
    }
    return binding.engine;
  }

  /** `GraphicsStatus.timer` as the still in progress froze it, `absent` with neither it nor a view. */
  get timer(): GpuTimer {
    return this.#bindingOf()?.timer ?? "absent";
  }

  async submittedWorkDone(): Promise<void> {
    await this.#engine().submittedWorkDone();
  }

  submitEmpty(): void {
    this.#engine().submitEmpty();
  }

  onPassTimes(listener: (passes: ReadonlyArray<MeasuredPass>) => void): () => void {
    this.#passListeners.add(listener);
    return () => {
      this.#passListeners.delete(listener);
    };
  }

  onLoss(listener: () => void): () => void {
    return this.#engine().onFault((fault) => {
      if (lossOf(fault)) {
        listener();
      }
    });
  }
}

/** Resolves at the next animation frame. */
function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      resolve();
    });
  });
}

/**
 * Why `drawn` gives no still, a {@link STILL_UNAVAILABLE} cause, or `null` where it gives one.
 */
function unavailableFor(drawn: DrawnPhotorealFrame | null): StillUnavailableCause | null {
  if (drawn === null) {
    return STILL_UNAVAILABLE.noImage;
  }
  const { frame, plan } = drawn;
  if (frame.present === false) {
    return STILL_UNAVAILABLE.settling;
  }
  if (frame.terrain !== undefined && [...plan.regimes.values()].includes("terrain")) {
    return STILL_UNAVAILABLE.terrain;
  }
  const { widthPx, heightPx } = frame.viewport;
  return isStillSide(widthPx) && isStillSide(heightPx) ? null : STILL_UNAVAILABLE.oversized;
}

/**
 * `VIEW`'s still source (plan R11, R11.T10.b): the photorealistic renderer's hook, which a view
 * attaches to as it draws, freezing its last photorealistic frame and its label's inputs at each
 * request, and drawing it through a {@link PhotorealStillRenderer} on the view's engine.
 *
 * @remarks
 * Its device is R01's: `submittedWorkDone()` and `submitEmpty()`, the animation frame, the clock,
 * each unit's GPU time summed from `onPassTimes`' resolves under its `<pass> <tile>` label, and
 * `onFault`'s `device-lost` and `gpu-process-gone` for the loss. One view is attached at a time; a
 * still in progress keeps the engine, the timer and the frame it froze, whether its view stays
 * attached or not. A scene whose plan draws no figure takes one sample, since the jitter moves
 * only the figures and R06's sky, whose texels the band and cube filter themselves (lean,
 * 2026-11-18).
 */
export class PhotorealStills implements StillSource {
  readonly device: StillDevice;
  readonly #options: PhotorealStillsOptions;
  #view: PhotorealStillView | null = null;
  /** The still in progress, from its renderer's making to its disposal. */
  #job: FrozenStill | null = null;
  readonly #scenes = new WeakMap<StillScene, FrozenStill>();
  readonly #passListeners = new Set<(passes: ReadonlyArray<MeasuredPass>) => void>();

  constructor(options: PhotorealStillsOptions = {}) {
    this.#options = options;
    this.device = new ViewStillDevice(
      () => {
        const view = this.#view;
        return this.#job ?? (view === null ? null : { engine: view.engine, timer: view.timer() });
      },
      this.#passListeners,
      options,
    );
  }

  /**
   * Attaches `view`, whose frames each still freezes from now on, in place of any attached before.
   *
   * @returns Its detachment, which leaves a later view attached.
   */
  attach(view: PhotorealStillView): () => void {
    this.#view = view;
    return () => {
      if (this.#view === view) {
        this.#view = null;
      }
    };
  }

  /** The attached view's setting, the high setting's while none is attached. */
  get setting(): QualitySetting {
    return this.#view?.setting() ?? "high";
  }

  frameIntervalNs(): number {
    return this.#view?.frameIntervalNs() ?? UNATTACHED_FRAME_INTERVAL_NS;
  }

  /**
   * Why the attached view gives no still now, a {@link STILL_UNAVAILABLE} cause, or `null` where it
   * gives one: what `TAKE STILL` is held back for (`StillSession.refresh`).
   */
  unavailable(): StillUnavailableCause | null {
    const view = this.#view;
    return view === null ? STILL_UNAVAILABLE.noView : unavailableFor(view.drawn());
  }

  snapshot(): StillSnapshot {
    const view = this.#view;
    if (view === null) {
      return { kind: "unavailable", cause: STILL_UNAVAILABLE.noView };
    }
    const drawn = view.drawn();
    const cause = unavailableFor(drawn);
    if (drawn === null || cause !== null) {
      return { kind: "unavailable", cause: cause ?? STILL_UNAVAILABLE.noImage };
    }
    const { frame, plan } = drawn;
    const scene: StillScene = {
      request: {
        ...view.request(),
        setting: frame.setting,
        size: { widthPx: frame.viewport.widthPx, heightPx: frame.viewport.heightPx },
      },
    };
    this.#scenes.set(scene, {
      engine: view.engine,
      timer: view.timer(),
      frame,
      figures: plan.meshes.length > 0,
    });
    return { kind: "scene", scene };
  }

  /**
   * A renderer of `scene`, whose engine and timer the device answers on until it is disposed.
   *
   * @throws Error for a scene that is not one of this source's snapshots.
   */
  renderer(scene: StillScene): PhotorealStillRenderer {
    const frozen = this.#scenes.get(scene);
    if (frozen === undefined) {
      throw new Error("a still's scene is not the still source's snapshot");
    }
    const { engine, frame } = frozen;
    const made = new PhotorealStillRenderer(engine, frame, {
      tileLimitPx: this.#options.tileLimitPx ?? engine.capabilities.maxTextureDimension2D,
      samples: this.#options.samples ?? (frozen.figures ? STILL_SAMPLES[frame.setting] : 1),
      onUnitTimes: (passes) => {
        for (const listener of this.#passListeners) {
          listener(passes);
        }
      },
      onDispose: () => {
        if (this.#job === frozen) {
          this.#job = null;
        }
      },
    });
    this.#job = frozen;
    return made;
  }
}
