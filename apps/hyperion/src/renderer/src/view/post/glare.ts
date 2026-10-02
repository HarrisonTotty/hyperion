/**
 * Veiling glare: the spread functions of an eye and of a camera, and the closed form of a glare
 * source whose excess light no `rgba16float` level can hold (plan R07, Design note 12).
 *
 * @remarks
 * Shared file. R06.T13.e's host-disc layer returns {@link GlareSource}s from here, and R11's glint
 * adds another; the exports below keep their names and meanings, and a later plan extends this
 * file rather than redeclaring them.
 */
import type { Vec3 } from "../../geometry/vec3";
import type { ViewRole } from "../camera/state";
import type { Rgb } from "../photometry/toneCurve";

/**
 * A light source too bright for the HDR target: its glare is evaluated in closed form in the
 * tone-mapping pass, in `f32`, rather than through the bloom chain (Design note 12).
 *
 * @remarks
 * R06's `HostDiscLayer.glareSources` returns one per host disc (in an eye view also for discs up to
 * 45° outside the frame); R11's glint is another.
 */
export interface GlareSource {
  /** The unit direction to the source's centre, in the view's camera-relative frame. */
  readonly direction: Vec3;
  /** The source's angular radius, rad; zero for a point. */
  readonly angularRadiusRad: number;
  /**
   * The luminance the target could not store, per channel, cd/m²: the source's mean luminance less
   * what its clamped texels hold, 65,504 ÷ the pre-exposure scale (Provides: "cd/m² above 65,504").
   */
  readonly excessLuminance: Rgb;
}

/** An eye observer of the CIE glare spread function (R06's `DEFAULT_EYE_OBSERVER` is age 25, 0.5). */
export interface EyeObserver {
  /** Age, years. */
  readonly ageYears: number;
  /**
   * Ocular pigmentation factor, in [0, 1.2]: 0 for very dark eyes, 0.5 brown, 1 blue-green, 1.2
   * blue (CIE 135/1999; Vos and van den Berg 1997).
   */
  readonly pigmentation: number;
}

const DEG_PER_RAD = 180 / Math.PI;

/**
 * The CIE 135/1999 glare spread function before renormalisation, sr⁻¹ (equivalent veiling
 * luminance per unit illuminance at the eye, (cd/m²)/lx).
 *
 * @remarks
 * Vos and van den Berg 1999, CIE 135/1999, "Report on disability glare", the complete equation,
 * as reproduced in McCann and Vonikakis 2018 (Front. Psychol. 8:2079, eq. 2), with θ in degrees:
 *
 * [1 − 0.08 (A/70)⁴] [9.2 × 10⁶ / (1 + (θ/0.0046)²)^1.5 + 1.5 × 10⁵ / (1 + (θ/0.045)²)^1.5]
 * + [1 + 1.6 (A/70)⁴] {[400 / (1 + (θ/0.1)²) + 3 × 10⁻⁸ θ²]
 *   + p [1300 / (1 + (θ/0.1)²)^1.5 + 0.8 / (1 + (θ/0.1)²)^0.5]} + 2.5 × 10⁻³ p.
 *
 * The first angular constant is 0.0046°: the 0.046° that open copies print integrates to about 37
 * over the sphere instead of about 1 (Vos and van den Berg 1997, TNO preprint, eqs. 4, 16 and 17,
 * print 0.0046°). With 0.0046° it integrates to 1.047 at age 25 and pigmentation 0.5, and 1.010 at
 * pigmentation 0. Age enters both terms. The equation is valid to 100° and is extrapolated beyond,
 * where 1.2% of its energy lies.
 */
export function cieGlareSpreadRaw(thetaRad: number, eye: EyeObserver): number {
  const theta = Math.abs(thetaRad) * DEG_PER_RAD;
  const age = (eye.ageYears / 70) ** 4;
  const p = eye.pigmentation;
  const u = 1 + (theta / 0.1) ** 2;
  const core =
    9.2e6 / (1 + (theta / 0.0046) ** 2) ** 1.5 + 1.5e5 / (1 + (theta / 0.045) ** 2) ** 1.5;
  const wide = 400 / u + 3e-8 * theta * theta + p * (1300 / u ** 1.5 + 0.8 / Math.sqrt(u));
  return (1 - 0.08 * age) * core + (1 + 1.6 * age) * wide + 2.5e-3 * p;
}

/**
 * The fraction of a camera's point-spread energy in its wide scattering tail (Design note 12).
 *
 * @remarks
 * Within the 1–10% veiling glare index (ISO 9358:1994's definition) reported for commercial
 * lenses, toward the good-lens end; the tail's shape is assumed. Low confidence (plan R07, Risks,
 * "The lens PSF").
 */
export const CAMERA_TAIL_FRACTION = 0.03;

/**
 * The camera core's Gaussian width σ, rad: a quarter of an arcminute, a seventh of a pixel at 1080p
 * across 60°, so that the core stays inside the source's pixel and only the tail blooms.
 */
export const CAMERA_CORE_SIGMA_RAD = Math.PI / (180 * 60 * 4);

/** The camera tail's knee, rad: 0.1°, about three pixels at 1080p across 60°. */
export const CAMERA_TAIL_KNEE_RAD = (0.1 * Math.PI) / 180;

/**
 * A camera's scattering tail before normalisation: a Harvey-type 1 ÷ (1 + (θ/θ₀)²), slope −2
 * beyond the knee θ₀ ({@link CAMERA_TAIL_KNEE_RAD}).
 */
function cameraTailRaw(thetaRad: number): number {
  return 1 / (1 + (thetaRad / CAMERA_TAIL_KNEE_RAD) ** 2);
}

/** A camera's core before normalisation: a Gaussian of width {@link CAMERA_CORE_SIGMA_RAD}. */
function cameraCoreRaw(thetaRad: number): number {
  return Math.exp(-0.5 * (thetaRad / CAMERA_CORE_SIGMA_RAD) ** 2);
}

/**
 * ∫ f(θ) dΩ over the whole sphere, by Simpson's rule in ln θ from 10⁻⁹ rad to π.
 *
 * @remarks
 * 8,000 intervals put the CIE function's integral within 10⁻⁶ of a 200,000-interval reference.
 */
export function sphereIntegral(f: (thetaRad: number) => number): number {
  const intervals = 8000;
  const low = Math.log(1e-9);
  const high = Math.log(Math.PI);
  const step = (high - low) / intervals;
  let sum = 0;
  for (let i = 0; i <= intervals; i += 1) {
    const theta = Math.exp(low + i * step);
    const weight = i === 0 || i === intervals ? 1 : i % 2 === 1 ? 4 : 2;
    sum += weight * f(theta) * 2 * Math.PI * Math.sin(theta) * theta;
  }
  return (sum * step) / 3;
}

const eyeNorms = new Map<string, number>();

function eyeNorm(eye: EyeObserver): number {
  const key = `${eye.ageYears}:${eye.pigmentation}`;
  const known = eyeNorms.get(key);
  if (known !== undefined) {
    return known;
  }
  const norm = sphereIntegral((theta) => cieGlareSpreadRaw(theta, eye));
  eyeNorms.set(key, norm);
  return norm;
}

const cameraCoreNorm = sphereIntegral(cameraCoreRaw);
const cameraTailNorm = sphereIntegral(cameraTailRaw);

/**
 * The glare spread function of a view, sr⁻¹, normalised so that its integral over the sphere is 1:
 * the fraction of a point source's light scattered per steradian at `thetaRad` from it.
 *
 * @remarks
 * An eye view (`"eye"`) takes the CIE function ({@link cieGlareSpreadRaw}) for `eye`, renormalised;
 * the call sites pass R06's `DEFAULT_EYE_OBSERVER`, age 25 and pigmentation 0.5. A camera view
 * (`"camera"`) takes a Gaussian core holding 1 − {@link CAMERA_TAIL_FRACTION} of the energy and a
 * Harvey-type tail holding the rest, from memory and of low confidence; `eye` is unused there.
 *
 * @param thetaRad - The angle from the source, rad, in [0, π].
 */
export function glareSpread(role: ViewRole, thetaRad: number, eye: EyeObserver): number {
  return glareSpreadFunction(role, eye)(thetaRad);
}

/**
 * {@link glareSpread} for one view role and observer, as a function of the angle alone, rad → sr⁻¹,
 * with its normalisation resolved once.
 */
export function glareSpreadFunction(
  role: ViewRole,
  eye: EyeObserver,
): (thetaRad: number) => number {
  let spread: (thetaRad: number) => number;
  switch (role) {
    case "eye": {
      const norm = eyeNorm(eye);
      spread = (thetaRad) => cieGlareSpreadRaw(thetaRad, eye) / norm;
      break;
    }
    case "camera":
      spread = (thetaRad) =>
        ((1 - CAMERA_TAIL_FRACTION) * cameraCoreRaw(thetaRad)) / cameraCoreNorm +
        (CAMERA_TAIL_FRACTION * cameraTailRaw(thetaRad)) / cameraTailNorm;
      break;
  }
  return spread;
}

/** A source's solid angle, sr: 2π (1 − cos ρ) for its angular radius ρ. */
export function glareSourceSolidAngleSr(source: GlareSource): number {
  return 2 * Math.PI * (1 - Math.cos(source.angularRadiusRad));
}

/**
 * The veil a glare source adds at `thetaRad` from its centre, per channel, in the units of its
 * `excessLuminance`: the excess illuminance L_ex × Ω times the spread function.
 *
 * @remarks
 * The source is treated as a point, which the clamped disc covers where the approximation fails;
 * over the sphere the veil integrates to exactly L_ex × Ω, so the stored disc and the injected
 * veil together hold the unclamped energy. The tone-mapping pass evaluates the same expression in
 * `f32` (R07.T14.b).
 */
export function glareSourceVeil(
  source: GlareSource,
  thetaRad: number,
  role: ViewRole,
  eye: EyeObserver,
): Rgb {
  const factor = glareSourceSolidAngleSr(source) * glareSpread(role, thetaRad, eye);
  return [
    source.excessLuminance[0] * factor,
    source.excessLuminance[1] * factor,
    source.excessLuminance[2] * factor,
  ];
}
