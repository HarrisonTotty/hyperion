/**
 * Orbits on the `SYSTEM` display: Kepler's equation, a body's state at a time, the ellipse an orbit
 * is drawn as, and a body's position composed up its chain of parents (plan 14, design note D18 and
 * P14.T39).
 *
 * @remarks
 * The server sends orbits as elements and the display propagates them itself to animate. That is
 * drawing, not generation: every number in a readout comes from the server. The mathematics follows
 * the server's `hyperion_sim::orbit` step for step, so that the two agree to 10⁻⁹ relative, which
 * `orbit.test.ts` checks against the server's own fixture:
 *
 * - the time since the epoch is reduced modulo the period from its whole seconds, exactly, before
 *   its nanoseconds are added, and the fraction of a period is centred on zero (`orbit/phase.rs`),
 *   so that a one-day orbit keeps its phase a thousand years out, where a float of seconds times
 *   the mean motion is wrong in the tenth figure;
 * - Kepler's equation is solved from Mikkola's cubic starter by exactly
 *   {@link KEPLER_HALLEY_ITERATIONS} Halley iterations, in the forms that keep their precision near
 *   periapsis at high eccentricity (`orbit/kepler.rs`);
 * - the state is built in the orbit's plane and turned into the parent's frame by the unit vectors
 *   P, towards periapsis, and Q, a quarter-turn ahead of it (`orbit/orientation.rs`).
 *
 * It need not agree bit for bit (D18): `Math.sin`, `Math.cos` and `Math.cbrt` are not `libm`'s.
 * JavaScript's `%` is the exact truncated remainder, C's `fmod`, so the phase itself is the server's
 * to the bit.
 *
 * Orbits so nearly parabolic that e ≥ {@link NEAR_PARABOLIC_ECCENTRICITY} are not propagated here:
 * the server carries them as open orbits with a sampled track (D18; the orchestrator's ruling 39),
 * so one reaching these functions is a bug upstream, and they throw.
 */
import type { UniverseTime } from "@hyperion/protocol";

import { add, scale, type Vec3 } from "../spatial/vec3";

/**
 * The number of Halley iterations {@link solveKepler} runs from Mikkola's starter: always this
 * many, never fewer on a tolerance, as `hyperion_sim::orbit::KEPLER_HALLEY_ITERATIONS`.
 *
 * @remarks
 * Two reach the rounding floor, a residual of at most 8.9 × 10⁻¹⁶ rad up to e = 0.999, as the
 * server's measurement records.
 */
export const KEPLER_HALLEY_ITERATIONS = 2;

/**
 * The eccentricity from which an orbit is not propagated on the client: the server's
 * `OpenOrbit::MIN_ECCENTRICITY`, from which it carries an orbit as an open one with a sampled track,
 * since Kepler's equation is ill-conditioned there (the orchestrator's ruling 39).
 */
export const NEAR_PARABOLIC_ECCENTRICITY = 0.9999;

/**
 * A bound Keplerian orbit of a body relative to its parent, in the axes of the parent's frame: plan
 * 11's `OrbitDto` as the client holds it (the orchestrator's ruling 33).
 *
 * @remarks
 * Angles follow the server's conventions: the inclination from +z, the ascending node from +x
 * towards +y, and the argument of periapsis from the node in the direction of motion. The elements
 * hold at the epoch, `UT` 0. The wire's gravitational parameter is not needed here, since the period
 * gives the mean motion, as it does on the server.
 */
export interface KeplerOrbit {
  /** The semi-major axis a, m, finite and positive. */
  readonly semiMajorAxisM: number;
  /** The eccentricity e, in `[0, NEAR_PARABOLIC_ECCENTRICITY)`. */
  readonly eccentricity: number;
  /** The inclination i, rad, in `[0, π]`. */
  readonly inclinationRad: number;
  /** The longitude of the ascending node Ω, rad. */
  readonly ascendingNodeRad: number;
  /** The argument of periapsis ω, rad. */
  readonly argumentOfPeriapsisRad: number;
  /** The mean anomaly at the epoch M₀, rad. */
  readonly meanAnomalyAtEpochRad: number;
  /** The period P, s, finite and positive. */
  readonly periodS: number;
}

/** A body's position and velocity relative to its parent, in the axes of the parent's frame. */
export interface OrbitState {
  readonly positionM: Vec3;
  readonly velocityMPerS: Vec3;
}

/**
 * Where a body is placed, for {@link composePosition}: at the origin of the system's frame, on an
 * orbit about another body, or as a member of a pair about the pair's barycentre.
 *
 * @remarks
 * A host alone at its system's barycentre is at the origin. Hosts that orbit a barycentre together
 * are placed as plan 11's `HierarchyDto` says: a pair's orbit gives its outer member's barycentre
 * relative to its inner member's, r, and each member sits about the pair's barycentre at `share`
 * times r, the inner one at −(M₂ ÷ M) r and the outer one at +(M₁ ÷ M) r, where M₁ and M₂ are the
 * inner and outer members' masses and M their sum. The `parentId` of a member is its pair's own
 * placement, which is itself a member of an outer pair, or the origin for the system's root.
 */
export type BodyPlacement =
  | { readonly kind: "origin" }
  | { readonly kind: "orbit"; readonly parentId: string; readonly orbit: KeplerOrbit }
  | {
      readonly kind: "member";
      readonly parentId: string;
      readonly orbit: KeplerOrbit;
      /** The member's share of the pair's relative orbit: −M₂ ÷ M inside, +M₁ ÷ M outside. */
      readonly share: number;
    };

const TAU = 2 * Math.PI;
const NANOS_PER_SECOND = 1_000_000_000;

/** Terms of the Stumpff series S, as the server's `STUMPFF_TERMS`. */
const STUMPFF_TERMS = 11;

/** The unit vectors of an orbit's plane in its parent's frame. */
interface Orientation {
  /** Towards periapsis. */
  readonly p: Vec3;
  /** A quarter-turn ahead of periapsis, in the direction of motion. */
  readonly q: Vec3;
}

/**
 * The orbit's plane in its parent's frame, as the server's `Orientation::new` builds it.
 *
 * @throws RangeError for an orbit that cannot be propagated here: an element that is not finite, a
 *   semi-major axis or period that is not positive, an inclination outside `[0, π]`, or an
 *   eccentricity outside `[0, NEAR_PARABOLIC_ECCENTRICITY)`.
 */
function orientationOf(orbit: KeplerOrbit): Orientation {
  const {
    semiMajorAxisM,
    eccentricity,
    inclinationRad,
    ascendingNodeRad,
    argumentOfPeriapsisRad,
    meanAnomalyAtEpochRad,
    periodS,
  } = orbit;
  if (!(eccentricity >= 0 && eccentricity < NEAR_PARABOLIC_ECCENTRICITY)) {
    throw new RangeError(
      `an eccentricity of ${String(eccentricity)} is not propagated on the client`,
    );
  }
  if (!(Number.isFinite(semiMajorAxisM) && semiMajorAxisM > 0)) {
    throw new RangeError(`a semi-major axis of ${String(semiMajorAxisM)} m is not an orbit's`);
  }
  if (!(Number.isFinite(periodS) && periodS > 0)) {
    throw new RangeError(`a period of ${String(periodS)} s is not an orbit's`);
  }
  if (!(inclinationRad >= 0 && inclinationRad <= Math.PI)) {
    throw new RangeError(`an inclination of ${String(inclinationRad)} rad is outside [0, π]`);
  }
  for (const angle of [ascendingNodeRad, argumentOfPeriapsisRad, meanAnomalyAtEpochRad]) {
    if (!Number.isFinite(angle)) {
      throw new RangeError(`an angle of ${String(angle)} rad is not finite`);
    }
  }
  const [sinI, cosI] = [Math.sin(inclinationRad), Math.cos(inclinationRad)];
  const [sinNode, cosNode] = [Math.sin(ascendingNodeRad), Math.cos(ascendingNodeRad)];
  const [sinArg, cosArg] = [Math.sin(argumentOfPeriapsisRad), Math.cos(argumentOfPeriapsisRad)];
  return {
    p: {
      x: cosNode * cosArg - sinNode * sinArg * cosI,
      y: sinNode * cosArg + cosNode * sinArg * cosI,
      z: sinArg * sinI,
    },
    q: {
      x: -cosNode * sinArg - sinNode * cosArg * cosI,
      y: -sinNode * sinArg + cosNode * cosArg * cosI,
      z: cosArg * sinI,
    },
  };
}

/** The vector with components `alongP` along P and `alongQ` along Q, summed as the server does. */
function planeToFrame({ p, q }: Orientation, alongP: number, alongQ: number): Vec3 {
  return {
    x: alongP * p.x + alongQ * q.x,
    y: alongP * p.y + alongQ * q.y,
    z: alongP * p.z + alongQ * q.z,
  };
}

/**
 * A value in `[−P, P]` brought into `[−P/2, P/2)` by at most one addition or subtraction of the
 * period, each exact there by Sterbenz's lemma.
 */
function centred(value: number, periodS: number): number {
  const half = 0.5 * periodS;
  if (value >= half) {
    return value - periodS;
  }
  if (value < -half) {
    return value + periodS;
  }
  return value;
}

/**
 * The fraction of a period elapsed at `time` since the epoch, centred into `[−½, ½)`: the server's
 * `phase::fraction_of_period`, to the bit.
 *
 * @remarks
 * The time is taken about its nearer whole second, so that the nanoseconds are a signed remainder
 * of at most half a second. The whole seconds, exact as a JavaScript number, are reduced modulo the
 * period by `%`, which is exact and keeps the dividend's sign, and centred; so are the nanoseconds
 * as a fraction of a second. The sum is the first rounding. Centring keeps the phase's relative
 * precision just before a whole period as well as just after one.
 *
 * @throws RangeError for a time whose seconds are not a safe integer or whose nanoseconds are not a
 *   whole number below 10⁹.
 */
function fractionOfPeriod(time: UniverseTime, periodS: number): number {
  if (!Number.isSafeInteger(time.seconds)) {
    throw new RangeError(`${String(time.seconds)} s is not a universe time's whole seconds`);
  }
  if (!(Number.isInteger(time.nanos) && time.nanos >= 0 && time.nanos < NANOS_PER_SECOND)) {
    throw new RangeError(`${String(time.nanos)} ns is not a universe time's nanoseconds`);
  }
  let { seconds, nanos } = time;
  if (nanos >= NANOS_PER_SECOND / 2) {
    seconds += 1;
    nanos -= NANOS_PER_SECOND;
  }
  const whole = centred(seconds % periodS, periodS);
  const ofSecond = centred((nanos / 1e9) % periodS, periodS);
  const fraction = centred(whole + ofSecond, periodS) / periodS;
  // A remainder in [−P/2, P/2) divides into [−½, ½]; the guard keeps ½ itself out.
  return fraction < 0.5 ? fraction : fraction - 1;
}

/**
 * An angle reduced into `[−π, π]`: kept exactly if it is there already, else by the exact remainder
 * modulo 2π and at most one exact addition or subtraction of 2π.
 */
function reduceToHalfTurn(angleRad: number): number {
  if (Math.abs(angleRad) <= Math.PI) {
    return angleRad;
  }
  const turn = angleRad % TAU;
  if (turn > Math.PI) {
    return turn - TAU;
  }
  if (turn < -Math.PI) {
    return turn + TAU;
  }
  return turn;
}

/** S(z) by its series, for |z| ≤ 1: ⅙ (1 − z ÷ (4·5) (1 − z ÷ (6·7) (1 − …))). */
function stumpffSSeries(z: number): number {
  let s = 1;
  for (let k = STUMPFF_TERMS - 1; k >= 1; k -= 1) {
    s = 1 - (z * s) / ((2 * k + 2) * (2 * k + 3));
  }
  return s / 6;
}

/** x − sin x, given sin x, without the cancellation of the direct form for small x. */
function sinDeficit(x: number, sin: number): number {
  const x2 = x * x;
  return x2 <= 1 ? x2 * x * stumpffSSeries(x2) : x - sin;
}

/** 1 − cos x from sin x and cos x, without the cancellation of the direct form near x = 0. */
function oneMinusCos(sin: number, cos: number): number {
  return cos > 0 ? (sin * sin) / (1 + cos) : 1 - cos;
}

/** Mikkola's (1987) starting value of E for M in `[0, π]` and e in `[0, 1)`. */
function mikkolaStarter(m: number, e: number): number {
  const denominator = 4 * e + 0.5;
  const alpha = (1 - e) / denominator;
  const beta = (0.5 * m) / denominator;
  const z = Math.cbrt(beta + Math.sqrt(beta * beta + alpha * alpha * alpha));
  const z2 = z * z;
  // `z − α ÷ z`, Mikkola's form, cancels for small M; this equal form does not.
  let s = (2 * beta) / (z2 + alpha + (alpha * alpha) / z2);
  const s2 = s * s;
  s -= (0.078 * (s2 * s2 * s)) / (1 + e);
  return m + e * s * (3 - 4 * s * s);
}

/** One Halley iteration on f(E) = E − e sin E − M, in the forms that hold near periapsis. */
function halleyStep(anomaly: number, m: number, e: number): number {
  const sin = Math.sin(anomaly);
  const cos = Math.cos(anomaly);
  const f = (1 - e) * anomaly + e * sinDeficit(anomaly, sin) - m;
  const f1 = 1 - e + e * oneMinusCos(sin, cos);
  const f2 = e * sin;
  return anomaly - (2 * f * f1) / (2 * f1 * f1 - f * f2);
}

/** E for M in `[−π, π]`: solved for |M|, the sign of M restored, as the server does. */
function eccentricAnomaly(m: number, e: number): number {
  const magnitude = Math.abs(m);
  let anomaly = mikkolaStarter(magnitude, e);
  for (let iteration = 0; iteration < KEPLER_HALLEY_ITERATIONS; iteration += 1) {
    anomaly = halleyStep(anomaly, magnitude, e);
  }
  return m < 0 || Object.is(m, -0) ? -Math.abs(anomaly) : Math.abs(anomaly);
}

/**
 * The eccentric anomaly E of a mean anomaly M: the solution of Kepler's equation E − e sin E = M.
 *
 * @remarks
 * M may be any finite angle; it is reduced into `[−π, π]`, and E is returned in the same interval
 * with the sign of the reduced M, so that E(−M) = −E(M) exactly. The method is the server's, fixed
 * so that the two sides take the same steps: Mikkola's (1987, Celestial Mechanics 40, 329) cubic
 * starter, then exactly {@link KEPLER_HALLEY_ITERATIONS} Halley iterations with no exit on a
 * tolerance.
 *
 * @param meanAnomalyRad - M, rad.
 * @param eccentricity - e, in `[0, NEAR_PARABOLIC_ECCENTRICITY)`.
 * @returns E, rad.
 * @throws RangeError for an eccentricity outside `[0, NEAR_PARABOLIC_ECCENTRICITY)` or a mean
 *   anomaly that is not finite.
 */
export function solveKepler(meanAnomalyRad: number, eccentricity: number): number {
  if (!(eccentricity >= 0 && eccentricity < NEAR_PARABOLIC_ECCENTRICITY)) {
    throw new RangeError(
      `an eccentricity of ${String(eccentricity)} is not propagated on the client`,
    );
  }
  if (!Number.isFinite(meanAnomalyRad)) {
    throw new RangeError(`a mean anomaly of ${String(meanAnomalyRad)} rad is not finite`);
  }
  return eccentricAnomaly(reduceToHalfTurn(meanAnomalyRad), eccentricity);
}

/**
 * A body's position and velocity relative to its parent at `time`, in the axes of the parent's
 * frame: the server's `KeplerElements::relative_state_at`.
 *
 * @remarks
 * A pure function of the elements and the time, symmetric in time: the epoch is no boundary, and
 * any universe time before or after it is valid. The mean anomaly is M₀ plus 2π times the centred
 * fraction of a period elapsed since the epoch, reduced exactly from the time's whole seconds (see
 * the module's remarks).
 *
 * @throws RangeError for an orbit that is not propagated on the client (e ≥
 *   {@link NEAR_PARABOLIC_ECCENTRICITY}) or has an element out of range, or for a malformed time.
 */
export function stateAt(orbit: KeplerOrbit, time: UniverseTime): OrbitState {
  const orientation = orientationOf(orbit);
  const { semiMajorAxisM: a, eccentricity: e, periodS } = orbit;
  const meanAnomaly = orbit.meanAnomalyAtEpochRad + TAU * fractionOfPeriod(time, periodS);
  const anomaly = eccentricAnomaly(reduceToHalfTurn(meanAnomaly), e);
  const sin = Math.sin(anomaly);
  const cos = Math.cos(anomaly);
  const cosineDeficit = oneMinusCos(sin, cos);
  const oneMinusE = 1 - e;
  const axisRatio = Math.sqrt(oneMinusE * (1 + e));
  // cos E − e as (1 − e) − (1 − cos E), and 1 − e cos E as (1 − e) + e (1 − cos E), which keep
  // their precision near periapsis at high eccentricity.
  const alongPeriapsis = a * (oneMinusE - cosineDeficit);
  const across = a * axisRatio * sin;
  const radiusOverA = oneMinusE + e * cosineDeficit;
  const speed = ((TAU / periodS) * a) / radiusOverA;
  return {
    positionM: planeToFrame(orientation, alongPeriapsis, across),
    velocityMPerS: planeToFrame(orientation, -speed * sin, speed * axisRatio * cos),
  };
}

/**
 * A body's position relative to its parent at `time`, m, in the axes of the parent's frame: the
 * position of {@link stateAt}.
 *
 * @throws RangeError as {@link stateAt}.
 */
export function positionAt(orbit: KeplerOrbit, time: UniverseTime): Vec3 {
  return stateAt(orbit, time).positionM;
}

/**
 * The ellipse an orbit is drawn as: `segments` + 1 points about its parent, m, in the axes of the
 * parent's frame, spaced evenly in eccentric anomaly from periapsis, the first repeated last so that
 * the path closes exactly.
 *
 * @remarks
 * Even steps in eccentric anomaly crowd the points towards both ends of the major axis, which keeps
 * a highly eccentric orbit's pericentre smooth where even steps in time or in true anomaly would
 * not. The point at E is a (cos E − e) along P and a √(1 − e²) sin E along Q, in the forms
 * {@link stateAt} uses.
 *
 * @param segments - A whole number, at least 3.
 * @throws RangeError for fewer than three segments or a fractional count, or as {@link stateAt} for
 *   the orbit.
 */
export function orbitPolyline(orbit: KeplerOrbit, segments: number): ReadonlyArray<Vec3> {
  if (!(Number.isInteger(segments) && segments >= 3)) {
    throw new RangeError(`an orbit is drawn with at least 3 whole segments, not ${segments}`);
  }
  const orientation = orientationOf(orbit);
  const { semiMajorAxisM: a, eccentricity: e } = orbit;
  const oneMinusE = 1 - e;
  const axisRatio = Math.sqrt(oneMinusE * (1 + e));
  const points: Vec3[] = [];
  for (let index = 0; index < segments; index += 1) {
    const anomaly = (TAU * index) / segments;
    const sin = Math.sin(anomaly);
    const cos = Math.cos(anomaly);
    points.push(
      planeToFrame(orientation, a * (oneMinusE - oneMinusCos(sin, cos)), a * axisRatio * sin),
    );
  }
  const [first] = points;
  if (first === undefined) {
    throw new Error("an orbit's polyline has no first point");
  }
  points.push(first);
  return points;
}

/**
 * A body's position at `time` in the system's frame, m: the sum of the positions along its chain of
 * parents, from the origin down.
 *
 * @remarks
 * Each body's position is its parent's plus its own position about the parent, so a moon's is its
 * planet's plus its offset from the planet, exactly. A planet's elements are in the system frame
 * and a moon's in its parent's body frame (P14.T35.a), and both are translations with the galactic
 * axes (plan 01's `coords`), so composing them is a sum and needs no rotation. A pair's member is
 * at its share of the pair's relative position about the pair's barycentre, so the two members'
 * mass-weighted positions sum to the barycentre's.
 *
 * @param bodies - Every body's placement, keyed by its ID as the wire has it.
 * @throws Error naming the body when `id` or a parent on its chain is not in `bodies`, or when the
 *   chain does not reach the origin, which data from the server never does.
 * @throws RangeError as {@link stateAt} for an orbit on the chain.
 */
export function composePosition(
  bodies: ReadonlyMap<string, BodyPlacement>,
  id: string,
  time: UniverseTime,
): Vec3 {
  const chain: Array<{ readonly orbit: KeplerOrbit; readonly share: number | null }> = [];
  let at = id;
  for (;;) {
    const placement = bodies.get(at);
    if (placement === undefined) {
      throw new Error(`no body ${at} is placed, on the chain of ${id}`);
    }
    if (placement.kind === "origin") {
      break;
    }
    if (chain.length >= bodies.size) {
      throw new Error(`the chain of parents of ${id} does not reach the origin`);
    }
    chain.push({
      orbit: placement.orbit,
      share: placement.kind === "member" ? placement.share : null,
    });
    at = placement.parentId;
  }
  let positionM: Vec3 = { x: 0, y: 0, z: 0 };
  for (const { orbit, share } of chain.toReversed()) {
    const offsetM = positionAt(orbit, time);
    positionM = add(positionM, share === null ? offsetM : scale(offsetM, share));
  }
  return positionM;
}
