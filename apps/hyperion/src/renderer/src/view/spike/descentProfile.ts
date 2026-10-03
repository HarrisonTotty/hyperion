/**
 * The descent spike's scripted path (plan R05, T13.a, Design note 19): a pure function of script
 * time in body-fixed coordinates, from a 400 km orbit to a metre above a landing site and approach
 * azimuth drawn from the spike's seed.
 *
 * @remarks
 * The path is two kinematic profiles over the script: the horizontal speed along the ground track
 * and the altitude above the spheroid. Each is a velocity that is piecewise linear in time and
 * continuous, so position and velocity are continuous everywhere: where the table of Design note 19
 * changes vertical speed between segments, the velocity is blended linearly over the last 5 s of
 * the earlier segment (1 s for a segment shorter than 20 s), a cubic in position between the two.
 * The segments' vertical speeds are re-fitted (the plan's "T13.a re-fits") so that every boundary
 * altitude holds exactly with the blends included; the durations are the table's. The path is a
 * scripted camera, not a flight: the blends' accelerations are not a craft's.
 *
 * The ground track is the great circle through the landing site along the approach azimuth; the
 * camera is `altitude` above the spheroid along its normal over the track's point. The camera looks
 * along the track, pitched down 30° while the horizontal speed is at least 300 m/s and turning to
 * the nadir as it falls to zero, so that the hover looks straight down.
 */

import { add, cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { quaternionFromRows } from "../camera/quaternion";
import { type BodyFigure, spheroidNormal, surfacePoint } from "../terrain/planet";

/** One segment of the descent (Design note 19), as data. */
export interface DescentSegment {
  readonly name: string;
  readonly durationS: number;
  /** The altitude above the spheroid at the segment's start and end, metres. */
  readonly startAltitudeM: number;
  readonly endAltitudeM: number;
  /** The horizontal speed at the segment's start and end, m/s, linear in between. */
  readonly startSpeedMps: number;
  readonly endSpeedMps: number;
  /**
   * The vertical speed's shape: `constant` (then blended into the next segment's), `falling`
   * (linear to the next segment's starting rate, the flare) or `hover` (0.05 m/s down, then still).
   */
  readonly verticalShape: VerticalShape;
  /**
   * Whether the segment is flown at its altitude above the highest terrain under the track
   * (`DescentTerrain.trackMaxHeightM`) rather than above the site: the low fast pass.
   */
  readonly clearsTrack: boolean;
}

/**
 * The terrain the script is flown over, measured once by the caller before the run, so that the
 * path stays a pure function of the seed and these two numbers (the orchestrator's ruling,
 * 2026-10-03).
 */
export interface DescentTerrain {
  /** The terrain's height at the landing site, metres above the spheroid; 0 by default. */
  readonly siteHeightM?: number;
  /**
   * An upper bound on the terrain's height under the low fast pass's ground track, metres above
   * the spheroid (baked ranges plus ε over the patches under the track); 0 by default.
   */
  readonly trackMaxHeightM?: number;
}

/** How a segment's vertical speed runs (Design note 19's table). */
export type VerticalShape = "constant" | "falling" | "hover";

/**
 * Design note 19's segments (provisional, re-fitted here): the orbit coast, the descent arc, the
 * approach and flare, the low fast pass that outruns the high setting's workers, the slowdown, the
 * vertical descent and the hover to touchdown.
 */
export const DESCENT_SEGMENTS: ReadonlyArray<DescentSegment> = [
  {
    name: "orbit coast",
    durationS: 60,
    startAltitudeM: 400_000,
    endAltitudeM: 400_000,
    startSpeedMps: 7670,
    endSpeedMps: 7670,
    verticalShape: "constant",
    clearsTrack: false,
  },
  {
    name: "descent arc",
    durationS: 900,
    startAltitudeM: 400_000,
    endAltitudeM: 20_000,
    startSpeedMps: 7670,
    endSpeedMps: 1000,
    verticalShape: "constant",
    clearsTrack: false,
  },
  {
    name: "approach and flare",
    durationS: 120,
    startAltitudeM: 20_000,
    endAltitudeM: 300,
    startSpeedMps: 1000,
    endSpeedMps: 300,
    verticalShape: "falling",
    clearsTrack: false,
  },
  {
    name: "low fast pass",
    durationS: 30,
    startAltitudeM: 300,
    endAltitudeM: 300,
    startSpeedMps: 300,
    endSpeedMps: 300,
    verticalShape: "constant",
    clearsTrack: true,
  },
  {
    name: "slowdown",
    durationS: 60,
    startAltitudeM: 300,
    endAltitudeM: 200,
    startSpeedMps: 300,
    endSpeedMps: 0,
    verticalShape: "constant",
    clearsTrack: false,
  },
  {
    name: "vertical descent",
    durationS: 10,
    startAltitudeM: 200,
    endAltitudeM: 2,
    startSpeedMps: 0,
    endSpeedMps: 0,
    verticalShape: "constant",
    clearsTrack: false,
  },
  {
    name: "hover and touchdown",
    durationS: 50,
    startAltitudeM: 2,
    endAltitudeM: 1,
    startSpeedMps: 0,
    endSpeedMps: 0,
    verticalShape: "hover",
    clearsTrack: false,
  },
];

/** The hover's descent rate before it stops, m/s (Design note 19). */
const HOVER_DESCENT_MPS = 0.05;

/** A velocity knot: the profile's rate at a script time, linear to the next knot. */
interface Knot {
  readonly tS: number;
  readonly rate: number;
}

/** A continuous piecewise-linear rate and its exact integral from the first knot. */
class LinearProfile {
  readonly #knots: ReadonlyArray<Knot>;
  /** The integral up to each knot. */
  readonly #integrals: ReadonlyArray<number>;
  readonly #start: number;

  constructor(start: number, knots: ReadonlyArray<Knot>) {
    this.#start = start;
    this.#knots = knots;
    const integrals = [start];
    for (let i = 1; i < knots.length; i += 1) {
      const a = knots[i - 1];
      const b = knots[i];
      if (a === undefined || b === undefined || !(b.tS >= a.tS)) {
        throw new Error("a profile's knots must ascend in time");
      }
      integrals.push((integrals[i - 1] ?? start) + ((a.rate + b.rate) / 2) * (b.tS - a.tS));
    }
    this.#integrals = integrals;
  }

  /** The knot interval holding `tS`, clamped to the profile's span. */
  #at(tS: number): { readonly i: number; readonly dt: number; readonly slope: number } {
    const knots = this.#knots;
    let i = 0;
    while (i + 2 < knots.length && (knots[i + 1]?.tS ?? Infinity) <= tS) {
      i += 1;
    }
    const a = knots[i];
    const b = knots[i + 1];
    if (a === undefined || b === undefined) {
      throw new Error("a profile needs two knots");
    }
    const t = Math.min(Math.max(tS, a.tS), b.tS);
    const span = b.tS - a.tS;
    return { i, dt: t - a.tS, slope: span > 0 ? (b.rate - a.rate) / span : 0 };
  }

  rate(tS: number): number {
    const { i, dt, slope } = this.#at(tS);
    return (this.#knots[i]?.rate ?? 0) + slope * dt;
  }

  value(tS: number): number {
    const { i, dt, slope } = this.#at(tS);
    const rate = this.#knots[i]?.rate ?? 0;
    return (this.#integrals[i] ?? this.#start) + rate * dt + (slope * dt * dt) / 2;
  }
}

/**
 * The segments with the track-clearing segment's altitudes, and its neighbours' ends that meet
 * them, raised by `liftM`: the low pass flies its altitude above the highest terrain under its
 * track, which lies `liftM` above the site's.
 */
function liftOverTrack(
  segments: ReadonlyArray<DescentSegment>,
  liftM: number,
): ReadonlyArray<DescentSegment> {
  if (liftM === 0) {
    return segments;
  }
  return segments.map((segment, i) => {
    const raiseStart = segment.clearsTrack || (segments[i - 1]?.clearsTrack ?? false);
    const raiseEnd = segment.clearsTrack || (segments[i + 1]?.clearsTrack ?? false);
    return {
      ...segment,
      startAltitudeM: segment.startAltitudeM + (raiseStart ? liftM : 0),
      endAltitudeM: segment.endAltitudeM + (raiseEnd ? liftM : 0),
    };
  });
}

/** The blend's length at the end of a segment of `durationS` (Design note 19). */
export function blendS(durationS: number): number {
  return durationS < 20 ? 1 : 5;
}

/** The segments' start times, s, and the script's end. */
function segmentStarts(segments: ReadonlyArray<DescentSegment>): number[] {
  const starts = [0];
  for (const segment of segments) {
    starts.push((starts.at(-1) ?? 0) + segment.durationS);
  }
  return starts;
}

/**
 * The vertical-speed knots that meet every boundary altitude with the blends included.
 *
 * @remarks
 * Each `constant` segment holds a rate c; a `falling` one (the approach and flare) falls linearly to
 * the next segment's rate, and the `hover` descends at 0.05 m/s and then stops. A
 * blend ramps the rate over the last b seconds of a segment to the next segment's starting rate.
 * The rates are solved from the last segment back: the drop across segment i is
 * c_i (T_i − b_i) + (c_i + s_{i+1}) b_i ÷ 2, where s_{i+1} is the next segment's starting rate, so
 * c_i is linear in the known drop and s_{i+1}. A segment with no drop and a level neighbour holds
 * c = 0 and needs no blend; a level segment before a descending one climbs gently (the orbit coast
 * by about 18 m/s, the low fast pass by about 0.07 m/s) so that the blend's drop leaves its end
 * altitude exact.
 */
function verticalKnots(segments: ReadonlyArray<DescentSegment>): Knot[] {
  const starts = segmentStarts(segments);
  const n = segments.length;
  /** Each segment's starting rate, solved back to front. */
  const startRate: number[] = Array.from({ length: n + 1 }, () => 0);
  const knotsBack: Knot[][] = Array.from({ length: n }, () => []);
  for (let i = n - 1; i >= 0; i -= 1) {
    const segment = segments[i];
    const t0 = starts[i];
    if (segment === undefined || t0 === undefined) {
      throw new Error(`segment ${i} is missing`);
    }
    const t1 = t0 + segment.durationS;
    const drop = segment.endAltitudeM - segment.startAltitudeM;
    const next = startRate[i + 1] ?? 0;
    if (segment.verticalShape === "hover") {
      // −0.05 m/s until the last metre is down, then a 1 s stop: the drop is −0.05 (t_s + 0.5).
      const stopS = -drop / HOVER_DESCENT_MPS - 0.5;
      startRate[i] = -HOVER_DESCENT_MPS;
      knotsBack[i] = [
        { tS: t0, rate: -HOVER_DESCENT_MPS },
        { tS: t0 + stopS, rate: -HOVER_DESCENT_MPS },
        { tS: t0 + stopS + 1, rate: 0 },
        { tS: t1, rate: 0 },
      ];
      continue;
    }
    if (segment.verticalShape === "falling") {
      // Falls linearly to the next segment's rate: the drop is (s + next) T ÷ 2.
      const s = (2 * drop) / segment.durationS - next;
      startRate[i] = s;
      knotsBack[i] = [
        { tS: t0, rate: s },
        { tS: t1, rate: next },
      ];
      continue;
    }
    const b = next === 0 && drop === 0 ? 0 : blendS(segment.durationS);
    const c = (drop - (next * b) / 2) / (segment.durationS - b / 2);
    startRate[i] = c;
    knotsBack[i] =
      b === 0
        ? [
            { tS: t0, rate: c },
            { tS: t1, rate: c },
          ]
        : [
            { tS: t0, rate: c },
            { tS: t1 - b, rate: c },
            { tS: t1, rate: next },
          ];
  }
  return knotsBack.flat();
}

/** The horizontal-speed knots: the table's speeds, continuous by construction. */
function horizontalKnots(segments: ReadonlyArray<DescentSegment>): Knot[] {
  const starts = segmentStarts(segments);
  return segments.flatMap((segment, i) => {
    const t0 = starts[i] ?? 0;
    return [
      { tS: t0, rate: segment.startSpeedMps },
      { tS: t0 + segment.durationS, rate: segment.endSpeedMps },
    ];
  });
}

/** A landing site and the azimuth the descent approaches it along. */
export interface LandingSite {
  /** The latitude and longitude of the site's direction from the centre, rad. */
  readonly latitudeRad: number;
  readonly longitudeRad: number;
  /** The approach's azimuth at the site, clockwise from north, rad. */
  readonly azimuthRad: number;
}

/** The camera's state at one moment of the script, body-fixed. */
export interface DescentPose {
  readonly tS: number;
  /** The segment under way, or the last one past the script's end. */
  readonly segment: string;
  /** The camera's position from the body's centre, metres. */
  readonly positionM: Vec3;
  /** Its velocity, m/s: the derivative of `positionM`. */
  readonly velocityMps: Vec3;
  /** Its orientation: right, up and backward axes in the body-fixed axes (looking down −z). */
  readonly orientation: Quaternion;
  /** Altitude above the spheroid, metres: the site's height plus `clearanceM`. */
  readonly altitudeM: number;
  /**
   * Height above the landing site's terrain, metres: the script's own altitude, the one Design
   * note 19's table and the demand prediction read.
   */
  readonly clearanceM: number;
  /** Horizontal speed along the track and vertical speed, m/s. */
  readonly horizontalSpeedMps: number;
  readonly verticalSpeedMps: number;
  /**
   * The point at the site's terrain height beneath the camera, metres: where a grounded contact is
   * placed (the site itself at touchdown).
   */
  readonly groundPointM: Vec3;
}

/** SplitMix64's step (Steele, Lea and Flood 2014): the next state and a 64-bit output. */
function splitMix64(state: bigint): readonly [bigint, bigint] {
  const mask = (1n << 64n) - 1n;
  const next = (state + 0x9e37_79b9_7f4a_7c15n) & mask;
  let z = next;
  z = ((z ^ (z >> 30n)) * 0xbf58_476d_1ce4_e5b9n) & mask;
  z = ((z ^ (z >> 27n)) * 0x94d0_49bb_1331_11ebn) & mask;
  return [next, z ^ (z >> 31n)];
}

/** The unit fraction of a 64-bit draw's top 53 bits, in [0, 1). */
function unitOf(draw: bigint): number {
  return Number(draw >> 11n) / 2 ** 53;
}

/** The latitudes a site is drawn between, rad: the band where the test planet's faces meet both poles' axes least. */
const SITE_LATITUDE_LIMIT_RAD = (60 * Math.PI) / 180;

/**
 * The landing site and approach azimuth of a seed, uniform over the surface between ±60° latitude
 * and uniform in azimuth.
 *
 * @remarks
 * Drawn by SplitMix64 from the seed alone, so the path is identical every run; the test planet
 * belongs to no universe, so the draw needs none of the sim's streams (Design note 13's tags are
 * for its heights).
 *
 * @param seed - The spike's seed, an unsigned 64-bit integer.
 * @throws RangeError if `seed` is not in [0, 2^64).
 */
export function landingSiteOf(seed: bigint): LandingSite {
  if (seed < 0n || seed >= 1n << 64n) {
    throw new RangeError(`a seed is a u64, got ${seed}`);
  }
  const [s1, a] = splitMix64(seed);
  const [s2, b] = splitMix64(s1);
  const [, c] = splitMix64(s2);
  const sinLimit = Math.sin(SITE_LATITUDE_LIMIT_RAD);
  return {
    latitudeRad: Math.asin((2 * unitOf(a) - 1) * sinLimit),
    longitudeRad: 2 * Math.PI * unitOf(b) - Math.PI,
    azimuthRad: 2 * Math.PI * unitOf(c),
  };
}

/** The pitch below the horizontal, rad: 30° at 300 m/s and above, the nadir at rest. */
function pitchRad(horizontalSpeedMps: number): number {
  const fast = Math.min(1, Math.max(0, horizontalSpeedMps / 300));
  return (Math.PI / 180) * (90 - 60 * fast);
}

/** The scripted descent for one seed and figure. */
export class DescentProfile {
  readonly figure: BodyFigure;
  readonly site: LandingSite;
  readonly segments: ReadonlyArray<DescentSegment>;
  /** The script's length, s. */
  readonly durationS: number;
  readonly #starts: ReadonlyArray<number>;
  readonly #vertical: LinearProfile;
  readonly #horizontal: LinearProfile;
  /** The ground track's length, metres: the distance flown to the site. */
  readonly #trackM: number;
  /** The site's unit direction and the track's unit tangent there (towards the site). */
  readonly #siteDir: Vec3;
  readonly #heading: Vec3;
  /** The radius the track's arc is measured on, metres: the mean of a and c. */
  readonly #arcRadiusM: number;
  /** The landing site's terrain height, metres above the spheroid. */
  readonly #siteHeightM: number;

  /**
   * @param terrain - The site's height and the low pass's track maximum; the script's altitudes
   *   are above the site, and the low pass flies its altitude above the track maximum instead.
   */
  constructor(
    figure: BodyFigure,
    site: LandingSite,
    terrain: DescentTerrain = {},
    baseSegments: ReadonlyArray<DescentSegment> = DESCENT_SEGMENTS,
  ) {
    this.figure = figure;
    this.site = site;
    this.#siteHeightM = terrain.siteHeightM ?? 0;
    const lift = Math.max(0, (terrain.trackMaxHeightM ?? 0) - this.#siteHeightM);
    const segments = liftOverTrack(baseSegments, lift);
    this.segments = segments;
    const starts = segmentStarts(segments);
    this.#starts = starts;
    this.durationS = starts.at(-1) ?? 0;
    this.#vertical = new LinearProfile(segments[0]?.startAltitudeM ?? 0, verticalKnots(segments));
    this.#horizontal = new LinearProfile(0, horizontalKnots(segments));
    this.#trackM = this.#horizontal.value(this.durationS);
    const { latitudeRad: lat, longitudeRad: lon, azimuthRad: az } = site;
    this.#siteDir = vec3(
      Math.cos(lat) * Math.cos(lon),
      Math.cos(lat) * Math.sin(lon),
      Math.sin(lat),
    );
    const east = vec3(-Math.sin(lon), Math.cos(lon), 0);
    const north = vec3(
      -Math.sin(lat) * Math.cos(lon),
      -Math.sin(lat) * Math.sin(lon),
      Math.cos(lat),
    );
    this.#heading = add(scale(north, Math.cos(az)), scale(east, Math.sin(az)));
    this.#arcRadiusM = (figure.equatorialRadiusM + figure.polarRadiusM) / 2;
  }

  /** The segment under way at `tS`, clamped to the script. */
  segmentAt(tS: number): DescentSegment {
    let i = 0;
    while (i + 1 < this.segments.length && (this.#starts[i + 1] ?? Infinity) <= tS) {
      i += 1;
    }
    const segment = this.segments[i];
    if (segment === undefined) {
      throw new Error("a descent has no segments");
    }
    return segment;
  }

  /** Each segment's start and end, s. */
  segmentSpans(): ReadonlyArray<{
    readonly name: string;
    readonly startS: number;
    readonly endS: number;
  }> {
    return this.segments.map((segment, i) => ({
      name: segment.name,
      startS: this.#starts[i] ?? 0,
      endS: (this.#starts[i] ?? 0) + segment.durationS,
    }));
  }

  /** The unit direction from the centre and the track's tangent at `remainingM` before the site. */
  #trackAt(remainingM: number): { readonly dir: Vec3; readonly tangent: Vec3 } {
    // Back along the great circle from the site: the angle is the remaining distance over R.
    const angle = remainingM / this.#arcRadiusM;
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    return {
      dir: add(scale(this.#siteDir, c), scale(this.#heading, -s)),
      tangent: add(scale(this.#siteDir, s), scale(this.#heading, c)),
    };
  }

  /** The camera's position at `tS`, body-fixed metres (the cheap half of {@link poseAt}). */
  positionAt(tS: number): Vec3 {
    const remaining = this.#trackM - this.#horizontal.value(tS);
    const { dir } = this.#trackAt(remaining);
    const p = surfacePoint(
      this.figure,
      [dir.x, dir.y, dir.z],
      this.#siteHeightM + this.#vertical.value(tS),
    );
    return vec3(p[0], p[1], p[2]);
  }

  /** The camera's state at script time `tS`, clamped to the script. */
  poseAt(tS: number): DescentPose {
    const t = Math.min(Math.max(tS, 0), this.durationS);
    const clearanceM = this.#vertical.value(t);
    const altitudeM = this.#siteHeightM + clearanceM;
    const horizontalSpeedMps = this.#horizontal.rate(t);
    const verticalSpeedMps = this.#vertical.rate(t);
    const { dir, tangent } = this.#trackAt(this.#trackM - this.#horizontal.value(t));
    const d: readonly [number, number, number] = [dir.x, dir.y, dir.z];
    const p = surfacePoint(this.figure, d, altitudeM);
    const g = surfacePoint(this.figure, d, this.#siteHeightM);
    const nu = spheroidNormal(this.figure, d);
    const up = vec3(nu[0], nu[1], nu[2]);
    // The velocity by a central difference of the exact position: the profiles are C¹, and the
    // geometry's own derivative is smooth, so a 1 ms step gives it to well under a mm/s.
    const h = 1e-3;
    const ahead = this.positionAt(Math.min(t + h, this.durationS));
    const behind = this.positionAt(Math.max(t - h, 0));
    const span = Math.min(t + h, this.durationS) - Math.max(t - h, 0);
    const velocityMps = scale(add(ahead, scale(behind, -1)), 1 / span);
    const heading = normalise(add(tangent, scale(up, -dot(tangent, up))));
    const pitch = pitchRad(horizontalSpeedMps);
    const forward = add(scale(heading, Math.cos(pitch)), scale(up, -Math.sin(pitch)));
    const cameraUp = add(scale(heading, Math.sin(pitch)), scale(up, Math.cos(pitch)));
    const right = cross(forward, cameraUp);
    const back = scale(forward, -1);
    // The rotation's columns are the camera's right, up and backward axes.
    const orientation = quaternionFromRows([
      vec3(right.x, cameraUp.x, back.x),
      vec3(right.y, cameraUp.y, back.y),
      vec3(right.z, cameraUp.z, back.z),
    ]);
    return {
      tS: t,
      segment: this.segmentAt(t).name,
      positionM: vec3(p[0], p[1], p[2]),
      velocityMps,
      orientation,
      altitudeM,
      clearanceM,
      horizontalSpeedMps,
      verticalSpeedMps,
      groundPointM: vec3(g[0], g[1], g[2]),
    };
  }
}
