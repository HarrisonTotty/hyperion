/**
 * The disc integral of a lunar-Lambert law over an oblate spheroid (plan R07, T8.a; Design note
 * 19), which ties the disc regime's albedo scale and the point regime's flux to one p.
 *
 * @remarks
 * Seen at zero phase along a direction at latitude β, the law's I/F = A [L + (1 − L) μ] integrates
 * over the visible half to A [L + (1 − L) m(β)] × the projected area, m(β) = ∫ μ² dS ÷ ∫ μ dS: ⅔ for
 * a sphere at every β, and otherwise a function of c ÷ a and β. p is defined against the projected
 * area π a c equator-on (Mallama et al. 2017's Saturn), so a spheroid's disc takes
 * A = p ÷ [L + (1 − L) m(0)]. Its point integrates the same law over the spheroid
 * ({@link spheroidGeometricIntegral}): no closed phase function describes a spheroid lit and seen
 * off its equator, where π a b′ Φ(α) errs by 4% at 90° and 12% at 150° of phase for f = 0.098 seen
 * from 45° latitude.
 */
import { cross, dot, norm, normalise, type Vec3, vec3 } from "../../geometry/vec3";

/** Quadrature intervals in the polar angle and in azimuth. */
const THETA_STEPS = 96;
const PHI_STEPS = 192;

/** Entries of each figure's table in sin β, from 0 to 1. */
const TABLE_ENTRIES = 33;

/** m(β) by midpoint quadrature over the surface (a = 1). */
function secondMomentRaw(cOverA: number, sinBeta: number): number {
  const c = cOverA;
  const cosBeta = Math.sqrt(Math.max(0, 1 - sinBeta * sinBeta));
  let first = 0;
  let second = 0;
  for (let i = 0; i < THETA_STEPS; i += 1) {
    const theta = ((i + 0.5) * Math.PI) / THETA_STEPS;
    const sinT = Math.sin(theta);
    const cosT = Math.cos(theta);
    for (let j = 0; j < PHI_STEPS; j += 1) {
      const phi = ((j + 0.5) * 2 * Math.PI) / PHI_STEPS;
      // ∂r/∂θ × ∂r/∂φ = (c sin²θ cos φ, c sin²θ sin φ, sin θ cos θ), outward, |·| = dS ÷ dθ dφ.
      const nx = c * sinT * sinT * Math.cos(phi);
      const nz = sinT * cosT;
      const projected = nx * cosBeta + nz * sinBeta;
      if (projected <= 0) {
        continue;
      }
      const ny = c * sinT * sinT * Math.sin(phi);
      first += projected;
      second += (projected * projected) / Math.hypot(nx, ny, nz);
    }
  }
  return second / first;
}

/** Each figure's m table by c ÷ a, made on first use. */
const TABLES = new Map<number, Float64Array>();

/**
 * m(β) = ∫ μ² dS ÷ ∫ μ dS over the half of a spheroid seen from latitude β, by linear
 * interpolation in sin β of a table made once per figure; exactly ⅔ for a sphere.
 *
 * @param cOverA - c ÷ a, in (0, 1].
 * @param sinBeta - |sin β| of the sub-observer latitude.
 */
export function projectedSecondMoment(cOverA: number, sinBeta: number): number {
  if (cOverA >= 1) {
    return 2 / 3;
  }
  let table = TABLES.get(cOverA);
  if (table === undefined) {
    table = new Float64Array(TABLE_ENTRIES);
    for (let k = 0; k < TABLE_ENTRIES; k += 1) {
      table[k] = secondMomentRaw(cOverA, k / (TABLE_ENTRIES - 1));
    }
    TABLES.set(cOverA, table);
  }
  const x = Math.min(1, Math.abs(sinBeta)) * (TABLE_ENTRIES - 1);
  const k = Math.min(Math.floor(x), TABLE_ENTRIES - 2);
  const t = x - k;
  return (table[k] ?? 0) + t * ((table[k + 1] ?? 0) - (table[k] ?? 0));
}

/** The law's zero-phase disc factor L + (1 − L) m(β). */
function discFactor(share: number, cOverA: number, sinBeta: number): number {
  return share + (1 - share) * projectedSecondMoment(cOverA, sinBeta);
}

/**
 * The factor on a law's A that makes a spheroid's disc reach p equator-on at zero phase:
 * [L + ⅔(1 − L)] ÷ [L + (1 − L) m(0)]; 1 for a sphere.
 */
export function oblateAlbedoScale(share: number, cOverA: number): number {
  return (share + (2 / 3) * (1 - share)) / discFactor(share, cOverA, 0);
}

/** Quadrature intervals of the point's integral over the spheroid. */
const POINT_THETA_STEPS = 96;
const POINT_PHI_STEPS = 192;

/**
 * K = ∫ [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀] μ dS over the lit and visible spheroid of a = 1, seen
 * from afar: the geometric part of a point's flux, F = (E ÷ π)(a ÷ Δ)² A f(α) K, by midpoint
 * quadrature; the directions are unit vectors along any one set of axes.
 *
 * @remarks
 * For a sphere K = π [L + ⅔(1 − L)] Φ_shape(α), which the point takes in closed form instead.
 */
export function spheroidGeometricIntegral(
  share: number,
  cOverA: number,
  pole: Vec3,
  toStar: Vec3,
  toCamera: Vec3,
): number {
  // A frame with the pole as z; the integral depends only on the two directions in it.
  const seed = Math.abs(pole.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const ex = normalise(cross(seed, pole));
  const ey = cross(pole, ex);
  const local = (v: Vec3): Vec3 => vec3(dot(v, ex), dot(v, ey), dot(v, pole));
  const s = local(toStar);
  const v = local(toCamera);
  // Remembered by the directions to 10⁻⁴ (a change of K well under 10⁻³): a point's geometry
  // moves slowly, and the quadrature costs some 2 × 10⁴ evaluations.
  const key = [share, cOverA, s.x, s.y, s.z, v.x, v.y, v.z]
    .map((x) => Math.round(x * 1e4))
    .join(",");
  const remembered = REMEMBERED.get(key);
  if (remembered !== undefined) {
    return remembered;
  }
  const k = integrate(share, cOverA, s, v);
  if (REMEMBERED.size >= REMEMBERED_MAX) {
    REMEMBERED.clear();
  }
  REMEMBERED.set(key, k);
  return k;
}

/** The integrals remembered, and how many before the memory is cleared. */
const REMEMBERED = new Map<string, number>();
const REMEMBERED_MAX = 4_096;

/** K by midpoint quadrature, the directions in the pole's frame. */
function integrate(share: number, cOverA: number, s: Vec3, v: Vec3): number {
  const c = cOverA;
  const dTheta = Math.PI / POINT_THETA_STEPS;
  const dPhi = (2 * Math.PI) / POINT_PHI_STEPS;
  let k = 0;
  for (let i = 0; i < POINT_THETA_STEPS; i += 1) {
    const theta = (i + 0.5) * dTheta;
    const sinT = Math.sin(theta);
    const cosT = Math.cos(theta);
    for (let j = 0; j < POINT_PHI_STEPS; j += 1) {
      const phi = (j + 0.5) * dPhi;
      // ∂r/∂θ × ∂r/∂φ, outward; its length is dS ÷ dθ dφ.
      const n = vec3(c * sinT * sinT * Math.cos(phi), c * sinT * sinT * Math.sin(phi), sinT * cosT);
      const area = norm(n);
      const mu = dot(n, v) / area;
      const mu0 = dot(n, s) / area;
      if (mu <= 0 || mu0 <= 0) {
        continue;
      }
      const disc = (share * 2 * mu0) / (mu0 + mu) + (1 - share) * mu0;
      k += disc * mu * area * dTheta * dPhi;
    }
  }
  return k;
}
