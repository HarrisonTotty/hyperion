/**
 * The `f64` oracle for the atmosphere's tables (plan R05, R05.T12.b): the optical depth of a
 * medium along a ray on a spherical shell, and the transmittance table's parameterisation, in
 * TypeScript as the WGSL has them.
 *
 * @remarks
 * The smoke harness reads the GPU's transmittance table back and compares it with
 * {@link opticalDepth} at fixed texels; the unit tests hold the integrator to closed forms. The
 * geometry is Bruneton and Neyret 2008, §4, as `shaders/common.wgsl` and `shaders/medium.wgsl`
 * port it from Bevy.
 */

import type { Rgb } from "../photometry/toneCurve";
import { type AtmosphereMedium, densityAt, extinction } from "./medium";

/** The spherical shell an atmosphere's tables are built on, radii from the centre, m. */
export interface Shell {
  readonly bottomRadiusM: number;
  readonly topRadiusM: number;
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
 * The (r, μ) a transmittance table's (u, v) stands for: Bruneton and Neyret 2008, §4, as
 * `transmittanceUvToRMu` in `shaders/medium.wgsl`.
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

/** The inverse of {@link transmittanceUvToRMu}: `transmittanceRMuToUv` in `medium.wgsl`. */
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
 * atmosphere or the ground, by composite Simpson's rule in `f64`.
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
  const tMax = maxDistanceM(shell, rM, mu);
  const n = steps + (steps % 2);
  const dt = tMax / n;
  const coefficients = medium.terms.map((term) => ({ term, sigma: extinction(term) }));
  const sum = [0, 0, 0];
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
  }
  const k = dt / 3;
  return [(sum[0] ?? 0) * k, (sum[1] ?? 0) * k, (sum[2] ?? 0) * k];
}
