/**
 * Lit bodies as points and discs in the photorealistic style's HDR target (plan R07, T8.a; Design
 * notes 1, 2, 4, 6, 10 and 19).
 *
 * @remarks
 * {@link planLitBodies} is pure: each body's regime (`litRegimes`), the painter's sequence by power
 * (`painterOrder`), and per body the stars that light it (the brightest two past
 * `STAR_CUT_RELATIVE`), their illuminance at it (`starIlluminance`), their annuli per channel
 * (`hostAnnuli`) and the bodies that may eclipse them (`occludersFor`, the two largest). A disc
 * becomes a {@link DiscRecord}; a point a sprite of flux F = E p (a ÷ Δ)² Φ(α) per channel ({@link
 * pointFlux}, the law integrated over the figure for a spheroid), cut by the same eclipse averaged
 * over the disc it would draw (`discEclipseVisible`, T10.b), laid into R02's point-spread sprite.
 * Each body is also lit by planetshine from the neighbours that light it most
 * (`planetshineSources`, T11), on its disc per lit point and on its point as a point source, each
 * neighbour's starlight cut by its own eclipse as the body sees it (T10.b). A body that carries its
 * lighting frame (`lightingFrameOf`, T10.a)
 * takes its stars, occluders and neighbours from it, retarded to the light that reaches it, while
 * the drawing keeps the apparent places. Host discs keep their
 * place in the order, where R06.T13.e's pass draws them (decision-r07-t8a, R06 coordination (d),
 * 2026-10-03): a host's step places R06's disc draw. {@link LitBodyRenderer} binds the plan to the
 * engine: one draw per disc for its wholly covered pixels and one for its limb, one sprite draw per
 * run of consecutive points through R06's HDR sprite (`POINT SPRITES HDR`), and before them one
 * dispatch of the `disc cells` pass, which shades every cell of the discs under 32 px at once for
 * their draws to read (R07.T8.d, {@link BODY_DISC_CELLS_KERNEL}). A disc shades from its
 * `DiscSurface` (T8.b): its photometry's uniform law, or R10's class map with a law per class; a
 * point keeps the photometry's law, or integrates the class map where the rotation is known
 * (R10.T10.f).
 *
 * A disc whose footprint overlaps geometry that writes depth (a mesh body, R10's terrain, a craft,
 * given as {@link BodyFrameOptions.depthWriters}) is promoted to the mesh regime for the frame
 * (Design note 2, `promoteOverlapping`, T9): its smooth figure (`smoothMesh.ts`) draws its wholly
 * covered pixels with depth in the `bodies` pass, before the painter's sequence, and its limb is the
 * disc's limb draw at its place in the sequence, on the limb's plane. Both shade from its disc
 * record, so a promoted disc and its mesh draw the same light.
 *
 * A disc whose relief subtends the photorealistic style's τ at its limb (plan R10, R10.T10.e; R10
 * Design note 13) is handed over to terrain (`terrain/handover.ts`' `handOverRegimes`), before
 * the promotion: its regime is `terrain`, drawn as a mesh body is, its figure and its limb the
 * reference surface beneath its terrain, discarded where its survey's coverage mask holds the
 * ground surveyed (R10.T10.c), and its terrain the view's own (`PhotorealFrame.terrain`); a view
 * that draws none draws its figure whole, as a mesh body's. The hand-over counts the pixels the
 * view's terrain selects at ({@link BodyFrameOptions.selectionPx}, the presented size's CSS px;
 * R10.T10.g), R07's 3 px switch the frame's own.
 *
 * A class-map disc whose surface carries the field's normal map (plan R10, R10.T10.g) shades its
 * surveyed ground with those normals, in its draws and in the point it switches with: its records'
 * flag (`packDiscRecords`' row 24) and the normal map's texture at binding 14 (`surfaceNormals`).
 *
 * A body given its ground's temperature field ({@link LitBodyInput.thermal}, plan R08, R08.T18.a)
 * above R08's visibility cut has its record carry its emission (`discReflectance.ts`'
 * `discEmissionOf`), and its disc, its mesh and its cells are drawn by the kernels' emissive forms
 * ({@link BODY_DISC_EMISSIVE_MATERIALS}, {@link SMOOTH_MESH_EMISSIVE_MATERIAL},
 * {@link BODY_DISC_CELLS_EMISSIVE_KERNEL}), made at the first such record; every other body is
 * drawn by the plain kernels, as before.
 *
 * A body given its atmosphere's disc table ({@link LitBodyInput.air}, plan R08, R08.T16.b) is drawn
 * by it beyond `GAS_GIANT_FULL_PASS_BOUNDARY_M` (`regime.ts`' `drawnByTable`, with R07's
 * `REGIME_HYSTERESIS`), once the air forms are prepared ({@link BodyFrameOptions.drawsAir}): its
 * record carries its air, it is drawn as a disc (not as a mesh for its air's composite), and the
 * plan names it among {@link BodyFramePlan.tabled}, whose full passes the renderer draws none of.
 * Its disc, its mesh and its cells are drawn by the kernels' air forms
 * ({@link BODY_DISC_AIR_MATERIALS}, {@link SMOOTH_MESH_AIR_MATERIAL},
 * {@link BODY_DISC_CELLS_AIR_KERNEL}), the table at group 2's binding 15.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { DiscSurface } from "../appearance/bodyAppearance";
import type { BodyFigure, BodyPhotometry } from "../appearance/fromWire";
import type { ClusterBuckets } from "../clusters/buckets";
import { type ClusterConsumer, ClusterConsumerPath } from "../clusters/consumer";
import { CLUSTER_CULL_KERNEL } from "../clusters/cull";
import { type CullPass, MAX_CULL_PASSES } from "../clusters/cullPass";
import { CLUSTER_SHAPE_CODE } from "../clusters/record";
import { smoothFigureClusterMesh } from "../clusters/templates";
import { TERRAIN_BLOCKS, TERRAIN_CLUSTERS_PER_PAGE } from "../clusters/terrain";
import { type GeometrySettings, type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import { coverageMaskTextureSpec } from "../terrain/coverage";
import { frustumOf } from "../terrain/cull";
import { patchMeshData } from "../terrain/gpu/resources";
import { INSTANCE_RECORD_BYTES } from "../terrain/gpu/uniforms";
import {
  type HandoverSphere,
  handOverRegimes,
  handoverTauPx,
  pixelAngleRad,
} from "../terrain/handover";
import patchVertexWgsl from "../terrain/shaders/patchVertex.wgsl?raw";
import { limbShareAt, PHASE_TABLE_SAMPLES, type PhotometricLaw } from "../appearance/law";
import { packPhaseFactorRows } from "../appearance/litBodyProbe";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import { EngineUnavailable } from "../engine/resilientEngine";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  PreparedTarget,
  RenderEngine,
  TextureBindingSpec,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import {
  type DiscAir,
  discEmissionOf,
  withDiscAir,
  withDiscEmission,
  withDiscEmissionAndAir,
} from "../atmosphere/discReflectance";
import type { SurfaceTemperatureField } from "../atmosphere/thermal";
import {
  createAirlessTables,
  SURFACE_LIGHTING_WGSL,
  surfaceLightingDrawTextures,
  surfaceLightingKernelTextures,
  type SurfaceLightingTables,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_ROTATION, type Rotation3, rotateToBody } from "../coords/rotation";
import { discEclipseVisible } from "../lighting/discEclipse";
import { hostAnnuli, type LightAtPoint, lightsAt, type PlacedLight } from "../lighting/hostLights";
import { type LightingBody, type LightingSphere, occludersFor } from "../lighting/occluders";
import type { LightingFrame } from "../lighting/retarded";
import {
  type LitNeighbour,
  litNeighbours,
  planetshineSources,
  type SecondarySource,
} from "../lighting/planetshine";
import { PSF_QUAD_PX } from "../photometry/magnitude";
import { type SpriteRecord, spriteRecord } from "../wireframe/drawList";
import { HDR_COLOUR_FORMAT, type Rgb } from "../photometry/toneCurve";
import frameWgsl from "../shaders/frame.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import cubeTexelWgsl from "../shaders/cubeTexel.wgsl?raw";
import maturityNodesWgsl from "../shaders/maturityNodes.wgsl?raw";
import bodyDiscCellsWgsl from "../shaders/bodyDiscCells.wgsl?raw";
import bodyDiscDrawWgsl from "../shaders/bodyDiscDraw.wgsl?raw";
import smoothMeshWgsl from "../shaders/smoothMesh.wgsl?raw";
import smoothMeshClusterWgsl from "../shaders/smoothMeshCluster.wgsl?raw";
import { sphereOutsideView, sphereScreenRect, WIREFRAME_MESHES } from "../wireframe/submit";
import {
  type DiscCellJobs,
  discCellJobs,
  type DiscLight,
  type DiscOccluder,
  type DiscRecord,
  type DiscSecondary,
  discSamples,
  type DrawnDiscSurface,
  MAX_DISC_LIGHTS,
  MAX_DISC_OCCLUDERS,
  packDiscRecords,
} from "./discShading";
import {
  classMapPointReflection,
  nearClassMapPointSwitch,
  warmClassMapPoint,
} from "./classMapPoint";
import { CLASS_MAP_FORMAT, discSurfaceLaws } from "./discSurface";
import { bodyReflection, figurePole } from "./oblate";
import { type HostSphere, type PainterEntry, painterOrder } from "./painter";
import {
  drawnAsMesh,
  drawnByTable,
  type LitRegime,
  litRegimes,
  promoteOverlapping,
  type ScreenCircle,
  sphereFootprint,
} from "./regime";
import {
  DISC_LIMB_DEPTHS,
  type LimbDepths,
  limbDepths,
  packSmoothClusterRecords,
  packSmoothMeshes,
  rotationColumns,
  type SmoothMesh,
  smoothMeshOf,
} from "./smoothMesh";
import { angularDiameterPx } from "../wireframe/bodies";

/** A lit body of the frame. */
export interface LitBodyInput {
  readonly id: BodyIdHex;
  /** Its centre from the camera, m along the galactic axes, in `f64`. */
  readonly centreM: Vec3;
  /** Its figure; a disc under a class map is drawn about the rotation's z axis instead of its pole. */
  readonly figure: BodyFigure;
  /** Its photometry, which its point and, without a `surface`, its disc shade with. */
  readonly photometry: BodyPhotometry;
  /** What its disc shades with (Design note 24); absent, the photometry's uniform law. */
  readonly surface?: DiscSurface;
  /**
   * Its rotation from body-fixed to galactic axes, which orients a class map; absent where it is
   * not known, when a class map's disc shades with its `elsewhere` law.
   */
  readonly rotation?: Rotation3;
  /**
   * Its stars and the other lit bodies where its light finds them, retarded (`lightingFrameOf`,
   * T10.a): what lights, eclipses and planetshines it. `undefined` states a static scene's
   * geometry: the frame's hosts and the other bodies where they are drawn.
   */
  readonly lighting: LightingFrame | undefined;
  /**
   * Its atmosphere's tables, through which its disc's light crosses the air (plan R08, R08.T9.b;
   * `surfaceLighting.wgsl`): each star's and planetshine source's transmittance and the sky's
   * irradiance. Absent, a body without one, whose draws bind the one-texel defaults. Until R08.T10.b
   * draws a body's atmosphere in the view, no caller gives one.
   */
  readonly atmosphere?: SurfaceLightingTables;
  /**
   * Its ground's temperature field (plan R08, R08.T18.a; `thermal.ts`' `groundTemperatureFieldOf`),
   * for a body drawn with no medium whose record gives one; above the visibility cut its disc adds
   * the light its ground emits (`discReflectance.ts`). Absent, its disc reflects alone.
   */
  readonly thermal?: SurfaceTemperatureField;
  /**
   * Its atmosphere's disc table (plan R08, R08.T16.b; `AtmosphereTables.discReflectance`), by which
   * its disc is drawn beyond the 10⁹ m boundary in place of its air's full passes; absent before
   * its bake lands, or for a body without air.
   */
  readonly air?: DiscAir;
  /**
   * The GPU's coverage mask of its survey (plan R10, R10.T10.c; `terrain/coverage.ts`'
   * `CoverageMask.texture`) with the axes it is read in, given by a view that draws its terrain,
   * with which its figure and its limb, once it is handed over (`terrain`), are the reference
   * surface beneath that terrain: discarded wherever the mask holds a cell surveyed, so that the
   * terrain alone draws that ground (R10's Design note 12). Read by no other regime; absent, the
   * figure and the limb are drawn whole.
   */
  readonly surveyCoverage?: MeshBodyCoverage;
  /**
   * Its relief for the hand-over to terrain (plan R10, R10.T10.e; `terrain/handover.ts`'
   * `handoverReliefM`: R09's realised relief, m, where its field's header is held and its survey
   * covers ground); absent, it never hands over.
   */
  readonly handoverReliefM?: number;
}

/** A mesh body's coverage mask and the body-fixed axes it is read in (R10.T10.c). */
export interface MeshBodyCoverage {
  readonly mask: TextureHandle;
  /** The body's rotation, body-fixed to galactic axes. */
  readonly rotation: Rotation3;
  /**
   * Whether the mask holds every cell surveyed (`CoverageMask.whole`), when the figure and the limb
   * discard every pixel and the `disc cells` pass shades none of the body's (R07.T8.h).
   */
  readonly whole: boolean;
}

/** What the plan needs of the view. */
export interface BodyFrameOptions {
  readonly camera: ProjectionCamera;
  /** The frame's own viewport, device px: the scene target's, at the frame's internal scale. */
  readonly viewport: Viewport;
  /**
   * The size the view is presented at, device px, its canvas's, whose centre pixel the hand-over to
   * terrain measures (plan R10, R10.T10.g; decision-r10-t16-leans §13), so that the resolution
   * controller's scale never moves it: `viewport` itself where the frame is drawn at the presented
   * size. R07's 3 px switch keeps `viewport`.
   */
  readonly presentedPx: Viewport;
  /**
   * The size the view's terrain selects at, whose centre pixel the hand-over measures in its place:
   * {@link BodyFrameOptions.presentedPx} ÷ max(1, the display's device-pixel ratio), its CSS px
   * (`selectionViewport`; lean 2026-11-01), so that the hand-over and the field's want stay
   * together; absent, `presentedPx`, as at a ratio of 1.
   */
  readonly selectionPx?: Viewport;
  /** The exposure scale the target is pre-exposed with (`exposureScale`), 1 ÷ (cd/m²). */
  readonly exposureScale: number;
  /** The eclipse term's annuli: `DISC_ANNULI_HIGH` or `DISC_ANNULI_LOW`. */
  readonly annuli: number;
  /**
   * The neighbours that light each body by planetshine at most: `PLANETSHINE_SOURCES_HIGH`, or
   * `PLANETSHINE_SOURCES_LOW` on the low setting.
   */
  readonly planetshine: number;
  /**
   * The footprints of the view's other geometry that writes depth (R10's terrain, a lit craft): a
   * disc overlapping one, or a mesh body, is drawn as a mesh (Design note 2). Absent, none.
   */
  readonly depthWriters?: ReadonlyArray<ScreenCircle>;
  /**
   * The bodies drawn with depth whenever they are drawn larger than a point, each with the radius
   * of its air's shell, m: those whose atmosphere the frame draws (plan R08, R08.T10.b), whose
   * composite lays the air over the ground by its depth and would otherwise draw its own bare
   * ground over a disc's pixels. A disc among them is drawn as a mesh, and promotes the discs it
   * overlaps as a mesh does, its footprint its shell's circle, over which its composite draws
   * (`decision-r08-t10b-leans.md` §3). Absent, none.
   */
  readonly depthBodyShellRadiiM?: ReadonlyMap<BodyIdHex, number>;
  /** The view's quality setting, which a mesh body's selection takes. */
  readonly setting: QualitySetting;
  /**
   * Whether the air forms are prepared to draw a body by its disc table (plan R08, R08.T16.b): while
   * they are not, every body with air keeps its full passes at every distance. Absent, `false`.
   */
  readonly drawsAir?: boolean;
  /**
   * The bodies drawn by their disc tables on the previous frame ({@link BodyFramePlan.tabled}), for
   * the boundary's hysteresis. Absent, none.
   */
  readonly previousTabled?: ReadonlySet<BodyIdHex>;
  /**
   * The cells per axis a disc of 32 px or more takes in its limb pixels (`discSamples`):
   * `PhotorealSettings.floorLimbSamples` while the view's frame-rate floor stands at its lowest
   * step (R07.T8.i). Absent, `LIMB_SAMPLES`.
   */
  readonly limbSamples?: number;
}

/** A body drawn as a mesh this frame (T9). */
export interface MeshBodyPlan {
  /** Its record in {@link BodyFramePlan.discs}, which its figure's pixels and its limb shade with. */
  readonly index: number;
  /** Its smooth figure on the view. */
  readonly mesh: SmoothMesh;
  /** Its limb draw's corner depths, on the limb's plane. */
  readonly limbDepths: LimbDepths;
  /** How it came to be drawn as a mesh, and with it what its figure leaves to its terrain. */
  readonly role: MeshBodyRole;
}

/**
 * Why a body is drawn as a mesh (T9; plan R10, R10.T10.e): `promoted` by its overlaps alone
 * (`mesh`), its figure drawn whole, or `handed-over` to terrain (`terrain`), whose ground R05's
 * quadtree draws over its figure, with its survey's coverage mask, which discards its figure where
 * the terrain draws the ground (R10.T10.c), or `null` where the view gives it none.
 */
export type MeshBodyRole =
  | { readonly kind: "promoted" }
  | { readonly kind: "handed-over"; readonly coverage: MeshBodyCoverage | null };

/**
 * The coverage mask that discards a mesh body's figure and its limb: a handed-over body's, a
 * promoted one none.
 *
 * @remarks
 * Over the image its datum gives way on the same mask (plan R10, R10.T16.f; `surveyedDatumsOf`).
 */
export function coverageOf(role: MeshBodyRole): MeshBodyCoverage | null {
  let coverage: MeshBodyCoverage | null;
  switch (role.kind) {
    case "promoted":
      coverage = null;
      break;
    case "handed-over":
      coverage = role.coverage;
      break;
  }
  return coverage;
}

/** A point body's sprite: R02's sprite record, its depth its own (decision-r07-t8a, item 2). */
export interface PointSprite {
  readonly id: BodyIdHex;
  readonly record: SpriteRecord;
}

/** One step of the painter's sequence as it is drawn. */
export type BodyStep =
  | { readonly kind: "disc"; readonly index: number }
  | { readonly kind: "points"; readonly sprites: ReadonlyArray<PointSprite> }
  /** A host star's place in the order, where R06's disc pass draws it (`HostDiscDto.star`). */
  | { readonly kind: "host"; readonly star: number }
  /** A mesh body's limb, `meshes[mesh]`'s, at its place in the order (T9). */
  | { readonly kind: "limb"; readonly mesh: number };

/** One frame's lit bodies, ready to bind. */
export interface BodyFramePlan {
  readonly regimes: ReadonlyMap<BodyIdHex, LitRegime>;
  /** The painter's sequence the steps follow, a mesh body in it for its limb. */
  readonly order: ReadonlyArray<PainterEntry>;
  /** Each disc's record, and each mesh body's, which its figure and its limb shade with. */
  readonly discs: ReadonlyArray<DiscRecord>;
  /** The bodies drawn as meshes, in the order's. */
  readonly meshes: ReadonlyArray<MeshBodyPlan>;
  /** The phase table's rows, the frame's distinct laws in first use, a class map's included. */
  readonly laws: ReadonlyArray<PhotometricLaw>;
  readonly steps: ReadonlyArray<BodyStep>;
  /**
   * The sources that light each body drawn as a disc or a mesh, by body: the arrays its record was
   * shaded from, which its atmosphere's sky takes as they are (plan R08, Design note 7's one list;
   * R08.T10.b), never ranked again.
   */
  readonly sources: ReadonlyMap<BodyIdHex, BodyLightSources>;
  /**
   * The bodies drawn by their disc tables beyond the 10⁹ m boundary (plan R08, R08.T16.b), each
   * with a record that carries its air: their air's full passes are not drawn. The next frame's
   * {@link BodyFrameOptions.previousTabled}.
   */
  readonly tabled: ReadonlySet<BodyIdHex>;
}

/**
 * The sources that light a body in a frame (plan R08, Design note 7's one list): its stars, R07's
 * `lightsAt` past `STAR_CUT_RELATIVE`, brightest first, at most {@link MAX_DISC_LIGHTS}, each
 * `toStarM` from its centre; and its planetshine sources (`planetshineSources`, at the setting's
 * count), their directions along the same axes.
 */
export interface BodyLightSources {
  readonly stars: ReadonlyArray<LightAtPoint>;
  readonly planetshine: ReadonlyArray<SecondarySource>;
}

/**
 * The sources that light `body` in a frame of `bodies`, as {@link planLitBodies} builds them for a
 * body it draws: for one it holds no record of, the camera's local body while terrain draws it
 * from inside its figure's sphere (plan R08, R08.T10.b), so that its air is lit as its surface is.
 *
 * @param bodies - The frame's lit bodies, which light it by planetshine.
 * @param hosts - The lights where they are drawn, for a body without its own lighting frame.
 */
export function bodyLightSources(
  body: LitBodyInput,
  bodies: ReadonlyArray<LitBodyInput>,
  hosts: ReadonlyArray<PlacedLight>,
  options: Pick<BodyFrameOptions, "annuli" | "planetshine">,
): BodyLightSources {
  return {
    stars: lightsOf(body, hosts),
    planetshine: planetshineSources(
      body,
      litNeighbours(bodies, hosts, options.annuli),
      options.planetshine,
    ),
  };
}

/** No body drawn with depth for its air. */
const NO_DEPTH_BODY_SHELLS: ReadonlyMap<BodyIdHex, number> = new Map();

/** No body. */
const NO_BODIES: ReadonlySet<BodyIdHex> = new Set();

/** A body whose centre is within this fraction of its radius is seen from inside, and not drawn. */
const INSIDE_MARGIN = 1e-9;

/**
 * The stars that light a body: past the cut, the brightest {@link MAX_DISC_LIGHTS}, from its
 * lighting frame where it has one.
 */
function lightsOf(body: LitBodyInput, hosts: ReadonlyArray<PlacedLight>): LightAtPoint[] {
  return lightsAt(body.centreM, body.lighting?.lights ?? hosts, MAX_DISC_LIGHTS);
}

/**
 * The bodies that may eclipse a body's lights, the largest seen from it first, at most two: of its
 * lighting frame's occluders where it has one, else of `lighting`, the bodies where they are drawn.
 */
function occludersOf(
  body: LitBodyInput,
  lights: ReadonlyArray<LightAtPoint>,
  lighting: ReadonlyArray<LightingBody>,
): LightingBody[] {
  const self: LightingBody = {
    id: body.id,
    centreM: body.centreM,
    radiusM: body.figure.equatorialRadiusM,
  };
  const stars: LightingSphere[] = lights.map((light) => ({
    centreM: light.host.centreM,
    radiusM: light.host.disc.radius_m,
  }));
  const angular = (other: LightingBody): number =>
    other.radiusM / norm(sub(other.centreM, body.centreM));
  return occludersFor(self, stars, body.lighting?.occluders ?? lighting)
    .toSorted((a, b) => angular(b) - angular(a) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .slice(0, MAX_DISC_OCCLUDERS);
}

/**
 * A point body's flux at the camera per display channel, lx: E p (a ÷ Δ)² Φ(α) for a sphere; for a
 * spheroid the law integrated over its figure, (E ÷ π)(a ÷ Δ)² A′ f(α) K (`bodyReflection`, A′ the
 * disc's scaled A), so that the point and the disc draw one flux. Under a class map, where the
 * body's rotation is known, each light's term is the map integrated over the figure by the disc's
 * own arithmetic (`classMapPointReflection`, plan R10, R10.T10.f), on its normal map's slopes where
 * it carries one (R10.T10.g), so that the point meets the patterned disc it switches with;
 * otherwise it is the photometry's law, which equals the map's `elsewhere`. Each star's light is
 * cut by its eclipse over the disc the camera sees
 * (`discEclipseVisible`, T10.b), the share of the light the disc regime's pixels sum, so that the
 * flux stays continuous at the 3 px switch through an eclipse; it takes the photometry's limb
 * share under a class map too. Each planetshine source adds its own term, as a point source at its
 * centre's direction and never eclipsed (Design note 7).
 *
 * @param hosts - The lights where they are drawn, for a body without its own lighting frame.
 * @param occluders - The bodies that may eclipse its stars (`occludersFor`).
 * @param k - The eclipse term's annuli.
 * @param secondaries - Its planetshine sources (`planetshineSources`).
 */
export function pointFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  k: number,
  secondaries: ReadonlyArray<SecondarySource> = [],
): Rgb {
  const { law } = body.photometry;
  const distanceM = norm(body.centreM);
  const toCamera = scale(body.centreM, -1 / distanceM);
  const solid = (body.figure.equatorialRadiusM / distanceM) ** 2;
  const blockers = occluders.map(({ centreM, radiusM }) => ({ centreM, radiusM }));
  const reflection = pointReflectionOf(body, toCamera);
  const flux: [number, number, number] = [0, 0, 0];
  for (const light of lightsOf(body, hosts)) {
    const reflected = reflection(normalise(light.toStarM), starLightKey(light));
    const visible = discEclipseVisible(
      light.host,
      body,
      blockers,
      toCamera,
      (alpha) => limbShareAt(law, alpha),
      k,
    );
    for (const ch of [0, 1, 2] as const) {
      flux[ch] += light.illuminance[ch] * solid * reflected[ch] * visible[ch];
    }
  }
  for (const source of secondaries) {
    const reflected = reflection(source.direction, source.body);
    for (const ch of [0, 1, 2] as const) {
      flux[ch] += source.illuminance[ch] * solid * reflected[ch];
    }
  }
  return flux;
}

/** A star's key among a body's lights, by which a class map's held ratios are its own. */
function starLightKey(light: LightAtPoint): string {
  return `star ${String(light.host.disc.star)}`;
}

/**
 * Begins, in the frames' slices, the point's ratio of each light of a body drawn as a disc near the
 * 3 px switch under a class map (plan R10, R10.T14.f's deferred lever; `warmClassMapPoint`), from
 * the geometry its point would take (`pointFlux`'s lights, directions and keys), so that its first
 * frame as a point finds each light's ratio held; and notes its map near the switch, so that the
 * slices step its build. Nothing for a body without a class map, a known rotation, or the band.
 */
function warmPointOf(
  body: LitBodyInput,
  lit: BodyLightSources,
  camera: ProjectionCamera,
  viewport: Viewport,
): void {
  const { surface, rotation } = body;
  if (
    surface?.kind !== "class-map" ||
    rotation === undefined ||
    !nearClassMapPointSwitch(
      angularDiameterPx(body.centreM, body.figure.equatorialRadiusM, camera, viewport),
    )
  ) {
    return;
  }
  const toCamera = scale(body.centreM, -1 / norm(body.centreM));
  for (const light of lit.stars) {
    warmClassMapPoint(
      body.figure,
      surface,
      rotation,
      normalise(light.toStarM),
      toCamera,
      starLightKey(light),
    );
  }
  for (const source of lit.planetshine) {
    warmClassMapPoint(body.figure, surface, rotation, source.direction, toCamera, source.body);
  }
}

/**
 * What a point body reflects of a light from its direction towards the camera, uneclipsed: its
 * class map integrated (`classMapPointReflection`, R10.T10.f) where its surface is one and its
 * rotation is known, and otherwise its photometry's law over its figure (`bodyReflection`). The
 * frame never makes a map's elements: until the frames' slices have, the point is the map's stand-in
 * (the map it replaced) or the uniform point, as before the map came (R10.T14.f). Each light is
 * named by its key, a star's index or a planetshine source's body, by which the map's held ratios
 * are its own.
 */
function pointReflectionOf(
  body: LitBodyInput,
  toCamera: Vec3,
): (towardsLight: Vec3, lightKey: string) => Rgb {
  const { surface, rotation } = body;
  if (surface?.kind === "class-map" && rotation !== undefined) {
    return (towardsLight, lightKey) =>
      classMapPointReflection(body.figure, surface, rotation, towardsLight, toCamera, lightKey);
  }
  return (towardsLight) => bodyReflection(body.figure, body.photometry.law, towardsLight, toCamera);
}

/**
 * The surface a body's disc shades with: its own, or its photometry's uniform law; a class map
 * whose body has no known rotation cannot be oriented, and shades with its `elsewhere` law.
 */
function drawnSurface(body: LitBodyInput): DrawnDiscSurface {
  const surface = body.surface ?? { kind: "uniform", law: body.photometry.law };
  let drawn: DrawnDiscSurface;
  switch (surface.kind) {
    case "uniform":
      drawn = surface;
      break;
    case "class-map":
      drawn =
        body.rotation === undefined
          ? { kind: "uniform", law: surface.elsewhere }
          : { ...surface, rotation: body.rotation };
      break;
  }
  return drawn;
}

/** The disc record of a body drawn as a disc, or `null` where it is off the view or about the camera. */
function discRecordOf(
  body: LitBodyInput,
  surface: DrawnDiscSurface,
  sources: BodyLightSources,
  occluders: ReadonlyArray<LightingBody>,
  tableRows: ReadonlyArray<number>,
  options: BodyFrameOptions,
  air: DiscAir | undefined,
): DiscRecord | null {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const distanceM = norm(body.centreM);
  if (!(distanceM > a * (1 + INSIDE_MARGIN))) {
    return null;
  }
  // A body wholly off the view draws no pixel. Behind the camera or across its plane its rectangle
  // is the whole view, which its two draws would cover.
  if (sphereOutsideView(body.centreM, a, options.camera, options.viewport)) {
    return null;
  }
  const rect = sphereScreenRect(body.centreM, a, options.camera, options.viewport);
  if (rect === null) {
    return null;
  }
  const direction = scale(body.centreM, 1 / distanceM);
  // A class map is oriented by the rotation, whose z axis is then the pole the figure is drawn about.
  const pole =
    surface.kind === "class-map"
      ? rotateToBody(surface.rotation, vec3(0, 0, 1))
      : figurePole(body.figure);
  const aOverD = a / distanceM;
  const samples = discSamples(
    angularDiameterPx(body.centreM, a, options.camera, options.viewport),
    options.limbSamples,
  );
  const lights: DiscLight[] = sources.stars.map((light) => {
    const d = norm(light.toStarM);
    return {
      direction: scale(light.toStarM, 1 / d),
      distance: d / a,
      radius: light.host.disc.radius_m / a,
      illuminance: light.illuminance,
      annuli: hostAnnuli(light.host.disc, options.annuli),
    };
  });
  const blockers: DiscOccluder[] = occluders.map((occluder) => ({
    centre: scale(sub(occluder.centreM, body.centreM), 1 / a),
    radius: occluder.radiusM / a,
  }));
  const shine: DiscSecondary[] = sources.planetshine.map((source) => ({
    direction: source.direction,
    distance: source.distanceM / a,
    radius: source.radiusM / a,
    illuminance: source.illuminance,
  }));
  // Its ground's emission above the cut (R08.T18.a), about its brightest star, beside its air's
  // table where it is drawn by it (R08.T18.b), whose third form dims it by the air.
  const emission =
    body.thermal === undefined
      ? null
      : discEmissionOf(
          body.thermal,
          discSurfaceLaws(surface).map((law) => law.a),
          lights.map((light) => light.illuminance),
          lights[0]?.direction ?? vec3(0, 0, 0),
          surface.kind === "class-map" ? (surface.meltClasses ?? []) : [],
        );
  return {
    body: body.id,
    rect,
    direction,
    radiusOverDistance: aOverD,
    pole,
    polarOverEquatorial: c / a,
    interiorSamples: samples.interior,
    limbSamples: samples.limb,
    surface,
    tableRows,
    exposureOverPi: options.exposureScale / Math.PI,
    lights,
    occluders: blockers,
    secondaries: shine,
    ...(body.atmosphere === undefined ? {} : { atmosphere: body.atmosphere }),
    ...(emission === null ? {} : { emission }),
    ...(air === undefined ? {} : { air }),
  };
}

/** How far outside the view a point's sprite may fall and still light it: half its quad, px. */
const SPRITE_MARGIN_PX = Math.ceil(PSF_QUAD_PX / 2);

/** A point body's sprite, or `null` where it falls off the view or behind the camera. */
function pointSpriteOf(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  neighbours: ReadonlyArray<LitNeighbour>,
  options: BodyFrameOptions,
): PointSprite | null {
  const { camera, viewport } = options;
  const p = project(body.centreM, camera, viewport);
  if (
    !p.inFront ||
    p.xPx < -SPRITE_MARGIN_PX ||
    p.yPx < -SPRITE_MARGIN_PX ||
    p.xPx > viewport.widthPx + SPRITE_MARGIN_PX ||
    p.yPx > viewport.heightPx + SPRITE_MARGIN_PX
  ) {
    return null;
  }
  const secondaries = planetshineSources(body, neighbours, options.planetshine);
  const flux = pointFlux(body, hosts, occluders, options.annuli, secondaries);
  return {
    id: body.id,
    record: spriteRecord(
      { xPx: p.xPx, yPx: p.yPx, depth: p.depth },
      flux,
      options.exposureScale,
      body.centreM,
      camera,
      viewport,
    ),
  };
}

/**
 * The frame's lit bodies: regimes, the painter's sequence, disc records, smooth figures and point
 * sprites.
 *
 * @remarks
 * A disc whose relief subtends the photorealistic style's τ on `options.setting` (R10's
 * `handoverTauPx`) at its limb, in the centre pixels of the size the view's terrain selects at
 * (`options.selectionPx`, the presented size's CSS px; plan R10, R10.T10.g), is handed over to
 * terrain (`handOverRegimes`, plan R10, R10.T10.e). The two switches now count different pixels:
 * R07's 3 px switch (`litRegimes`) the frame's own (`options.viewport`), in which the disc's summed
 * flux meets the point's, and the hand-over the selection's, so that the resolution controller's
 * internal scale, which moves the first, never moves the second (decision-r10-t16-leans §13). A
 * disc whose footprint overlaps one of `options.depthWriters`, a mesh body or a body handed over is
 * promoted to a mesh (`promoteOverlapping`). A mesh body's limb, a handed-over body's included, is
 * drawn in the sequence as a disc's is, so the order takes it as a disc (Design note 2); its
 * figure's pixels are drawn before the sequence, with depth. The order is the drawing's, by the
 * drawn centres; each body is lit by its own lighting frame where it has one (T10.a).
 *
 * Under a class map the plan has two side effects on the point's module-wide work (plan R10,
 * R10.T14.f's levers; `classMapPoint.ts`): a point it draws queues its map's build or a refresh
 * where they are missing, and a disc it draws within `CLASS_MAP_POINT_BAND` × 3 px of the frame's
 * own pixels queues its lights' refreshes (`warmClassMapPoint`); either notes its map near the
 * switch, so that the terrain session's slices step them. A caller with no session (the smoke
 * page) steps none of that work.
 *
 * @param hosts - The host stars where they are drawn, which keep their place in the order and
 *   light each body that has no lighting frame of its own (`LitBodyInput.lighting`).
 * @param previous - Each body's regime on the previous frame, for the hysteresis.
 */
export function planLitBodies(
  bodies: ReadonlyArray<LitBodyInput>,
  hosts: ReadonlyArray<PlacedLight>,
  options: BodyFrameOptions,
  previous: ReadonlyMap<BodyIdHex, LitRegime>,
): BodyFramePlan {
  const spheres: HandoverSphere[] = bodies.map((body) => ({
    id: body.id,
    centreM: body.centreM,
    radiusM: body.figure.equatorialRadiusM,
    reliefM: body.handoverReliefM,
  }));
  const { camera, viewport } = options;
  // A body with its disc table beyond the 10⁹ m boundary, the air forms prepared, is drawn by its
  // table (R08.T16.b), with R07's hysteresis about the boundary.
  const wasTabled = options.previousTabled ?? NO_BODIES;
  const byTable = new Set<BodyIdHex>();
  if (options.drawsAir === true) {
    for (const body of bodies) {
      if (body.air !== undefined && drawnByTable(norm(body.centreM), wasTabled.has(body.id))) {
        byTable.add(body.id);
      }
    }
  }
  // A body whose air is drawn is drawn with depth (R08.T10.b), a disc of it as a mesh, and its
  // footprint is its air's shell, which its composite draws over; one drawn by its table is a disc.
  const givenShells = options.depthBodyShellRadiiM ?? NO_DEPTH_BODY_SHELLS;
  const depthBodyShellRadiiM =
    byTable.size === 0 ? givenShells : new Map([...givenShells].filter(([id]) => !byTable.has(id)));
  const footprints = new Map<BodyIdHex, ScreenCircle>();
  for (const sphere of spheres) {
    const footprint = sphereFootprint(
      sphere.centreM,
      Math.max(sphere.radiusM, depthBodyShellRadiiM.get(sphere.id) ?? 0),
      options.camera,
      options.viewport,
    );
    if (footprint !== null) {
      footprints.set(sphere.id, footprint);
    }
  }
  // The 3 px switch counts the frame's own pixels, where its flux match is drawn; the hand-over
  // the presented ones, as the field's want does (R10.T10.g).
  const handed = handOverRegimes(
    litRegimes(spheres, camera, viewport, previous),
    spheres,
    handoverTauPx("photorealistic", options.setting),
    pixelAngleRad(camera, options.selectionPx ?? options.presentedPx),
    previous,
  );
  const meshed = new Map<BodyIdHex, LitRegime>(
    [...handed].map(([id, regime]) => [
      id,
      regime === "disc" && depthBodyShellRadiiM.has(id) ? "mesh" : regime,
    ]),
  );
  const regimes = promoteOverlapping(meshed, footprints, options.depthWriters ?? []);
  const hostSpheres: HostSphere[] = hosts.map((host) => ({
    star: host.disc.star,
    centreM: host.centreM,
    radiusM: host.disc.radius_m,
  }));
  // A mesh body's limb takes its place in the sequence as a disc's does.
  const ordered = new Map<BodyIdHex, LitRegime>(
    [...regimes].map(([id, regime]) => [id, drawnAsMesh(regime) ? "disc" : regime]),
  );
  const order = painterOrder(spheres, ordered, hostSpheres);
  const byId = new Map(bodies.map((body) => [body.id, body]));
  const lighting: LightingBody[] = spheres.map(({ id, centreM, radiusM }) => ({
    id,
    centreM,
    radiusM,
  }));
  // Each body's starlight once, for the planetshine it gives the others.
  const neighbours = litNeighbours(bodies, hosts, options.annuli);
  const laws: PhotometricLaw[] = [];
  const discs: DiscRecord[] = [];
  const meshes: MeshBodyPlan[] = [];
  const steps: BodyStep[] = [];
  const sources = new Map<BodyIdHex, BodyLightSources>();
  const tabled = new Set<BodyIdHex>();
  let run: PointSprite[] = [];
  const closeRun = (): void => {
    if (run.length > 0) {
      steps.push({ kind: "points", sprites: run });
      run = [];
    }
  };
  for (const entry of order) {
    if (entry.kind === "host") {
      closeRun();
      steps.push({ kind: "host", star: entry.star });
      continue;
    }
    const body = byId.get(entry.body);
    if (body === undefined) {
      continue;
    }
    const occluders = occludersOf(body, lightsOf(body, hosts), lighting);
    if (regimes.get(body.id) === "point") {
      const sprite = pointSpriteOf(body, hosts, occluders, neighbours, options);
      if (sprite !== null) {
        run.push(sprite);
      }
      continue;
    }
    const drawn = drawnSurface(body);
    const rows = discSurfaceLaws(drawn).map((law) => {
      let row = laws.indexOf(law);
      if (row < 0) {
        row = laws.length;
        laws.push(law);
      }
      return row;
    });
    const lit: BodyLightSources = {
      stars: lightsOf(body, hosts),
      planetshine: planetshineSources(body, neighbours, options.planetshine),
    };
    const air = byTable.has(body.id) ? body.air : undefined;
    const record = discRecordOf(body, drawn, lit, occluders, rows, options, air);
    if (record === null) {
      continue;
    }
    if (air !== undefined) {
      tabled.add(body.id);
    }
    sources.set(body.id, lit);
    closeRun();
    const regime = regimes.get(body.id);
    // A disc near the 3 px switch begins its point's lights in the slices (R10.T14.f).
    if (regime !== "terrain") {
      warmPointOf(body, lit, camera, viewport);
    }
    if (drawnAsMesh(regime)) {
      // Only a body handed over has terrain over its figure to give the surveyed ground to.
      const role: MeshBodyRole =
        regime === "terrain"
          ? { kind: "handed-over", coverage: body.surveyCoverage ?? null }
          : { kind: "promoted" };
      steps.push({ kind: "limb", mesh: meshes.length });
      meshes.push({
        index: discs.length,
        mesh: smoothMeshOf(
          body.id,
          body.centreM,
          body.figure,
          record.pole,
          camera,
          viewport,
          options.setting,
        ),
        limbDepths: limbDepths(
          body.centreM,
          body.figure,
          record.pole,
          record.rect,
          camera,
          viewport,
        ),
        role,
      });
    } else {
      steps.push({ kind: "disc", index: discs.length });
    }
    discs.push(record);
  }
  closeRun();
  return { regimes, order, discs, meshes, laws, steps, sources, tabled };
}

/**
 * What every lit-body module composes before its own: R01's frame, R08's surface lighting (plan
 * R08, R08.T9.b: `preciseTrig.wgsl`, `common.wgsl`, `oblate.wgsl` and `surfaceLighting.wgsl`, in
 * place of `litBody.wgsl`'s stubs), the lit-body library, the cube sphere's texel lookup
 * (`cubeTexel.wgsl`, R10.T11), the maturity nodes' formula (`maturityNodes.wgsl`, R10.T1.c) and
 * the disc's shading.
 */
const LIT_SOURCE =
  frameWgsl +
  SURFACE_LIGHTING_WGSL +
  litBodyWgsl +
  cubeTexelWgsl +
  maturityNodesWgsl +
  bodyDiscWgsl;

const DISC_SOURCE = LIT_SOURCE + bodyDiscDrawWgsl;

/** The group 2 binding of the first of the body's atmosphere tables (`bodyDisc.wgsl`), past 0–5. */
const ATMOSPHERE_FIRST_BINDING = 6;

/** The group 2 binding of a class map's normal map (`bodyDisc.wgsl`'s `surface_normals`, R10.T10.g). */
const SURFACE_NORMALS_BINDING = 14;

/**
 * The group 2 binding of the body's table below a cloud deck (`bodyDisc.wgsl`'s
 * `atmosphere_below_deck`, R08.T15.b.2): past the mesh's coverage mask (13), the normal map (14)
 * and the disc's table (15).
 */
const BELOW_DECK_BINDING = 16;

/**
 * The disc's textures: the frame's phase table, its class map (any texture where uniform), its
 * body's atmosphere tables at bindings 6 to 12 (plan R08, R08.T9.b), the one-texel defaults for a
 * body without one, and its class map's normal map at binding 14 (plan R10, R10.T10.g), any
 * texture where it has none (its record's flag then 0, and the texture unread).
 */
const DISC_TEXTURES: ReadonlyArray<TextureBindingSpec> = [
  { name: "phaseFactorTable", binding: 1, sampleType: "unfilterable-float" },
  { name: "classWeights", binding: 2, viewDimension: "2d-array" },
  ...surfaceLightingTextures(ATMOSPHERE_FIRST_BINDING, BELOW_DECK_BINDING),
  { name: "surfaceNormals", binding: SURFACE_NORMALS_BINDING, viewDimension: "2d-array" },
];

/**
 * The group 2 binding of a body's coverage mask (`smoothMesh.wgsl` and `bodyDiscDraw.wgsl`,
 * R10.T10.c), past the disc's.
 */
const COVERAGE_MASK_BINDING = 13;

/** The coverage mask's entry in the mesh's and the disc draws' layouts, read by `textureLoad`. */
const COVERAGE_MASK_TEXTURE: TextureBindingSpec = {
  name: "coverageMask",
  binding: COVERAGE_MASK_BINDING,
  viewDimension: "2d-array",
};

/**
 * The disc draws' textures: {@link DISC_TEXTURES} with the body's coverage mask at binding 13
 * (plan R10, R10.T10.c), which a handed-over mesh body's limb reads to discard its pixels over its
 * surveyed terrain; the mask of no surveyed cell for every other draw.
 */
const DISC_DRAW_TEXTURES: ReadonlyArray<TextureBindingSpec> = [
  ...DISC_TEXTURES,
  COVERAGE_MASK_TEXTURE,
];

/** The group 2 binding of a body's disc table (`discAir.wgsl`'s `disc_reflectance`, R08.T16.b). */
const DISC_TABLE_BINDING = 15;

/** The disc table's entry in the air forms' layouts, read by `textureLoad`. */
const DISC_TABLE_TEXTURE: TextureBindingSpec = {
  name: "discReflectance",
  binding: DISC_TABLE_BINDING,
  sampleType: "unfilterable-float",
  viewDimension: "2d-array",
};

/**
 * The air forms' textures (plan R08, R08.T16.b): {@link DISC_DRAW_TEXTURES} with the body's disc
 * table at binding 15, the first free past R08.T9.b's atmosphere tables, R10.T10.c's coverage mask
 * (13) and R10.T10.g's normal map (14); the plain forms' layouts are as they were.
 */
const DISC_AIR_TEXTURES: ReadonlyArray<TextureBindingSpec> = [
  ...DISC_DRAW_TEXTURES,
  DISC_TABLE_TEXTURE,
];

/**
 * The disc draws' uniforms: the record, the draw (0 its wholly covered pixels, 1 its limb), its
 * rectangle's corner depths (0 for a disc body; a mesh body's limb plane's, T9), and the axes its
 * coverage mask is read in (R10.T10.c).
 */
const DISC_UNIFORMS = [
  { name: "disc", type: "u32" },
  { name: "edgePass", type: "u32" },
  { name: "depths", type: "vec4f" },
  { name: "coverageAxes", type: "mat4x4f" },
] as const;

/**
 * The disc draws' storage: the records, and the `disc cells` pass's sums (R07.T8.d), which a disc
 * under 32 px reads in place of summing its own cells.
 */
const DISC_STORAGE = [
  { name: "discs", binding: 0 },
  { name: "cellSums", binding: 5 },
] as const;

/**
 * The disc's two materials: its wholly covered pixels, opaque, and its limb, premultiplied, which
 * for a handed-over mesh body discards its pixels over the ground its coverage mask holds
 * surveyed (`bodyDiscDraw.wgsl`, R10.T10.c).
 */
export const BODY_DISC_MATERIALS: Readonly<Record<"interior" | "limb", WgslMaterialSpec>> = {
  interior: {
    name: "bodies:disc",
    displayName: "BODY DISCS",
    vertexWgsl: DISC_SOURCE,
    fragmentWgsl: DISC_SOURCE,
    uniforms: DISC_UNIFORMS,
    samplers: [],
    textures: DISC_DRAW_TEXTURES,
    storageBuffers: DISC_STORAGE,
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "none",
  },
  limb: {
    name: "bodies:discLimb",
    displayName: "BODY DISC LIMBS",
    vertexWgsl: DISC_SOURCE,
    fragmentWgsl: DISC_SOURCE,
    uniforms: DISC_UNIFORMS,
    samplers: [],
    textures: DISC_DRAW_TEXTURES,
    storageBuffers: DISC_STORAGE,
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  },
};

/**
 * The disc's two materials' emissive forms (plan R08, R08.T18.a): their source with `shade` adding
 * the ground's emission (`discReflectance.ts`' `withDiscEmission`), drawn for the records that carry
 * it; the same bindings, uniforms and states.
 */
export const BODY_DISC_EMISSIVE_MATERIALS: Readonly<Record<"interior" | "limb", WgslMaterialSpec>> =
  (() => {
    const source = withDiscEmission(DISC_SOURCE);
    const { interior, limb } = BODY_DISC_MATERIALS;
    return {
      interior: {
        ...interior,
        name: "bodies:discEmissive",
        displayName: "EMISSIVE BODY DISCS",
        vertexWgsl: source,
        fragmentWgsl: source,
      },
      limb: {
        ...limb,
        name: "bodies:discLimbEmissive",
        displayName: "EMISSIVE DISC LIMBS",
        vertexWgsl: source,
        fragmentWgsl: source,
      },
    };
  })();

/**
 * The disc's two materials' air forms (plan R08, R08.T16.b): their source with `shade` handing its
 * light to `disc_air` (`discReflectance.ts`' `withDiscAir`), drawn for the records that carry their
 * air beyond the 10⁹ m boundary; the same uniforms and states, and the disc table at binding 15.
 */
export const BODY_DISC_AIR_MATERIALS: Readonly<Record<"interior" | "limb", WgslMaterialSpec>> =
  (() => {
    const source = withDiscAir(DISC_SOURCE);
    const { interior, limb } = BODY_DISC_MATERIALS;
    return {
      interior: {
        ...interior,
        name: "bodies:discAir",
        displayName: "ATMOSPHERIC BODY DISCS",
        vertexWgsl: source,
        fragmentWgsl: source,
        textures: DISC_AIR_TEXTURES,
      },
      limb: {
        ...limb,
        name: "bodies:discLimbAir",
        displayName: "ATMOSPHERIC DISC LIMBS",
        vertexWgsl: source,
        fragmentWgsl: source,
        textures: DISC_AIR_TEXTURES,
      },
    };
  })();

/**
 * The disc's two materials' forms for emission and air together (plan R08, R08.T18.b;
 * `discReflectance.ts`' `withDiscEmissionAndAir`), drawn for the records that carry both: a body
 * with air whose ground glows, drawn by its disc table beyond the 10⁹ m boundary; the air forms'
 * uniforms, states and table.
 */
export const BODY_DISC_EMISSIVE_AIR_MATERIALS: Readonly<
  Record<"interior" | "limb", WgslMaterialSpec>
> = (() => {
  const source = withDiscEmissionAndAir(DISC_SOURCE);
  const { interior, limb } = BODY_DISC_AIR_MATERIALS;
  return {
    interior: {
      ...interior,
      name: "bodies:discEmissiveAir",
      displayName: "GLOWING ATMOSPHERIC DISCS",
      vertexWgsl: source,
      fragmentWgsl: source,
    },
    limb: {
      ...limb,
      name: "bodies:discLimbEmissiveAir",
      displayName: "GLOWING ATMOSPHERIC LIMBS",
      vertexWgsl: source,
      fragmentWgsl: source,
    },
  };
})();

const SMOOTH_MESH_SOURCE = LIT_SOURCE + patchVertexWgsl + smoothMeshWgsl;

/**
 * The mesh regime's material (T9): R05's patches of a body's spheroid at zero height, one
 * instanced draw a body, opaque with depth, shading the pixels the body covers wholly from its
 * disc record (`shaders/smoothMesh.wgsl`), and, beneath a body's surveyed terrain, discarding the
 * pixels whose ground the coverage mask holds surveyed (plan R10, R10.T10.c): the reference
 * surface drawn with the inverse mask.
 */
export const SMOOTH_MESH_MATERIAL: WgslMaterialSpec = {
  name: "bodies:smoothMesh",
  displayName: "BODY MESHES",
  vertexWgsl: SMOOTH_MESH_SOURCE,
  fragmentWgsl: SMOOTH_MESH_SOURCE,
  uniforms: [
    { name: "disc", type: "u32" },
    { name: "firstInstance", type: "u32" },
    { name: "bodyRotation", type: "mat4x4f" },
    { name: "coverageAxes", type: "mat4x4f" },
  ],
  samplers: [],
  textures: [...DISC_TEXTURES, COVERAGE_MASK_TEXTURE],
  storageBuffers: [
    { name: "discs", binding: 0 },
    { name: "slots", binding: 3 },
    { name: "instances", binding: 4 },
    { name: "cellSums", binding: 5 },
  ],
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

/**
 * The mesh regime's material's emissive form (plan R08, R08.T18.a; `withDiscEmission`), for the
 * records that carry their ground's emission; the same bindings, uniforms and states.
 */
export const SMOOTH_MESH_EMISSIVE_MATERIAL: WgslMaterialSpec = (() => {
  const source = withDiscEmission(SMOOTH_MESH_SOURCE);
  return {
    ...SMOOTH_MESH_MATERIAL,
    name: "bodies:smoothMeshEmissive",
    displayName: "EMISSIVE BODY MESHES",
    vertexWgsl: source,
    fragmentWgsl: source,
  };
})();

/**
 * The mesh regime's material's air form (plan R08, R08.T16.b; `withDiscAir`), for the records that
 * carry their air; the same uniforms and states, and the disc table at binding 15.
 */
export const SMOOTH_MESH_AIR_MATERIAL: WgslMaterialSpec = (() => {
  const source = withDiscAir(SMOOTH_MESH_SOURCE);
  return {
    ...SMOOTH_MESH_MATERIAL,
    name: "bodies:smoothMeshAir",
    displayName: "ATMOSPHERIC BODY MESHES",
    vertexWgsl: source,
    fragmentWgsl: source,
    textures: [...(SMOOTH_MESH_MATERIAL.textures ?? []), DISC_TABLE_TEXTURE],
  };
})();

/**
 * The mesh regime's material's form for emission and air together (plan R08, R08.T18.b;
 * `withDiscEmissionAndAir`), for the records that carry both; the air form's bindings and states.
 */
export const SMOOTH_MESH_EMISSIVE_AIR_MATERIAL: WgslMaterialSpec = (() => {
  const source = withDiscEmissionAndAir(SMOOTH_MESH_SOURCE);
  return {
    ...SMOOTH_MESH_AIR_MATERIAL,
    name: "bodies:smoothMeshEmissiveAir",
    displayName: "GLOWING ATMOSPHERIC MESHES",
    vertexWgsl: source,
    fragmentWgsl: source,
  };
})();

/** smoothMesh.wgsl's vertex stage's head, which the figure's cluster path calls as `smooth_vertex`. */
const FIGURE_VERTEX_HEAD =
  "@vertex\nfn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> MeshVarying {";

/** {@link FIGURE_VERTEX_HEAD} as a function, no longer an entry point. */
const FIGURE_VERTEX_FUNCTION = "fn smooth_vertex(grid : vec3f, instance : u32) -> MeshVarying {";

/** smoothMesh.wgsl's `Draw`'s last field and its end, after which the cluster path takes its base. */
const FIGURE_DRAW_END = "  coverageAxes : mat4x4f,\n}";

/**
 * A form of the mesh regime's figure on the cluster path (plan R14, R14.T4.a): `spec`'s bindings,
 * uniforms, states and fragment stage, its `Draw` taking `clusterBase` after its own fields, its
 * vertex stage renamed `smooth_vertex`, a function, which `smoothMeshCluster.wgsl`'s entry point
 * hands each template vertex as R05's patch mesh's own and its instance, read from a bucket's list;
 * every placement as the instanced draw's.
 *
 * @throws Error unless the figure's vertex stage and its `Draw`'s end each occur once in `spec`.
 */
export function figureClusterMaterial(
  spec: WgslMaterialSpec,
  displayName: string,
): WgslMaterialSpec {
  const heads = spec.vertexWgsl.split(FIGURE_VERTEX_HEAD);
  const draws = heads.join(FIGURE_VERTEX_FUNCTION).split(FIGURE_DRAW_END);
  if (heads.length !== 2 || draws.length !== 2) {
    throw new Error(
      `a figure's cluster form needs smoothMesh.wgsl's vertex stage and Draw once each, found ${String(heads.length - 1)} and ${String(draws.length - 1)}`,
    );
  }
  const source = `${draws.join(
    "  coverageAxes : mat4x4f,\n  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).\n  clusterBase : u32,\n}",
  )}\n${smoothMeshClusterWgsl}`;
  return {
    ...spec,
    name: `${spec.name}Cluster`,
    displayName,
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [...spec.uniforms, { name: "clusterBase", type: "u32" }],
    storageBuffers: [
      ...(spec.storageBuffers ?? []),
      { name: "clusterLists", binding: 22, stages: ["vertex"] },
    ],
  };
}

/** The mesh regime's figure's material on the cluster path (plan R14, R14.T4.a). */
export const SMOOTH_MESH_CLUSTER_MATERIAL = figureClusterMaterial(
  SMOOTH_MESH_MATERIAL,
  "BODY MESH CLUSTERS",
);

/** Its emissive form on the cluster path (plan R14, R14.T4.a; R08.T18.a). */
export const SMOOTH_MESH_EMISSIVE_CLUSTER_MATERIAL = figureClusterMaterial(
  SMOOTH_MESH_EMISSIVE_MATERIAL,
  "EMISSIVE MESH CLUSTERS",
);

/** Its air form on the cluster path (plan R14, R14.T4.a; R08.T16.b). */
export const SMOOTH_MESH_AIR_CLUSTER_MATERIAL = figureClusterMaterial(
  SMOOTH_MESH_AIR_MATERIAL,
  "ATMOSPHERIC MESH CLUSTERS",
);

/** Its form for emission and air together on the cluster path (plan R14, R14.T4.a; R08.T18.b). */
export const SMOOTH_MESH_EMISSIVE_AIR_CLUSTER_MATERIAL = figureClusterMaterial(
  SMOOTH_MESH_EMISSIVE_AIR_MATERIAL,
  "GLOWING MESH CLUSTERS",
);

/** The figure's cluster mesh and its templates (plan R14, R14.T4.a). */
export const FIGURE_CLUSTER_MESH = smoothFigureClusterMesh();

/**
 * The mesh regime's figure as a consumer of the cluster path (plan R14, R14.T4.a): a patch's 64
 * blocks and 32 skirt strips, drawn at stride 1 alone, R05's instance records
 * (`packSmoothMeshes`), each patch its own page (`packSmoothClusterRecords`).
 */
export const FIGURE_CLUSTER_CONSUMER: ClusterConsumer = {
  name: "bodies:figure clusters",
  templates: FIGURE_CLUSTER_MESH.templates,
  clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
  firstShape: CLUSTER_SHAPE_CODE["terrain-block"],
  templateClusters: FIGURE_CLUSTER_MESH.templates.map(({ shape, stride }) =>
    stride !== 1
      ? 0
      : shape === "terrain-block"
        ? TERRAIN_BLOCKS
        : TERRAIN_CLUSTERS_PER_PAGE - TERRAIN_BLOCKS,
  ),
  instanceBytes: INSTANCE_RECORD_BYTES,
  vertexWgsl: smoothMeshClusterWgsl,
};

/** The figures' cull's timed label (plan R14, R14.T4.a): one dispatch a frame of mesh bodies. */
export const FIGURE_CLUSTER_CULL_LABEL = "body cluster cull";

/**
 * How far each side of a figure's frustum is opened for its cull, rad: past the `f32` rounding of
 * the planes, as the terrain's overlay's is (`terrainPass.ts`), so that no cluster that draws is
 * refused.
 */
const FIGURE_CULL_MARGIN_RAD = 0.01;

/**
 * The `disc cells` pass's label (R07.T8.d), between R06's sky and the mesh bodies' figures, which
 * read its sums as the `discs` pass does (`PHOTOREAL_PASS_LABELS.discCells`).
 */
export const DISC_CELLS_PASS = "disc cells";

/**
 * The `disc cells` kernel (R07.T8.d–g, decision-r07-small-disc-cost §1.2, decision-r08-t11-frame
 * §2; `shaders/bodyDiscCells.wgsl`): every cell of every pixel of the frame's small discs, and of
 * its larger discs' limbs and their faces' near-limb bands, shaded at once, one invocation a cell,
 * as many pixels a workgroup as fit (R07.T8.g), and summed in each pixel's order for the draws to
 * read.
 *
 * @remarks
 * Its one source serves both capability paths: it has no subgroup variant. Its sums are a pure
 * function of its inputs, so its readback is `bit-exact`, though nothing reads them back. Each
 * dispatch binds one set of a record's textures: its body's atmosphere tables (plan R08, R08.T9.b)
 * or the one-texel defaults, and its class map and normal map or the texel of no weight
 * (R07.T8.e), one dispatch a set ({@link discCellJobs}' sets).
 */
export const BODY_DISC_CELLS_KERNEL: KernelPair = {
  name: "BODY DISC CELLS",
  reference: LIT_SOURCE + bodyDiscCellsWgsl,
  subgroup: null,
  readback: "bit-exact",
};

/**
 * The `disc cells` kernel's emissive form (plan R08, R08.T18.a; `withDiscEmission`), one dispatch
 * a set of records that carry their ground's emission (`DiscCellSet.emissive`): one kernel more in
 * R01.T15's bounded cache, made at the first frame that has such a record.
 */
export const BODY_DISC_CELLS_EMISSIVE_KERNEL: KernelPair = {
  name: "BODY DISC CELLS, emissive",
  reference: withDiscEmission(LIT_SOURCE + bodyDiscCellsWgsl),
  subgroup: null,
  readback: "bit-exact",
};

/**
 * The `disc cells` kernel's air form (plan R08, R08.T16.b; `withDiscAir`), one dispatch a set of
 * records that carry their air (`DiscCellSet.air`): one kernel more in R01.T15's bounded cache,
 * made at the first frame that has such a record.
 */
export const BODY_DISC_CELLS_AIR_KERNEL: KernelPair = {
  name: "BODY DISC CELLS, with air",
  reference: withDiscAir(LIT_SOURCE + bodyDiscCellsWgsl),
  subgroup: null,
  readback: "bit-exact",
};

/**
 * The `disc cells` kernel's form for emission and air together (plan R08, R08.T18.b;
 * `withDiscEmissionAndAir`), one dispatch a set of records that carry both
 * (`DiscCellSet.emissive` and `DiscCellSet.air`): one kernel more in R01.T15's bounded cache, made
 * at the first frame that has such a record.
 */
export const BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL: KernelPair = {
  name: "BODY DISC CELLS, emissive, with air",
  reference: withDiscEmissionAndAir(LIT_SOURCE + bodyDiscCellsWgsl),
  subgroup: null,
  readback: "bit-exact",
};

/** The workgroups a dispatch lays along x at most, the kernel's `GROUPS_PER_ROW`. */
const CELL_GROUPS_PER_ROW = 65_535;

/**
 * The `disc cells` dispatch's grid for a set's `groups` workgroups (its `DiscCellSet.groups`,
 * each a run of pixels, R07.T8.g): (min(G, 65,535), ⌈G ÷ 65,535⌉, 1), within WebGPU's 65,535 a
 * dimension; the kernel leaves the last row's excess.
 */
export function cellWorkgroups(groups: number): readonly [number, number, number] {
  return [Math.min(groups, CELL_GROUPS_PER_ROW), Math.ceil(groups / CELL_GROUPS_PER_ROW), 1];
}

/** Bytes of one pixel's sums in `cell_sums`: two `vec4f`. */
const CELL_SUM_BYTES = 32;

/**
 * What the `disc cells` pass takes of the `discs` pass it serves (R07.T8.d): that pass's frame,
 * packed as the engine packs a submission's (`frame.wgsl`), so that each cell is the one the
 * pixel's draw would shade.
 */
export interface DiscCellFrame {
  /** The submission's `viewRotation`, column-major. */
  readonly viewRotation: Float32Array;
  /** The submission's `projection`, column-major. */
  readonly projection: Float32Array;
  /** The scene target's size, px: the `discs` pass's output. */
  readonly size: ViewSize;
}

/**
 * The `Frame` block of `frame.wgsl` for a {@link DiscCellFrame}: two `mat4x4f`, then the output's
 * width and height and their reciprocals.
 */
export function discCellFrameBlock(frame: DiscCellFrame): Float32Array {
  const block = new Float32Array(36);
  block.set(frame.viewRotation.subarray(0, 16), 0);
  block.set(frame.projection.subarray(0, 16), 16);
  const { widthPx, heightPx } = frame.size;
  block.set([widthPx, heightPx, 1 / widthPx, 1 / heightPx], 32);
  return block;
}

/** The smallest buffer made, bytes; each grows by doubling. */
const MIN_BUFFER_BYTES = 4_096;

/** The part of the engine the renderer uses. */
export type LitBodyEngine = Pick<
  RenderEngine,
  | "createMaterial"
  | "createMesh"
  | "createBuffer"
  | "createTexture"
  | "writeBuffer"
  | "writeTexture"
  | "releaseBuffer"
  | "releaseTexture"
  | "dispatch"
  | "createComputeAsync"
  | "releaseCompute"
  | "onRestored"
  | "capabilities"
  | "prepareMaterial"
>;

interface DeviceResources {
  readonly interior: MaterialHandle;
  readonly limb: MaterialHandle;
  readonly sprite: MaterialHandle;
  readonly quad: MeshHandle;
  discs: BufferHandle;
  /** One sprite buffer per run of points in the frame, reused frame to frame. */
  readonly sprites: BufferHandle[];
  table: TextureHandle;
  tableRows: number;
  /**
   * A texel of no weight, bound where a disc's surface is uniform, and where it carries no normal
   * map (R10.T10.g), and never read.
   */
  readonly noClassMap: TextureHandle;
  /**
   * A coverage mask of no surveyed cell, one texel a face, bound where a mesh body has no survey
   * (R10.T10.c): its figure is drawn whole.
   */
  readonly noCoverage: TextureHandle;
  /**
   * The one-texel atmosphere tables of a body without one (plan R08, R08.T9.b), and the `disc
   * cells` pass's always: the lookups read 1 and 0 through them.
   */
  readonly noAtmosphere: SurfaceLightingTables;
  /** A disc table of zeros, bound where a record carries no air (R08.T16.b), never read. */
  readonly noDiscTable: TextureHandle;
  /** The mesh regime's material and R05's 65 × 65 patch with skirts (T9). */
  readonly mesh: MaterialHandle;
  readonly patch: MeshHandle;
  /**
   * The figures' cluster path (plan R14, R14.T4.a): its material, its cluster mesh, its path's
   * buckets, the frame's records, and the cull's kernel once compiled, `null` before and where it
   * could not be, when every figure draws instanced.
   */
  readonly meshCluster: MaterialHandle;
  readonly clusterMesh: MeshHandle;
  readonly clusters: ClusterConsumerPath;
  clusterRecords: BufferHandle;
  cullKernel: ComputeHandle | null;
  /** The frame's smooth figures' slot and instance records. */
  slots: BufferHandle;
  instances: BufferHandle;
  /** The `disc cells` pass's jobs and the pixels' sums it leaves (R07.T8.d). */
  cellJobs: BufferHandle;
  /** Its workgroups' runs of the jobs (R07.T8.g). */
  cellGroups: BufferHandle;
  cellSums: BufferHandle;
  /**
   * The disc's and the mesh's emissive forms (R08.T18.a), made and prepared at the first record
   * that carries its ground's emission, and drawn once prepared; `null` until then.
   */
  emissive: FormMaterials | null;
  /**
   * Their air forms (R08.T16.b), made and prepared at the first body given its disc table, and drawn
   * once prepared; `null` until then.
   */
  air: FormMaterials | null;
  /**
   * Their forms for emission and air together (R08.T18.b), made and prepared at the first record
   * that carries both; `null` until then.
   */
  emissiveAir: FormMaterials | null;
}

/**
 * A form of a disc's two materials and the mesh's (R08.T18.a's emissive, R08.T16.b's air), and
 * whether they are prepared.
 */
interface FormMaterials {
  readonly interior: MaterialHandle;
  readonly limb: MaterialHandle;
  readonly mesh: MaterialHandle;
  /** The mesh's form on the cluster path (plan R14, R14.T4.a). */
  readonly meshCluster: MaterialHandle;
  ready: boolean;
  /** Settles once their pipelines are prepared, or could not be. */
  prepared: Promise<void>;
}

/**
 * The kernels' forms beside the plain ones: R08.T18.a's emissive, R08.T16.b's air, and R08.T18.b's
 * emissive with air.
 */
type FormKind = "emissive" | "air" | "emissiveAir";

/** Each form's materials and what its failure to prepare is reported as. */
const FORM_SPECS: Readonly<
  Record<
    FormKind,
    {
      readonly interior: WgslMaterialSpec;
      readonly limb: WgslMaterialSpec;
      readonly mesh: WgslMaterialSpec;
      readonly meshCluster: WgslMaterialSpec;
      readonly what: string;
    }
  >
> = {
  emissive: {
    interior: BODY_DISC_EMISSIVE_MATERIALS.interior,
    limb: BODY_DISC_EMISSIVE_MATERIALS.limb,
    mesh: SMOOTH_MESH_EMISSIVE_MATERIAL,
    meshCluster: SMOOTH_MESH_EMISSIVE_CLUSTER_MATERIAL,
    what: "the emissive forms of the lit bodies",
  },
  air: {
    interior: BODY_DISC_AIR_MATERIALS.interior,
    limb: BODY_DISC_AIR_MATERIALS.limb,
    mesh: SMOOTH_MESH_AIR_MATERIAL,
    meshCluster: SMOOTH_MESH_AIR_CLUSTER_MATERIAL,
    what: "the air forms of the lit bodies",
  },
  emissiveAir: {
    interior: BODY_DISC_EMISSIVE_AIR_MATERIALS.interior,
    limb: BODY_DISC_EMISSIVE_AIR_MATERIALS.limb,
    mesh: SMOOTH_MESH_EMISSIVE_AIR_MATERIAL,
    meshCluster: SMOOTH_MESH_EMISSIVE_AIR_CLUSTER_MATERIAL,
    what: "the emissive forms with air of the lit bodies",
  },
};

/**
 * A plan's records as the renderer draws them (R08.T18.a): its own, or with no record's emission
 * while the emissive forms are not prepared, and no record's air while the air forms are not
 * (R08.T16.b), so that a frame draws its emission in all its passes or in none.
 */
interface DrawnPlan {
  readonly plan: BodyFramePlan;
  readonly records: ReadonlyArray<DiscRecord>;
  /** The prepared emissive forms its emitting records are drawn by; `null` where none emits. */
  readonly emissive: FormMaterials | null;
  /** The prepared air forms its records with air are drawn by; `null` where none has air. */
  readonly air: FormMaterials | null;
  /**
   * The prepared forms its records with emission and air are drawn by (R08.T18.b); `null` where
   * none has both.
   */
  readonly emissiveAir: FormMaterials | null;
}

/** Whether a record carries its ground's emission and its air together (R08.T18.b). */
function withEmissionAndAir(record: DiscRecord): boolean {
  return record.emission !== undefined && record.air !== undefined;
}

/** A record without its emission, as the plain forms draw it. */
function withoutEmission(record: DiscRecord): DiscRecord {
  if (record.emission === undefined) {
    return record;
  }
  const { emission: _emission, ...plain } = record;
  return plain;
}

/** A record without its air, as the plain forms draw it. */
function withoutAir(record: DiscRecord): DiscRecord {
  if (record.air === undefined) {
    return record;
  }
  const { air: _air, ...plain } = record;
  return plain;
}

/** The forms a record is drawn by: its air's, its emission's, or the plain ones. */
function formsOf(
  record: DiscRecord,
  drawn: DrawnPlan,
  plain: Pick<FormMaterials, "interior" | "limb" | "mesh" | "meshCluster">,
): Pick<FormMaterials, "interior" | "limb" | "mesh" | "meshCluster"> {
  if (record.air !== undefined && record.emission !== undefined && drawn.emissiveAir !== null) {
    return drawn.emissiveAir;
  }
  if (record.air !== undefined && drawn.air !== null) {
    return drawn.air;
  }
  if (record.emission !== undefined && drawn.emissive !== null) {
    return drawn.emissive;
  }
  return plain;
}

/**
 * Binds a frame's lit-body plan to the engine (plan R07, T8.a).
 *
 * @remarks
 * It makes its materials, the quad, the patch, the disc buffer and the phase table on construction
 * and again after a device loss; buffers grow by doubling and the table by rows. Each disc is two
 * draws, its wholly covered pixels and then its limb; each run of points one draw of
 * `spriteMaterial`, an HDR twin of R02's star sprite reading `sprites` at binding 0. A mesh body is
 * one instanced draw of its figure, or on the cluster path (plan R14, R14.T4.a) its buckets' draws
 * ({@link LitBodyRenderer.meshDraws}, for the `bodies` pass), and its limb's draw in the sequence. The small discs' cells, and the larger discs' limb pixels' and
 * their faces' near-limb bands', are shaded first by the `disc cells` pass
 * ({@link LitBodyRenderer.dispatchCells}, R07.T8.d–g),
 * whose sums their draws read. A plan's
 * records are written once, by whichever of the three is called first for it, so all three must
 * be called for a plan before its passes are submitted, {@link LitBodyRenderer.dispatchCells}
 * first: a plan whose records were written by the others first sums every disc's cells in its
 * draws, as before the pass.
 */
export class LitBodyRenderer {
  readonly #engine: LitBodyEngine;
  readonly #spriteMaterial: WgslMaterialSpec;
  #resources: DeviceResources;
  #tableLaws: ReadonlyArray<PhotometricLaw> = [];
  /** The plan whose records were written last, and each of its meshes' first instance. */
  #written: { readonly plan: BodyFramePlan; readonly firstInstance: ReadonlyArray<number> } | null =
    null;
  /** The plan whose records were last drawn, as they are drawn (R08.T18.a). */
  #drawn: DrawnPlan | null = null;
  /**
   * The plan whose figures were last culled on the cluster path (plan R14, R14.T4.a), and each
   * culled mesh's pass, by its index in the plan's meshes.
   */
  #culled: { readonly plan: BodyFramePlan; readonly passes: ReadonlyMap<number, number> } | null =
    null;
  /** How the figures' geometry is drawn (plan R14): the setting's, or a launch's override. */
  #geometry: GeometrySettings = SETTINGS.high.geometry;
  /** Settles once the current device's cull kernel is made, or could not be (R14.T4.a). */
  #cullReady: Promise<boolean> = Promise.resolve(false);
  #disposed = false;
  readonly #unsubscribe: () => void;

  constructor(engine: LitBodyEngine, spriteMaterial: WgslMaterialSpec) {
    this.#engine = engine;
    this.#spriteMaterial = spriteMaterial;
    this.#resources = this.#create();
    this.#prepareRegimes(this.#resources);
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#create();
      this.#prepareRegimes(this.#resources);
      this.#tableLaws = [];
      this.#written = null;
      this.#drawn = null;
      this.#culled = null;
    });
  }

  /** How it draws the figures' geometry (plan R14, R14.T4.a): the high setting's until set. */
  get geometry(): GeometrySettings {
    return this.#geometry;
  }

  /**
   * Draws the figures' geometry by `geometry` from the next plan on: on the cluster path where its
   * `clusterPath` and `figureClusters` are on (plan R14, R14.T4.a), else instanced, as before.
   */
  setGeometry(geometry: GeometrySettings): void {
    this.#geometry = geometry;
    this.#culled = null;
  }

  /**
   * Makes the regimes' pipelines for the scene target, with its depth, ahead of their first draws
   * in a running view (plan R10, R10.T14.d; decision-r10-t14d-stall §1 and §2): the mesh regime's
   * on the patch, whose first draw is a body's hand-over to the terrain (its synchronous compile
   * stopped the page for 1,547 ms on a cold shader cache), and the disc's interior and limb on the
   * quad, whose first draw is a body's first disc past the 3.3 px switch (1,128 and 1,093 ms). A
   * draw before they are made makes its own, as before: a disc in VIEW's first frames does (R01's,
   * deferred). The point sprites draw from the first frame, with the stars, and are not prepared.
   */
  #prepareRegimes(resources: DeviceResources): void {
    const scene = [{ colour: HDR_COLOUR_FORMAT, depth: true }] as const;
    const prepared: ReadonlyArray<readonly [MaterialHandle, MeshHandle]> = [
      [resources.mesh, resources.patch],
      [resources.meshCluster, resources.clusterMesh],
      [resources.interior, resources.quad],
      [resources.limb, resources.quad],
    ];
    this.#readyCull(resources);
    for (const [material, mesh] of prepared) {
      void this.#engine.prepareMaterial(material, scene, [mesh]).catch((error: unknown) => {
        // A loss meanwhile is no failure: the restore makes and prepares them again.
        if (!(error instanceof EngineUnavailable)) {
          console.error(`the pipelines of ${material.name} could not be prepared:`, error);
        }
      });
    }
  }

  /**
   * Compiles the cluster cull's kernel for `resources`' device (plan R14, R14.T4.a), off the frame;
   * never rejects. Until it is made, or where it cannot be, every figure draws instanced.
   */
  #readyCull(resources: DeviceResources): void {
    // A refusal thrown at the call, as a lost device's, settles as one rejected.
    this.#cullReady = (async () => this.#engine.createComputeAsync(CLUSTER_CULL_KERNEL))().then(
      (kernel) => {
        if (this.#resources === resources && !this.#disposed && resources.cullKernel === null) {
          resources.cullKernel = kernel;
          return true;
        }
        this.#engine.releaseCompute(kernel);
        return false;
      },
      (error: unknown) => {
        if (!(error instanceof EngineUnavailable)) {
          console.error("the figures' cluster cull failed: every figure draws instanced", error);
        }
        return false;
      },
    );
  }

  /**
   * Settles once the figures' cluster cull's kernel is made on the current device (plan R14,
   * R14.T4.a), `true`, or could not be, `false`; until then every figure draws instanced.
   */
  clustersReady(): Promise<boolean> {
    return this.#cullReady;
  }

  /**
   * The figures' buckets as the last cull left them (plan R14, R14.T4.a), a pass a culled body in
   * the plan's order, for the counters' reduction (`clusters/stats.ts`' `ClusterStats.sample`);
   * `null` before the first.
   */
  get figureBuckets(): ClusterBuckets | null {
    return this.#resources.clusters.buckets;
  }

  #create(): DeviceResources {
    const engine = this.#engine;
    return {
      interior: engine.createMaterial(BODY_DISC_MATERIALS.interior),
      limb: engine.createMaterial(BODY_DISC_MATERIALS.limb),
      sprite: engine.createMaterial(this.#spriteMaterial),
      quad: engine.createMesh({ ...WIREFRAME_MESHES.quad, name: "bodies:quad" }),
      discs: this.#buffer("bodies:discs", MIN_BUFFER_BYTES),
      sprites: [],
      table: this.#table(1),
      tableRows: 1,
      // Two layers, so that it is a `2d-array` by its own dimension wherever it binds: the cell
      // kernel's layout takes a texture's view as the texture is (R07.T8.d).
      noClassMap: engine.createTexture({
        name: "bodies:no class map",
        size: { width: 1, height: 1, depthOrArrayLayers: 2 },
        dimension: "2d",
        format: CLASS_MAP_FORMAT,
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING,
        category: "other",
      }),
      noAtmosphere: createAirlessTables(engine, "bodies:no atmosphere"),
      noCoverage: this.#noCoverage(),
      mesh: engine.createMaterial(SMOOTH_MESH_MATERIAL),
      patch: engine.createMesh({
        name: "bodies:smooth patch",
        ...patchMeshData(),
        topology: "triangle-list",
        attributes: {},
      }),
      meshCluster: engine.createMaterial(SMOOTH_MESH_CLUSTER_MATERIAL),
      clusterMesh: engine.createMesh(FIGURE_CLUSTER_MESH.mesh),
      clusters: new ClusterConsumerPath(engine, FIGURE_CLUSTER_CONSUMER),
      clusterRecords: this.#clusterBuffer("bodies:figure cluster records", MIN_BUFFER_BYTES),
      cullKernel: null,
      slots: this.#buffer("bodies:mesh slots", MIN_BUFFER_BYTES),
      instances: this.#buffer("bodies:mesh instances", MIN_BUFFER_BYTES),
      cellJobs: this.#buffer("bodies:disc cell jobs", MIN_BUFFER_BYTES),
      cellGroups: this.#buffer("bodies:disc cell groups", MIN_BUFFER_BYTES),
      cellSums: this.#buffer("bodies:disc cell sums", MIN_BUFFER_BYTES),
      noDiscTable: this.#noDiscTable(),
      emissive: null,
      air: null,
      emissiveAir: null,
    };
  }

  /**
   * A form, prepared (R08.T18.a's emissive, R08.T16.b's air), or `null`: it is made and its
   * pipelines prepared for the scene target at the first call, off the frame, as the plain forms are
   * at construction (decision-r10-t14d-stall), so that a body's first frame that needs it never
   * stalls on its compile; until it is prepared the records that need it are drawn as the plain
   * forms draw them.
   */
  #formMaterials(kind: FormKind): FormMaterials | null {
    const made = this.#formMade(kind);
    return made.ready ? made : null;
  }

  /** A form, made at the first call on a device with its pipelines' preparing begun. */
  #formMade(kind: FormKind): FormMaterials {
    const resources = this.#resources;
    const held = resources[kind];
    if (held !== null) {
      return held;
    }
    const engine = this.#engine;
    const specs = FORM_SPECS[kind];
    const interior = engine.createMaterial(specs.interior);
    const limb = engine.createMaterial(specs.limb);
    const mesh = engine.createMaterial(specs.mesh);
    const meshCluster = engine.createMaterial(specs.meshCluster);
    const scene = [{ colour: HDR_COLOUR_FORMAT, depth: true }] as const;
    const prepared: ReadonlyArray<readonly [MaterialHandle, MeshHandle]> = [
      [mesh, resources.patch],
      [meshCluster, resources.clusterMesh],
      [interior, resources.quad],
      [limb, resources.quad],
    ];
    const made: FormMaterials = {
      interior,
      limb,
      mesh,
      meshCluster,
      ready: false,
      prepared: Promise.resolve(),
    };
    made.prepared = this.#readyForm(made, prepared, scene, specs.what);
    resources[kind] = made;
    return made;
  }

  /** Prepares a form's pipelines, and marks it ready once they are; never rejects. */
  async #readyForm(
    made: FormMaterials,
    prepared: ReadonlyArray<readonly [MaterialHandle, MeshHandle]>,
    scene: ReadonlyArray<PreparedTarget>,
    what: string,
  ): Promise<void> {
    try {
      await Promise.all(
        prepared.map(([material, each]) => this.#engine.prepareMaterial(material, scene, [each])),
      );
      made.ready = true;
    } catch (error: unknown) {
      // A loss meanwhile is no failure: the restore makes the forms again at their next record.
      if (!(error instanceof EngineUnavailable)) {
        console.error(`${what} could not be prepared:`, error);
      }
    }
  }

  /**
   * Makes the emissive forms and prepares their pipelines (R08.T18.a), ahead of a body's first
   * frame above the cut, which until they are prepared draws its records as the plain forms do;
   * resolves once they are, or could not be. A record drawn after it draws its emission.
   */
  prepareEmissive(): Promise<void> {
    return this.#formMade("emissive").prepared;
  }

  /**
   * Makes the air forms and prepares their pipelines (R08.T16.b), ahead of a body's first frame
   * drawn by its disc table; resolves once they are, or could not be.
   */
  prepareAir(): Promise<void> {
    return this.#formMade("air").prepared;
  }

  /**
   * Makes the forms for emission and air together and prepares their pipelines (R08.T18.b), ahead
   * of a glowing body's first frame drawn by its disc table; resolves once they are, or could not
   * be.
   */
  prepareEmissiveAir(): Promise<void> {
    return this.#formMade("emissiveAir").prepared;
  }

  /**
   * Whether the air forms are prepared to draw a body by its disc table (R08.T16.b), their making
   * begun at the first call: a plan's `drawsAir`, so that a body keeps its full passes until they
   * are.
   */
  airReady(): boolean {
    return this.#formMaterials("air") !== null;
  }

  /**
   * A plan's records as this renderer draws them, decided once per plan ({@link DrawnPlan}): its
   * own where its emission and air can be drawn, every record's emission or air dropped while it
   * cannot.
   */
  #drawnPlan(plan: BodyFramePlan): DrawnPlan {
    if (this.#drawn?.plan === plan) {
      return this.#drawn;
    }
    const both = withEmissionAndAir;
    const emits = plan.discs.some((record) => record.emission !== undefined && !both(record));
    const emissive = emits ? this.#formMaterials("emissive") : null;
    const aired = plan.discs.some((record) => record.air !== undefined);
    const air = aired ? this.#formMaterials("air") : null;
    const glowing = plan.discs.some(both);
    const emissiveAir = glowing ? this.#formMaterials("emissiveAir") : null;
    // A record keeps what a prepared form draws: with emission and air, by their form, or else by
    // the air's without its emission (R08.T18.b); either alone by its own; the rest dropped.
    const drawnOf = (record: DiscRecord): DiscRecord => {
      if (both(record) && emissiveAir !== null) {
        return record;
      }
      let out = both(record) ? withoutEmission(record) : record;
      if (out.emission !== undefined && emissive === null) {
        out = withoutEmission(out);
      }
      if (out.air !== undefined && air === null) {
        out = withoutAir(out);
      }
      return out;
    };
    const records = emits || aired ? plan.discs.map(drawnOf) : plan.discs;
    this.#drawn = { plan, records, emissive, air, emissiveAir };
    return this.#drawn;
  }

  /**
   * {@link DeviceResources.noDiscTable}: one zero texel, bound where a record carries no air and
   * never read; two layers, so that it is a `2d-array` by its own dimension wherever it binds.
   */
  #noDiscTable(): TextureHandle {
    return this.#engine.createTexture({
      name: "bodies:no disc table",
      size: { width: 1, height: 1, depthOrArrayLayers: 2 },
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING,
      category: "other",
    });
  }

  /** {@link DeviceResources.noCoverage}: one 0 texel a face, of the coverage mask's format. */
  #noCoverage(): TextureHandle {
    const texture = this.#engine.createTexture({
      ...coverageMaskTextureSpec("bodies:no coverage", 0),
      category: "other",
    });
    this.#engine.writeTexture(
      texture,
      { x: 0, y: 0, z: 0 },
      { width: 1, height: 1, depthOrArrayLayers: 6 },
      new Uint8Array(6),
    );
    return texture;
  }

  #buffer(name: string, bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
  }

  /** A buffer of the cluster path's (plan R14): `geometry-clusters`. */
  #clusterBuffer(name: string, bytes: number): BufferHandle {
    return this.#engine.createBuffer({
      name,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
  }

  #table(rows: number): TextureHandle {
    return this.#engine.createTexture({
      name: "bodies:phase table",
      size: { width: PHASE_TABLE_SAMPLES, height: rows },
      dimension: "2d",
      format: "rgba32float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "other",
    });
  }

  /**
   * A buffer of at least `bytes`: `handle`, or a doubled one in its place, no larger than one
   * storage binding of the device holds where `bytes` fits in one.
   */
  #reserve(handle: BufferHandle, bytes: number): BufferHandle {
    if (bytes <= handle.bytes) {
      return handle;
    }
    let size = handle.bytes;
    while (size < bytes) {
      size *= 2;
    }
    const binding = this.#engine.capabilities.maxStorageBufferBindingSize;
    this.#engine.releaseBuffer(handle);
    return this.#buffer(handle.name, bytes <= binding ? Math.min(size, binding) : size);
  }

  /** Writes `data` into a buffer, replacing it by a doubled one if it is too small. */
  #write(handle: BufferHandle, data: Float32Array | Int32Array | Uint32Array): BufferHandle {
    const target = this.#reserve(handle, data.byteLength);
    if (data.byteLength > 0) {
      this.#engine.writeBuffer(target, 0, data);
    }
    return target;
  }

  #writeTable(laws: ReadonlyArray<PhotometricLaw>): void {
    const same =
      laws.length === this.#tableLaws.length && laws.every((law, i) => law === this.#tableLaws[i]);
    if (same || laws.length === 0) {
      return;
    }
    const resources = this.#resources;
    if (laws.length > resources.tableRows) {
      this.#engine.releaseTexture(resources.table);
      resources.table = this.#table(laws.length);
      resources.tableRows = laws.length;
    }
    this.#engine.writeTexture(
      resources.table,
      { x: 0, y: 0 },
      { width: PHASE_TABLE_SAMPLES, height: laws.length },
      packPhaseFactorRows(laws),
    );
    this.#tableLaws = laws;
  }

  /** The class map a record's draws bind: its own, or the texel of no weight. */
  #classWeights(surface: DrawnDiscSurface): TextureHandle {
    return surface.kind === "class-map" ? surface.weights : this.#resources.noClassMap;
  }

  /**
   * The normal map a record's draws bind (R10.T10.g): its class map's, or, where it has none, the
   * texel of no weight, which its record's flag (row 24's w, 0) leaves unread.
   */
  #surfaceNormals(surface: DrawnDiscSurface): TextureHandle {
    return surface.kind === "class-map" && surface.normals !== null
      ? surface.normals.texture
      : this.#resources.noClassMap;
  }

  /**
   * The textures a record's draws bind: the frame's phase table, its class map, its body's
   * atmosphere tables, or the defaults of a body without one (R08.T9.b), and its class map's normal
   * map (R10.T10.g).
   */
  #recordTextures(record: DiscRecord): Readonly<Record<string, TextureHandle>> {
    const resources = this.#resources;
    return {
      phaseFactorTable: resources.table,
      classWeights: this.#classWeights(record.surface),
      ...surfaceLightingDrawTextures(record.atmosphere ?? resources.noAtmosphere),
      surfaceNormals: this.#surfaceNormals(record.surface),
      discReflectance: record.air?.texture ?? resources.noDiscTable,
    };
  }

  /**
   * One of a disc's two draws: 0 its wholly covered pixels, 1 its limb, at `depths`; by the
   * emissive forms where its record, as drawn, carries its emission (R08.T18.a), and by the air
   * forms where it carries its air (R08.T16.b). A handed-over mesh body's limb takes its
   * `coverage`, by which it leaves its surveyed ground to the terrain (R10.T10.c); every other
   * draw `null`, the mask of no surveyed cell.
   */
  #discDraw(
    index: number,
    record: DiscRecord,
    edgePass: 0 | 1,
    depths: LimbDepths,
    drawn: DrawnPlan,
    coverage: MeshBodyCoverage | null,
  ): DrawItem {
    const resources = this.#resources;
    const forms = formsOf(record, drawn, resources);
    return {
      mesh: resources.quad,
      material: edgePass === 0 ? forms.interior : forms.limb,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {
        disc: new Float32Array([index]),
        edgePass: new Float32Array([edgePass]),
        depths: new Float32Array(depths),
        coverageAxes: rotationColumns(coverage?.rotation ?? IDENTITY_ROTATION),
      },
      textures: {
        ...this.#recordTextures(record),
        coverageMask: coverage?.mask ?? resources.noCoverage,
      },
      storageBuffers: { discs: resources.discs, cellSums: resources.cellSums },
    };
  }

  /**
   * Writes a plan's phase table, records and smooth figures, once per plan.
   *
   * @param cells - The `disc cells` pass's jobs whose sums the records point at (row 51); `null`
   *   where no pass runs for the plan, when every disc sums its own cells.
   */
  #upload(plan: BodyFramePlan, cells: DiscCellJobs | null = null): ReadonlyArray<number> {
    if (this.#written?.plan === plan) {
      return this.#written.firstInstance;
    }
    const resources = this.#resources;
    this.#writeTable(plan.laws);
    const drawn = this.#drawnPlan(plan);
    resources.discs = this.#write(resources.discs, packDiscRecords(drawn.records, cells));
    const records = packSmoothMeshes(plan.meshes.map(({ mesh }) => mesh));
    resources.slots = this.#write(resources.slots, records.slots);
    resources.instances = this.#write(resources.instances, records.instances);
    this.#written = { plan, firstInstance: records.firstInstance };
    return records.firstInstance;
  }

  /**
   * Shades a plan's discs' cells in parallel: the `disc cells` pass (R07.T8.d–g,
   * decision-r07-small-disc-cost §1.2, decision-r08-t11-frame §2), one dispatch of `kernel`
   * ({@link BODY_DISC_CELLS_KERNEL}) for each set of textures its records bind, over every pixel of
   * each disc under 32 px and each larger disc's limb pixels and its face's near-limb band
   * ({@link discCellJobs}), and none where the plan has no such pixel.
   *
   * @remarks
   * It writes the plan's records with each such disc's sums in row 51, and is called for a plan
   * before {@link LitBodyRenderer.meshDraws} and {@link LitBodyRenderer.draws}, whose submissions
   * then follow the dispatches', after the sky's: each `render` and `dispatch` is its own queue
   * submission, in call order. A plan already written by either of those is left to sum its own
   * cells, and nothing is dispatched. Each dispatch binds its set's textures, its body's
   * atmosphere tables and its class map and normal map, or the defaults (R07.T8.e), takes its run
   * of the workgroups from its first, and has {@link cellWorkgroups}' grid for them, each workgroup
   * a run of up to `pixelsPerGroup` pixels of one cell count (R07.T8.g).
   *
   * A set of records that carry their ground's emission (R08.T18.a) is dispatched with
   * `emissiveKernel` ({@link BODY_DISC_CELLS_EMISSIVE_KERNEL}); without it, or while the emissive
   * forms are not prepared, such records take no jobs and their draws sum their own cells. A set of
   * records that carry their air (R08.T16.b) is dispatched with `airKernel`
   * ({@link BODY_DISC_CELLS_AIR_KERNEL}) alike.
   *
   * @param frame - The `discs` pass's own frame, which the kernel shades in and a large disc's limb
   *   is found in.
   * @param emissiveKernel - The kernel's emissive form, or `null` where it is not made yet.
   * @param airKernel - The kernel's air form, or `null` where it is not made yet.
   * @returns Whether it dispatched.
   */
  dispatchCells(
    plan: BodyFramePlan,
    kernel: ComputeHandle,
    frame: DiscCellFrame,
    emissiveKernel: ComputeHandle | null = null,
    airKernel: ComputeHandle | null = null,
    emissiveAirKernel: ComputeHandle | null = null,
  ): boolean {
    if (this.#written?.plan === plan) {
      return false;
    }
    // As many pixels' sums as one storage binding holds; a disc past them sums its own cells.
    const fits = Math.floor(this.#engine.capabilities.maxStorageBufferBindingSize / CELL_SUM_BYTES);
    const { records } = this.#drawnPlan(plan);
    // A body handed over under a mask of every cell surveyed reads none of its sums (R07.T8.h).
    const discarded = new Set(
      plan.meshes.filter(({ role }) => coverageOf(role)?.whole === true).map(({ index }) => index),
    );
    const cells = discCellJobs(
      records,
      fits,
      frame,
      emissiveKernel !== null,
      airKernel !== null,
      emissiveAirKernel !== null,
      discarded,
    );
    if (cells.count === 0) {
      this.#upload(plan);
      return false;
    }
    this.#upload(plan, cells);
    const resources = this.#resources;
    resources.cellJobs = this.#write(resources.cellJobs, cells.jobs);
    resources.cellGroups = this.#write(resources.cellGroups, cells.groups);
    resources.cellSums = this.#reserve(resources.cellSums, cells.count * CELL_SUM_BYTES);
    const block = discCellFrameBlock(frame);
    for (const set of cells.sets) {
      const record = records[set.record];
      if (record === undefined) {
        throw new Error(`the cell pass's set from job ${set.first} has no record ${set.record}`);
      }
      const setKernel =
        set.air && set.emissive
          ? emissiveAirKernel
          : set.air
            ? airKernel
            : set.emissive
              ? emissiveKernel
              : kernel;
      if (setKernel === null) {
        const form = set.air && set.emissive ? "emissive with air" : set.air ? "air" : "emissive";
        throw new Error(`the cell pass's ${form} set from job ${set.first} has no kernel`);
      }
      this.#engine.dispatch(
        setKernel,
        {
          uniforms: {
            frame: block,
            cell_pass: new Uint32Array([set.groups, set.firstGroup, 0, 0]),
          },
          buffers: {
            discs: resources.discs,
            cell_jobs: resources.cellJobs,
            cell_groups: resources.cellGroups,
            cell_sums: resources.cellSums,
          },
          sampled: {
            phase_factor_table: resources.table,
            class_weights: this.#classWeights(record.surface),
            ...surfaceLightingKernelTextures(record.atmosphere ?? resources.noAtmosphere),
            surface_normals: this.#surfaceNormals(record.surface),
            // The air form's table alone, which no other form declares (R08.T16.b).
            ...(set.air && record.air !== undefined
              ? { disc_reflectance: record.air.texture }
              : {}),
          },
          storage: {},
        },
        cellWorkgroups(set.groups),
        DISC_CELLS_PASS,
      );
    }
    return true;
  }

  /**
   * The draws of a plan's mesh bodies, for the `bodies` pass: opaque, with depth, before the
   * painter's sequence (Design note 8). Each figure is one instanced draw, or on the cluster path
   * (plan R14, R14.T4.a) the draws of its pass's stride-1 buckets, one a template, its clusters
   * kept by the cull's frustum ({@link LitBodyRenderer.cullFigures}).
   */
  meshDraws(plan: BodyFramePlan): DrawItem[] {
    const firstInstance = this.#upload(plan);
    const resources = this.#resources;
    const drawn = this.#drawnPlan(plan);
    const { records } = drawn;
    const passes = this.cullFigures(plan);
    const lists = resources.clusters.buckets?.lists;
    return plan.meshes.flatMap((body, i) => {
      const record = records[body.index];
      if (record === undefined) {
        throw new Error(`the plan's mesh ${i} has no record ${body.index}`);
      }
      const first = firstInstance[i];
      if (first === undefined) {
        throw new Error(`the plan's mesh ${i} has no first instance`);
      }
      if (body.mesh.patches.length === 0) {
        return [];
      }
      const coverage = coverageOf(body.role);
      const forms = formsOf(record, drawn, resources);
      const pass = passes.get(i);
      const uniforms = {
        disc: new Float32Array([body.index]),
        // A bucket's entry names its instance in the frame's records.
        firstInstance: new Float32Array([pass === undefined ? first : 0]),
        bodyRotation: rotationColumns(body.mesh.axes),
        coverageAxes: rotationColumns(coverage?.rotation ?? IDENTITY_ROTATION),
      };
      const textures = {
        ...this.#recordTextures(record),
        coverageMask: coverage?.mask ?? resources.noCoverage,
      };
      const storageBuffers = {
        discs: resources.discs,
        slots: resources.slots,
        instances: resources.instances,
        cellSums: resources.cellSums,
      };
      if (pass !== undefined && lists !== undefined) {
        return resources.clusters.drawItems(
          pass,
          resources.clusterMesh,
          {
            material: forms.meshCluster,
            offsetFromCameraM: new Float32Array(3),
            uniforms,
            textures,
            storageBuffers: { ...storageBuffers, clusterLists: lists },
          },
          1,
        );
      }
      return [
        {
          mesh: resources.patch,
          material: forms.mesh,
          offsetFromCameraM: new Float32Array(3),
          uniforms,
          textures,
          storageBuffers,
          instanceCount: body.mesh.patches.length,
        },
      ];
    });
  }

  /**
   * Culls a plan's figures' clusters into their buckets on the cluster path (plan R14, R14.T4.a),
   * once a plan, and returns each culled mesh's pass by its index in the plan's meshes; none where
   * the geometry keeps the instanced draw or the cull's kernel is not made.
   *
   * @remarks
   * Each mesh body with patches, up to the cull's {@link MAX_CULL_PASSES}, is a camera pass of its
   * own, its frustum its figure's camera's, each side opened by {@link FIGURE_CULL_MARGIN_RAD}, no
   * horizon (selection's own refuses its patches behind it), at stride 1, its clusters appended:
   * every fragment of a figure's pixel shades the same colour from the pixel's own rays, and the
   * depth test keeps the nearest, so the buckets' order leaves the image as the instanced draw's.
   * Each instance enters its body's pass alone. A body past the passes draws instanced. The
   * dispatch is submitted at the call, before the `bodies` pass that draws the buckets.
   */
  cullFigures(plan: BodyFramePlan): ReadonlyMap<number, number> {
    if (this.#culled?.plan === plan) {
      return this.#culled.passes;
    }
    const resources = this.#resources;
    const geometry = this.#geometry;
    const kernel = resources.cullKernel;
    const passes = new Map<number, number>();
    this.#culled = { plan, passes };
    if (!geometry.clusterPath || !geometry.figureClusters || kernel === null) {
      return passes;
    }
    this.#upload(plan);
    const count = plan.meshes.reduce((sum, body) => sum + body.mesh.patches.length, 0);
    if (count === 0) {
      return passes;
    }
    const inputs = new Uint32Array(2 * count);
    const cullPasses: CullPass[] = [];
    let instance = 0;
    plan.meshes.forEach((body, i) => {
      const patches = body.mesh.patches.length;
      if (patches > 0 && cullPasses.length < MAX_CULL_PASSES) {
        passes.set(i, cullPasses.length);
        for (let n = instance; n < instance + patches; n += 1) {
          inputs[2 * n] = 1 << cullPasses.length;
        }
        cullPasses.push({
          kind: "camera",
          planes: frustumOf(body.mesh.camera, FIGURE_CULL_MARGIN_RAD).planes,
          horizon: null,
          order: "appended",
          // Unread: no figure's record is a DAG's.
          toleranceRad: 0,
          strideCap: 1,
        });
      }
      instance += patches;
    });
    const records = new Float32Array(packSmoothClusterRecords(plan.meshes.map(({ mesh }) => mesh)));
    resources.clusterRecords = this.#writeClusterRecords(resources.clusterRecords, records);
    resources.clusters.cull(
      kernel,
      {
        records: resources.clusterRecords,
        instances: resources.instances,
        count,
        inputs,
        passes: cullPasses,
        rotationColumns: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
        viewAxis: vec3(0, 0, -1),
      },
      FIGURE_CLUSTER_CULL_LABEL,
    );
    return passes;
  }

  /** Writes the figures' cluster records, replacing the buffer by a doubled one if it is too small. */
  #writeClusterRecords(handle: BufferHandle, data: Float32Array): BufferHandle {
    let target = handle;
    if (data.byteLength > handle.bytes) {
      let size = handle.bytes;
      while (size < data.byteLength) {
        size *= 2;
      }
      this.#engine.releaseBuffer(handle);
      target = this.#clusterBuffer(handle.name, size);
    }
    this.#engine.writeBuffer(target, 0, data);
    return target;
  }

  /** A run of points' one instanced draw, from the run's own buffer. */
  #pointDraw(sprites: ReadonlyArray<PointSprite>, run: number): DrawItem {
    const resources = this.#resources;
    const rows = new Float32Array(sprites.length * 8);
    sprites.forEach((sprite, i) => {
      rows.set(sprite.record, i * 8);
    });
    const existing =
      resources.sprites[run] ?? this.#buffer(`bodies:sprites ${run}`, MIN_BUFFER_BYTES);
    const buffer = this.#write(existing, rows);
    resources.sprites[run] = buffer;
    return {
      mesh: resources.quad,
      material: resources.sprite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
      instanceCount: sprites.length,
      storageBuffers: { sprites: buffer },
    };
  }

  /**
   * The draws of a plan, in its painter's order, to add to the scene target's submission: each
   * disc's two draws, each run of points, each host's draws and each mesh body's limb (its figure
   * is {@link LitBodyRenderer.meshDraws}', drawn before).
   *
   * @param hostDraws - R06's host-disc draws by star (`DiscFrame.draws`), placed at each host's
   *   entry in the order; a host with none (a sprite, or off the view) draws nothing there.
   */
  draws(
    plan: BodyFramePlan,
    hostDraws: ReadonlyMap<number, ReadonlyArray<DrawItem>> = new Map(),
  ): DrawItem[] {
    this.#upload(plan);
    const drawn = this.#drawnPlan(plan);
    const { records } = drawn;
    const draws: DrawItem[] = [];
    let run = 0;
    for (const step of plan.steps) {
      switch (step.kind) {
        case "host":
          draws.push(...(hostDraws.get(step.star) ?? []));
          break;
        case "disc": {
          const record = records[step.index];
          if (record === undefined) {
            throw new Error(`the plan's disc step ${step.index} has no record`);
          }
          draws.push(
            this.#discDraw(step.index, record, 0, DISC_LIMB_DEPTHS, drawn, null),
            this.#discDraw(step.index, record, 1, DISC_LIMB_DEPTHS, drawn, null),
          );
          break;
        }
        case "limb": {
          const body = plan.meshes[step.mesh];
          const record = body === undefined ? undefined : records[body.index];
          if (body === undefined || record === undefined) {
            throw new Error(`the plan's limb step ${step.mesh} has no mesh or record`);
          }
          draws.push(
            this.#discDraw(body.index, record, 1, body.limbDepths, drawn, coverageOf(body.role)),
          );
          break;
        }
        case "points":
          draws.push(this.#pointDraw(step.sprites, run));
          run += 1;
          break;
      }
    }
    return draws;
  }

  /**
   * Stops following device restores and releases the buffers and textures it made, the `disc
   * cells` pass's among them (R07.T8.d). Its materials and meshes stay the engine's, which has no
   * release for them; a class map's texture stays its surface's.
   */
  dispose(): void {
    this.#unsubscribe();
    this.#disposed = true;
    const resources = this.#resources;
    resources.clusters.dispose();
    if (resources.cullKernel !== null) {
      this.#engine.releaseCompute(resources.cullKernel);
      resources.cullKernel = null;
    }
    const buffers = [
      resources.clusterRecords,
      resources.discs,
      ...resources.sprites,
      resources.slots,
      resources.instances,
      resources.cellJobs,
      resources.cellGroups,
      resources.cellSums,
    ];
    for (const buffer of buffers) {
      this.#engine.releaseBuffer(buffer);
    }
    this.#engine.releaseTexture(resources.table);
    this.#engine.releaseTexture(resources.noClassMap);
    this.#engine.releaseTexture(resources.noCoverage);
    this.#engine.releaseTexture(resources.noDiscTable);
    this.#engine.releaseTexture(resources.noAtmosphere.body);
  }
}
