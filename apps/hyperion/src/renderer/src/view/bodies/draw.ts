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
 * the promotion: its regime is `terrain`, drawn as a mesh body is, its figure the reference surface
 * beneath its terrain, discarded where its survey's coverage mask holds the ground surveyed
 * (R10.T10.c), and its terrain the view's own (`PhotorealFrame.terrain`); a view that draws none
 * draws its figure whole, as a mesh body's. The hand-over counts the pixels the view is presented
 * at ({@link BodyFrameOptions.presentedPx}, R10.T10.g), R07's 3 px switch the frame's own.
 *
 * A class-map disc whose surface carries the field's normal map (plan R10, R10.T10.g) shades its
 * surveyed ground with those normals, in its draws and in the point it switches with: its records'
 * flag (`packDiscRecords`' row 24) and the normal map's texture at binding 14 (`surfaceNormals`).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { DiscSurface } from "../appearance/bodyAppearance";
import type { BodyFigure, BodyPhotometry } from "../appearance/fromWire";
import type { QualitySetting } from "../quality/qualitySetting";
import { coverageMaskTextureSpec } from "../terrain/coverage";
import { patchMeshData } from "../terrain/gpu/resources";
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
  RenderEngine,
  TextureBindingSpec,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
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
import bodyDiscCellsWgsl from "../shaders/bodyDiscCells.wgsl?raw";
import bodyDiscDrawWgsl from "../shaders/bodyDiscDraw.wgsl?raw";
import smoothMeshWgsl from "../shaders/smoothMesh.wgsl?raw";
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
import { classMapPointReflection } from "./classMapPoint";
import { CLASS_MAP_FORMAT, discSurfaceLaws } from "./discSurface";
import { bodyReflection, figurePole } from "./oblate";
import { type HostSphere, type PainterEntry, painterOrder } from "./painter";
import {
  drawnAsMesh,
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
   * The GPU's coverage mask of its survey (plan R10, R10.T10.c; `terrain/coverage.ts`'
   * `CoverageMask.texture`) with the axes it is read in, given by a view that draws its terrain,
   * with which its figure, once it is handed over (`terrain`), is the reference surface beneath
   * that terrain: discarded wherever the mask holds a cell surveyed, so that the terrain alone
   * draws that ground (R10's Design note 12). Read by no other regime; absent, the figure is drawn
   * whole.
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
  /** The view's quality setting, which a mesh body's selection takes. */
  readonly setting: QualitySetting;
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

/** The coverage mask that discards a mesh body's figure: a handed-over body's, a promoted one none. */
function coverageOf(role: MeshBodyRole): MeshBodyCoverage | null {
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
}

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
    const reflected = reflection(normalise(light.toStarM));
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
    const reflected = reflection(source.direction);
    for (const ch of [0, 1, 2] as const) {
      flux[ch] += source.illuminance[ch] * solid * reflected[ch];
    }
  }
  return flux;
}

/**
 * What a point body reflects of a light from its direction towards the camera, uneclipsed: its
 * class map integrated (`classMapPointReflection`, R10.T10.f) where its surface is one and its
 * rotation is known, and otherwise its photometry's law over its figure (`bodyReflection`).
 */
function pointReflectionOf(body: LitBodyInput, toCamera: Vec3): (towardsLight: Vec3) => Rgb {
  const { surface, rotation } = body;
  if (surface?.kind === "class-map" && rotation !== undefined) {
    return (towardsLight) =>
      classMapPointReflection(body.figure, surface, rotation, towardsLight, toCamera);
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
  hosts: ReadonlyArray<PlacedLight>,
  occluders: ReadonlyArray<LightingBody>,
  neighbours: ReadonlyArray<LitNeighbour>,
  tableRows: ReadonlyArray<number>,
  options: BodyFrameOptions,
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
  const samples = discSamples(angularDiameterPx(body.centreM, a, options.camera, options.viewport));
  const lights: DiscLight[] = lightsOf(body, hosts).map((light) => {
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
  const secondaries = planetshineSources(body, neighbours, options.planetshine);
  const shine: DiscSecondary[] = secondaries.map((source) => ({
    direction: source.direction,
    distance: source.distanceM / a,
    radius: source.radiusM / a,
    illuminance: source.illuminance,
  }));
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
 * `handoverTauPx`) at its limb, in the centre pixels of the size the view is presented at
 * (`options.presentedPx`, plan R10, R10.T10.g), is handed over to terrain (`handOverRegimes`, plan
 * R10, R10.T10.e). The two switches now count different pixels: R07's 3 px switch (`litRegimes`)
 * the frame's own (`options.viewport`), in which the disc's summed flux meets the point's, and the
 * hand-over the presented ones, so that the resolution controller's internal scale, which moves the
 * first, never moves the second (decision-r10-t16-leans §13). A disc whose footprint
 * overlaps one of `options.depthWriters`, a mesh body or a body handed over is promoted to a mesh
 * (`promoteOverlapping`). A mesh body's limb, a handed-over body's included, is drawn in the
 * sequence as a disc's is, so the order takes it as a disc (Design note 2); its figure's pixels are
 * drawn before the sequence, with depth. The order is the drawing's, by the drawn centres; each
 * body is lit by its own lighting frame where it has one (T10.a).
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
  const footprints = new Map<BodyIdHex, ScreenCircle>();
  for (const sphere of spheres) {
    const footprint = sphereFootprint(
      sphere.centreM,
      sphere.radiusM,
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
    pixelAngleRad(camera, options.presentedPx),
    previous,
  );
  const regimes = promoteOverlapping(handed, footprints, options.depthWriters ?? []);
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
    const record = discRecordOf(body, drawn, hosts, occluders, neighbours, rows, options);
    if (record === null) {
      continue;
    }
    closeRun();
    const regime = regimes.get(body.id);
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
  return { regimes, order, discs, meshes, laws, steps };
}

/**
 * What every lit-body module composes before its own: R01's frame, R08's surface lighting (plan
 * R08, R08.T9.b: `preciseTrig.wgsl`, `common.wgsl`, `oblate.wgsl` and `surfaceLighting.wgsl`, in
 * place of `litBody.wgsl`'s stubs), the lit-body library, the cube sphere's texel lookup
 * (`cubeTexel.wgsl`, R10.T11) and the disc's shading.
 */
const LIT_SOURCE = frameWgsl + SURFACE_LIGHTING_WGSL + litBodyWgsl + cubeTexelWgsl + bodyDiscWgsl;

const DISC_SOURCE = LIT_SOURCE + bodyDiscDrawWgsl;

/** The group 2 binding of the first of the body's atmosphere tables (`bodyDisc.wgsl`), past 0–5. */
const ATMOSPHERE_FIRST_BINDING = 6;

/** The group 2 binding of a class map's normal map (`bodyDisc.wgsl`'s `surface_normals`, R10.T10.g). */
const SURFACE_NORMALS_BINDING = 14;

/**
 * The disc's textures: the frame's phase table, its class map (any texture where uniform), its
 * body's atmosphere tables at bindings 6 to 12 (plan R08, R08.T9.b), the one-texel defaults for a
 * body without one, and its class map's normal map at binding 14 (plan R10, R10.T10.g), any
 * texture where it has none (its record's flag then 0, and the texture unread).
 */
const DISC_TEXTURES: ReadonlyArray<TextureBindingSpec> = [
  { name: "phaseFactorTable", binding: 1, sampleType: "unfilterable-float" },
  { name: "classWeights", binding: 2, viewDimension: "2d-array" },
  ...surfaceLightingTextures(ATMOSPHERE_FIRST_BINDING),
  { name: "surfaceNormals", binding: SURFACE_NORMALS_BINDING, viewDimension: "2d-array" },
];

/**
 * The disc draws' uniforms: the record, the draw (0 its wholly covered pixels, 1 its limb), and its
 * rectangle's corner depths (0 for a disc body; a mesh body's limb plane's, T9).
 */
const DISC_UNIFORMS = [
  { name: "disc", type: "u32" },
  { name: "edgePass", type: "u32" },
  { name: "depths", type: "vec4f" },
] as const;

/**
 * The disc draws' storage: the records, and the `disc cells` pass's sums (R07.T8.d), which a disc
 * under 32 px reads in place of summing its own cells.
 */
const DISC_STORAGE = [
  { name: "discs", binding: 0 },
  { name: "cellSums", binding: 5 },
] as const;

/** The disc's two materials: its wholly covered pixels, opaque, and its limb, premultiplied. */
export const BODY_DISC_MATERIALS: Readonly<Record<"interior" | "limb", WgslMaterialSpec>> = {
  interior: {
    name: "bodies:disc",
    displayName: "BODY DISCS",
    vertexWgsl: DISC_SOURCE,
    fragmentWgsl: DISC_SOURCE,
    uniforms: DISC_UNIFORMS,
    samplers: [],
    textures: DISC_TEXTURES,
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
    textures: DISC_TEXTURES,
    storageBuffers: DISC_STORAGE,
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  },
};

const SMOOTH_MESH_SOURCE = LIT_SOURCE + patchVertexWgsl + smoothMeshWgsl;

/** The group 2 binding of a mesh body's coverage mask (`smoothMesh.wgsl`, R10.T10.c), past the disc's. */
const COVERAGE_MASK_BINDING = 13;

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
  textures: [
    ...DISC_TEXTURES,
    { name: "coverageMask", binding: COVERAGE_MASK_BINDING, viewDimension: "2d-array" },
  ],
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
 * The `disc cells` pass's label (R07.T8.d), between R06's sky and the mesh bodies' figures, which
 * read its sums as the `discs` pass does (`PHOTOREAL_PASS_LABELS.discCells`).
 */
export const DISC_CELLS_PASS = "disc cells";

/**
 * The `disc cells` kernel (R07.T8.d, decision-r07-small-disc-cost §1.2; `shaders/bodyDiscCells.wgsl`):
 * every cell of every pixel of the frame's small discs shaded at once, one invocation a cell, and
 * summed in each pixel's order for the draws to read.
 *
 * @remarks
 * Its one source serves both capability paths: it has no subgroup variant. Its sums are a pure
 * function of its inputs, so its readback is `bit-exact`, though nothing reads them back. It binds
 * the one-texel atmosphere defaults (plan R08, R08.T9.b): a disc with an atmosphere sums its own
 * cells in its draws ({@link drawnThroughCells}), since one dispatch binds one set of tables.
 */
export const BODY_DISC_CELLS_KERNEL: KernelPair = {
  name: "BODY DISC CELLS",
  reference: LIT_SOURCE + bodyDiscCellsWgsl,
  subgroup: null,
  readback: "bit-exact",
};

/** The workgroups a dispatch lays along x at most, the kernel's `JOBS_PER_ROW`. */
const CELL_JOBS_PER_ROW = 65_535;

/**
 * The `disc cells` dispatch's workgroups for `jobs` jobs, one a job: (min(J, 65,535),
 * ⌈J ÷ 65,535⌉, 1), within WebGPU's 65,535 a dimension; the kernel leaves the last row's excess.
 */
export function cellWorkgroups(jobs: number): readonly [number, number, number] {
  return [Math.min(jobs, CELL_JOBS_PER_ROW), Math.ceil(jobs / CELL_JOBS_PER_ROW), 1];
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
  /** The mesh regime's material and R05's 65 × 65 patch with skirts (T9). */
  readonly mesh: MaterialHandle;
  readonly patch: MeshHandle;
  /** The frame's smooth figures' slot and instance records. */
  slots: BufferHandle;
  instances: BufferHandle;
  /** The `disc cells` pass's jobs and the pixels' sums it leaves (R07.T8.d). */
  cellJobs: BufferHandle;
  cellSums: BufferHandle;
}

/**
 * Binds a frame's lit-body plan to the engine (plan R07, T8.a).
 *
 * @remarks
 * It makes its materials, the quad, the patch, the disc buffer and the phase table on construction
 * and again after a device loss; buffers grow by doubling and the table by rows. Each disc is two
 * draws, its wholly covered pixels and then its limb; each run of points one draw of
 * `spriteMaterial`, an HDR twin of R02's star sprite reading `sprites` at binding 0. A mesh body is
 * one instanced draw of its figure ({@link LitBodyRenderer.meshDraws}, for the `bodies` pass) and
 * its limb's draw in the sequence. The small discs' cells are shaded first by the `disc cells`
 * pass ({@link LitBodyRenderer.dispatchCells}, R07.T8.d), whose sums their draws read. A plan's
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
    });
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
      [resources.interior, resources.quad],
      [resources.limb, resources.quad],
    ];
    for (const [material, mesh] of prepared) {
      void this.#engine.prepareMaterial(material, scene, [mesh]).catch((error: unknown) => {
        // A loss meanwhile is no failure: the restore makes and prepares them again.
        if (!(error instanceof EngineUnavailable)) {
          console.error(`the pipelines of ${material.name} could not be prepared:`, error);
        }
      });
    }
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
      slots: this.#buffer("bodies:mesh slots", MIN_BUFFER_BYTES),
      instances: this.#buffer("bodies:mesh instances", MIN_BUFFER_BYTES),
      cellJobs: this.#buffer("bodies:disc cell jobs", MIN_BUFFER_BYTES),
      cellSums: this.#buffer("bodies:disc cell sums", MIN_BUFFER_BYTES),
    };
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
  #write(handle: BufferHandle, data: Float32Array | Int32Array): BufferHandle {
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
    };
  }

  /** One of a disc's two draws: 0 its wholly covered pixels, 1 its limb, at `depths`. */
  #discDraw(index: number, record: DiscRecord, edgePass: 0 | 1, depths: LimbDepths): DrawItem {
    const resources = this.#resources;
    return {
      mesh: resources.quad,
      material: edgePass === 0 ? resources.interior : resources.limb,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {
        disc: new Float32Array([index]),
        edgePass: new Float32Array([edgePass]),
        depths: new Float32Array(depths),
      },
      textures: this.#recordTextures(record),
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
    resources.discs = this.#write(resources.discs, packDiscRecords(plan.discs, cells));
    const records = packSmoothMeshes(plan.meshes.map(({ mesh }) => mesh));
    resources.slots = this.#write(resources.slots, records.slots);
    resources.instances = this.#write(resources.instances, records.instances);
    this.#written = { plan, firstInstance: records.firstInstance };
    return records.firstInstance;
  }

  /**
   * Shades a plan's small discs' cells in parallel: the `disc cells` pass (R07.T8.d,
   * decision-r07-small-disc-cost §1.2), one dispatch of `kernel` ({@link BODY_DISC_CELLS_KERNEL})
   * for every disc whose pixels all take the same cells (`drawnThroughCells`: every disc under 32
   * px of one law), and none where the plan has no such disc.
   *
   * @remarks
   * It writes the plan's records with each such disc's sums in row 51, and is called for a plan
   * before {@link LitBodyRenderer.meshDraws} and {@link LitBodyRenderer.draws}, whose submissions
   * then follow the dispatch's, after the sky's: each `render` and `dispatch` is its own queue
   * submission, in call order. A plan already written by either of those is left to sum its own
   * cells, and nothing is dispatched. Its workgroups are {@link cellWorkgroups}' for its jobs, one
   * a pixel.
   *
   * @param frame - The `discs` pass's own frame, which the kernel shades in.
   * @returns Whether it dispatched.
   */
  dispatchCells(plan: BodyFramePlan, kernel: ComputeHandle, frame: DiscCellFrame): boolean {
    if (this.#written?.plan === plan) {
      return false;
    }
    // As many pixels' sums as one storage binding holds; a disc past them sums its own cells.
    const fits = Math.floor(this.#engine.capabilities.maxStorageBufferBindingSize / CELL_SUM_BYTES);
    const cells = discCellJobs(plan.discs, fits);
    if (cells.count === 0) {
      this.#upload(plan);
      return false;
    }
    this.#upload(plan, cells);
    const resources = this.#resources;
    resources.cellJobs = this.#write(resources.cellJobs, cells.jobs);
    resources.cellSums = this.#reserve(resources.cellSums, cells.count * CELL_SUM_BYTES);
    this.#engine.dispatch(
      kernel,
      {
        uniforms: {
          frame: discCellFrameBlock(frame),
          cell_pass: new Uint32Array([cells.count, 0, 0, 0]),
        },
        buffers: {
          discs: resources.discs,
          cell_jobs: resources.cellJobs,
          cell_sums: resources.cellSums,
        },
        sampled: {
          phase_factor_table: resources.table,
          class_weights: resources.noClassMap,
          ...surfaceLightingKernelTextures(resources.noAtmosphere),
          // Reached through `shade`, never read: the pass's discs are uniform (R10.T10.g).
          surface_normals: resources.noClassMap,
        },
        storage: {},
      },
      cellWorkgroups(cells.count),
      DISC_CELLS_PASS,
    );
    return true;
  }

  /**
   * The draws of a plan's mesh bodies, one instanced draw of its figure each, for the `bodies`
   * pass: opaque, with depth, before the painter's sequence (Design note 8).
   */
  meshDraws(plan: BodyFramePlan): DrawItem[] {
    const firstInstance = this.#upload(plan);
    const resources = this.#resources;
    return plan.meshes.flatMap((body, i) => {
      const record = plan.discs[body.index];
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
      return [
        {
          mesh: resources.patch,
          material: resources.mesh,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            disc: new Float32Array([body.index]),
            firstInstance: new Float32Array([first]),
            bodyRotation: rotationColumns(body.mesh.axes),
            coverageAxes: rotationColumns(coverage?.rotation ?? IDENTITY_ROTATION),
          },
          textures: {
            ...this.#recordTextures(record),
            coverageMask: coverage?.mask ?? resources.noCoverage,
          },
          storageBuffers: {
            discs: resources.discs,
            slots: resources.slots,
            instances: resources.instances,
            cellSums: resources.cellSums,
          },
          instanceCount: body.mesh.patches.length,
        },
      ];
    });
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
    const draws: DrawItem[] = [];
    let run = 0;
    for (const step of plan.steps) {
      switch (step.kind) {
        case "host":
          draws.push(...(hostDraws.get(step.star) ?? []));
          break;
        case "disc": {
          const record = plan.discs[step.index];
          if (record === undefined) {
            throw new Error(`the plan's disc step ${step.index} has no record`);
          }
          draws.push(
            this.#discDraw(step.index, record, 0, DISC_LIMB_DEPTHS),
            this.#discDraw(step.index, record, 1, DISC_LIMB_DEPTHS),
          );
          break;
        }
        case "limb": {
          const body = plan.meshes[step.mesh];
          const record = body === undefined ? undefined : plan.discs[body.index];
          if (body === undefined || record === undefined) {
            throw new Error(`the plan's limb step ${step.mesh} has no mesh or record`);
          }
          draws.push(this.#discDraw(body.index, record, 1, body.limbDepths));
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
    const resources = this.#resources;
    const buffers = [
      resources.discs,
      ...resources.sprites,
      resources.slots,
      resources.instances,
      resources.cellJobs,
      resources.cellSums,
    ];
    for (const buffer of buffers) {
      this.#engine.releaseBuffer(buffer);
    }
    this.#engine.releaseTexture(resources.table);
    this.#engine.releaseTexture(resources.noClassMap);
    this.#engine.releaseTexture(resources.noCoverage);
    this.#engine.releaseTexture(resources.noAtmosphere.body);
  }
}
