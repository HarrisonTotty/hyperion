/**
 * The point regime's light of a body with air (plan R07, R07.T8.a; plan R08, R08.T16.b): what its
 * disc drawn by its table reflects, so that the point's flux stays the disc's it switches with at
 * 3 px (R07.T8.a's 1%) where that disc draws its air.
 *
 * @remarks
 * - **The ratio.** For each star, the tabled disc's own arithmetic (`discShading.ts`' twin of
 *   `bodyDisc.wgsl`: its ground shaded as the disc reads it, on the figure and the surface the disc
 *   is drawn on, then `airAt`, the twin of `disc_air`: that ground times the table's mean
 *   transmittance, plus the air's light at the star's light slot) summed over the disc's samples
 *   ({@link AIR_POINT_RINGS} by {@link AIR_POINT_AZIMUTHS}), over the same sum of the body's own disc
 *   drawn airless (its own figure and surface, the point's): the point is its airless reflection
 *   (`bodyReflection`) times that ratio, so that the samples' error on the limb and the
 *   terminator largely cancels in it, and their near field with it to (k_air − k) a ÷ D. Where the airless sum is 0 (a star wholly behind the body), and
 *   under a class map, whose point is a stand-in's until the frames' slices find its own
 *   (`classMapPointReflection`), the tabled sum itself over (a ÷ D)²: the samples' airless sum
 *   met the map's own point to 5 × 10⁻⁴ on the six cases' class map at 60° of phase
 *   (2026-11-06).
 * - **What it leaves as it was.** Each star's term is uneclipsed: the caller cuts it by its eclipse
 *   (`discEclipseVisible`) as before. Planetshine keeps the airless reflection. A body without air,
 *   and without the glow below, is drawn bit for bit as before.
 * - **The point's own light** (R08.T18; {@link GlowPointBody}). A body whose disc draws its ground's
 *   emission (`discReflectance.ts`' `discEmissionOf`, R08.T18.a) or its air's thermal light (the
 *   table's thermal layers, `discTable.ts`' `discThermalAt`, R08.T18.b) has its point add what that
 *   disc emits: the same samples' sum of the twin's light with every star and planetshine source at
 *   none, so that each sample is `discEmissionAt`'s, with the same melt classes, dimmed by the
 *   table's alpha, plus the table's own light, as `disc_air` adds them; over (a ÷ D)², no ratio, and
 *   cut by no eclipse, being no star's ({@link glowPointLight}). It is held and found off the frame
 *   as the ratios are, keyed by its table or, airless, by its field, the star's direction its
 *   field's turns about and the observer's; until found the point draws without it, as before. A
 *   body under the thermal cut whose table holds no thermal layer has no glow.
 * - **The ground's lighting.** The disc lights its ground through the air's surface-lighting tables
 *   (R08.T9.b), which only the GPU holds; the sum lights it as an airless body's (transmittance 1,
 *   no sky), as R07.T8.a's CPU raster does (`rasteriseDisc`'s default `AIRLESS_LIGHTING`).
 * - **The samples** are a disc's view rays, taken from (a ÷ D) = {@link AIR_POINT_RADIUS_OVER_DISTANCE},
 *   about the 3 px switch's on a 1080p view: Gauss–Legendre in t = √(1 − ρ²) over the figure's
 *   outline made a unit circle, ρ its radius, which puts most of them near the limb, where the
 *   air's light rises and a crescent lies, and an even azimuth.
 * - **The cache.** A ratio is held per table and light, by the light's and the observer's
 *   directions in the body's frame (a class map's body-fixed axes, or the drawn pole's), and taken
 *   again while both stay within `classMapPointCacheAngleRad` of where it was found; past it, the
 *   light's nearest within `classMapPointStaleAngleRad` stands in while the frames' slices find a
 *   fresh one. The entries go with their table ({@link HELD_PER_TABLE} a table).
 * - **Off the frame.** The frame's path ({@link airPointReflection}) never sums: a light with no
 *   ratio held near is drawn airless, as before, and its ratio queued for the frames' slices
 *   ({@link stepAirPoints}, the terrain session's step after the class-mapped points'). While a body
 *   is a disc within `CLASS_MAP_POINT_BAND` × 3 px of its frame's own pixels its lights' ratios are
 *   found there ({@link warmAirPoint}), so that its first frame as a point finds each light's held.
 */
import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { DiscAir, DiscEmission } from "../atmosphere/discReflectance";
import type { DiscReflectanceTable } from "../atmosphere/discTable";
import type { SpheroidFigure } from "../atmosphere/hillaire";
import { AIRLESS_LIGHTING, type SurfaceLightingReader } from "../atmosphere/surfaceLighting";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { ProjectionCamera } from "../camera/projection";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type { AnnulusSet } from "../lighting/annuli";
import type { Rgb } from "../photometry/toneCurve";
import { classMapPointCacheAngleRad, classMapPointStaleAngleRad } from "./classMapPoint";
import {
  type DiscLight,
  discRadianceAlong,
  type DiscRecord,
  type DrawnDiscSurface,
} from "./discShading";
import type { ClassMapTexels } from "./discSurface";
import { discSurfaceLaws } from "./discSurface";

/**
 * The rings of the disc's samples: Gauss–Legendre nodes in t = √(1 − ρ²) over [0, 1].
 *
 * @remarks
 * With {@link AIR_POINT_AZIMUTHS}, the tabled sum of each of the six air bodies at the 3 px switch
 * (`test/airSwitchCases.ts`) at 0°, 45°, 90° and 150° of phase stands within 2.4 × 10⁻⁴ of the
 * same sum at 256 rings by 512 azimuths, and TEST VENUS's crescent at 150° within 1.3 × 10⁻³
 * (2026-11-06), some 10 ms a light (vitest's V8, pod, 32 vCPU); a Lambert sphere's airless sum
 * within 5.9 × 10⁻⁴ at 150° (the terminator's kink; science check, 2026-11-06). The ratio's error
 * is the two's difference.
 */
export const AIR_POINT_RINGS = 32;

/** The azimuths of the disc's samples, evenly spaced. */
export const AIR_POINT_AZIMUTHS = 64;

/**
 * a ÷ D of the disc the samples are taken from: about the 3 px switch's, 1 ÷ 1,109, on a view
 * 1,920 px across at 60° (R07's), where the disc's near field is the one the point switches with.
 *
 * @remarks
 * On the six air bodies the tabled disc's flux times D² at this a ÷ D stands within 7 × 10⁻⁴ of
 * its far field (10⁻⁵) to 90° of phase, and within 7.3 × 10⁻³ at 150° (TEST VENUS's crescent;
 * 2026-11-06): what the tabled sum's fallback carries of the near field at every distance. The
 * ratio cancels the near field but for the air's own share, so a uniform body's point keeps the
 * airless point's far field: −6.1 a ÷ D off its disc at the switch at 150° (`draw.test.ts`).
 */
export const AIR_POINT_RADIUS_OVER_DISTANCE = 1e-3;

/** The ratios a table holds at most: two stars from four views. */
export const HELD_PER_TABLE = 8;

/** The ratios and glows the frames' slices hold queued at most, the oldest let go past it. */
export const AIR_POINT_QUEUE = 16;

/**
 * The slices a frame's note of a table, or of a glow's holder, near the switch holds for, as a
 * class map's.
 */
export const AIR_POINT_NEAR_SLICES = 4;

/** The camera the samples are taken in: the galactic axes are the view's. */
const SAMPLE_CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

const CHANNELS = [0, 1, 2] as const;

/** One star of the body, as the samples' records take it. */
export interface AirPointLight {
  /** The unit direction from the body's centre to the star, along the galactic axes. */
  readonly towardsLight: Vec3;
  /** The star's distance from the body's centre, m. */
  readonly distanceM: number;
  /** The star's radius, m. */
  readonly radiusM: number;
  /** Its annuli per channel, as a disc record's light carries them (`hostAnnuli`). */
  readonly annuli: readonly [AnnulusSet, AnnulusSet, AnnulusSet];
  /** Its key among the body's lights, `pointFlux`'s: a held ratio is its own light's. */
  readonly key: string;
}

/**
 * A body with air as its point's samples see it: the disc its table draws, and the airless disc of
 * its own figure and surface that the point's reflection is.
 */
export interface AirPointBody {
  readonly air: DiscAir;
  /** The body's own figure, the point's, along the galactic axes. */
  readonly figure: SpheroidFigure;
  /** What the point's own disc shades with (`draw.ts`' `drawnSurface`). */
  readonly surface: DrawnDiscSurface;
  /**
   * The figure its table's disc is drawn on: its ground's table figure beneath a gas envelope's air
   * (`LitBodyInput.ground`), else its own.
   */
  readonly airedFigure: SpheroidFigure;
  /** What its table's disc shades its ground with: black beneath a gas envelope's air, else `surface`. */
  readonly airedSurface: DrawnDiscSurface;
  /** The pole both discs are drawn about, unit, along the galactic axes. */
  readonly pole: Vec3;
  /** A class map's body-fixed axes, whose frame its held ratios are found in; `null` otherwise. */
  readonly rotation: Rotation3 | null;
  /** The texels of a class map either disc shades with, or `null`. */
  readonly classMap: ClassMapTexels | null;
  /** The unit direction from the body's centre to the observer, along the galactic axes. */
  readonly towardsObserver: Vec3;
  /** Its stars, the brightest first, as its point takes them. */
  readonly lights: ReadonlyArray<AirPointLight>;
  /**
   * Whether the point's airless reflection is its own disc's integral at every frame, which the
   * ratio scales: a uniform law's (`bodyReflection`). A class map's point is a stand-in's until the
   * frames' slices make its elements and find each light's ratio (`classMapPointReflection`), so
   * under a map the tabled sum itself is taken.
   */
  readonly airlessIsDisc: boolean;
}

/** A light's ratio and the tabled sum it falls back to, per channel. */
export interface AirPointRatio {
  /** The tabled disc's sum over the airless disc's. */
  readonly ratio: Rgb;
  /** The tabled disc's sum over (a ÷ D)², the own figure's a: its reflection where the airless is 0. */
  readonly reflection: Rgb;
}

/** The glows a body holds at most: one from each of four views. */
export const GLOWS_HELD = 4;

/**
 * A body whose disc glows (the module's remarks; R08.T18) as its point's samples see it: the disc
 * its record draws, every light at none, so that it draws its own light alone.
 */
export interface GlowPointBody {
  /**
   * Its ground's emission as its disc's record forms it (`discReflectance.ts`' `discEmissionOf`),
   * its star its brightest's direction, galactic; `null` below the thermal cut.
   */
  readonly emission: DiscEmission | null;
  /**
   * Its air drawn by its table, which dims its ground's emission by its alpha and adds its own light
   * where it holds a thermal layer; `null` for a body without one.
   */
  readonly air: DiscAir | null;
  /** The body's own figure, the point's, along the galactic axes: the glow is over its (a ÷ Δ)². */
  readonly figure: SpheroidFigure;
  /**
   * The figure the glowing disc is drawn on: its ground's table figure beneath a gas envelope's air
   * drawn by its table (`LitBodyInput.ground`), else its own.
   */
  readonly glowFigure: SpheroidFigure;
  /** What that disc shades its ground with: black beneath a gas envelope's air, else its own. */
  readonly glowSurface: DrawnDiscSurface;
  /** The pole the disc is drawn about, unit, along the galactic axes. */
  readonly pole: Vec3;
  /** A class map's body-fixed axes, whose frame its held glows are found in; `null` otherwise. */
  readonly rotation: Rotation3 | null;
  /** The texels of a class map the disc shades with, or `null`. */
  readonly classMap: ClassMapTexels | null;
  /** The unit direction from the body's centre to the observer, along the galactic axes. */
  readonly towardsObserver: Vec3;
}

/** Each surface the ratio reads, by what its identity is held by: a map, or a uniform law. */
function surfaceKeyOf(surface: DrawnDiscSurface): object {
  return surface.kind === "class-map" ? surface.texels : surface.law;
}

/** A light at a geometry: what a held ratio, a refresh and a frame's ask share. */
interface AirLightGeometry {
  readonly lightKey: string;
  /** The light's slot, its index among the body's lights, which the table and lookups read. */
  readonly slot: number;
  readonly surfaceKey: object;
  readonly airedKey: object;
  /** The class map's texels either disc shades with, which drop it with them; `null` for none. */
  readonly mapKey: ClassMapTexels | null;
  /** The two figures' semi-axes, m. */
  readonly figureKey: string;
  /** The light's and the observer's directions in the body's frame. */
  readonly towardsLight: Vec3;
  readonly towardsObserver: Vec3;
}

/** A ratio found at one geometry. */
interface HeldAirRatio extends AirLightGeometry {
  readonly found: AirPointRatio;
}

/** A ratio being found in the frames' slices. */
interface QueuedAirRefresh extends AirLightGeometry {
  readonly kind: "ratio";
  readonly table: DiscReflectanceTable;
  readonly steps: Generator<void, AirPointRatio>;
}

/** A body's glow at a geometry (R08.T18): what a held glow, a refresh and a frame's ask share. */
interface GlowGeometry {
  /** The ground's field its emission is of, or `null` for none. */
  readonly fieldKey: object | null;
  readonly surfaceKey: object;
  /** The class map's texels its disc shades with, which drop it with them; `null` for none. */
  readonly mapKey: ClassMapTexels | null;
  /** The body's own figure's and the glowing disc's semi-axes, m. */
  readonly figureKey: string;
  /**
   * The direction its field's substellar term turns about (`DiscEmission.star`) and the observer's,
   * in the body's frame; the first zero for a body with no emission or no star.
   */
  readonly towardsLight: Vec3;
  readonly towardsObserver: Vec3;
}

/** A glow found at one geometry: Σ L Ω over (a ÷ D)², cd m⁻² per channel. */
interface HeldGlow extends GlowGeometry {
  readonly glow: Rgb;
}

/** A glow being found in the frames' slices, for its holder (`glowHolderOf`). */
interface QueuedGlowRefresh extends GlowGeometry {
  readonly kind: "glow";
  readonly holder: object;
  readonly steps: Generator<void, Rgb>;
}

/** A refresh of either kind. */
type QueuedRefresh = QueuedAirRefresh | QueuedGlowRefresh;

/** Each table's ratios, the latest found last. */
const HELD = new WeakMap<DiscReflectanceTable, HeldAirRatio[]>();

/** Each holder's glows (`glowHolderOf`), the latest found last. */
const HELD_GLOWS = new WeakMap<object, HeldGlow[]>();

/** The refreshes queued, the oldest first. */
const REFRESHES: QueuedRefresh[] = [];

/**
 * The tables holding a ratio found on each class map's texels, by the texels: what
 * {@link cancelAirPointsOf} clears as its map is dropped. Held while the texels are.
 */
const TABLES_OF_MAP = new WeakMap<object, Set<DiscReflectanceTable>>();

/** The holders of a glow found on each class map's texels, by the texels, as {@link TABLES_OF_MAP}. */
const GLOW_HOLDERS_OF_MAP = new WeakMap<object, Set<object>>();

/** The slices run so far, the clock of {@link NEAR}'s notes. */
let SLICES_RUN = 0;

/**
 * Each table's, or glow holder's, latest note that a frame drew its body near the switch, as
 * {@link SLICES_RUN}.
 */
const NEAR = new WeakMap<object, number>();

/** Whether a refresh failed already, said once. */
const SAID = { failed: false };

/** The Gauss–Legendre nodes and weights of `n` points over [0, 1]. */
function gaussLegendre(n: number): {
  readonly nodes: Float64Array;
  readonly weights: Float64Array;
} {
  const nodes = new Float64Array(n);
  const weights = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let x = Math.cos((Math.PI * (i + 0.75)) / (n + 0.5));
    let derivative = 1;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      let p0 = 1;
      let p1 = x;
      for (let k = 2; k <= n; k += 1) {
        const p2 = ((2 * k - 1) * x * p1 - (k - 1) * p0) / k;
        p0 = p1;
        p1 = p2;
      }
      derivative = (n * (x * p1 - p0)) / (x * x - 1);
      const step = p1 / derivative;
      x -= step;
      if (Math.abs(step) < 1e-15) {
        break;
      }
    }
    nodes[i] = 0.5 * (x + 1);
    weights[i] = 1 / ((1 - x * x) * derivative * derivative);
  }
  return { nodes, weights };
}

/** The rings' nodes in t and their weights. */
const RINGS = gaussLegendre(AIR_POINT_RINGS);

/** Each azimuth's cosine and sine. */
const AZIMUTHS = Array.from({ length: AIR_POINT_AZIMUTHS }, (_, j) => {
  const phi = ((j + 0.5) / AIR_POINT_AZIMUTHS) * 2 * Math.PI;
  return [Math.cos(phi), Math.sin(phi)] as const;
});

/** A unit vector perpendicular to the unit `v`. */
function perpendicularTo(v: Vec3): Vec3 {
  const seed = Math.abs(v.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  return normalise(cross(v, seed));
}

/** A side of the sum: its record, and the light it shades along a ray. */
interface SampledSide {
  readonly record: DiscRecord;
  readonly lighting: SurfaceLightingReader;
}

/**
 * Σ L Ω over the side's samples, per channel, a ring a step: each ray through its node of the
 * figure's outline, made a unit circle, about the view direction.
 */
function* sumSteps(side: SampledSide, classMap: ClassMapTexels | null): Generator<void, Rgb> {
  const { record } = side;
  const radiance = discRadianceAlong(record, SAMPLE_CAMERA, classMap, side.lighting);
  const c = record.direction;
  const p = record.pole;
  const along = dot(p, c);
  // The outline's axes: across the projected pole, and along it, A and B its half-widths, rad.
  const towardsPole = sub(p, scale(c, along));
  const v = norm(towardsPole) > 1e-9 ? normalise(towardsPole) : perpendicularTo(c);
  const u = normalise(cross(v, c));
  const a = Math.tan(Math.asin(Math.min(1, record.radiusOverDistance)));
  const sin2 = Math.max(0, 1 - along * along);
  const b =
    a * Math.sqrt(along * along + record.polarOverEquatorial * record.polarOverEquatorial * sin2);
  const sum: [number, number, number] = [0, 0, 0];
  const azimuthWeight = (2 * Math.PI) / AIR_POINT_AZIMUTHS;
  for (let i = 0; i < AIR_POINT_RINGS; i += 1) {
    const t = RINGS.nodes[i] ?? 0;
    const rho = Math.sqrt(Math.max(0, 1 - t * t));
    const area = a * b * t * (RINGS.weights[i] ?? 0) * azimuthWeight;
    for (const [cosPhi, sinPhi] of AZIMUTHS) {
      const x = a * rho * cosPhi;
      const y = b * rho * sinPhi;
      const ray = normalise(
        vec3(c.x + x * u.x + y * v.x, c.y + x * u.y + y * v.y, c.z + x * u.z + y * v.z),
      );
      const light = radiance(ray);
      if (light === null) {
        continue;
      }
      const omega = area / (1 + x * x + y * y) ** 1.5;
      for (const ch of CHANNELS) {
        sum[ch] += light[ch] * omega;
      }
    }
    yield;
  }
  return sum;
}

/**
 * One side's record of `body`, lit by light `slot` alone: on `figure` with `surface`, its air where
 * given, the light at unit illuminance and the others at none, so that each keeps its slot.
 */
function sideRecord(
  body: AirPointBody,
  figure: SpheroidFigure,
  surface: DrawnDiscSurface,
  slot: number,
  air: DiscAir | undefined,
): DiscRecord {
  const a = figure.equatorialRadiusM;
  const lights: DiscLight[] = body.lights.map((light, j) => ({
    direction: light.towardsLight,
    distance: light.distanceM / a,
    radius: light.radiusM / a,
    illuminance: j === slot ? [1, 1, 1] : [0, 0, 0],
    annuli: light.annuli,
  }));
  return {
    ...sampledRecord(body, figure, surface, lights),
    ...(air === undefined ? {} : { air: { ...air, table: litTableOf(air.table) } }),
  };
}

/**
 * A record of the samples' disc of `body` on `figure` with `surface`, lit by `lights`, at the
 * samples' a ÷ D for the body's own figure, its exposure 1.
 */
function sampledRecord(
  body: Pick<AirPointBody, "figure" | "pole" | "towardsObserver">,
  figure: SpheroidFigure,
  surface: DrawnDiscSurface,
  lights: ReadonlyArray<DiscLight>,
): DiscRecord {
  const a = figure.equatorialRadiusM;
  const radiusOverDistance = (AIR_POINT_RADIUS_OVER_DISTANCE * a) / body.figure.equatorialRadiusM;
  return {
    body: "0000000000000000.0000",
    rect: { leftPx: 0, topPx: 0, rightPx: 0, bottomPx: 0 },
    direction: scale(body.towardsObserver, -1),
    radiusOverDistance,
    pole: body.pole,
    polarOverEquatorial: figure.polarRadiusM / a,
    interiorSamples: 1,
    limbSamples: 1,
    surface,
    tableRows: discSurfaceLaws(surface).map((_, m) => m),
    exposureOverPi: 1 / Math.PI,
    lights,
    occluders: [],
    secondaries: [],
  };
}

/**
 * The glowing disc's record of `body` (the module's remarks; R08.T18): on its glow's figure and
 * surface, its emission and its whole table, its thermal layers among it, and no light, so that each
 * sample's light is its own alone, cd m⁻².
 */
function glowRecord(body: GlowPointBody): DiscRecord {
  return {
    ...sampledRecord(body, body.glowFigure, body.glowSurface, []),
    ...(body.emission === null ? {} : { emission: body.emission }),
    ...(body.air === null ? {} : { air: body.air }),
  };
}

/** A body's glow as the frames' slices find it, a ring of samples a step: Σ L Ω over (a ÷ D)². */
function* glowSteps(body: GlowPointBody): Generator<void, Rgb> {
  const sum = yield* sumSteps(
    { record: glowRecord(body), lighting: AIRLESS_LIGHTING },
    body.classMap,
  );
  const solid = AIR_POINT_RADIUS_OVER_DISTANCE * AIR_POINT_RADIUS_OVER_DISTANCE;
  return [sum[0] / solid, sum[1] / solid, sum[2] / solid];
}

/**
 * Runs a glow's steps to their end at once (R08.T18), the synchronous path, for tests and harnesses:
 * what `body`'s disc emits towards the observer, Σ L Ω over its samples over (a ÷ D)², cd m⁻² per
 * channel, so that its flux at Δ is this times (a ÷ Δ)².
 */
export function glowPointSum(body: GlowPointBody): Rgb {
  return finished(glowSteps(body));
}

/** Runs a generator's steps to their end at once. */
function finished<T>(steps: Generator<void, T>): T {
  for (;;) {
    const step = steps.next();
    if (step.done === true) {
      return step.value;
    }
  }
}

/** A table without its thermal layers' scale, so that its air adds no light of its own. */
function litTableOf(table: DiscReflectanceTable): DiscReflectanceTable {
  if (table.thermalScale === undefined) {
    return table;
  }
  return { bands: table.bands, slots: table.slots, sunFloor: table.sunFloor, values: table.values };
}

/**
 * The ratio of light `slot` of `body` as the frames' slices find it, a ring of either side's
 * samples a step: the tabled disc's sum, then the airless disc's.
 */
function* ratioSteps(body: AirPointBody, slot: number): Generator<void, AirPointRatio> {
  const aired = yield* sumSteps(
    {
      record: sideRecord(body, body.airedFigure, body.airedSurface, slot, body.air),
      lighting: AIRLESS_LIGHTING,
    },
    body.classMap,
  );
  const plain = yield* sumSteps(
    {
      record: sideRecord(body, body.figure, body.surface, slot, undefined),
      lighting: AIRLESS_LIGHTING,
    },
    body.classMap,
  );
  const solid = AIR_POINT_RADIUS_OVER_DISTANCE * AIR_POINT_RADIUS_OVER_DISTANCE;
  const ratioAt = (c: 0 | 1 | 2): number => (plain[c] > 0 ? aired[c] / plain[c] : 0);
  return {
    ratio: [ratioAt(0), ratioAt(1), ratioAt(2)],
    reflection: [aired[0] / solid, aired[1] / solid, aired[2] / solid],
  };
}

/** Runs a ratio's steps to their end at once: the synchronous path, for tests and harnesses. */
export function airPointRatio(body: AirPointBody, slot: number): AirPointRatio {
  return finished(ratioSteps(body, slot));
}

/** A direction in the body's frame: a class map's body-fixed axes, or the drawn pole's. */
function inBodyFrame(body: Pick<AirPointBody, "rotation" | "pole">, v: Vec3): Vec3 {
  if (body.rotation !== null) {
    return rotateToBodyFixed(body.rotation, v);
  }
  const z = body.pole;
  const x = perpendicularTo(z);
  const y = cross(z, x);
  return vec3(dot(v, x), dot(v, y), dot(v, z));
}

/** Light `slot`'s geometry for `body`. */
function geometryOf(body: AirPointBody, slot: number): AirLightGeometry | null {
  const light = body.lights[slot];
  if (light === undefined) {
    return null;
  }
  const { figure, airedFigure } = body;
  return {
    lightKey: light.key,
    slot,
    surfaceKey: surfaceKeyOf(body.surface),
    airedKey: surfaceKeyOf(body.airedSurface),
    mapKey: body.classMap,
    figureKey: [
      figure.equatorialRadiusM,
      figure.polarRadiusM,
      airedFigure.equatorialRadiusM,
      airedFigure.polarRadiusM,
    ].join(" "),
    towardsLight: inBodyFrame(body, light.towardsLight),
    towardsObserver: inBodyFrame(body, body.towardsObserver),
  };
}

/** The squared chord between two unit directions. */
function chordSquared(a: Vec3, b: Vec3): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return dx * dx + dy * dy + dz * dz;
}

/** A geometry's two directions in the body's frame, which its cache's angles are measured on. */
interface Directed {
  readonly towardsLight: Vec3;
  readonly towardsObserver: Vec3;
}

/** Whether `entry` is within `angleRad` of `at` on both directions. */
function within(entry: Directed, at: Directed, angleRad: number): boolean {
  const limit = angleRad * angleRad;
  return (
    chordSquared(entry.towardsLight, at.towardsLight) <= limit &&
    chordSquared(entry.towardsObserver, at.towardsObserver) <= limit
  );
}

/** Whether `entry` is of `at`'s light, slot, surfaces and figures. */
function sameLight(entry: AirLightGeometry, at: AirLightGeometry): boolean {
  return (
    entry.lightKey === at.lightKey &&
    entry.slot === at.slot &&
    entry.surfaceKey === at.surfaceKey &&
    entry.airedKey === at.airedKey &&
    entry.figureKey === at.figureKey
  );
}

/** Whether `entry` is of `at`'s light and within `angleRad` of it on both directions. */
function nearAs(entry: AirLightGeometry, at: AirLightGeometry, angleRad: number): boolean {
  return sameLight(entry, at) && within(entry, at, angleRad);
}

/** The held entry `same` as `at` nearest it of those within `angleRad`, if any. */
function nearestOf<T extends Directed, G extends Directed>(
  held: ReadonlyArray<T>,
  at: G,
  angleRad: number,
  same: (entry: T, at: G) => boolean,
): T | undefined {
  let nearest: T | undefined;
  let nearestChord = Number.POSITIVE_INFINITY;
  for (const entry of held) {
    if (!same(entry, at) || !within(entry, at, angleRad)) {
      continue;
    }
    const chord = Math.max(
      chordSquared(entry.towardsLight, at.towardsLight),
      chordSquared(entry.towardsObserver, at.towardsObserver),
    );
    if (chord < nearestChord) {
      nearest = entry;
      nearestChord = chord;
    }
  }
  return nearest;
}

/** The held entry of `at`'s light nearest it of those within `angleRad`, if any. */
function nearestHeld(
  held: ReadonlyArray<HeldAirRatio>,
  at: AirLightGeometry,
  angleRad: number,
): HeldAirRatio | undefined {
  return nearestOf(held, at, angleRad, sameLight);
}

/** The phase angle of a geometry, rad: π ÷ 2 where its light is zero. */
function phaseOf(at: Directed): number {
  return Math.acos(Math.min(1, Math.max(-1, dot(at.towardsLight, at.towardsObserver))));
}

/**
 * Queues light `slot`'s ratio at `at` for the frames' slices, unless one of the same light is
 * queued for the table already, at any geometry: one a light, however fast it moves.
 */
function queueRefresh(body: AirPointBody, slot: number, at: AirLightGeometry): void {
  const { table } = body.air;
  if (
    REFRESHES.some(
      (refresh) => refresh.kind === "ratio" && refresh.table === table && sameLight(refresh, at),
    )
  ) {
    return;
  }
  enqueue({ kind: "ratio", ...at, table, steps: ratioSteps(body, slot) });
}

/** Queues a refresh, the oldest let go past {@link AIR_POINT_QUEUE}. */
function enqueue(refresh: QueuedRefresh): void {
  REFRESHES.push(refresh);
  if (REFRESHES.length > AIR_POINT_QUEUE) {
    REFRESHES.shift();
  }
}

/**
 * What a body with air reflects of its star `slot` towards the observer, uneclipsed, over the
 * illuminance on it and (a ÷ Δ)²: `airless`, the point's reflection without air, times its held
 * ratio (the module's remarks), or its held tabled reflection where its airless one is 0 or is not
 * its disc's ({@link AirPointBody.airlessIsDisc});
 * `airless` itself while none is held near, its ratio queued for the frames' slices. Never sums on
 * the frame. Notes the body's table near the switch.
 *
 * @param airless - The point's reflection of the light without air (`pointFlux`'s).
 */
export function airPointReflection(body: AirPointBody, slot: number, airless: Rgb): Rgb {
  const { table } = body.air;
  NEAR.set(table, SLICES_RUN);
  const at = geometryOf(body, slot);
  if (at === null) {
    return airless;
  }
  const alpha = phaseOf(at);
  const held = HELD.get(table) ?? [];
  let found = held.find((entry) => nearAs(entry, at, classMapPointCacheAngleRad(alpha)));
  if (found === undefined) {
    found = nearestHeld(held, at, classMapPointStaleAngleRad(alpha));
    queueRefresh(body, slot, at);
  }
  if (found === undefined) {
    return airless;
  }
  const { ratio, reflection } = found.found;
  const at3 = (c: 0 | 1 | 2): number =>
    body.airlessIsDisc && airless[c] > 0 && ratio[c] > 0 ? airless[c] * ratio[c] : reflection[c];
  return [at3(0), at3(1), at3(2)];
}

/**
 * Begins each star's ratio in the frames' slices while the body is a disc near the switch (the
 * module's remarks), from the geometry its point would take, unless the table holds one of that
 * light within the cache's angle of it, or one of that light is queued. Nothing is summed here.
 * Notes the body's table near the switch.
 */
export function warmAirPoint(body: AirPointBody): void {
  const { table } = body.air;
  NEAR.set(table, SLICES_RUN);
  const held = HELD.get(table) ?? [];
  for (const slot of body.lights.keys()) {
    const at = geometryOf(body, slot);
    if (at === null) {
      continue;
    }
    if (held.some((entry) => nearAs(entry, at, classMapPointCacheAngleRad(phaseOf(at))))) {
      continue;
    }
    queueRefresh(body, slot, at);
  }
}

/**
 * Holds a found ratio for its table in place of its light's held one nearest it within the stale
 * angle, the one that stood in, and otherwise as the latest, the oldest let go past
 * {@link HELD_PER_TABLE}.
 */
function hold(table: DiscReflectanceTable, entry: HeldAirRatio): void {
  const held = HELD.get(table) ?? [];
  const replaced = nearestHeld(held, entry, classMapPointStaleAngleRad(phaseOf(entry)));
  const kept = held.filter((each) => each !== replaced);
  HELD.set(table, [...kept.slice(-(HELD_PER_TABLE - 1)), entry]);
  // A class map's alone: a uniform law's set would hold its tables while the law lives, which for
  // the black ground beneath a gas envelope's air (`draw.ts`' `GROUND_SURFACE`) is the session.
  if (entry.mapKey !== null) {
    const tables = TABLES_OF_MAP.get(entry.mapKey) ?? new Set<DiscReflectanceTable>();
    tables.add(table);
    TABLES_OF_MAP.set(entry.mapKey, tables);
  }
}

/** What stands for a glow's holder with neither a table nor a field, which no body draws. */
const NO_GLOW_HOLDER = {};

/** What a body's glows are held by: its table, or, airless, its ground's field. */
function glowHolderOf(body: GlowPointBody): object {
  return body.air?.table ?? body.emission?.field ?? NO_GLOW_HOLDER;
}

/** A body's glow geometry: its field, surface and figures, and its directions in its frame. */
function glowGeometryOf(body: GlowPointBody): GlowGeometry {
  const { figure, glowFigure } = body;
  return {
    fieldKey: body.emission?.field ?? null,
    surfaceKey: surfaceKeyOf(body.glowSurface),
    mapKey: body.classMap,
    figureKey: [
      figure.equatorialRadiusM,
      figure.polarRadiusM,
      glowFigure.equatorialRadiusM,
      glowFigure.polarRadiusM,
    ].join(" "),
    towardsLight: inBodyFrame(body, body.emission?.star ?? vec3(0, 0, 0)),
    towardsObserver: inBodyFrame(body, body.towardsObserver),
  };
}

/** Whether `entry` is of `at`'s field, surface and figures. */
function sameGlow(entry: GlowGeometry, at: GlowGeometry): boolean {
  return (
    entry.fieldKey === at.fieldKey &&
    entry.surfaceKey === at.surfaceKey &&
    entry.figureKey === at.figureKey
  );
}

/** Queues `body`'s glow at `at` for the frames' slices, unless one of its holder's is queued. */
function queueGlowRefresh(body: GlowPointBody, holder: object, at: GlowGeometry): void {
  if (
    REFRESHES.some(
      (refresh) => refresh.kind === "glow" && refresh.holder === holder && sameGlow(refresh, at),
    )
  ) {
    return;
  }
  enqueue({ kind: "glow", ...at, holder, steps: glowSteps(body) });
}

/** No light. */
const NO_GLOW: Rgb = [0, 0, 0];

/**
 * What a body's disc emits towards the observer (the module's remarks; R08.T18), over (a ÷ Δ)², cd
 * m⁻² per channel: a glow held within the cache's angle of its geometry; else, its glow queued for
 * the frames' slices, the held one nearest it within the stale angle, or none. Never sums on the
 * frame, and is cut by no eclipse. Notes its holder near the switch.
 */
export function glowPointLight(body: GlowPointBody): Rgb {
  const holder = glowHolderOf(body);
  NEAR.set(holder, SLICES_RUN);
  const at = glowGeometryOf(body);
  const alpha = phaseOf(at);
  const held = HELD_GLOWS.get(holder) ?? [];
  let found = held.find(
    (entry) => sameGlow(entry, at) && within(entry, at, classMapPointCacheAngleRad(alpha)),
  );
  if (found === undefined) {
    found = nearestOf(held, at, classMapPointStaleAngleRad(alpha), sameGlow);
    queueGlowRefresh(body, holder, at);
  }
  return found === undefined ? NO_GLOW : found.glow;
}

/**
 * Begins a body's glow in the frames' slices while it is a disc near the switch, from the geometry
 * its point would take, unless one is held within the cache's angle of it, or one is queued.
 * Nothing is summed here. Notes its holder near the switch.
 */
export function warmGlowPoint(body: GlowPointBody): void {
  const holder = glowHolderOf(body);
  NEAR.set(holder, SLICES_RUN);
  const at = glowGeometryOf(body);
  const angleRad = classMapPointCacheAngleRad(phaseOf(at));
  if ((HELD_GLOWS.get(holder) ?? []).some((e) => sameGlow(e, at) && within(e, at, angleRad))) {
    return;
  }
  queueGlowRefresh(body, holder, at);
}

/**
 * Holds a found glow for its holder in place of its held one nearest it within the stale angle,
 * and otherwise as the latest, the oldest let go past {@link GLOWS_HELD}.
 */
function holdGlow(holder: object, entry: HeldGlow): void {
  const held = HELD_GLOWS.get(holder) ?? [];
  const replaced = nearestOf(held, entry, classMapPointStaleAngleRad(phaseOf(entry)), sameGlow);
  const kept = held.filter((each) => each !== replaced);
  HELD_GLOWS.set(holder, [...kept.slice(-(GLOWS_HELD - 1)), entry]);
  // A class map's alone, as `hold`'s.
  if (entry.mapKey !== null) {
    const holders = GLOW_HOLDERS_OF_MAP.get(entry.mapKey) ?? new Set<object>();
    holders.add(holder);
    GLOW_HOLDERS_OF_MAP.set(entry.mapKey, holders);
  }
}

/**
 * Drops the air points' queued ratios and glows and the held ones found on a class map's texels, as
 * the map is dropped or replaced (`ClassMapStore`, beside `cancelClassMapPoint`), so that none is
 * stepped, or holds a table's place, for a map no frame draws again.
 */
export function cancelAirPointsOf(texels: ClassMapTexels): void {
  const ofMap = (entry: AirLightGeometry): boolean =>
    entry.surfaceKey === texels || entry.airedKey === texels;
  const glowOfMap = (entry: GlowGeometry): boolean => entry.surfaceKey === texels;
  for (let q = REFRESHES.length - 1; q >= 0; q -= 1) {
    const refresh = REFRESHES[q];
    if (refresh !== undefined && refreshOfMap(refresh, texels)) {
      REFRESHES.splice(q, 1);
    }
  }
  for (const table of TABLES_OF_MAP.get(texels) ?? []) {
    HELD.set(
      table,
      (HELD.get(table) ?? []).filter((entry) => !ofMap(entry)),
    );
  }
  TABLES_OF_MAP.delete(texels);
  for (const holder of GLOW_HOLDERS_OF_MAP.get(texels) ?? []) {
    HELD_GLOWS.set(
      holder,
      (HELD_GLOWS.get(holder) ?? []).filter((entry) => !glowOfMap(entry)),
    );
  }
  GLOW_HOLDERS_OF_MAP.delete(texels);
}

/** Whether a queued refresh was found on a class map's texels. */
function refreshOfMap(refresh: QueuedRefresh, texels: ClassMapTexels): boolean {
  let of: boolean;
  switch (refresh.kind) {
    case "ratio":
      of = refresh.surfaceKey === texels || refresh.airedKey === texels;
      break;
    case "glow":
      of = refresh.surfaceKey === texels;
      break;
  }
  return of;
}

/** What a queued refresh's frame notes near the switch: its table, or its glow's holder. */
function holderOfRefresh(refresh: QueuedRefresh): object {
  let holder: object;
  switch (refresh.kind) {
    case "ratio":
      holder = refresh.table;
      break;
    case "glow":
      holder = refresh.holder;
      break;
  }
  return holder;
}

/** Removes `item` from `queue`, where it is. */
function removeFrom<T>(queue: T[], item: T): void {
  const at = queue.indexOf(item);
  if (at >= 0) {
    queue.splice(at, 1);
  }
}

/**
 * Which queued ratios and glows a slice steps: all, or those of tables and glow holders a frame drew
 * near the switch lately.
 */
export type AirPointScope = "all" | "near";

/** The ratios and glows the frames' slices hold queued. */
export function airPointWork(): { readonly refreshes: number } {
  return { refreshes: REFRESHES.length };
}

/** Steps a refresh once, and holds what it found where it is done; whether it is. */
function advance(refresh: QueuedRefresh): boolean {
  let done: boolean;
  switch (refresh.kind) {
    case "ratio": {
      const step = refresh.steps.next();
      done = step.done === true;
      if (step.done === true) {
        hold(refresh.table, {
          lightKey: refresh.lightKey,
          slot: refresh.slot,
          surfaceKey: refresh.surfaceKey,
          airedKey: refresh.airedKey,
          mapKey: refresh.mapKey,
          figureKey: refresh.figureKey,
          towardsLight: refresh.towardsLight,
          towardsObserver: refresh.towardsObserver,
          found: step.value,
        });
      }
      break;
    }
    case "glow": {
      const step = refresh.steps.next();
      done = step.done === true;
      if (step.done === true) {
        holdGlow(refresh.holder, {
          fieldKey: refresh.fieldKey,
          surfaceKey: refresh.surfaceKey,
          mapKey: refresh.mapKey,
          figureKey: refresh.figureKey,
          towardsLight: refresh.towardsLight,
          towardsObserver: refresh.towardsObserver,
          glow: step.value,
        });
      }
      break;
    }
  }
  return done;
}

/**
 * A frame's slice of the air points' work, once a frame from the terrain session's step after the
 * class-mapped points': the oldest queued ratio's or glow's steps, a ring of samples each, of those
 * `scope` takes, until `budgetMs` has passed since the slice began, read on `now` before each step;
 * with none it takes, the slice reads no clock and steps nothing. A finished ratio is held for its
 * table, a glow for its holder; a step that throws drops its refresh, said once.
 *
 * @param budgetMs - The slice's length, ms.
 * @param now - A monotonic clock, ms.
 * @param scope - `all`, or `near`: the tables and glows whose bodies a frame drew near the switch
 *   within the last {@link AIR_POINT_NEAR_SLICES} slices.
 * @returns Whether work it would step remains queued.
 */
export function stepAirPoints(
  budgetMs: number,
  now: () => number = () => globalThis.performance.now(),
  scope: AirPointScope = "all",
): boolean {
  SLICES_RUN += 1;
  const taken = (refresh: QueuedRefresh): boolean => {
    if (scope === "all") {
      return true;
    }
    const noted = NEAR.get(holderOfRefresh(refresh));
    return noted !== undefined && SLICES_RUN - noted <= AIR_POINT_NEAR_SLICES;
  };
  if (!REFRESHES.some(taken)) {
    return false;
  }
  const startMs = now();
  for (;;) {
    const refresh = REFRESHES.find(taken);
    if (refresh === undefined) {
      return false;
    }
    if (!(now() - startMs < budgetMs)) {
      return true;
    }
    let done: boolean;
    try {
      done = advance(refresh);
    } catch (error: unknown) {
      removeFrom(REFRESHES, refresh);
      if (!SAID.failed) {
        SAID.failed = true;
        console.error("the air of a body's point could not be found:", error);
      }
      continue;
    }
    if (done) {
      removeFrom(REFRESHES, refresh);
    }
  }
}
