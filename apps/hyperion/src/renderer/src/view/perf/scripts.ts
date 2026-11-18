/**
 * The perf scenes' camera scripts that R05's descent is not (plan R12, R12.T5.a; Design notes 8 and
 * 13): pure functions of script time in the shape of `descentProfile.ts`, a script's length, its
 * named segments and its pose at a time.
 *
 * @remarks
 * Each is evaluated through `lib/pinnedMath.ts` and `Math.sqrt`, which IEEE 754 rounds correctly,
 * so that a pose is the same to the bit at one time on every evaluation, as R05's fixed-step mode
 * needs (`fixedStep.ts`). A camera here is a display control, not a flight: its accelerations are
 * not a craft's. Its orientation is R02's: the right, up and backward axes in the script's frame,
 * looking down −z.
 *
 * - {@link ringedGiantApproach}: a ringed gas giant from 10¹⁰ m to inside its rings' annulus in
 *   300 s, across the 10⁹ m boundary and R11's slab-to-particle criterion, looking at the giant.
 * - {@link arrivalHold}: `arrival-memory`'s two poses, the descent's first (the orbit, script time
 *   0) while the field arrives and the descent's deepest (a metre above the site) once it has, held
 *   for {@link ARRIVAL_HELD_S}.
 * - {@link nuclearDiscSweep}: 120 s about a system of the nuclear disc, 300 au along the galactic
 *   x axis while the camera turns once about galactic z, so that the star bake is redone as it
 *   moves, about ten times ({@link SWEEP_LENGTH_M}).
 */

import { add, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { atan2, cos, exp, log, sin } from "../../lib/pinnedMath";
import type { Quaternion } from "../camera/pose";
import { quaternionFromRows } from "../camera/quaternion";
import type { DescentPose, DescentProfile } from "../spike/descentProfile";

/** The astronomical unit, m (IAU 2012 Resolution B2). */
export const AU_M = 149_597_870_700;

/**
 * The frame a script's poses are in: a body's own (R05's body-fixed, z its spin axis), or a system's
 * with the galactic axes about its barycentre.
 */
export type PerfScriptFrame = "body-fixed" | "system";

/** The camera at one moment of a perf script. */
export interface PerfPose {
  readonly tS: number;
  /** The segment under way, or the last past the script's end. */
  readonly segment: string;
  /** The camera's position in the script's frame, m. */
  readonly positionM: Vec3;
  /** Its orientation in the script's frame. */
  readonly orientation: Quaternion;
}

/** One named stretch of a script, s from its start. */
export interface PerfSegmentSpan {
  readonly name: string;
  readonly startS: number;
  readonly endS: number;
}

/** A perf scene's camera script: a pure function of script time. */
export interface PerfScript {
  readonly frame: PerfScriptFrame;
  /** The script's length, s. */
  readonly durationS: number;
  readonly segments: ReadonlyArray<PerfSegmentSpan>;
  /** The pose at `tS`, held at the script's ends outside it. */
  poseAt(tS: number): PerfPose;
}

/** Clamps `tS` to `[0, durationS]`. */
function clampS(tS: number, durationS: number): number {
  return Math.min(Math.max(tS, 0), durationS);
}

/** The segment of `segments` at `tS`, the last past the end. */
function segmentAt(segments: ReadonlyArray<PerfSegmentSpan>, tS: number): string {
  const found = segments.find(({ endS }) => tS < endS) ?? segments.at(-1);
  if (found === undefined) {
    throw new RangeError("a script has at least one segment");
  }
  return found.name;
}

/** `v` over its length, by `Math.sqrt`; the length must not be 0. */
function unit(v: Vec3): Vec3 {
  const length = Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
  if (!(length > 0)) {
    throw new RangeError("a look direction must have a length");
  }
  return vec3(v.x / length, v.y / length, v.z / length);
}

/**
 * The orientation looking along `forward` with `up` as near the view's up as it can be: R02's
 * rows, as `descentProfile.ts` builds its own. `up` must not be parallel to `forward`.
 */
export function lookingAlong(forward: Vec3, up: Vec3): Quaternion {
  const back = unit(scale(forward, -1));
  const right = unit(
    vec3(
      up.y * back.z - up.z * back.y,
      up.z * back.x - up.x * back.z,
      up.x * back.y - up.y * back.x,
    ),
  );
  const upright = vec3(
    back.y * right.z - back.z * right.y,
    back.z * right.x - back.x * right.z,
    back.x * right.y - back.y * right.x,
  );
  return quaternionFromRows([
    vec3(right.x, upright.x, back.x),
    vec3(right.y, upright.y, back.y),
    vec3(right.z, upright.z, back.z),
  ]);
}

/** The smoothstep 3u² − 2u³ over `[0, 1]`: 0 and 1 at the ends, its slope 0 there. */
function smoothstep(u: number): number {
  return u * u * (3 - 2 * u);
}

/**
 * The `u` in `[0, 1]` at which {@link smoothstep} is `s`, by 64 bisections: exact to the double for
 * a monotone curve, and the same on every evaluation.
 */
function smoothstepInverse(s: number): number {
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 64; i += 1) {
    const mid = (lo + hi) / 2;
    if (smoothstep(mid) < s) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return (lo + hi) / 2;
}

/** `ringed-giant-approach`'s span, s (Design note 13). */
export const APPROACH_DURATION_S = 300;

/** The approach's first distance from the giant's centre, m (Design note 8). */
export const APPROACH_START_DISTANCE_M = 1e10;

/** The boundary the approach crosses, m: Design note 8's 10⁹ m. */
export const APPROACH_BOUNDARY_M = 1e9;

/**
 * The camera's elevation above the ring plane at the approach's start, rad: 20°, so that the
 * annulus reads as an ellipse from afar (a lean, 2026-11-17).
 */
export const APPROACH_START_ELEVATION_RAD = (20 * Math.PI) / 180;

/**
 * The camera's height above the ring plane at the approach's end, m: inside the annulus, at its
 * middle radius, a hundred metres off the plane, where R11's rings draw particles rather than the
 * slab (a lean, 2026-11-17; the annulus's thickness is R11's).
 */
export const APPROACH_END_HEIGHT_M = 100;

/** A ringed giant as the approach reads it: its rings' annulus in its equatorial plane, m. */
export interface RingedGiant {
  /** The rings' inner and outer radius from the giant's centre, m. */
  readonly ringInnerM: number;
  readonly ringOuterM: number;
}

/**
 * The approach to a ringed giant (`ringed-giant-approach`): from {@link APPROACH_START_DISTANCE_M}
 * at {@link APPROACH_START_ELEVATION_RAD} above the ring plane to the annulus's middle radius
 * {@link APPROACH_END_HEIGHT_M} above it in {@link APPROACH_DURATION_S}, in the giant's body-fixed
 * frame (z its spin axis, the rings in its equator), along the prime meridian.
 *
 * @remarks
 * The distance from the centre falls geometrically, r = r₀ (r₁ ÷ r₀)^s, with s the smoothstep of
 * the script's fraction, so that each decade takes a like share of the span and the camera leaves
 * and arrives at rest; the elevation falls linearly in s. The camera looks at the giant's centre.
 * The segments are `far` above 10⁹ m, `near` down to the annulus's outer radius, and `annulus`
 * inside it.
 *
 * @throws RangeError if the annulus is not inside the start distance with its inner radius below
 *   its outer.
 */
export function ringedGiantApproach(giant: RingedGiant): PerfScript {
  const { ringInnerM, ringOuterM } = giant;
  if (!(ringInnerM > 0 && ringInnerM < ringOuterM && ringOuterM < APPROACH_BOUNDARY_M)) {
    throw new RangeError("a giant's rings lie inside 10⁹ m, their inner radius below their outer");
  }
  const middleM = (ringInnerM + ringOuterM) / 2;
  const endM = Math.sqrt(middleM * middleM + APPROACH_END_HEIGHT_M * APPROACH_END_HEIGHT_M);
  const endElevationRad = atan2(APPROACH_END_HEIGHT_M, middleM);
  const ratio = log(endM / APPROACH_START_DISTANCE_M);
  const timeAt = (distanceM: number): number =>
    APPROACH_DURATION_S * smoothstepInverse(log(distanceM / APPROACH_START_DISTANCE_M) / ratio);
  const boundaryS = timeAt(APPROACH_BOUNDARY_M);
  const annulusS = timeAt(ringOuterM);
  const segments: ReadonlyArray<PerfSegmentSpan> = [
    { name: "far", startS: 0, endS: boundaryS },
    { name: "near", startS: boundaryS, endS: annulusS },
    { name: "annulus", startS: annulusS, endS: APPROACH_DURATION_S },
  ];
  return {
    frame: "body-fixed",
    durationS: APPROACH_DURATION_S,
    segments,
    poseAt(tS: number): PerfPose {
      const t = clampS(tS, APPROACH_DURATION_S);
      const s = smoothstep(t / APPROACH_DURATION_S);
      const distanceM = APPROACH_START_DISTANCE_M * exp(ratio * s);
      const elevationRad =
        APPROACH_START_ELEVATION_RAD + (endElevationRad - APPROACH_START_ELEVATION_RAD) * s;
      const positionM = vec3(distanceM * cos(elevationRad), 0, distanceM * sin(elevationRad));
      return {
        tS: t,
        segment: segmentAt(segments, t),
        positionM,
        orientation: lookingAlong(scale(positionM, -1), vec3(0, 0, 1)),
      };
    },
  };
}

/** How long `arrival-memory` holds the deepest pose once its patches are resident, s (Design note 13). */
export const ARRIVAL_HELD_S = 60;

/** `arrival-memory`'s two poses (Design notes 8 and 13), in the world's body-fixed frame. */
export interface ArrivalHold {
  /** The descent's first pose, script time 0, from which the arrival and the survey are made. */
  readonly arrival: PerfPose;
  /** The descent's deepest pose, its last, a metre above the landing site. */
  readonly deepest: PerfPose;
  /** How long the deepest pose is held once its patches are resident, s. */
  readonly heldS: number;
}

/** A descent's pose as a perf pose. */
function descentPose(pose: DescentPose, segment: string): PerfPose {
  return { tS: pose.tS, segment, positionM: pose.positionM, orientation: pose.orientation };
}

/**
 * `arrival-memory`'s script over the pinned world's descent: its first pose while the arrival's
 * work and the survey's field land, then its deepest, held {@link ARRIVAL_HELD_S} once the field
 * and its patches are resident. The waits are the runner's; the poses are the profile's, so the
 * memory's peak is the descent's own deepest state.
 */
export function arrivalHold(profile: DescentProfile): ArrivalHold {
  return {
    arrival: descentPose(profile.poseAt(0), "arrival"),
    deepest: descentPose(profile.poseAt(profile.durationS), "deepest"),
    heldS: ARRIVAL_HELD_S,
  };
}

/** `nuclear-disc-sky`'s span, s (Design note 13). */
export const SWEEP_DURATION_S = 120;

/**
 * How far the sweep moves along galactic x, m: 300 au, so that the sky's bake is redone about ten
 * times in the 120 s (a lean, 2026-11-17).
 *
 * @remarks
 * R06's rule (`view/sky/model.ts`): a star is baked only beyond `bakedBeyondM`, where a 30 au
 * crossing (`PARALLAX_BASELINE_M`) shifts it by less than a tenth of a pixel
 * (`PARALLAX_THRESHOLD_PX`), about 8.7 ly for a 60° view 1920 px wide (`view/sky/select.ts`
 * splits the stars there), and the bake is redone once the nearest baked star has shifted by that
 * tenth (`view/sky/setting.ts`'s `rebakeShiftPx`). Each re-bake therefore needs about 30 au of
 * motion: a 10 au sweep shifts the nearest baked star by under 0.033 px and never re-bakes.
 */
export const SWEEP_LENGTH_M = 300 * AU_M;

/** The sweep's height above the barycentre along galactic z, m: 1 au, clear of the system's stars. */
export const SWEEP_HEIGHT_M = AU_M;

/**
 * `nuclear-disc-sky`'s sweep about the pinned system's barycentre, in the system's frame
 * (galactic axes): from x = −150 au to +150 au at {@link SWEEP_HEIGHT_M}, at a constant speed,
 * while the camera turns once about galactic z, looking along the galactic plane, so that every
 * direction of the nuclear disc's sky is drawn and the bake follows the motion. One segment,
 * `sweep`.
 */
export function nuclearDiscSweep(): PerfScript {
  const segments: ReadonlyArray<PerfSegmentSpan> = [
    { name: "sweep", startS: 0, endS: SWEEP_DURATION_S },
  ];
  const start = vec3(-SWEEP_LENGTH_M / 2, 0, SWEEP_HEIGHT_M);
  return {
    frame: "system",
    durationS: SWEEP_DURATION_S,
    segments,
    poseAt(tS: number): PerfPose {
      const t = clampS(tS, SWEEP_DURATION_S);
      const fraction = t / SWEEP_DURATION_S;
      const turnRad = 2 * Math.PI * fraction;
      return {
        tS: t,
        segment: segmentAt(segments, t),
        positionM: add(start, vec3(SWEEP_LENGTH_M * fraction, 0, 0)),
        orientation: lookingAlong(vec3(cos(turnRad), sin(turnRad), 0), vec3(0, 0, 1)),
      };
    },
  };
}
