/**
 * A body's rotation angle, body-fixed axes and spin rate at a time, from the rotation law plan 14
 * sends (P14.T46.f; rendering plan R07, T2.b; R08.T3.d): the client's twin of the simulation's
 * `RotationLaw::angle_at`, `planetary::frames::body_fixed_at` and `RotationLaw::rate_at`.
 *
 * @remarks
 * With s the seconds from the epoch to t, d = τ − `s_e` and Δ = max(s, 0), W is, reduced into
 * `[0, 2π)` (`RotationLawParts` in `planetary/derive/rotation.rs`):
 *
 * - **locked**, at or after the lock's instant (a lock `in_clock`): `W_p` + p M(t), p 1 for a
 *   synchronous body and 3 for a 3:2 one, M the clock's mean anomaly in `[0, 2π)`, its fraction of
 *   a period reduced from the time's whole seconds as the simulation's
 *   `KeplerElements::mean_anomaly_at` reduces it (`fractionOfPeriod`, to the bit);
 * - **with no lock in the clock's range** (`never`, `outside_clock`): W₀ + the angle swept from 0
 *   to s;
 * - **before the lock**, d > 0: W₀ + the angle swept from 0 to s + δ (Δ ÷ d)²;
 * - **before the lock**, d ≤ 0: the locked angle at the lock's instant less the angle swept from s
 *   to d.
 *
 * The angle swept from a to b is ω(`s_e` + a) (b − a) + (`ω_L` − ω₀) (b − a)² ÷ 2τ, with
 * ω(x) = ω₀ + (`ω_L` − ω₀) clamp(x ÷ τ, 0, 1), or ω₀ (b − a) for a body that never locks. The
 * operations are the simulation's in its order; `Math.sin` and `Math.cos` are not `libm`'s, so the
 * axes agree to rounding, and W to 10⁻⁹ rad, which `rotation.test.ts` holds against the
 * simulation's `frame/body_rotations.golden`.
 */
import type { UniverseTime } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import { fractionOfPeriod } from "../orbit";
import type { SystemBodyRotation } from "./model";

const TAU = 2 * Math.PI;

/** `x` reduced into `[0, 2π)`, zero where it is not finite: the simulation's `reduce`. */
function reduced(x: number): number {
  if (!Number.isFinite(x)) {
    return 0;
  }
  // Rust's `rem_euclid`: the truncated remainder, lifted by 2π where it is below zero, which can
  // round to 2π itself.
  const r = x % TAU;
  const lifted = r < 0 ? r + TAU : r;
  return lifted < TAU ? lifted : 0;
}

/** The seconds from the epoch to `time`: the simulation's `since_epoch().as_seconds_f64()`. */
function secondsFromEpoch(time: UniverseTime): number {
  return time.seconds + time.nanos * 1e-9;
}

/** Whether `a` is at or after `b`. */
function atOrAfter(a: UniverseTime, b: UniverseTime): boolean {
  return a.seconds === b.seconds ? a.nanos >= b.nanos : a.seconds > b.seconds;
}

/** The locking age τ, s from the system's birth, or `null` for a body that never locks. */
function lockingAgeOf(law: SystemBodyRotation): number | null {
  return law.lock.kind === "never" ? null : law.lock.lockingAgeS;
}

/** The spin rate at system age `ageS` on the law, before any lock, rad/s. */
function rateAtAge(law: SystemBodyRotation, ageS: number): number {
  const tau = lockingAgeOf(law);
  if (tau === null) {
    return law.initialRateRadPerS;
  }
  const share = Math.min(Math.max(ageS / tau, 0), 1);
  return law.initialRateRadPerS + (law.lockedRateRadPerS - law.initialRateRadPerS) * share;
}

/** The angle the law's rate sweeps from `fromS` to `toS` seconds after the epoch, before the lock. */
function swept(law: SystemBodyRotation, fromS: number, toS: number): number {
  const span = toS - fromS;
  const start = rateAtAge(law, law.ageAtEpochS + fromS);
  const tau = lockingAgeOf(law);
  return tau === null
    ? start * span
    : start * span + ((law.lockedRateRadPerS - law.initialRateRadPerS) * span * span) / (2 * tau);
}

/** The clock's mean anomaly at `time` in `[0, 2π)`, as `KeplerElements::mean_anomaly_at`. */
function clockMeanAnomaly(law: SystemBodyRotation, time: UniverseTime): number {
  const m = law.clockMeanAnomalyAtEpochRad + TAU * fractionOfPeriod(time, law.clockPeriodS);
  if (m < 0) {
    const lifted = m + TAU;
    return lifted < TAU ? lifted : 0;
  }
  return m >= TAU ? m - TAU : m;
}

/** The locked angle at `time`: `W_p` + p M(t). */
function lockedAngle(law: SystemBodyRotation, time: UniverseTime): number {
  let p: number;
  switch (law.resonance) {
    case "synchronous":
      p = 1;
      break;
    case "three_to_two":
      p = 3;
      break;
  }
  return law.subPrimaryAngleRad + p * clockMeanAnomaly(law, time);
}

/**
 * The body's rotation angle W at `time`, rad, in `[0, 2π)`: the prime meridian's angle from the
 * equator's node about the pole.
 *
 * @throws RangeError as `fractionOfPeriod`, for a time whose seconds are not a safe integer where
 *   the body is locked, or a time before a lock by the epoch whose seconds are not, which no law the
 *   simulation gives has.
 */
export function rotationAngleAt(law: SystemBodyRotation, time: UniverseTime): number {
  const s = secondsFromEpoch(time);
  const { lock } = law;
  let angle: number;
  if (lock.kind !== "in_clock") {
    angle = law.phaseAtEpochRad + swept(law, 0, s);
  } else if (atOrAfter(time, lock.locksAt)) {
    angle = lockedAngle(law, time);
  } else {
    const d = lock.lockingAgeS - law.ageAtEpochS;
    if (d > 0) {
      const share = Math.max(s, 0) / d;
      angle = law.phaseAtEpochRad + swept(law, 0, s) + law.capturePhaseRad * share * share;
    } else {
      angle = lockedAngle(law, lock.locksAt) - swept(law, s, d);
    }
  }
  return reduced(angle);
}

/**
 * The body's spin rate at `time`, rad/s: the simulation's `RotationLaw::rate_at`, which the figure
 * was flattened at and the atmosphere's normal gravity turns at (R08.T3.d).
 *
 * @remarks
 * At or after a lock within the clock it is the locked rate `ω_L`; otherwise the law's rate at the
 * system's age `s_e` + s, ω₀ + (`ω_L` − ω₀) clamp((`s_e` + s) ÷ τ, 0, 1), or ω₀ for a body that
 * never locks. As the simulation's, it leaves out the capture's phase δ (Δ ÷ d)², whose rate,
 * 2δΔ ÷ d² with δ in `[−π, π)` and Δ < d, is under 2π ÷ d.
 */
export function spinRateAt(law: SystemBodyRotation, time: UniverseTime): number {
  const { lock } = law;
  if (lock.kind === "in_clock" && atOrAfter(time, lock.locksAt)) {
    return law.lockedRateRadPerS;
  }
  return rateAtAge(law, law.ageAtEpochS + secondsFromEpoch(time));
}

/**
 * A body's fixed axes at a moment, each a unit vector along the galactic axes: the prime meridian,
 * the axis a quarter-turn east of it, and the pole.
 */
export interface BodyFixedAxes {
  readonly meridian: Vec3;
  readonly east: Vec3;
  readonly pole: Vec3;
}

/**
 * The body's fixed axes at `time`: the equator's node and quarter turned about the pole by
 * {@link rotationAngleAt}, as the simulation's `body_fixed_at` turns them.
 *
 * @throws RangeError as {@link rotationAngleAt}.
 */
export function bodyFixedAxesAt(law: SystemBodyRotation, time: UniverseTime): BodyFixedAxes {
  const w = rotationAngleAt(law, time);
  const sinW = Math.sin(w);
  const cosW = Math.cos(w);
  const { equatorNode: x, equatorQuarter: y } = law;
  return {
    meridian: {
      x: cosW * x.x + sinW * y.x,
      y: cosW * x.y + sinW * y.y,
      z: cosW * x.z + sinW * y.z,
    },
    east: {
      x: -sinW * x.x + cosW * y.x,
      y: -sinW * x.y + cosW * y.y,
      z: -sinW * x.z + cosW * y.z,
    },
    pole: law.pole,
  };
}
