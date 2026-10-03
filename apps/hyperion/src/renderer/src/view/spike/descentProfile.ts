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
   * (`DescentTerrain.trackMaxHeightM`) rather than above the site: the low fast pass, the 300 m
   * piece of the clearance ruling.
   */
  readonly clearsTrack: boolean;
  /**
   * How the segment's ground is cut into stretches and bounded (decision-r05-descent-clearance.md,
   * rule 1), or `null` where the camera is over the site itself, whose height is exact (the
   * vertical descent and the hover).
   */
  readonly stretches: SegmentStretches | null;
}

/** A segment's stretch rule (decision-r05-descent-clearance.md's table). */
export interface SegmentStretches {
  /** The pieces' length, s, a whole number; the last piece ends with the segment. */
  readonly pieceS: number;
  /** The bound level and the clearance C, metres. */
  readonly level: number;
  readonly clearanceM: number;
  /**
   * A coarser level for the pieces whose table clearance at their end is at least `aboveM`
   * (the flare above 2 km: level 12, whose 2.3 km edge exceeds C).
   */
  readonly high?: { readonly aboveM: number; readonly level: number };
  /** The last piece's own name, level and clearance (the slowdown's final approach). */
  readonly last?: { readonly piece: string; readonly level: number; readonly clearanceM: number };
}

/**
 * The terrain the script is flown over, measured once by the caller before the run, so that the
 * path stays a pure function of the seed, the site's height and the per-stretch floors (the
 * orchestrator's ruling and decision-r05-descent-clearance.md, 2026-10-03).
 */
export interface DescentTerrain {
  /** The terrain's height at the landing site, metres above the spheroid; 0 by default. */
  readonly siteHeightM?: number;
  /**
   * An upper bound on the terrain's height under the low fast pass's ground track, metres above
   * the spheroid (baked ranges plus ε over the patches under the track); 0 by default. Kept for
   * older callers: without `stretchMaxHeightsM` it is the low pass's floor and every other
   * stretch's is the site's height; with it, it is ignored (the low pass's entry there is the
   * readout's).
   */
  readonly trackMaxHeightM?: number;
  /**
   * F_k, a true upper bound on the finest mesh's height over the ground of `trackStretches(…)[k]`,
   * metres above the spheroid, in the same order and of the same length. Absent, every stretch's
   * floor is the site's height and the profile is the one without it, bit for bit.
   */
  readonly stretchMaxHeightsM?: ReadonlyArray<number>;
}

/**
 * One stretch of the ground track and the bound it is flown over (decision-r05-descent-clearance.md,
 * rule 1): the same for every terrain, since the track and the horizontal profile do not depend on
 * it.
 */
export interface TrackStretch {
  /** Its name: the segment's, numbered where the segment is split ("slowdown 3"), or "final approach". */
  readonly piece: string;
  /** The segment it is a piece of. */
  readonly segment: string;
  /** The along-track distance before the site at its start and end, metres; from > to ≥ 0. */
  readonly fromRemainingM: number;
  readonly toRemainingM: number;
  /** Its script times, s, on whole seconds (so on the 64 Hz grid). */
  readonly startS: number;
  readonly endS: number;
  /** The level whose patches its floor is taken over (ε_n plus the baked maximum). */
  readonly level: number;
  /** C, the clearance it is flown at above its floor at least, metres. */
  readonly clearanceM: number;
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
    stretches: { pieceS: 100, level: 4, clearanceM: 1000 },
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
    stretches: { pieceS: 100, level: 4, clearanceM: 1000 },
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
    stretches: { pieceS: 10, level: 14, clearanceM: 200, high: { aboveM: 2000, level: 12 } },
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
    stretches: { pieceS: 30, level: 14, clearanceM: 300 },
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
    stretches: {
      pieceS: 10,
      level: 14,
      clearanceM: 200,
      last: { piece: "final approach", level: 16, clearanceM: 100 },
    },
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
    stretches: null,
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
    stretches: null,
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

/** What the vertical solve reads of a segment or of a split segment's piece. */
type VerticalPiece = Pick<
  DescentSegment,
  "durationS" | "startAltitudeM" | "endAltitudeM" | "verticalShape"
>;

/** The blend's length at the end of a segment of `durationS` (Design note 19). */
export function blendS(durationS: number): number {
  return durationS < 20 ? 1 : 5;
}

/** The segments' start times, s, and the script's end. */
function segmentStarts(segments: ReadonlyArray<{ readonly durationS: number }>): number[] {
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
function verticalKnots(segments: ReadonlyArray<VerticalPiece>): Knot[] {
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

/**
 * The stretch plan (decision-r05-descent-clearance.md, rule 1): each segment's pieces with their
 * bound levels and clearances, from the table's own (unlifted) altitudes `datum`, so the same for
 * every terrain.
 */
function planStretches(
  segments: ReadonlyArray<DescentSegment>,
  datum: LinearProfile,
  horizontal: LinearProfile,
  trackM: number,
): TrackStretch[] {
  const starts = segmentStarts(segments);
  const out: TrackStretch[] = [];
  for (const [i, segment] of segments.entries()) {
    const rule = segment.stretches;
    const t0 = starts[i] ?? 0;
    if (rule === null) {
      continue;
    }
    const count = Math.ceil(segment.durationS / rule.pieceS);
    for (let k = 0; k < count; k += 1) {
      const startS = t0 + k * rule.pieceS;
      const endS = Math.min(t0 + (k + 1) * rule.pieceS, t0 + segment.durationS);
      const isLast = k === count - 1;
      const coarse =
        rule.high !== undefined && datum.value(endS) >= rule.high.aboveM ? rule.high.level : null;
      const last = isLast ? rule.last : undefined;
      out.push({
        piece: last?.piece ?? (count === 1 ? segment.name : `${segment.name} ${k + 1}`),
        segment: segment.name,
        fromRemainingM: trackM - horizontal.value(startS),
        toRemainingM: trackM - horizontal.value(endS),
        startS,
        endS,
        level: last?.level ?? coarse ?? rule.level,
        clearanceM: last?.clearanceM ?? rule.clearanceM,
      });
    }
  }
  return out;
}

/** How many times the interior check may lift a piece's boundaries before giving up (rule 4). */
const MAX_LIFT_ROUNDS = 4;

/** What the interior check adds to a deficit when it lifts, metres (rule 4). */
const LIFT_MARGIN_M = 0.1;

/**
 * The deficit the interior check lets pass, metres: the rounding of the profile's integrals, so
 * that a boundary sitting exactly at its floor plus C (the low pass at the site's 300 m) is not
 * lifted for 10⁻¹³ m.
 */
export const FLOOR_TOLERANCE_M = 1e-6;

/** The sampling rate of the interior check, Hz: the fixed-step run's (rule 4). */
const CHECK_RATE_HZ = 64;

/** The vertical solve's result: the table's segments as flown, the pieces solved and the margins. */
interface FlownProfile {
  readonly segments: ReadonlyArray<DescentSegment>;
  readonly vertical: LinearProfile;
  readonly minFloorMarginM: number;
}

/**
 * Flies the table over the floors (decision-r05-descent-clearance.md, rules 3 and 4), in metres
 * above the site: each piece boundary b at A_b = max(table_b, f_L + C_L, f_R + C_R), the low pass
 * level at the larger of its two, the vertical descent from the last boundary, stretched to keep
 * its speed; a segment none of whose interior boundaries rose is flown whole, as the table has it,
 * and a split one as `constant` pieces. Then the 64 Hz check lifts any piece below its clearance
 * by the deficit and 0.1 m, at most four times.
 *
 * @throws RangeError if a piece is still below its clearance after the fourth lift.
 */
function flyOverFloors(
  segments: ReadonlyArray<DescentSegment>,
  stretches: ReadonlyArray<TrackStretch>,
  floorsM: ReadonlyArray<number>,
  datum: LinearProfile,
): FlownProfile {
  const starts = segmentStarts(segments);
  const count = stretches.length;
  // The table's clearance at each boundary: a segment's own altitude where the boundary is a
  // segment's start or the stretches' end, so that an unlifted profile is the table's bit for bit.
  const table = Array.from({ length: count + 1 }, (_, b) => {
    const tS = b < count ? (stretches[b]?.startS ?? 0) : (stretches[count - 1]?.endS ?? 0);
    const segment = segments[starts.indexOf(tS)];
    return segment === undefined ? datum.value(tS) : segment.startAltitudeM;
  });
  const lowPass = stretches.findIndex(
    (stretch) => segments.find((s) => s.name === stretch.segment)?.clearsTrack === true,
  );
  const raise: number[] = Array.from({ length: count + 1 }, () => 0);
  for (let round = 0; ; round += 1) {
    const boundary = table.map((tableM, b) => {
      const left = stretches[b - 1];
      const right = stretches[b];
      return (
        Math.max(
          tableM,
          left === undefined ? -Infinity : (floorsM[b - 1] ?? 0) + left.clearanceM,
          right === undefined ? -Infinity : (floorsM[b] ?? 0) + right.clearanceM,
        ) + (raise[b] ?? 0)
      );
    });
    if (lowPass >= 0) {
      const level = Math.max(boundary[lowPass] ?? 0, boundary[lowPass + 1] ?? 0);
      boundary[lowPass] = level;
      boundary[lowPass + 1] = level;
    }
    const flown = flySegments(segments, stretches, table, boundary);
    const vertical = new LinearProfile(
      flown.pieces[0]?.startAltitudeM ?? 0,
      verticalKnots(flown.pieces),
    );
    const margins = stretches.map((stretch, k) => {
      let least = Infinity;
      const samples = Math.round((stretch.endS - stretch.startS) * CHECK_RATE_HZ);
      for (let n = 0; n <= samples; n += 1) {
        const tS = stretch.startS + n / CHECK_RATE_HZ;
        least = Math.min(least, vertical.value(tS) - (floorsM[k] ?? 0) - stretch.clearanceM);
      }
      return least;
    });
    const minFloorMarginM = Math.min(...margins);
    if (minFloorMarginM >= -FLOOR_TOLERANCE_M) {
      return { segments: flown.segments, vertical, minFloorMarginM };
    }
    if (round >= MAX_LIFT_ROUNDS) {
      const worst = stretches[margins.indexOf(minFloorMarginM)]?.piece ?? "?";
      throw new RangeError(
        `the descent cannot clear its floors: ${worst} is ${(-minFloorMarginM).toFixed(2)} m short after ${MAX_LIFT_ROUNDS} lifts`,
      );
    }
    for (const [k, margin] of margins.entries()) {
      if (margin < -FLOOR_TOLERANCE_M) {
        raise[k] = (raise[k] ?? 0) - margin + LIFT_MARGIN_M;
        raise[k + 1] = (raise[k + 1] ?? 0) - margin + LIFT_MARGIN_M;
      }
    }
  }
}

/**
 * The table's segments and the vertical solve's pieces for the boundary altitudes `boundary` (above
 * the site), `table` being the unlifted ones.
 */
function flySegments(
  segments: ReadonlyArray<DescentSegment>,
  stretches: ReadonlyArray<TrackStretch>,
  table: ReadonlyArray<number>,
  boundary: ReadonlyArray<number>,
): { readonly segments: DescentSegment[]; readonly pieces: VerticalPiece[] } {
  const flown: DescentSegment[] = [];
  const pieces: VerticalPiece[] = [];
  let previousEnd: number | null = null;
  for (const segment of segments) {
    const first = stretches.findIndex((s) => s.segment === segment.name);
    if (first < 0) {
      // Over the site: the vertical descent starts from the stretches' last boundary, and is
      // stretched in time to keep its rate when that rose; the hover is the table's.
      let out = segment;
      if (previousEnd !== null && previousEnd !== segment.startAltitudeM) {
        const scaleS =
          (previousEnd - segment.endAltitudeM) / (segment.startAltitudeM - segment.endAltitudeM);
        out = { ...segment, startAltitudeM: previousEnd, durationS: segment.durationS * scaleS };
      }
      previousEnd = null;
      flown.push(out);
      pieces.push(out);
      continue;
    }
    let last = first;
    while (stretches[last + 1]?.segment === segment.name) {
      last += 1;
    }
    const startM = boundary[first] ?? segment.startAltitudeM;
    const endM = boundary[last + 1] ?? segment.endAltitudeM;
    const out = { ...segment, startAltitudeM: startM, endAltitudeM: endM };
    flown.push(out);
    let split = false;
    for (let b = first + 1; b <= last; b += 1) {
      split ||= (boundary[b] ?? 0) !== (table[b] ?? 0);
    }
    if (split) {
      for (let k = first; k <= last; k += 1) {
        const stretch = stretches[k];
        pieces.push({
          durationS: (stretch?.endS ?? 0) - (stretch?.startS ?? 0),
          startAltitudeM: boundary[k] ?? 0,
          endAltitudeM: boundary[k + 1] ?? 0,
          verticalShape: "constant",
        });
      }
    } else {
      pieces.push(out);
    }
    previousEnd = endM;
  }
  return { segments: flown, pieces };
}

/**
 * Each stretch's floor less the site's height, metres: `stretchMaxHeightsM`'s, or for older callers
 * `trackMaxHeightM` (0 by default) under the low pass alone and the site's height elsewhere.
 */
function floorsOf(
  terrain: DescentTerrain,
  stretches: ReadonlyArray<TrackStretch>,
  segments: ReadonlyArray<DescentSegment>,
): number[] {
  const siteM = terrain.siteHeightM ?? 0;
  const given = terrain.stretchMaxHeightsM;
  if (given !== undefined) {
    if (given.length !== stretches.length || !given.every(Number.isFinite)) {
      throw new RangeError(
        `a descent needs ${stretches.length} finite stretch floors, got ${given.length}`,
      );
    }
    return given.map((floorM) => floorM - siteM);
  }
  // Today's default: the track's maximum at the datum, 0 m.
  const track = terrain.trackMaxHeightM ?? 0;
  return stretches.map((stretch) =>
    segments.find((s) => s.name === stretch.segment)?.clearsTrack === true ? track - siteM : 0,
  );
}

/** A landing site and the azimuth the descent approaches it along. */
export interface LandingSite {
  /**
   * The site's parametric (reduced) latitude β and its longitude, rad: the site is the spheroid's
   * point M·d over the unit direction d of latitude β, so its geodetic latitude is a little
   * larger, 60.083° at β = 60°.
   */
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
  /**
   * The floor under the camera, metres above the spheroid: the F_k of the stretch under way, or
   * the site's height over the site (the vertical descent and the hover) and where no floor was
   * given (decision-r05-descent-clearance.md).
   */
  readonly floorM: number;
  /**
   * The camera's height above that floor, metres: the honest h the demand prediction reads, equal
   * to `clearanceM` on flat ground.
   */
  readonly heightAboveFloorM: number;
  /** Horizontal speed along the track and vertical speed, m/s. */
  readonly horizontalSpeedMps: number;
  readonly verticalSpeedMps: number;
  /**
   * The point at the site's terrain height beneath the camera, metres: where a grounded contact is
   * placed (the site itself at touchdown).
   */
  readonly groundPointM: Vec3;
  /**
   * The unit direction d of the track's point beneath the camera, the one the camera and
   * `groundPointM` stand over along the spheroid's normal (M·d + h·ν, R05 Design note 5). This is
   * the direction a height query (`surfaceHeightM`) or a patch key (`xyzToFaceUv`) takes, not the
   * geocentric direction of either point.
   */
  readonly groundDir: Vec3;
}

/**
 * The unit direction d of a body-fixed point p on the spheroid, p = M·d: d = M⁻¹p ÷ |M⁻¹p|, with
 * M = diag(a, a, c) (R05 Design note 5). This is the direction the bake, the collision interpolant
 * and the patch keys take; p ÷ |p|, the geocentric direction, is off by about f sin 2β ÷ 2 (f ≈ 1 ÷ 298
 * the flattening, β the latitude), up to 0.1° at 45° and 0.036° (4 km) at seed 7's site, which
 * lands a height query on other ground.
 *
 * @remarks
 * Exact for a point on the spheroid. A point h above it along the normal maps to a direction off
 * by about |h| f sin 2β ÷ a (up to 6 m on the ground at h = 1.8 km): take the direction the point was
 * built from (`DescentPose.groundDir`) where there is one.
 */
export function datumDirection(figure: BodyFigure, p: Vec3): Vec3 {
  return normalise(
    vec3(p.x / figure.equatorialRadiusM, p.y / figure.equatorialRadiusM, p.z / figure.polarRadiusM),
  );
}

/**
 * SplitMix64's step, as Vigna's `splitmix64.c` and JDK 8's `SplittableRandom` have it: the golden
 * gamma increment 0x9e3779b97f4a7c15 (Steele, Lea and Flood 2014, OOPSLA, Fig. 16) and Stafford's
 * Mix13 output mixer (the paper's `mix64variant13`): the next state and a 64-bit output.
 */
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

/**
 * The parametric latitudes a site is drawn between, rad: ±60°, a geodetic ±60.083°, so that the
 * scene's equinoctial Sun stands at least 29.9° above the site's horizon (lane C's scene).
 */
const SITE_LATITUDE_LIMIT_RAD = (60 * Math.PI) / 180;

/**
 * The landing site and approach azimuth of a seed, uniform in direction between ±60° parametric
 * latitude (within 0.7% of uniform over the spheroid's surface)
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

  /** The stretch plan, the same for every terrain (`trackStretches`). */
  readonly stretches: ReadonlyArray<TrackStretch>;
  /** Each stretch's floor F_k less the site's height, metres. */
  readonly #floorsM: ReadonlyArray<number>;
  /**
   * The least, over the 64 Hz poses of every stretch, of the height above its floor less its
   * clearance C, metres: never below −{@link FLOOR_TOLERANCE_M} (decision-r05-descent-clearance.md,
   * rule 4).
   */
  readonly minFloorMarginM: number;

  /**
   * @param terrain - The site's height and the stretches' floors; the script's altitudes are
   *   above the site, lifted where a stretch's floor less its clearance would be above them.
   * @throws RangeError if `terrain.stretchMaxHeightsM` does not match the stretch plan, or a
   *   floor cannot be cleared in four lifts.
   */
  constructor(
    figure: BodyFigure,
    site: LandingSite,
    terrain: DescentTerrain = {},
    baseSegments: ReadonlyArray<DescentSegment> = DESCENT_SEGMENTS,
  ) {
    this.figure = figure;
    this.site = site;
    const siteHeightM = terrain.siteHeightM ?? 0;
    this.#siteHeightM = siteHeightM;
    const datum = new LinearProfile(
      baseSegments[0]?.startAltitudeM ?? 0,
      verticalKnots(baseSegments),
    );
    this.#horizontal = new LinearProfile(0, horizontalKnots(baseSegments));
    this.#trackM = this.#horizontal.value(segmentStarts(baseSegments).at(-1) ?? 0);
    const stretches = planStretches(baseSegments, datum, this.#horizontal, this.#trackM);
    this.stretches = stretches;
    this.#floorsM = floorsOf(terrain, stretches, baseSegments);
    const flown = flyOverFloors(baseSegments, stretches, this.#floorsM, datum);
    this.segments = flown.segments;
    this.#vertical = flown.vertical;
    this.minFloorMarginM = flown.minFloorMarginM;
    const starts = segmentStarts(this.segments);
    this.#starts = starts;
    this.durationS = starts.at(-1) ?? 0;
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

  /** The landing site's unit direction d (the spheroid point M·d, Design note 5). */
  get siteDir(): Vec3 {
    return this.#siteDir;
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

  /** The floor less the site's height at `tS`, metres: the stretch's, 0 over the site. */
  #floorAboveSiteM(tS: number): number {
    const stretches = this.stretches;
    for (let k = 0; k < stretches.length; k += 1) {
      const stretch = stretches[k];
      if (stretch !== undefined && tS >= stretch.startS && tS < stretch.endS) {
        return this.#floorsM[k] ?? 0;
      }
    }
    return 0;
  }

  /**
   * The ground track's unit direction d at `tS` (`DescentPose.groundDir`, the cheap part of
   * {@link poseAt}): the same for every terrain.
   */
  groundDirAt(tS: number): Vec3 {
    const t = Math.min(Math.max(tS, 0), this.durationS);
    return this.#trackAt(this.#trackM - this.#horizontal.value(t)).dir;
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
    const floorAboveSiteM = this.#floorAboveSiteM(t);
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
      floorM: this.#siteHeightM + floorAboveSiteM,
      heightAboveFloorM: clearanceM - floorAboveSiteM,
      horizontalSpeedMps,
      verticalSpeedMps,
      groundPointM: vec3(g[0], g[1], g[2]),
      groundDir: dir,
    };
  }
}

/**
 * The stretch plan of a profile (decision-r05-descent-clearance.md, rule 1): the pieces of the
 * ground track from the orbit to the site, with their bound levels and clearances, which tile the
 * along-track distance to 0 and depend on the seed only. The caller bounds the finest mesh over
 * each (lane C's `SurfaceQuery.maxHeightsM`) and passes the floors back as
 * `DescentTerrain.stretchMaxHeightsM`.
 */
export function trackStretches(profile: DescentProfile): ReadonlyArray<TrackStretch> {
  return profile.stretches;
}
