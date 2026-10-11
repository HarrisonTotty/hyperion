/**
 * The `f64` oracle for the atmosphere's tables (plan R05, R05.T12.b): the optical depth of a
 * medium along a ray on a spherical shell, and the transmittance table's parameterisation, in
 * TypeScript as the WGSL has them.
 *
 * @remarks
 * The smoke harness reads the GPU's transmittance table back and compares it with
 * {@link opticalDepth} at fixed texels, in optical depth since the table stores it (plan R08,
 * R08.T6.c); the unit tests hold the integrator to closed forms. An absorber's share is a light's
 * curve of growth at the amount the ray crosses ({@link absorberAmounts},
 * {@link curvesOpticalDepth}). The geometry is Bruneton 2017's
 * (precomputed_atmospheric_scattering, `atmosphere/functions.glsl`), as
 * `shaders/common.wgsl` and `shaders/medium.wgsl` port it from Bevy: the transmittance table's
 * mapping is its `GetTransmittanceTextureUvFromRMu` and `GetRMuFromTransmittanceTextureUv` before
 * their texel mapping, of which only x_r = ρ ÷ H is Bruneton and Neyret 2008's (§4; plan R08's
 * Risks, "Deviations in T6.a, as built"). The texel mapping is Bruneton's exact pair,
 * {@link unitFromTexelUv} and {@link texelUvFromUnit}, on every table since R08.T6.g (plan R08,
 * Design note 19), where R05, as Bevy and sebh, took none for the transmittance table and sebh's
 * sub-texel remap, whose read did not invert its write, for the others.
 */

import type { Rgb } from "../photometry/toneCurve";
import { type AbsorberAmount, type AbsorberCurve, absorberTransmittance } from "./absorbers";
import { type AtmosphereMedium, densityAt, extinction, type TermAbsorber } from "./medium";

/** The spherical shell an atmosphere's tables are built on, radii from the centre, m. */
export interface Shell {
  readonly bottomRadiusM: number;
  readonly topRadiusM: number;
}

/**
 * A unit range's value at a texel coordinate on an axis of `size` texels: Bruneton 2017's
 * `GetUnitRangeFromTextureCoord` (precomputed_atmospheric_scattering, `functions.glsl`), as
 * `common.wgsl`'s `unitFromTexelUv`. Texel i's centre, (i + ½) ÷ size, holds i ÷ (size − 1).
 *
 * @param size - The axis's texels, at least 2.
 */
export function unitFromTexelUv(uv: number, size: number): number {
  return (uv * size - 0.5) / (size - 1);
}

/**
 * The texel coordinate at which an axis of `size` texels holds a unit range's value: Bruneton
 * 2017's `GetTextureCoordFromUnitRange`, as `common.wgsl`'s `texelUvFromUnit`, the exact inverse of
 * {@link unitFromTexelUv}, so that 0 and 1 stand at the first and last texels' centres.
 *
 * @param size - The axis's texels, at least 2.
 */
export function texelUvFromUnit(unit: number, size: number): number {
  return (unit * (size - 1) + 0.5) / size;
}

/** The radius at distance `tM` along a ray from radius `rM` at zenith cosine `mu`, m. */
export function localRadiusM(rM: number, mu: number, tM: number): number {
  return Math.sqrt(Math.max(tM * tM + 2 * rM * mu * tM + rM * rM, 0));
}

/** The distance from radius `rM` along zenith cosine `mu` to the top of the shell, m. */
export function distanceToTopM(shell: Shell, rM: number, mu: number): number {
  const discriminant = Math.max(rM * rM * (mu * mu - 1) + shell.topRadiusM ** 2, 0);
  return Math.max(-rM * mu + Math.sqrt(discriminant), 0);
}

/** The distance from radius `rM` along zenith cosine `mu` to the ground, m, if it is hit. */
export function distanceToBottomM(shell: Shell, rM: number, mu: number): number {
  const discriminant = Math.max(rM * rM * (mu * mu - 1) + shell.bottomRadiusM ** 2, 0);
  return Math.max(-rM * mu - Math.sqrt(discriminant), 0);
}

/** Whether a ray from radius `rM` along zenith cosine `mu` meets the ground. */
export function intersectsGround(shell: Shell, rM: number, mu: number): boolean {
  return mu < 0 && rM * rM * (mu * mu - 1) + shell.bottomRadiusM ** 2 >= 0;
}

/** The ray's length inside the atmosphere, to the ground or to the top, m. */
export function maxDistanceM(shell: Shell, rM: number, mu: number): number {
  return intersectsGround(shell, rM, mu)
    ? distanceToBottomM(shell, rM, mu)
    : distanceToTopM(shell, rM, mu);
}

/** A point of the transmittance table: a radius, m, and a zenith cosine. */
export interface RadiusAndCosine {
  readonly rM: number;
  readonly mu: number;
}

/**
 * The (r, μ) a transmittance table's unit range (u, v) stands for: Bruneton 2017's
 * `GetRMuFromTransmittanceTextureUv` before its texel mapping ({@link unitFromTexelUv}), as
 * `shaders/medium.wgsl`'s `transmittanceTexelOf` takes it from heights.
 *
 * @param u - The distance to the top, from its least (straight up) to its greatest, in [0, 1].
 * @param v - The distance to the horizon over its greatest, in [0, 1].
 */
export function transmittanceUvToRMu(shell: Shell, u: number, v: number): RadiusAndCosine {
  const { bottomRadiusM: bottom, topRadiusM: top } = shell;
  const bigH = Math.sqrt(top * top - bottom * bottom);
  const rho = bigH * v;
  const rM = Math.sqrt(rho * rho + bottom * bottom);
  const dMin = top - rM;
  const dMax = rho + bigH;
  const d = dMin + u * (dMax - dMin);
  const mu = d === 0 ? 1 : (bigH * bigH - rho * rho - d * d) / (2 * rM * d);
  return { rM, mu: Math.min(Math.max(mu, -1), 1) };
}

/**
 * The inverse of {@link transmittanceUvToRMu}, the unit range before the texel mapping
 * ({@link texelUvFromUnit}): `common.wgsl`'s `transmittanceUnitAt`, which takes it from heights.
 */
export function transmittanceRMuToUv(
  shell: Shell,
  rM: number,
  mu: number,
): readonly [number, number] {
  const { bottomRadiusM: bottom, topRadiusM: top } = shell;
  const bigH = Math.sqrt(top * top - bottom * bottom);
  const rho = Math.sqrt(Math.max(rM * rM - bottom * bottom, 0));
  const d = distanceToTopM(shell, rM, mu);
  const dMin = top - rM;
  const dMax = rho + bigH;
  return [(d - dMin) / (dMax - dMin), rho / bigH];
}

/** The steps {@link opticalDepth} takes by default: Simpson's rule converges far below 10⁻⁶. */
export const ORACLE_STEPS = 20_000;

/**
 * The optical depth per channel from radius `rM` along zenith cosine `mu` to the top of the
 * atmosphere or the ground, by composite Simpson's rule in `f64`, of every term but the absorbers.
 *
 * @remarks
 * An absorber term's coefficients are 0 (`medium.ts`'s `extinction`): its optical depth is a
 * light's curve of growth at the amount the ray crosses, {@link absorberAmounts}, which
 * {@link curvesOpticalDepth} reads (plan R08, Design notes 5 and 8, R08.T6.c).
 *
 * @param bottomRadiusM - The ground's radius, m; the top is `medium.topHeightM` above it.
 * @param steps - Simpson intervals, even.
 */
export function opticalDepth(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  steps = ORACLE_STEPS,
): Rgb {
  const shell: Shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  return alongRay(medium, bottomRadiusM, rM, mu, maxDistanceM(shell, rM, mu), steps).depth;
}

/**
 * The optical depth per channel from radius `rM` along zenith cosine `mu` to the top of the
 * atmosphere, as the transmittance table holds it: {@link opticalDepth} for a ray that does not meet
 * the ground, and for the ray tangent to it (the table's last column since R08.T6.g's texel
 * mapping) the whole grazing path, where {@link opticalDepth}'s ground test would decide by
 * rounding whether it stops at the tangent point. Every term but the absorbers, as
 * {@link opticalDepth}.
 *
 * @param bottomRadiusM - The ground's radius, m; the top is `medium.topHeightM` above it.
 * @param steps - Simpson intervals, even.
 */
export function opticalDepthToTop(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  steps = ORACLE_STEPS,
): Rgb {
  const shell: Shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  return alongRay(medium, bottomRadiusM, rM, mu, distanceToTopM(shell, rM, mu), steps).depth;
}

/**
 * Each absorber term's amount u along a ray from radius `rM` at zenith cosine `mu` to the top or
 * the ground, in its kind's unit: n ∫ ρ ds by {@link opticalDepth}'s Simpson rule, one an absorber
 * term in the medium's order of them (`tables.ts`'s `absorberSlots`), n its density at relative
 * density 1. That is a column, molecules m⁻², for a column absorber, and a pair column, molecule
 * pairs m⁻⁵, for a pair absorber (R08.T4.c), whose term's ρ is already the square of its column's
 * (R08.T6.c).
 *
 * @param bottomRadiusM - The ground's radius, m; the top is `medium.topHeightM` above it.
 * @param steps - Simpson intervals, even.
 */
export function absorberAmounts(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  steps = ORACLE_STEPS,
): ReadonlyArray<AbsorberAmount> {
  const shell: Shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  return alongRay(medium, bottomRadiusM, rM, mu, maxDistanceM(shell, rM, mu), steps).amounts;
}

/**
 * {@link absorberAmounts} to the top of the atmosphere, as the tables hold them, along the grazing
 * ray too ({@link opticalDepthToTop}).
 *
 * @param bottomRadiusM - The ground's radius, m; the top is `medium.topHeightM` above it.
 * @param steps - Simpson intervals, even.
 */
export function absorberAmountsToTop(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  steps = ORACLE_STEPS,
): ReadonlyArray<AbsorberAmount> {
  const shell: Shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  return alongRay(medium, bottomRadiusM, rM, mu, distanceToTopM(shell, rM, mu), steps).amounts;
}

/**
 * The optical depth per channel that a light's curves of growth give a path's absorber amounts:
 * Σₖ −ln T_c(u_k), each read by `absorbers.ts`'s `absorberTransmittance`, clamp included (plan R08,
 * Design note 5, R08.T6.c).
 *
 * @param curves - The light's curves, one an absorber term in the medium's order of them.
 * @param amounts - Each absorber's amount u, of its curve's kind, as {@link absorberAmounts} gives.
 * @throws RangeError when the counts differ, or as `absorberTransmittance` for an amount that is
 *   not finite and at least 0.
 * @throws Error as `absorberTransmittance` for an amount of the other kind than its curve's.
 */
export function curvesOpticalDepth(
  curves: ReadonlyArray<AbsorberCurve>,
  amounts: ReadonlyArray<AbsorberAmount>,
): Rgb {
  if (curves.length !== amounts.length) {
    throw new RangeError(`${curves.length} curves for ${amounts.length} absorber amounts`);
  }
  const depth: [number, number, number] = [0, 0, 0];
  for (const [k, curve] of curves.entries()) {
    const amount = amounts[k];
    if (amount === undefined) {
      throw new RangeError(`no amount for absorber ${k}`);
    }
    const t = absorberTransmittance(curve, amount);
    depth[0] -= Math.log(t[0]);
    depth[1] -= Math.log(t[1]);
    depth[2] -= Math.log(t[2]);
  }
  return depth;
}

/**
 * A term absorber's amount along a path over which its relative density integrates to
 * `densityPathM`, m: its density at relative density 1 times that, in its kind's unit.
 */
function amountAlong(absorber: TermAbsorber, densityPathM: number): AbsorberAmount {
  let amount: AbsorberAmount;
  switch (absorber.kind) {
    case "column":
      amount = { kind: "column", columnPerM2: absorber.numberDensityPerM3 * densityPathM };
      break;
    case "pair":
      amount = { kind: "pair", pairColumnPerM5: absorber.pairDensityPerM6 * densityPathM };
      break;
  }
  return amount;
}

/**
 * Composite Simpson's rule over [0, tMax] along the ray, heights below the ground as the ground's:
 * the optical depth of every term but the absorbers, and each absorber term's amount.
 */
function alongRay(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  tMax: number,
  steps: number,
): { readonly depth: Rgb; readonly amounts: ReadonlyArray<AbsorberAmount> } {
  const n = steps + (steps % 2);
  const dt = tMax / n;
  const coefficients = medium.terms.map((term) => ({ term, sigma: extinction(term) }));
  const absorbers = medium.terms.flatMap((term) =>
    term.absorber === undefined ? [] : [{ term, absorber: term.absorber }],
  );
  const sum = [0, 0, 0];
  const paths = new Float64Array(absorbers.length);
  for (let i = 0; i <= n; i += 1) {
    let weight = 2;
    if (i === 0 || i === n) {
      weight = 1;
    } else if (i % 2 === 1) {
      weight = 4;
    }
    const h = localRadiusM(rM, mu, i * dt) - bottomRadiusM;
    for (const { term, sigma } of coefficients) {
      const d = densityAt(term.density, h) * weight;
      sum[0] = (sum[0] ?? 0) + sigma[0] * d;
      sum[1] = (sum[1] ?? 0) + sigma[1] * d;
      sum[2] = (sum[2] ?? 0) + sigma[2] * d;
    }
    for (const [k, { term }] of absorbers.entries()) {
      paths[k] = (paths[k] ?? 0) + densityAt(term.density, h) * weight;
    }
  }
  const k = dt / 3;
  return {
    depth: [(sum[0] ?? 0) * k, (sum[1] ?? 0) * k, (sum[2] ?? 0) * k],
    amounts: absorbers.map(({ absorber }, i) => amountAlong(absorber, (paths[i] ?? 0) * k)),
  };
}
