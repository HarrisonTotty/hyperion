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
 * surveyed ground as its reference surface does (R10.T16.f; {@link PhotorealOverlay}). Each frame
 * that carries a meter takes the scene target's exposure histogram under it (R07.T12), the veil of
 * its glare sources in it, read back for the view's `AutoExposure`
 * ({@link PhotorealRenderer.takeHistogram}); a frame of a view that
 * meters nothing, an instrument's, takes none (R07.T19.c). R08's, R10's and R11's passes join
 * when they exist. Its resources are made asynchronously, the pipelines compiled
 * before the first frame, and again after a device loss; until then {@link
 * PhotorealRenderer.render} draws nothing and says so, and the view keeps its wireframe.
 */
import type { AtmosphereLeversOverride } from "../../../../preload/api";
import type { BodyIdHex } from "@hyperion/protocol";

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
import type { DiscAir } from "../atmosphere/discReflectance";
import { gravityRatio } from "../atmosphere/oblate";
import { skySources } from "../atmosphere/sources";
import { surfaceLightingTablesOf } from "../atmosphere/surfaceLighting";
import { absorberCount, type AtmosphereTables } from "../atmosphere/tables";
import { CORRECTION_ROUTED } from "../atmosphere/thick/routing";
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
import type { LitRegime, ScreenCircle } from "../bodies/regime";
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
  TexelRect,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import type { KernelPair } from "../engine/kernels";
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
      this.#drawnColour = null;
      // Its buffer died with the device; the next frame that occludes makes it again.
      this.#pyramid?.dispose();
      this.#pyramid = null;
      // Their shared kernels died with the device; each is made again at its next frame.
      this.#releaseAtmospheres();
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
    try {
      [tonemap, overlayDepth, sprite, kernel, discCells, pyramidKernel, atmosphere] =
        await Promise.all([
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
   * @returns The frame's lit-body plan (its regimes are the next frame's `previousRegimes`), or
   *   `null` while the resources are being made, when nothing was drawn.
   */
  render(view: RenderView, frame: PhotorealFrame): BodyFramePlan | null {
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
    const projection = perspectiveReversedInfinite(
      camera.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    );
    const viewRotation = viewRotation4(camera.orientation);
    // Each body whose air is drawn lights its surface through it, and is drawn with depth, so that
    // its composite lays the air over its ground (R08.T10.b).
    const atmospheres = frame.atmospheres ?? [];
    const bodies = withAtmospheres(
      frame.bodies,
      atmospheres,
      SETTINGS[frame.setting].atmosphereLevers.sourceTransmittance,
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
      { bodies, shellRadiiM },
      {
        viewRotation,
        projection,
      },
    );
    this.#drawnColour = hdrColour;
    // The glare sources are written before the histogram is dispatched, which weighs their veil
    // as the tone-mapping pass draws it.
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
    if (frame.meter !== null) {
      resources.histograms.measure({
        hdrColour,
        size: resources.size,
        mode: frame.meter,
        stride: settings.histogramStride,
        preExposure: frame.exposureScale,
        glare: {
          sources: resources.glare,
          count: frame.glareSources.length,
          viewRotation,
          projection,
          terms,
        },
      });
    }
    if (frame.present === false) {
      // Held back: no frame of a cut is shown at the exposure from before it.
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
            sourceCount: frame.glareSources.length,
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
    // A cut: the next frame's two-pass occlusion keeps nothing of this one's visibility (R14.T3.d).
    this.#cut = true;
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
      try {
        const atmosphere = this.#atmosphereOf(
          resources,
          air,
          frame.atmosphereSetting ?? frame.setting,
          frame.atmosphereLevers,
        );
        const suns: ReadonlyArray<SunState> = skySources(
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
      this.#engine.releaseMaterial(resources.overlayDepth);
      this.#engine.releaseMesh(resources.atmosphere.triangle);
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
