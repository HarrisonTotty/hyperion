/**
 * The photorealistic style's `ocean` pass (plan R11, R11.T8.b; Design notes 11, 12 and 18): its
 * material, the uniforms and draw of each body's far sea, and the glint's light above the display's
 * range as each sea's glare source.
 *
 * @remarks
 * - **Where it is drawn.** After the opaque passes (the bodies', the terrain's and their late and
 *   resolve passes) and before R07's `discs` and R08's composite, one full-screen draw a body whose
 *   sea the frame draws: a body drawn as a mesh or handed over to terrain (`drawnAsMesh`), whose
 *   rotation is known and whose sea the terrain session holds (`TerrainSession.oceans`); a disc's sea
 *   is not drawn (Design note 11's disc-statistical glint is its own later work). The shader
 *   (`oceanPass.wgsl`) writes the sea's depth, so that the composite lays the air over the sea.
 * - **Its light** at each hit is each of the body's two brightest sources (`skySources`' order, its
 *   light slot with it) through R08's surface lighting at the sea's height above the datum, the body's
 *   air's tables where the frame draws them and none (`createAirlessTables`) elsewhere; the texels are
 *   pre-exposed, the glint capped at the bloom threshold in the pre-exposed units.
 * - **Eclipses.** Where the plan holds the body's disc record, its sources are the record's lights
 *   (the brightest first, each its light slot), and their eclipses by the record's occluders are
 *   drawn per pixel ({@link packOceanEclipse}, `oceanPass.wgsl`'s `ocean_source_visible`, R07's
 *   eclipse term), the sky's light sharing the source's; a body without a record takes `skySources`'
 *   order and no eclipse.
 * - **The glare source** ({@link OceanGlare}): `glint.ts`' `oceanGlintGlare` of the brightest source,
 *   at the specular point of the sea-level sphere taken down onto the sea-level spheroid along its
 *   radius, so that its air lines start at the sea: its illuminance carried to that point through
 *   the air and dimmed by its eclipse there, the excess carried from it to the camera, each by
 *   `glareLineAir` over the sea-level spheroid (a lean, 2026-11-18: the medium's heights are then
 *   taken from the sea, not the datum, a sea level's shift of the air), formed in the body-fixed
 *   axes and kept while the camera, the source, the cap, the air and the map stand within
 *   {@link GLARE_HOLD} of where it was formed, its direction taken afresh each frame.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { AbsorberCurve } from "../atmosphere/absorbers";
import type { AtmosphereCamera, SunState } from "../atmosphere/hillaire";
import type { AtmosphereMedium } from "../atmosphere/medium";
import type { DiscLight, DiscOccluder } from "../bodies/discShading";
import { MAX_DISC_OCCLUDERS } from "../bodies/discShading";
import {
  SURFACE_LIGHTING_WGSL,
  type SurfaceLightingTables,
  surfaceLightingDrawTextures,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import type { Quaternion } from "../camera/pose";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import { conjugate, IDENTITY_QUATERNION, rotate } from "../camera/quaternion";
import type {
  BufferHandle,
  DrawItem,
  MeshHandle,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import { annulusVisibleFraction, occultationFrom } from "../lighting/annuli";
import type { GlareSource } from "../post/glare";
import { glareLineAir } from "../post/glareAir";
import type { Rgb } from "../photometry/toneCurve";
import frameWgsl from "../shaders/frame.wgsl?raw";
import { oceanGlintGlare, specularPoint } from "./glint";
import { oceanFarOpticsOf, type OceanSea } from "./oceanBody";
import type { OceanFarOptics } from "./oceanFar";
import { OCEAN_FAR_WGSL, packOceanFar } from "./oceanFarProbe";
import oceanPassWgsl from "./oceanPass.wgsl?raw";

/** The pass's label in `PassTimes` and R12's `PASS_ROWS` (Design note 18's `ocean`). */
export const OCEAN_PASS_LABEL = "ocean";

/** The pass's WGSL: `frame.wgsl`, R08's surface lighting, the far ocean's library and the pass. */
export const OCEAN_PASS_WGSL = `${frameWgsl}\n${SURFACE_LIGHTING_WGSL}\n${OCEAN_FAR_WGSL}\n${oceanPassWgsl}`;

/** The material's bindings in `@group(2)` (`oceanPass.wgsl`'s). */
export const OCEAN_PASS_BINDINGS = {
  oceanMap: 0,
  /** The first of R08's surface lighting's nine tables (`SURFACE_LIGHTING_BINDINGS`). */
  atmosphere: 1,
  /** The eclipse rows ({@link packOceanEclipse}), a storage buffer. */
  eclipse: 10,
} as const;

/** The name a draw binds its eclipse rows under. */
export const OCEAN_ECLIPSE_BUFFER = "oceanEclipse";

/** The name a draw binds its body's ocean map under. */
export const OCEAN_MAP_TEXTURE = "oceanMap";

/** The draw's `Draw` members after `offsetFromCameraM`, each a `vec4f` (`oceanPass.wgsl`'s). */
const OCEAN_MEMBERS = [
  "camera",
  "viewX",
  "viewY",
  "viewZ",
  "datum",
  "source0",
  "sourceLight0",
  "source1",
  "sourceLight1",
  "farFigure",
  "farSun",
  "farSunLux",
  "farSkyLux",
  "farIndex",
  "farAttenuation",
  "farDeep",
  "farBed",
] as const;

/**
 * The far sea's material: a full-screen triangle, opaque, writing each hit's depth, tested against
 * the scene's (`oceanPass.wgsl`).
 */
export const OCEAN_FAR_PASS_MATERIAL: WgslMaterialSpec = {
  name: "ocean far pass",
  displayName: "OCEAN",
  vertexWgsl: OCEAN_PASS_WGSL,
  fragmentWgsl: OCEAN_PASS_WGSL,
  uniforms: OCEAN_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [],
  textures: [
    { name: OCEAN_MAP_TEXTURE, binding: OCEAN_PASS_BINDINGS.oceanMap, viewDimension: "2d-array" },
    ...surfaceLightingTextures(OCEAN_PASS_BINDINGS.atmosphere),
  ],
  storageBuffers: [{ name: OCEAN_ECLIPSE_BUFFER, binding: OCEAN_PASS_BINDINGS.eclipse }],
  cullMode: "none",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

/**
 * The share of its brightest source's light above which a sea's pixel is a lit body's in R07's
 * meter, not an unlit one's (`oceanPass.wgsl`'s OCEAN_LIT_SHARE): 10⁻⁶, past the eclipse term's
 * rounding in an umbra.
 */
export const OCEAN_LIT_SHARE = 1e-6;

/** The sources a sea's draw is lit by at most: its body's two brightest (`skySources`' order). */
export const OCEAN_SOURCES = 2;

/** The rows of a light in the eclipse rows: its place, its radius, and two a channel's annuli. */
export const OCEAN_ECLIPSE_LIGHT_ROWS = 8;

/** The eclipse rows' first occluder's row: after the header and {@link OCEAN_SOURCES} lights. */
export const OCEAN_ECLIPSE_FIRST_OCCLUDER = 1 + OCEAN_SOURCES * OCEAN_ECLIPSE_LIGHT_ROWS;

/** The eclipse rows: the header, the lights and R07's {@link MAX_DISC_OCCLUDERS} occluders. */
export const OCEAN_ECLIPSE_ROWS = OCEAN_ECLIPSE_FIRST_OCCLUDER + MAX_DISC_OCCLUDERS;

/**
 * A sea's eclipses (the module's remarks): its disc record's lights and occluders, turned into the
 * body-fixed axes, each place and size ÷ the datum's equatorial radius as the record holds them.
 */
export interface OceanEclipse {
  /** The record's lights, the brightest first; the first {@link OCEAN_SOURCES} are drawn. */
  readonly lights: ReadonlyArray<DiscLight>;
  readonly occluders: ReadonlyArray<DiscOccluder>;
  /** The datum's equatorial radius, m, by which the record's places are given. */
  readonly radiusM: number;
}

/**
 * A sea's eclipse rows as `oceanPass.wgsl` reads them, {@link OCEAN_ECLIPSE_ROWS} `vec4f`s: a header
 * of (lights, occluders, annuli a channel, a), each light's direction and distance ÷ a, its radius
 * ÷ a, and per channel its annuli's outer edges and fluxes, then each occluder's centre ÷ a and
 * radius ÷ a; `null`, none (no source is eclipsed).
 */
export function packOceanEclipse(eclipse: OceanEclipse | null): Float32Array<ArrayBuffer> {
  const out = new Float32Array(4 * OCEAN_ECLIPSE_ROWS);
  if (eclipse === null) {
    out[3] = 1;
    return out;
  }
  const lights = eclipse.lights.slice(0, OCEAN_SOURCES);
  const occluders = eclipse.occluders.slice(0, MAX_DISC_OCCLUDERS);
  out.set([
    lights.length,
    occluders.length,
    lights[0]?.annuli[0].flux.length ?? 0,
    eclipse.radiusM,
  ]);
  lights.forEach((light, j) => {
    const base = 4 * (1 + j * OCEAN_ECLIPSE_LIGHT_ROWS);
    out.set([light.direction.x, light.direction.y, light.direction.z, light.distance], base);
    out.set([0, 0, 0, light.radius], base + 4);
    light.annuli.forEach((set, c) => {
      out.set(
        [1, 2, 3, 4].map((edge) => set.edges[edge] ?? 1),
        base + 8 + 8 * c,
      );
      out.set(
        [0, 1, 2, 3].map((annulus) => set.flux[annulus] ?? 0),
        base + 12 + 8 * c,
      );
    });
  });
  occluders.forEach((occluder, k) => {
    const c = occluder.centre;
    out.set([c.x, c.y, c.z, occluder.radius], 4 * (OCEAN_ECLIPSE_FIRST_OCCLUDER + k));
  });
  return out;
}

/**
 * A sea's sources and eclipses from its body's disc record (the module's remarks): the record's
 * lights turned into the body-fixed axes by the inverse of `rotation` (body-fixed to galactic), each
 * light's slot its rank, those past `lightSlots` left out, and the record's occluders turned with
 * them.
 *
 * @param radiusM - The datum's equatorial radius, m, by which the record's places are given.
 */
export function oceanSourcesOfRecord(
  record: {
    readonly lights: ReadonlyArray<DiscLight>;
    readonly occluders: ReadonlyArray<DiscOccluder>;
  },
  rotation: Rotation3,
  radiusM: number,
  lightSlots: number,
): { readonly suns: ReadonlyArray<SunState>; readonly eclipse: OceanEclipse } {
  const lights = record.lights
    .slice(0, Math.min(OCEAN_SOURCES, lightSlots))
    .map((light): DiscLight => ({
      direction: rotateToBodyFixed(rotation, light.direction),
      distance: light.distance,
      radius: light.radius,
      illuminance: light.illuminance,
      annuli: light.annuli,
    }));
  return {
    suns: lights.map((light, j) => ({
      directionBodyFixed: light.direction,
      illuminanceLx: light.illuminance,
      angularRadiusRad: Math.asin(Math.min(light.radius / light.distance, 1)),
      curveSlot: j,
    })),
    eclipse: {
      lights,
      occluders: record.occluders.map((occluder): DiscOccluder => ({
        centre: rotateToBodyFixed(rotation, occluder.centre),
        radius: occluder.radius,
      })),
      radiusM,
    },
  };
}

/**
 * The share of light `j`'s light per channel that reaches `pointM` (from the body's centre,
 * body-fixed, m) past the eclipse's occluders, the twin of `oceanPass.wgsl`'s
 * `ocean_source_visible`: 1 for a light the eclipse does not hold (`annuli.ts`' terms).
 */
export function oceanSourceVisible(eclipse: OceanEclipse | null, j: number, pointM: Vec3): Rgb {
  const light = eclipse?.lights.slice(0, OCEAN_SOURCES)[j];
  if (eclipse === null || light === undefined) {
    return [1, 1, 1];
  }
  const star = { centreM: scale(light.direction, light.distance), radiusM: light.radius };
  const from = scale(pointM, 1 / eclipse.radiusM);
  const visible: [number, number, number] = [1, 1, 1];
  for (const occluder of eclipse.occluders.slice(0, MAX_DISC_OCCLUDERS)) {
    const occultation = occultationFrom(star, from, {
      centreM: occluder.centre,
      radiusM: occluder.radius,
    });
    if (occultation === "inside") {
      return [0, 0, 0];
    }
    if (occultation !== null) {
      light.annuli.forEach((set, c) => {
        visible[c] =
          (visible[c] ?? 1) -
          (1 - annulusVisibleFraction(set, occultation.ratio, occultation.separation));
      });
    }
  }
  return [Math.max(visible[0], 0), Math.max(visible[1], 0), Math.max(visible[2], 0)];
}

/** What a body's sea is drawn with in a frame. */
export interface OceanDrawView {
  readonly sea: OceanSea;
  /** The body's camera: from its centre and in its body-fixed axes (`bodyAtmosphereCamera`). */
  readonly camera: Pick<AtmosphereCamera, "positionM" | "orientation">;
  /** The body's gravity, m/s² (`oceanBody.ts`' remarks). */
  readonly gravityMps2: number;
  /** Whether the body's figure stands on its datum as a mesh (the `mesh` regime), not terrain. */
  readonly figureOnDatum: boolean;
  /** The sources that light it, brightest first; the first {@link OCEAN_SOURCES} are drawn. */
  readonly suns: ReadonlyArray<SunState>;
  /** The frame's pre-exposure, 1 ÷ (cd/m²). */
  readonly exposureScale: number;
  /** The bloom threshold in the pre-exposed units (`bloomThreshold`), the glint's cap. */
  readonly thresholdPreExposed: number;
  /** A pixel's angle at the view's centre, rad. */
  readonly pixelAngleRad: number;
  /** The setting's `whitecaps`. */
  readonly whitecaps: boolean;
  /** Its eclipses, where the plan holds its disc record; `null`, none. */
  readonly eclipse: OceanEclipse | null;
}

/** No light: an unlit sea's source rows. */
const DARK: Rgb = [0, 0, 0];

/**
 * The far ocean's optics of a sea in a frame, in cd/m² and lux, by the brightest source's light
 * face-on and no sky (the probe's own light, which the pass's draw replaces at each hit by its
 * sources' through the air): the twin's view of the draw, and the glare source's.
 */
export function oceanDrawOptics(view: OceanDrawView): OceanFarOptics {
  const sun = view.suns[0];
  return oceanFarOpticsOf(view.sea, {
    gravityMps2: view.gravityMps2,
    pixelAngleRad: view.pixelAngleRad,
    whitecaps: view.whitecaps,
    sunDirection: sun?.directionBodyFixed ?? vec3(0, 0, 1),
    glintCapCdM2: view.thresholdPreExposed / view.exposureScale,
    sunLux: sun?.illuminanceLx ?? DARK,
    skyLux: DARK,
  });
}

/**
 * A sea's draw's uniforms, formed in `f64` and narrowed once: its camera and the view's axes in the
 * body-fixed ones, the datum and sea level, its two brightest sources pre-exposed, and its
 * `OceanFar` with its light and cap pre-exposed too (`oceanPass.wgsl`'s `Draw`).
 */
export function oceanPassUniforms(view: OceanDrawView): Readonly<Record<string, Float32Array>> {
  const { camera, sea, exposureScale: e } = view;
  const o = camera.positionM;
  const axis = (v: Vec3): Float32Array => {
    const r = rotate(camera.orientation, v);
    return new Float32Array([r.x, r.y, r.z, 0]);
  };
  const sources = view.suns.slice(0, OCEAN_SOURCES);
  const source = (i: number): readonly [Float32Array, Float32Array] => {
    const sun = sources[i];
    if (sun === undefined) {
      return [new Float32Array([0, 0, 1, 0]), new Float32Array(4)];
    }
    const d = sun.directionBodyFixed;
    const [r, g, b] = sun.illuminanceLx;
    return [
      new Float32Array([d.x, d.y, d.z, sun.curveSlot ?? 0]),
      new Float32Array([r * e, g * e, b * e, 0]),
    ];
  };
  const [source0, sourceLight0] = source(0);
  const [source1, sourceLight1] = source(1);
  const optics = oceanDrawOptics(view);
  const exposed = (rgb: Rgb): Rgb => [rgb[0] * e, rgb[1] * e, rgb[2] * e];
  const far = packOceanFar({
    ...optics,
    glintCapCdM2: view.thresholdPreExposed,
    sunLux: exposed(optics.sunLux),
    skyLux: exposed(optics.skyLux),
  });
  const row = (k: number): Float32Array => far.slice(4 * k, 4 * k + 4);
  return {
    camera: new Float32Array([o.x, o.y, o.z, view.figureOnDatum ? 1 : 0]),
    viewX: axis(vec3(1, 0, 0)),
    viewY: axis(vec3(0, 1, 0)),
    viewZ: axis(vec3(0, 0, 1)),
    datum: new Float32Array([
      sea.equatorialRadiusM,
      sea.polarRadiusM,
      sea.seaLevelM,
      sources.length,
    ]),
    source0,
    sourceLight0,
    source1,
    sourceLight1,
    farFigure: row(0),
    farSun: row(1),
    farSunLux: row(2),
    farSkyLux: row(3),
    farIndex: row(4),
    farAttenuation: row(5),
    farDeep: row(6),
    farBed: row(7),
  };
}

/** A pixel's ray as the pass forms it ({@link oceanPassRay}). */
export interface OceanPassRay {
  /** The unit direction, body-fixed. */
  readonly direction: Vec3;
  /** |r| of the view-space ray r = (x, y, −1), by which a distance along it is a view depth. */
  readonly viewLength: number;
}

/**
 * The ray through the centre of pixel (`xPx`, `yPx`) of an output of `sizePx`, the twin of
 * `oceanPass.wgsl`'s: through `projection` (column-major, reversed-Z, its off-axis terms included),
 * r = ((x_ndc + P₂₀) ÷ P₀₀, (y_ndc + P₂₁) ÷ P₁₁, −1) in the view's axes, turned into the body-fixed
 * ones by the camera's `orientation`.
 */
export function oceanPassRay(
  xPx: number,
  yPx: number,
  sizePx: { readonly widthPx: number; readonly heightPx: number },
  projection: ArrayLike<number>,
  orientation: Quaternion,
): OceanPassRay {
  const p = (k: number): number => projection[k] ?? Number.NaN;
  const ndcX = ((xPx + 0.5) / sizePx.widthPx) * 2 - 1;
  const ndcY = 1 - ((yPx + 0.5) / sizePx.heightPx) * 2;
  const view = vec3((ndcX + p(8)) / p(0), (ndcY + p(9)) / p(5), -1);
  const viewLength = norm(view);
  return { direction: normalise(rotate(orientation, view)), viewLength };
}

/**
 * The reversed-Z depth the pass writes for a hit `tM` along a ray of {@link OceanPassRay.viewLength}
 * through `projection`: n ÷ (−z), −z = t ÷ |r|, held to [0, 1].
 */
export function oceanPassDepth(
  tM: number,
  viewLength: number,
  projection: ArrayLike<number>,
): number {
  const nearM = projection[14] ?? Number.NaN;
  return Math.min(Math.max((nearM * viewLength) / tM, 0), 1);
}

/**
 * A sea's draw: the full-screen triangle with its uniforms, over its ocean map, its air's tables and
 * its eclipse rows ({@link packOceanEclipse}, written by the caller).
 */
export function oceanPassDraw(
  triangle: MeshHandle,
  material: DrawItem["material"],
  oceanMap: TextureHandle,
  tables: SurfaceLightingTables,
  eclipseRows: BufferHandle,
  uniforms: Readonly<Record<string, Float32Array>>,
): DrawItem {
  return {
    mesh: triangle,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms,
    textures: { [OCEAN_MAP_TEXTURE]: oceanMap, ...surfaceLightingDrawTextures(tables) },
    storageBuffers: { [OCEAN_ECLIPSE_BUFFER]: eclipseRows },
  };
}

/** A body's air as the glare's lines cross it: its medium and its brightest source's curves. */
export interface OceanGlareAir {
  readonly medium: AtmosphereMedium;
  readonly curves: ReadonlyArray<AbsorberCurve>;
}

/**
 * How far the glare source's inputs may move before it is formed again (the module's remarks): the
 * camera by 10⁻³ of its distance from the body's centre (7 km in a low orbit, the specular point
 * moved by half of it), the source's direction by 10⁻³ rad, its light and the cap by 5% (a lean,
 * 2026-11-18: a forming's 64² grid of `oceanGlintGlare` took 10 ms on a pod's core, so it is formed
 * seldom, its direction taken afresh each frame).
 */
export const GLARE_HOLD = { position: 1e-3, directionRad: 1e-3, light: 0.05 } as const;

/** A glare source formed in a body's axes, held across frames. */
interface HeldGlare {
  readonly cameraM: Vec3;
  readonly sun: Vec3;
  readonly lightLx: number;
  readonly capCdM2: number;
  readonly texels: object;
  /** The air's parts it was formed through, each the tables' own object, compared by identity. */
  readonly medium: AtmosphereMedium | null;
  readonly curves: ReadonlyArray<AbsorberCurve> | null;
  readonly eclipse: OceanEclipse | null;
  /** The source, its direction as the body-fixed point it lies at; `null` for none. */
  readonly glare: {
    readonly pointM: Vec3;
    readonly angularRadiusRad: number;
    readonly excessLuminance: Rgb;
    readonly luminance: Rgb;
  } | null;
}

/** Whether `a` and `b` differ by at most `share` of the larger. */
function near(a: number, b: number, share: number): boolean {
  return Math.abs(a - b) <= share * Math.max(Math.abs(a), Math.abs(b));
}

/**
 * Whether two eclipses hold the same occluders within {@link GLARE_HOLD}'s share of the datum's
 * radius, and the same lights' directions within its angle; two nones are the same.
 */
function sameEclipse(a: OceanEclipse | null, b: OceanEclipse | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  return (
    a.lights.length === b.lights.length &&
    a.occluders.length === b.occluders.length &&
    a.occluders.every((occluder, k) => {
      const other = b.occluders[k];
      return (
        other !== undefined &&
        norm(sub(occluder.centre, other.centre)) <= GLARE_HOLD.position * norm(occluder.centre)
      );
    }) &&
    a.lights.every((light, j) => {
      const other = b.lights[j];
      return (
        other !== undefined &&
        norm(sub(light.direction, other.direction)) <= GLARE_HOLD.directionRad
      );
    })
  );
}

/** A point of a sphere taken along its radius onto the spheroid of semi-axes a and c, with its normal. */
function ontoSpheroid(
  pointM: Vec3,
  equatorialRadiusM: number,
  polarRadiusM: number,
): { readonly pointM: Vec3; readonly normal: Vec3 } {
  const u = normalise(pointM);
  const a2 = equatorialRadiusM * equatorialRadiusM;
  const c2 = polarRadiusM * polarRadiusM;
  const r = 1 / Math.sqrt((u.x * u.x + u.y * u.y) / a2 + (u.z * u.z) / c2);
  const onSpheroid = scale(u, r);
  return {
    pointM: onSpheroid,
    normal: normalise(vec3(onSpheroid.x / a2, onSpheroid.y / a2, onSpheroid.z / c2)),
  };
}

/** Rgb `a` times `b`, channel by channel. */
function times(a: Rgb, b: Rgb): Rgb {
  return [a[0] * b[0], a[1] * b[1], a[2] * b[2]];
}

/**
 * Each sea's glare source, formed in its body's axes and held across frames (the module's
 * remarks): the glint above the cap, its light carried through the body's air.
 */
export class OceanGlare {
  readonly #held = new Map<BodyIdHex, HeldGlare>();

  /**
   * `view`'s sea's glare source this frame, in the scene's camera-relative axes, or `null` where
   * its glint stays under the cap or its specular point is not seen.
   *
   * @param toBodyFixed - Rotates the scene's axes into the body's body-fixed ones.
   * @param air - The body's air, where the frame draws it; none, the light reaches the sea whole.
   */
  sourceOf(
    view: OceanDrawView,
    toBodyFixed: Quaternion,
    air: OceanGlareAir | null,
  ): GlareSource | null {
    const sun = view.suns[0];
    if (sun === undefined) {
      return null;
    }
    const body = view.sea.body;
    const cameraM = view.camera.positionM;
    const capCdM2 = view.thresholdPreExposed / view.exposureScale;
    const lightLx = Math.max(...sun.illuminanceLx);
    const texels = view.sea.map.texels;
    let held = this.#held.get(body);
    if (
      held === undefined ||
      held.texels !== texels ||
      held.medium !== (air?.medium ?? null) ||
      held.curves !== (air?.curves ?? null) ||
      !sameEclipse(held.eclipse, view.eclipse) ||
      norm(sub(held.cameraM, cameraM)) > GLARE_HOLD.position * norm(cameraM) ||
      norm(sub(held.sun, sun.directionBodyFixed)) > GLARE_HOLD.directionRad ||
      !near(held.lightLx, lightLx, GLARE_HOLD.light) ||
      !near(held.capCdM2, capCdM2, GLARE_HOLD.light)
    ) {
      held = {
        cameraM,
        sun: sun.directionBodyFixed,
        lightLx,
        capCdM2,
        texels,
        medium: air?.medium ?? null,
        curves: air?.curves ?? null,
        eclipse: view.eclipse,
        glare: this.#form(view, air),
      };
      this.#held.set(body, held);
    }
    const glare = held.glare;
    if (glare === null) {
      return null;
    }
    return {
      direction: rotate(conjugate(toBodyFixed), normalise(sub(glare.pointM, cameraM))),
      angularRadiusRad: glare.angularRadiusRad,
      excessLuminance: glare.excessLuminance,
      luminance: glare.luminance,
    };
  }

  /** Lets go of every sea's source but those of `kept`. */
  keepOnly(kept: ReadonlySet<BodyIdHex>): void {
    for (const body of this.#held.keys()) {
      if (!kept.has(body)) {
        this.#held.delete(body);
      }
    }
  }

  /** The source in the body's axes, its light through the air both ways. */
  #form(view: OceanDrawView, air: OceanGlareAir | null): HeldGlare["glare"] {
    const optics = oceanDrawOptics(view);
    const cameraM = view.camera.positionM;
    const sphere = specularPoint(cameraM, optics.sunDirection, optics.equatorialRadiusM);
    if (sphere === null) {
      return null;
    }
    // Taken down onto the sea-level spheroid along its radius, its normal the spheroid's there.
    const specular = ontoSpheroid(sphere.pointM, optics.equatorialRadiusM, optics.polarRadiusM);
    // The air over the sea-level spheroid: the sun's light down to the specular point, and the
    // glint's back up to the camera.
    const lines =
      air === null
        ? null
        : {
            medium: air.medium,
            equatorialRadiusM: optics.equatorialRadiusM,
            polarRadiusM: optics.polarRadiusM,
            toBodyFixed: IDENTITY_QUATERNION,
          };
    let down: Rgb = [1, 1, 1];
    let up: Rgb = [1, 1, 1];
    if (lines !== null && air !== null) {
      // A metre above the sea, so that the line starts outside the spheroid it is held off.
      const lifted = add(specular.pointM, specular.normal);
      down = glareLineAir(
        { ...lines, cameraM: lifted },
        air.curves,
        optics.sunDirection,
        Number.POSITIVE_INFINITY,
      ).transmittance;
      const towards = sub(lifted, cameraM);
      const distanceM = norm(towards);
      up = glareLineAir(
        { ...lines, cameraM },
        air.curves,
        scale(towards, 1 / distanceM),
        distanceM,
      ).transmittance;
    }
    // The source's eclipse at the specular point, as the pass dims its light there.
    down = times(down, oceanSourceVisible(view.eclipse, 0, specular.pointM));
    if (!(Math.max(...down) > 0)) {
      return null;
    }
    const lit: OceanFarOptics = { ...optics, sunLux: times(optics.sunLux, down) };
    const glare = oceanGlintGlare(lit, view.sea.map.texels, cameraM, (d) => d);
    if (glare === null) {
      return null;
    }
    return {
      pointM: specular.pointM,
      angularRadiusRad: glare.angularRadiusRad,
      excessLuminance: times(glare.excessLuminance, up),
      luminance: times(glare.luminance, up),
    };
  }
}
