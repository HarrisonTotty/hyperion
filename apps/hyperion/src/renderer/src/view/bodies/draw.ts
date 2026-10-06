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
 * run of consecutive points through R06's HDR sprite (`POINT SPRITES HDR`). A disc shades from its
 * `DiscSurface` (T8.b): its photometry's uniform law, or R10's class map with a law per class; a
 * point keeps the photometry's law.
 *
 * A disc whose footprint overlaps geometry that writes depth (a mesh body, R10's terrain, a craft,
 * given as {@link BodyFrameOptions.depthWriters}) is promoted to the mesh regime for the frame
 * (Design note 2, `promoteOverlapping`, T9): its smooth figure (`smoothMesh.ts`) draws its wholly
 * covered pixels with depth in the `bodies` pass, before the painter's sequence, and its limb is the
 * disc's limb draw at its place in the sequence, on the limb's plane. Both shade from its disc
 * record, so a promoted disc and its mesh draw the same light.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { DiscSurface } from "../appearance/bodyAppearance";
import type { BodyFigure, BodyPhotometry } from "../appearance/fromWire";
import type { QualitySetting } from "../quality/qualitySetting";
import { patchMeshData } from "../terrain/gpu/resources";
import patchVertexWgsl from "../terrain/shaders/patchVertex.wgsl?raw";
import { PHASE_TABLE_SAMPLES, type PhotometricLaw } from "../appearance/law";
import { packPhaseFactorRows } from "../appearance/litBodyProbe";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { type Rotation3, rotateToBody } from "../coords/rotation";
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
import type { Rgb } from "../photometry/toneCurve";
import frameWgsl from "../shaders/frame.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import bodyDiscDrawWgsl from "../shaders/bodyDiscDraw.wgsl?raw";
import smoothMeshWgsl from "../shaders/smoothMesh.wgsl?raw";
import { sphereOutsideView, sphereScreenRect, WIREFRAME_MESHES } from "../wireframe/submit";
import {
  type DiscLight,
  type DiscOccluder,
  type DiscRecord,
  type DiscSecondary,
  type DrawnDiscSurface,
  LIMB_SAMPLES,
  MAX_DISC_LIGHTS,
  MAX_DISC_OCCLUDERS,
  packDiscRecords,
  SMALL_DISC_PX,
  SMALL_DISC_SAMPLES,
} from "./discShading";
import { CLASS_MAP_FORMAT, discSurfaceLaws } from "./discSurface";
import { bodyReflection, figurePole } from "./oblate";
import { type HostSphere, type PainterEntry, painterOrder } from "./painter";
import {
  type LitRegime,
  type LitSphere,
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
}

/** What the plan needs of the view. */
export interface BodyFrameOptions {
  readonly camera: ProjectionCamera;
  readonly viewport: Viewport;
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
 * disc's scaled A), so that the point and the disc draw one flux. Each star's light is cut by its
 * eclipse over the disc the camera sees (`discEclipseVisible`, T10.b), the share of the light the
 * disc regime's pixels sum, so that the flux stays continuous at the 3 px switch through an
 * eclipse. Each planetshine source adds its own term, as a point source at its centre's direction
 * and never eclipsed (Design note 7).
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
  const flux: [number, number, number] = [0, 0, 0];
  for (const light of lightsOf(body, hosts)) {
    const reflected = bodyReflection(body.figure, law, normalise(light.toStarM), toCamera);
    const visible = discEclipseVisible(
      light.host,
      body,
      blockers,
      toCamera,
      law.lommelSeeligerShare,
      k,
    );
    for (const ch of [0, 1, 2] as const) {
      flux[ch] += light.illuminance[ch] * solid * reflected[ch] * visible[ch];
    }
  }
  for (const source of secondaries) {
    const reflected = bodyReflection(body.figure, law, source.direction, toCamera);
    for (const ch of [0, 1, 2] as const) {
      flux[ch] += source.illuminance[ch] * solid * reflected[ch];
    }
  }
  return flux;
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
  const small =
    angularDiameterPx(body.centreM, a, options.camera, options.viewport) < SMALL_DISC_PX;
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
    interiorSamples: small ? SMALL_DISC_SAMPLES : 1,
    limbSamples: small ? SMALL_DISC_SAMPLES : LIMB_SAMPLES,
    surface,
    tableRows,
    exposureOverPi: options.exposureScale / Math.PI,
    lights,
    occluders: blockers,
    secondaries: shine,
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
 * A disc whose footprint overlaps one of `options.depthWriters` or a mesh body is promoted to a
 * mesh (`promoteOverlapping`). A mesh body's limb is drawn in the sequence as a disc's is, so the
 * order takes it as a disc (Design note 2); its figure's pixels are drawn before the sequence, with
 * depth. The order is the drawing's, by the drawn centres; each body is lit by its own lighting
 * frame where it has one (T10.a).
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
  const spheres: LitSphere[] = bodies.map((body) => ({
    id: body.id,
    centreM: body.centreM,
    radiusM: body.figure.equatorialRadiusM,
  }));
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
  const regimes = promoteOverlapping(
    litRegimes(spheres, options.camera, options.viewport, previous),
    footprints,
    options.depthWriters ?? [],
  );
  const hostSpheres: HostSphere[] = hosts.map((host) => ({
    star: host.disc.star,
    centreM: host.centreM,
    radiusM: host.disc.radius_m,
  }));
  // A mesh body's limb takes its place in the sequence as a disc's does.
  const ordered = new Map<BodyIdHex, LitRegime>(
    [...regimes].map(([id, regime]) => [id, regime === "mesh" ? "disc" : regime]),
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
    if (regimes.get(body.id) === "mesh") {
      const { camera, viewport } = options;
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
      });
    } else {
      steps.push({ kind: "disc", index: discs.length });
    }
    discs.push(record);
  }
  closeRun();
  return { regimes, order, discs, meshes, laws, steps };
}

const DISC_SOURCE = frameWgsl + litBodyWgsl + bodyDiscWgsl + bodyDiscDrawWgsl;

/** The disc's textures: the frame's phase table, and its class map (any texture where uniform). */
const DISC_TEXTURES = [
  { name: "phaseFactorTable", binding: 1, sampleType: "unfilterable-float" },
  { name: "classWeights", binding: 2, viewDimension: "2d-array" },
] as const;

/**
 * The disc draws' uniforms: the record, the draw (0 its wholly covered pixels, 1 its limb), and its
 * rectangle's corner depths (0 for a disc body; a mesh body's limb plane's, T9).
 */
const DISC_UNIFORMS = [
  { name: "disc", type: "u32" },
  { name: "edgePass", type: "u32" },
  { name: "depths", type: "vec4f" },
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
    storageBuffers: [{ name: "discs", binding: 0 }],
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
    storageBuffers: [{ name: "discs", binding: 0 }],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  },
};

const SMOOTH_MESH_SOURCE =
  frameWgsl + litBodyWgsl + bodyDiscWgsl + patchVertexWgsl + smoothMeshWgsl;

/**
 * The mesh regime's material (T9): R05's patches of a body's spheroid at zero height, one
 * instanced draw a body, opaque with depth, shading the pixels the body covers wholly from its
 * disc record (`shaders/smoothMesh.wgsl`).
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
  ],
  samplers: [],
  textures: DISC_TEXTURES,
  storageBuffers: [
    { name: "discs", binding: 0 },
    { name: "slots", binding: 3 },
    { name: "instances", binding: 4 },
  ],
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

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
  | "onRestored"
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
  /** A texel of no weight, bound where a disc's surface is uniform and never read. */
  readonly noClassMap: TextureHandle;
  /** The mesh regime's material and R05's 65 × 65 patch with skirts (T9). */
  readonly mesh: MaterialHandle;
  readonly patch: MeshHandle;
  /** The frame's smooth figures' slot and instance records. */
  slots: BufferHandle;
  instances: BufferHandle;
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
 * its limb's draw in the sequence. A plan's records are written once, by whichever of the two is
 * called first for it, so both must be called for a plan before its passes are submitted.
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
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#create();
      this.#tableLaws = [];
      this.#written = null;
    });
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
      noClassMap: engine.createTexture({
        name: "bodies:no class map",
        size: { width: 1, height: 1 },
        dimension: "2d",
        format: CLASS_MAP_FORMAT,
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING,
        category: "other",
      }),
      mesh: engine.createMaterial(SMOOTH_MESH_MATERIAL),
      patch: engine.createMesh({
        name: "bodies:smooth patch",
        ...patchMeshData(),
        topology: "triangle-list",
        attributes: {},
      }),
      slots: this.#buffer("bodies:mesh slots", MIN_BUFFER_BYTES),
      instances: this.#buffer("bodies:mesh instances", MIN_BUFFER_BYTES),
    };
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

  /** Writes `data` into a buffer, replacing it by a doubled one if it is too small. */
  #write(handle: BufferHandle, data: Float32Array): BufferHandle {
    let target = handle;
    if (data.byteLength > handle.bytes) {
      let bytes = handle.bytes;
      while (bytes < data.byteLength) {
        bytes *= 2;
      }
      this.#engine.releaseBuffer(handle);
      target = this.#buffer(handle.name, bytes);
    }
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

  /** One of a disc's two draws: 0 its wholly covered pixels, 1 its limb, at `depths`. */
  #discDraw(
    index: number,
    surface: DrawnDiscSurface,
    edgePass: 0 | 1,
    depths: LimbDepths,
  ): DrawItem {
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
      textures: { phaseFactorTable: resources.table, classWeights: this.#classWeights(surface) },
      storageBuffers: { discs: resources.discs },
    };
  }

  /** Writes a plan's phase table, records and smooth figures, once per plan. */
  #upload(plan: BodyFramePlan): ReadonlyArray<number> {
    if (this.#written?.plan === plan) {
      return this.#written.firstInstance;
    }
    const resources = this.#resources;
    this.#writeTable(plan.laws);
    resources.discs = this.#write(resources.discs, packDiscRecords(plan.discs));
    const records = packSmoothMeshes(plan.meshes.map(({ mesh }) => mesh));
    resources.slots = this.#write(resources.slots, records.slots);
    resources.instances = this.#write(resources.instances, records.instances);
    this.#written = { plan, firstInstance: records.firstInstance };
    return records.firstInstance;
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
      return [
        {
          mesh: resources.patch,
          material: resources.mesh,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            disc: new Float32Array([body.index]),
            firstInstance: new Float32Array([first]),
            bodyRotation: rotationColumns(body.mesh.axes),
          },
          textures: {
            phaseFactorTable: resources.table,
            classWeights: this.#classWeights(record.surface),
          },
          storageBuffers: {
            discs: resources.discs,
            slots: resources.slots,
            instances: resources.instances,
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
            this.#discDraw(step.index, record.surface, 0, DISC_LIMB_DEPTHS),
            this.#discDraw(step.index, record.surface, 1, DISC_LIMB_DEPTHS),
          );
          break;
        }
        case "limb": {
          const body = plan.meshes[step.mesh];
          const record = body === undefined ? undefined : plan.discs[body.index];
          if (body === undefined || record === undefined) {
            throw new Error(`the plan's limb step ${step.mesh} has no mesh or record`);
          }
          draws.push(this.#discDraw(body.index, record.surface, 1, body.limbDepths));
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

  /** Stops following device restores. The engine owns its resources' release. */
  dispose(): void {
    this.#unsubscribe();
  }
}
