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
 *   (`discEclipseVisible`) as before. Planetshine keeps the airless reflection, and the ground's and
 *   the air's own emission are not in it (R08.T18), which the point draws neither of. A body without
 *   air is drawn bit for bit as before.
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
import type { DiscAir } from "../atmosphere/discReflectance";
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

/** The ratios the frames' slices hold queued at most, the oldest let go past it. */
export const AIR_POINT_QUEUE = 16;

/** The slices a frame's note of a table near the switch holds for, as a class map's. */
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
  readonly table: DiscReflectanceTable;
  readonly steps: Generator<void, AirPointRatio>;
}

/** Each table's ratios, the latest found last. */
const HELD = new WeakMap<DiscReflectanceTable, HeldAirRatio[]>();

/** The refreshes queued, the oldest first. */
const REFRESHES: QueuedAirRefresh[] = [];

/**
 * The tables holding a ratio found on each class map's texels, by the texels: what
 * {@link cancelAirPointsOf} clears as its map is dropped. Held while the texels are.
 */
const TABLES_OF_MAP = new WeakMap<object, Set<DiscReflectanceTable>>();

/** The slices run so far, the clock of {@link NEAR}'s notes. */
let SLICES_RUN = 0;

/** Each table's latest note that a frame drew its body near the switch, as {@link SLICES_RUN}. */
const NEAR = new WeakMap<DiscReflectanceTable, number>();

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
  const radiusOverDistance = (AIR_POINT_RADIUS_OVER_DISTANCE * a) / body.figure.equatorialRadiusM;
  const lights: DiscLight[] = body.lights.map((light, j) => ({
    direction: light.towardsLight,
    distance: light.distanceM / a,
    radius: light.radiusM / a,
    illuminance: j === slot ? [1, 1, 1] : [0, 0, 0],
    annuli: light.annuli,
  }));
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
    ...(air === undefined ? {} : { air: { ...air, table: litTableOf(air.table) } }),
  };
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
  const steps = ratioSteps(body, slot);
  for (;;) {
    const step = steps.next();
    if (step.done === true) {
      return step.value;
    }
  }
}

/** A direction in the body's frame: a class map's body-fixed axes, or the drawn pole's. */
function inBodyFrame(body: AirPointBody, v: Vec3): Vec3 {
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
  const limit = angleRad * angleRad;
  return (
    sameLight(entry, at) &&
    chordSquared(entry.towardsLight, at.towardsLight) <= limit &&
    chordSquared(entry.towardsObserver, at.towardsObserver) <= limit
  );
}

/** The held entry of `at`'s light nearest it of those within `angleRad`, if any. */
function nearestHeld(
  held: ReadonlyArray<HeldAirRatio>,
  at: AirLightGeometry,
  angleRad: number,
): HeldAirRatio | undefined {
  let nearest: HeldAirRatio | undefined;
  let nearestChord = Number.POSITIVE_INFINITY;
  for (const entry of held) {
    if (!nearAs(entry, at, angleRad)) {
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

/** The phase angle of a geometry, rad. */
function phaseOf(at: AirLightGeometry): number {
  return Math.acos(Math.min(1, Math.max(-1, dot(at.towardsLight, at.towardsObserver))));
}

/**
 * Queues light `slot`'s ratio at `at` for the frames' slices, unless one of the same light is
 * queued for the table already, at any geometry: one a light, however fast it moves.
 */
function queueRefresh(body: AirPointBody, slot: number, at: AirLightGeometry): void {
  const { table } = body.air;
  if (REFRESHES.some((refresh) => refresh.table === table && sameLight(refresh, at))) {
    return;
  }
  REFRESHES.push({ ...at, table, steps: ratioSteps(body, slot) });
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
  for (const key of [entry.surfaceKey, entry.airedKey]) {
    const tables = TABLES_OF_MAP.get(key) ?? new Set<DiscReflectanceTable>();
    tables.add(table);
    TABLES_OF_MAP.set(key, tables);
  }
}

/**
 * Drops the air points' queued ratios and held ones found on a class map's texels, as the map is
 * dropped or replaced (`ClassMapStore`, beside `cancelClassMapPoint`), so that none is stepped, or
 * holds a table's place, for a map no frame draws again.
 */
export function cancelAirPointsOf(texels: ClassMapTexels): void {
  const ofMap = (entry: AirLightGeometry): boolean =>
    entry.surfaceKey === texels || entry.airedKey === texels;
  for (let q = REFRESHES.length - 1; q >= 0; q -= 1) {
    const refresh = REFRESHES[q];
    if (refresh !== undefined && ofMap(refresh)) {
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
}

/** Removes `item` from `queue`, where it is. */
function removeFrom<T>(queue: T[], item: T): void {
  const at = queue.indexOf(item);
  if (at >= 0) {
    queue.splice(at, 1);
  }
}

/** Which queued ratios a slice steps: all, or those of tables a frame drew near the switch lately. */
export type AirPointScope = "all" | "near";

/** The ratios the frames' slices hold queued. */
export function airPointWork(): { readonly refreshes: number } {
  return { refreshes: REFRESHES.length };
}

/**
 * A frame's slice of the air points' work, once a frame from the terrain session's step after the
 * class-mapped points': the oldest queued ratio's steps, a ring of samples each, of those `scope`
 * takes, until `budgetMs` has passed since the slice began, read on `now` before each step; with
 * none it takes, the slice reads no clock and steps nothing. A finished ratio is held for its table;
 * a step that throws drops its ratio, said once.
 *
 * @param budgetMs - The slice's length, ms.
 * @param now - A monotonic clock, ms.
 * @param scope - `all`, or `near`: the tables whose bodies a frame drew near the switch within the
 *   last {@link AIR_POINT_NEAR_SLICES} slices.
 * @returns Whether work it would step remains queued.
 */
export function stepAirPoints(
  budgetMs: number,
  now: () => number = () => globalThis.performance.now(),
  scope: AirPointScope = "all",
): boolean {
  SLICES_RUN += 1;
  const taken = (refresh: QueuedAirRefresh): boolean => {
    if (scope === "all") {
      return true;
    }
    const noted = NEAR.get(refresh.table);
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
    let step: IteratorResult<void, AirPointRatio>;
    try {
      step = refresh.steps.next();
    } catch (error: unknown) {
      removeFrom(REFRESHES, refresh);
      if (!SAID.failed) {
        SAID.failed = true;
        console.error("the air of a body's point could not be found:", error);
      }
      continue;
    }
    if (step.done === true) {
      removeFrom(REFRESHES, refresh);
      hold(refresh.table, {
        lightKey: refresh.lightKey,
        slot: refresh.slot,
        surfaceKey: refresh.surfaceKey,
        airedKey: refresh.airedKey,
        figureKey: refresh.figureKey,
        towardsLight: refresh.towardsLight,
        towardsObserver: refresh.towardsObserver,
        found: step.value,
      });
    }
  }
}
