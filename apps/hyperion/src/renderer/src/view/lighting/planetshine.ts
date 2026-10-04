/**
 * Planetshine: the light a lit neighbour reflects onto a body, the dominant light of a moon's night
 * side (plan R07, T11; Design note 7).
 *
 * @remarks
 * A neighbour N of equatorial radius R at distance Δ from a body B gives B, face-on at B's centre,
 * the illuminance E = E★ (R ÷ Δ)² p Φ(α) per channel, where E★ is its stars' illuminance on N and
 * α the angle star–N–B: the light N reflects towards B, which is what N's point regime reflects
 * towards a camera (`bodyReflection`, the law integrated over the figure for a spheroid, whose p is
 * defined against π a c). Each body is lit by the neighbours whose photopic E is largest, at most
 * {@link PLANETSHINE_SOURCES_HIGH} (one on the low setting), each a uniform sphere of angular radius
 * asin(R ÷ Δ) through the same `sphereIrradianceFactor` as a star, its face-on illuminance carried
 * from B's centre to each lit point by the inverse square ({@link planetshineIrradiance}). A source
 * is never shadow-tested against a third body on its way to B. The starlight on the neighbour is
 * cut by the eclipse term from its centre of the bodies larger than it only, so that a moon in its
 * planet's shadow gives no planetshine: an interim, which R07.T10.b's disc-averaged eclipse
 * replaces (decision-r07-earth-albedo). Its stated errors (science check, 2026-10-04):
 *
 * - the lit crescent's light centroid lies off the neighbour's centre, by 0.4 R at 60° of phase and
 *   3π ÷ 16 ≈ 0.59 R at quarter phase for a Lambert sphere (5.7° for Jupiter, 9.8° in radius,
 *   seen from Io), and towards 0.9 R in a thin crescent;
 * - the neighbour's limb darkening is taken as uniform (−0.29% of face-on at Io's terminator for a
 *   Lambert Jupiter);
 * - the far-field E errs at first order in R ÷ Δ for a disc brighter at its centre: the exact
 *   illuminance is 12% above it at full phase and 10% below it at quarter phase for Jupiter seen
 *   from Io, and 1.2% above it for Earth seen from the Moon;
 * - beyond quarter phase B sees less than N's hemisphere, which hides the limb crescent: the exact
 *   is 34% below E at 120°, 71% below at 150° and nothing from 170.6° for Jupiter seen from Io,
 *   10% below at 150° for Earth seen from the Moon;
 * - a smaller body's shadow on the neighbour is left out (about 0.1% of Jupiter-shine for Io's, up
 *   to 11% of earthshine in a central solar eclipse);
 * - partial phases are taken at the neighbour's centre, so a moon wider than its planet's penumbra
 *   fades too fast (in 44 s rather than 254 s for Io entering Jupiter's shadow), and a pair of
 *   equal moons never shadows each other.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { BodyFigure, BodyPhotometry } from "../appearance/fromWire";
import {
  PHASE_TABLE_SAMPLES,
  PHASE_TABLE_STEP_RAD,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "../appearance/law";
import { geometricAlbedo } from "../appearance/phase";
import { shapePhase } from "../appearance/shapes";
import { bodyReflection } from "../bodies/oblate";
import type { Rgb } from "../photometry/toneCurve";
import { eclipseVisible } from "./annuli";
import { type LightAtPoint, lightsAt, MAX_BODY_LIGHTS, type PlacedLight } from "./hostLights";
import { photopicIlluminance } from "./illuminance";
import { asOccluders, type LightingBody, type LightingSphere, occludersFor } from "./occluders";
import { sphereIrradianceFactor } from "./sphereIrradiance";

/** The neighbours that light a body by planetshine at most, on the high setting (Design note 7). */
export const PLANETSHINE_SOURCES_HIGH = 2;

/** The neighbours that light a body by planetshine at most, on the low setting (Design note 18). */
export const PLANETSHINE_SOURCES_LOW = 1;

/** A lit neighbour as a light on a body: a uniform sphere seen from the body's centre. */
export interface SecondarySource {
  /** The neighbour. */
  readonly body: BodyIdHex;
  /** Its centre's unit direction from the body's centre, along the galactic axes. */
  readonly direction: Vec3;
  /** Its angular radius from the body's centre, asin(R ÷ Δ), rad. */
  readonly angularRadiusRad: number;
  /** Its distance Δ from the body's centre, m, so that each lit point takes its own geometry. */
  readonly distanceM: number;
  /** Its equatorial radius R, m. */
  readonly radiusM: number;
  /** Its illuminance face-on at the body's centre, lx, per display channel (r, g, b). */
  readonly illuminance: Rgb;
}

/** A lit body as planetshine takes it, as a neighbour or as the body it lights. */
export interface ReflectingBody {
  readonly id: BodyIdHex;
  /** Its centre, m, in the frame of the lights' centres. */
  readonly centreM: Vec3;
  readonly figure: BodyFigure;
  readonly photometry: BodyPhotometry;
}

/**
 * A lit body as a planetshine neighbour in one frame: the stars that light it, found once a frame
 * rather than once for each body it lights.
 */
export interface LitNeighbour {
  readonly body: ReflectingBody;
  /**
   * The stars that light it, as its own regime takes them (`lightsAt`, `MAX_BODY_LIGHTS`), each
   * one's illuminance cut by the eclipse term from its centre of the bodies larger than it.
   */
  readonly lights: ReadonlyArray<LightAtPoint>;
  /** The sphere its p is defined against (π a c equator-on): radius √(a c). */
  readonly sphere: BodyFigure;
  /**
   * An upper bound on its photopic illuminance at a distance Δ, times (Δ ÷ √(a c))², lx: its
   * starlight's photopic sum times its largest p Φ(α) over every phase ({@link phaseMaximum}).
   */
  readonly boundLx: number;
}

/** Each law's {@link phaseMaximum}, found once: laws are immutable values. */
const PHASE_MAXIMA = new WeakMap<PhotometricLaw, number>();

/** The margin on the sampled maximum for the interpolation between samples, which bends little. */
const PHASE_MAXIMUM_MARGIN = 1.01;

/**
 * An upper bound on a law's p Φ(α) over every phase and channel: its largest p times the largest
 * Φ = f Φ_shape at the table's samples, with a 1% margin for the interpolation between them (the
 * 0.5° step bends f Φ_shape by far less).
 */
export function phaseMaximum(law: PhotometricLaw): number {
  const known = PHASE_MAXIMA.get(law);
  if (known !== undefined) {
    return known;
  }
  const { rgb } = phaseFactorTableOf(law);
  let phase = 0;
  for (let i = 0; i < PHASE_TABLE_SAMPLES; i += 1) {
    const shape = shapePhase(law.lommelSeeligerShare, i * PHASE_TABLE_STEP_RAD);
    for (let c = 0; c < 3; c += 1) {
      phase = Math.max(phase, (rgb[3 * i + c] ?? 0) * shape);
    }
  }
  const bound = Math.max(...geometricAlbedo(law), 0) * phase * PHASE_MAXIMUM_MARGIN;
  PHASE_MAXIMA.set(law, bound);
  return bound;
}

/**
 * The stars that light a neighbour, each cut by the eclipse term from its centre (Design note 6)
 * of the bodies larger than it: a moon in its planet's shadow, as in a total lunar eclipse, gives
 * no planetshine. A smaller body's shadow is left out, since from the neighbour's centre it would
 * hide the whole star where it hides a spot (Io's on Jupiter takes about 0.1% of Jupiter-shine, a
 * solar eclipse about 10% of earthshine).
 */
function neighbourLights(
  body: ReflectingBody,
  bodies: ReadonlyArray<ReflectingBody>,
  hosts: ReadonlyArray<PlacedLight>,
  annuli: number,
): LightAtPoint[] {
  const lights = lightsAt(body.centreM, hosts, MAX_BODY_LIGHTS);
  const radiusM = body.figure.equatorialRadiusM;
  const larger: LightingBody[] = bodies.flatMap((other) =>
    other.figure.equatorialRadiusM > radiusM
      ? [{ id: other.id, centreM: other.centreM, radiusM: other.figure.equatorialRadiusM }]
      : [],
  );
  const stars: LightingSphere[] = lights.map((light) => ({
    centreM: light.host.centreM,
    radiusM: light.host.disc.radius_m,
  }));
  const shadowing = asOccluders(
    occludersFor({ id: body.id, centreM: body.centreM, radiusM }, stars, larger),
  );
  if (shadowing.length === 0) {
    return lights;
  }
  return lights.map((light) => {
    const [bLaw, vLaw, rLaw] = light.host.disc.limb;
    const visible = [rLaw, vLaw, bLaw].map((law) =>
      eclipseVisible(
        {
          centreM: light.host.centreM,
          radiusM: light.host.disc.radius_m,
          limbC: law.c,
          limbAlpha: law.alpha,
        },
        body.centreM,
        shadowing,
        annuli,
      ),
    );
    const illuminance: Rgb = [
      light.illuminance[0] * (visible[0] ?? 1),
      light.illuminance[1] * (visible[1] ?? 1),
      light.illuminance[2] * (visible[2] ?? 1),
    ];
    return { host: light.host, toStarM: light.toStarM, illuminance };
  });
}

/**
 * The frame's lit bodies as planetshine neighbours.
 *
 * @param annuli - The eclipse term's annuli: `DISC_ANNULI_HIGH` or `DISC_ANNULI_LOW`.
 */
export function litNeighbours(
  bodies: ReadonlyArray<ReflectingBody>,
  hosts: ReadonlyArray<PlacedLight>,
  annuli: number,
): LitNeighbour[] {
  return bodies.map((body) => {
    const lights = neighbourLights(body, bodies, hosts, annuli);
    const radiusM = Math.sqrt(body.figure.equatorialRadiusM * body.figure.polarRadiusM);
    const starlight = lights.reduce(
      (sum, light) => sum + photopicIlluminance(light.illuminance),
      0,
    );
    return {
      body,
      lights,
      sphere: { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: body.figure.pole },
      boundLx: starlight * phaseMaximum(body.photometry.law),
    };
  });
}

/** A neighbour whose centre is within this fraction of its radius of the point lights nothing. */
const INSIDE_MARGIN = 1e-9;

/**
 * The illuminance a neighbour reflects face-on at a point, lx, per display channel (r, g, b):
 * Σ E★ (R ÷ Δ)² × the light its figure and law reflect towards the point, over its stars.
 *
 * @param figure - The figure the reflection is integrated over: the neighbour's own, or its
 *   equivalent sphere for the ranking.
 */
function reflectedIlluminance(
  neighbour: LitNeighbour,
  figure: BodyFigure,
  towardsPoint: Vec3,
  distanceM: number,
): Rgb {
  const solid = (figure.equatorialRadiusM / distanceM) ** 2;
  const out: [number, number, number] = [0, 0, 0];
  for (const light of neighbour.lights) {
    const reflected = bodyReflection(
      figure,
      neighbour.body.photometry.law,
      normalise(light.toStarM),
      towardsPoint,
    );
    for (const c of [0, 1, 2] as const) {
      out[c] += light.illuminance[c] * solid * reflected[c];
    }
  }
  return out;
}

/** One candidate neighbour: its geometry from the body and its light by its equivalent sphere. */
interface Candidate {
  readonly neighbour: LitNeighbour;
  readonly towards: Vec3;
  readonly distanceM: number;
  readonly estimateLx: number;
}

/** The ranking: the larger estimate first, then the identifier, so that input order is moot. */
function byEstimate(a: Candidate, b: Candidate): number {
  const idA = a.neighbour.body.id;
  const idB = b.neighbour.body.id;
  return b.estimateLx - a.estimateLx || (idA < idB ? -1 : idA > idB ? 1 : 0);
}

/**
 * The neighbours that light a body by planetshine: the `max` whose photopic illuminance at its
 * centre is largest, largest first, each with its illuminance there (Design note 7). A neighbour
 * that reflects nothing towards the body (at new phase, or lit by no star) is none.
 *
 * @remarks
 * The candidates are ranked by each one's equivalent sphere of radius √(a c), exact for a sphere
 * and needing no quadrature; one whose upper bound ({@link LitNeighbour.boundLx}) cannot reach the
 * `max`-th best so far is not evaluated. The chosen ones' illuminance integrates their own figure.
 *
 * @param lit - The frame's lit bodies (`litNeighbours`); the body itself among them is skipped.
 * @param max - The sources at most: {@link PLANETSHINE_SOURCES_HIGH} or
 *   {@link PLANETSHINE_SOURCES_LOW}.
 */
export function planetshineSources(
  body: ReflectingBody,
  lit: ReadonlyArray<LitNeighbour>,
  max: number,
): SecondarySource[] {
  if (!(max > 0)) {
    return [];
  }
  let best: Candidate[] = [];
  for (const neighbour of lit) {
    if (neighbour.body.id === body.id || !(neighbour.boundLx > 0)) {
      continue;
    }
    const toBody = sub(body.centreM, neighbour.body.centreM);
    const distanceM = norm(toBody);
    if (!(distanceM > neighbour.body.figure.equatorialRadiusM * (1 + INSIDE_MARGIN))) {
      continue;
    }
    // `boundLx` bounds the estimate from above, so a neighbour below the `max`-th best so far
    // cannot enter the best.
    const last = best.length < max ? undefined : best[best.length - 1];
    const boundLx = neighbour.boundLx * (neighbour.sphere.equatorialRadiusM / distanceM) ** 2;
    if (last !== undefined && boundLx < last.estimateLx) {
      continue;
    }
    const towards = scale(toBody, 1 / distanceM);
    const estimateLx = photopicIlluminance(
      reflectedIlluminance(neighbour, neighbour.sphere, towards, distanceM),
    );
    if (estimateLx > 0) {
      best = [...best, { neighbour, towards, distanceM, estimateLx }]
        .toSorted(byEstimate)
        .slice(0, max);
    }
  }
  return best.flatMap(({ neighbour, towards, distanceM }): SecondarySource[] => {
    const illuminance = reflectedIlluminance(neighbour, neighbour.body.figure, towards, distanceM);
    if (!(photopicIlluminance(illuminance) > 0)) {
      return [];
    }
    return [
      {
        body: neighbour.body.id,
        direction: scale(towards, -1),
        angularRadiusRad: Math.asin(neighbour.body.figure.equatorialRadiusM / distanceM),
        distanceM,
        radiusM: neighbour.body.figure.equatorialRadiusM,
        illuminance,
      },
    ];
  });
}

/**
 * The factor on a source's face-on illuminance at the body's centre that reaches a lit point: the
 * uniform sphere's irradiance factor there (`sphereIrradianceFactor`) times the inverse square from
 * the centre to the point (Design note 7); the twin of `litBody.wgsl`'s `planetshine_irradiance`.
 *
 * @param toSource - From the point to the source's centre; it, `sourceRadius` and
 *   `centreDistance` share one unit of length (the disc's is the body's equatorial radius).
 * @param sourceRadius - The source's radius.
 * @param centreDistance - The source's distance from the body's centre.
 * @param normal - The point's unit outward normal.
 * @param horizonRad - The local horizon's elevation towards the source; 0 on the smooth figure.
 */
export function planetshineIrradiance(
  toSource: Vec3,
  sourceRadius: number,
  centreDistance: number,
  normal: Vec3,
  horizonRad = 0,
): number {
  const distance = norm(toSource);
  const mu0 = dot(normal, toSource) / distance;
  const near = centreDistance / distance;
  return (
    sphereIrradianceFactor(
      distance / sourceRadius,
      Math.acos(Math.min(1, Math.max(-1, mu0))),
      horizonRad,
    ) *
    near *
    near
  );
}
