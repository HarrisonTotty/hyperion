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
  /**
   * The unit direction to the source's centre, in the scene's camera-relative frame: the one
   * `FrameSubmission.viewRotation` turns into view space.
   */
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

/** A term a ÷ (1 + (θ/c)²)^power of a spread function, its amplitude in sr⁻¹. */
export interface SpreadTerm {
  readonly amplitude: number;
  /** c, rad. */
  readonly scaleRad: number;
}

/**
 * A view's normalised glare spread function as a sum of terms, sr⁻¹ with θ in rad: the form both
 * the TypeScript and the tone-mapping pass's WGSL evaluate, so that they agree term for term.
 *
 * @remarks
 * PSF(θ) = Σ poisson aᵢ (1 + (θ/cᵢ)²)^−1.5 + Σ lorentz aⱼ (1 + (θ/cⱼ)²)^−1 + Σ root aₖ (1 +
 * (θ/cₖ)²)^−0.5 + q θ² + k + g exp(−θ² ÷ 2σ²). The `poisson` terms are the narrow ones whose
 * integral over a resolved source has a closed form ({@link glareSourceVeil}).
 */
export interface GlareSpreadTerms {
  /** At most {@link GLARE_POISSON_TERMS}. */
  readonly poisson: ReadonlyArray<SpreadTerm>;
  /** At most one. */
  readonly lorentz: ReadonlyArray<SpreadTerm>;
  /** At most one. */
  readonly root: ReadonlyArray<SpreadTerm>;
  /** q, sr⁻¹ rad⁻². */
  readonly quadratic: number;
  /** k, sr⁻¹. */
  readonly constant: number;
  /** g, sr⁻¹, and σ, rad. */
  readonly gaussian: { readonly amplitude: number; readonly sigmaRad: number };
}

/** The most `poisson` terms a spread function has: the CIE function's three. */
export const GLARE_POISSON_TERMS = 3;

const RAD_PER_DEG = Math.PI / 180;

/**
 * The terms of a view's normalised spread function: the CIE function for `eye`, divided by its
 * integral over the sphere, or the camera's core and tail.
 */
export function glareSpreadTerms(role: ViewRole, eye: EyeObserver): GlareSpreadTerms {
  let terms: GlareSpreadTerms;
  switch (role) {
    case "eye": {
      const n = eyeNorm(eye);
      const age = (eye.ageYears / 70) ** 4;
      const core = (1 - 0.08 * age) / n;
      const wide = (1 + 1.6 * age) / n;
      const p = eye.pigmentation;
      terms = {
        poisson: [
          { amplitude: 9.2e6 * core, scaleRad: 0.0046 * RAD_PER_DEG },
          { amplitude: 1.5e5 * core, scaleRad: 0.045 * RAD_PER_DEG },
          { amplitude: 1300 * p * wide, scaleRad: 0.1 * RAD_PER_DEG },
        ],
        lorentz: [{ amplitude: 400 * wide, scaleRad: 0.1 * RAD_PER_DEG }],
        root: [{ amplitude: 0.8 * p * wide, scaleRad: 0.1 * RAD_PER_DEG }],
        // 3 × 10⁻⁸ θ² with θ in degrees.
        quadratic: (3e-8 * wide) / (RAD_PER_DEG * RAD_PER_DEG),
        constant: (2.5e-3 * p) / n,
        gaussian: { amplitude: 0, sigmaRad: 1 },
      };
      break;
    }
    case "camera":
      terms = {
        poisson: [],
        lorentz: [
          {
            amplitude: CAMERA_TAIL_FRACTION / cameraTailNorm,
            scaleRad: CAMERA_TAIL_KNEE_RAD,
          },
        ],
        root: [],
        quadratic: 0,
        constant: 0,
        gaussian: {
          amplitude: (1 - CAMERA_TAIL_FRACTION) / cameraCoreNorm,
          sigmaRad: CAMERA_CORE_SIGMA_RAD,
        },
      };
      break;
  }
  return terms;
}

/** The terms other than the `poisson` ones at θ, sr⁻¹: those a source is taken as a point for. */
function broadSpread(terms: GlareSpreadTerms, thetaRad: number): number {
  let sum = terms.quadratic * thetaRad * thetaRad + terms.constant;
  for (const { amplitude, scaleRad } of terms.lorentz) {
    sum += amplitude / (1 + (thetaRad / scaleRad) ** 2);
  }
  for (const { amplitude, scaleRad } of terms.root) {
    sum += amplitude / Math.sqrt(1 + (thetaRad / scaleRad) ** 2);
  }
  const { amplitude, sigmaRad } = terms.gaussian;
  return sum + amplitude * Math.exp(-0.5 * (thetaRad / sigmaRad) ** 2);
}

/** A spread function's value at θ from its terms, sr⁻¹. */
export function evaluateSpread(terms: GlareSpreadTerms, thetaRad: number): number {
  let sum = broadSpread(terms, thetaRad);
  for (const { amplitude, scaleRad } of terms.poisson) {
    sum += amplitude / (1 + (thetaRad / scaleRad) ** 2) ** 1.5;
  }
  return sum;
}

/**
 * The glare spread function of a view, sr⁻¹, normalised so that its integral over the sphere is 1:
 * the fraction of a point source's light scattered per steradian at `thetaRad` from it.
 *
 * @remarks
 * An eye view (`"eye"`) takes the CIE function ({@link cieGlareSpreadRaw}) for `eye`, renormalised;
 * the call sites pass R06's `DEFAULT_EYE_OBSERVER`, age 25 and pigmentation 0.5. A camera view
 * (`"camera"`) takes a Gaussian core holding 1 − {@link CAMERA_TAIL_FRACTION} of the energy and a
 * Harvey-type tail holding the rest, of low confidence; `eye` is unused there.
 *
 * @param thetaRad - The angle from the source, rad, in [0, π].
 */
export function glareSpread(role: ViewRole, thetaRad: number, eye: EyeObserver): number {
  return glareSpreadFunction(role, eye)(thetaRad);
}

/**
 * {@link glareSpread} for one view role and observer, as a function of the angle alone, rad → sr⁻¹,
 * with its terms resolved once.
 */
export function glareSpreadFunction(
  role: ViewRole,
  eye: EyeObserver,
): (thetaRad: number) => number {
  const terms = glareSpreadTerms(role, eye);
  return (thetaRad) => evaluateSpread(terms, thetaRad);
}

/**
 * A source's solid angle, sr: 2π (1 − cos ρ) for its angular radius ρ, written 4π sin²(ρ ÷ 2) so
 * that a star's milliarcseconds keep their digits.
 */
export function glareSourceSolidAngleSr(source: GlareSource): number {
  const half = Math.sin(source.angularRadiusRad / 2);
  return 4 * Math.PI * half * half;
}

/**
 * A source smaller than this fraction of a term's scale is a point for that term: the rectangle's
 * closed form loses its digits there, in `f32` above all, and the two agree.
 */
export const POINT_SOURCE_FRACTION = 0.01;

/**
 * ∫∫ a (1 + r²/c²)^−1.5 dA over the rectangle x ∈ [x₁, x₂], |y| ≤ Y, in the plane at the pixel,
 * sr⁻¹ × rad²: a c² times the rectangle's solid angle seen from height c, 2 [F(x₂) − F(x₁)] with
 * F(X) = atan(X Y ÷ (c √(c² + X² + Y²))).
 *
 * @remarks
 * dΩ = h dA ÷ (h² + r²)^1.5 and the corner formula for a rectangle's solid angle (Mathar, "Solid
 * angle of a rectangular plate", MPIA, 2005; checked here numerically). For x₁ ≥ 0, beyond the
 * limb, the difference is taken without cancellation, which far from a small source loses every
 * digit in `f32`: with s = √(k + x²), k = c² + Y² and u = x Y ÷ (c s), F(x₂) − F(x₁) = atan((Y ÷ c)
 * (x₂² − x₁²) k ÷ ((x₂ s₁ + x₁ s₂) s₁ s₂) ÷ (1 + u₁ u₂)) (science check, 2026-10-02).
 */
export function poissonOverRectangle(
  term: SpreadTerm,
  x1Rad: number,
  x2Rad: number,
  halfHeightRad: number,
): number {
  const c = term.scaleRad;
  const y = halfHeightRad;
  const k = c * c + y * y;
  const s1 = Math.sqrt(k + x1Rad * x1Rad);
  const s2 = Math.sqrt(k + x2Rad * x2Rad);
  let difference: number;
  if (x1Rad >= 0) {
    const u1 = (x1Rad * y) / (c * s1);
    const u2 = (x2Rad * y) / (c * s2);
    const spread = (x2Rad - x1Rad) * (x2Rad + x1Rad);
    const numerator = ((y / c) * spread * k) / ((x2Rad * s1 + x1Rad * s2) * s1 * s2);
    difference = Math.atan(numerator / (1 + u1 * u2));
  } else {
    difference = Math.atan((x2Rad * y) / (c * s2)) - Math.atan((x1Rad * y) / (c * s1));
  }
  return term.amplitude * c * c * 2 * difference;
}

/** Steps of ln(ρ ÷ c) per unit at which {@link rectangleInsideLevel}'s outer integral is kept. */
const INSIDE_STEPS_PER_LN = 2000;

const outerIntegrals = new Map<number, number>();

/**
 * ∫_{θ>ρ} R(θ) 2πθ dθ for a term of amplitude 1 and scale 1 about a source of radius s, R being
 * {@link poissonOverRectangle}: Simpson's rule in ln(θ − ρ) beyond the limb, the θ⁻³ tail beyond
 * in closed form.
 */
function outerIntegral(s: number): number {
  const unit: SpreadTerm = { amplitude: 1, scaleRad: 1 };
  const halfHeight = (Math.PI * s) / 4;
  const intervals = 4000;
  const low = Math.log(1e-6 * Math.min(s, 1));
  const top = 1e4 * (s + 1);
  const high = Math.log(top);
  const step = (high - low) / intervals;
  let sum = 0;
  for (let i = 0; i <= intervals; i += 1) {
    const beyond = Math.exp(low + i * step);
    const theta = s + beyond;
    const weight = i === 0 || i === intervals ? 1 : i % 2 === 1 ? 4 : 2;
    sum +=
      weight *
      poissonOverRectangle(unit, beyond, theta + s, halfHeight) *
      2 *
      Math.PI *
      theta *
      beyond;
  }
  // Far out R(θ) → 4ρY ÷ θ³, so ∫ beyond the top is 2π × 4ρY ÷ top.
  return (sum * step) / 3 + (2 * Math.PI * 4 * s * halfHeight) / (s + top);
}

/**
 * The level of one narrow term of amplitude 1 at a pixel inside its source's disc, sr⁻¹ × sr,
 * chosen so that the term holds the source's energy: (2π c² Ω − ∫_{θ>ρ} R(θ) 2πθ dθ) ÷ πρ², R
 * being {@link poissonOverRectangle}, never below 0.
 *
 * @remarks
 * The rectangle turns to face each pixel, so its integral over every pixel is not its area: a term
 * whose scale is near the source's radius gains a few per cent near the limb. Inside the disc the
 * image is the clamped disc, white through AgX whatever the veil adds, so the term's energy is
 * balanced there and the veil outside keeps the rectangle's accuracy. In the plane the outer
 * integral is c⁴ times a function of ρ ÷ c alone, kept at steps of 0.05% in ρ ÷ c, so that a source
 * whose radius changes every frame costs one lookup and the store stays bounded.
 */
export function rectangleInsideLevel(rhoRad: number, scaleRad: number): number {
  const key = Math.round(Math.log(rhoRad / scaleRad) * INSIDE_STEPS_PER_LN);
  let outer = outerIntegrals.get(key);
  if (outer === undefined) {
    outer = outerIntegral(Math.exp(key / INSIDE_STEPS_PER_LN));
    outerIntegrals.set(key, outer);
  }
  const c2 = scaleRad * scaleRad;
  const s = rhoRad / scaleRad;
  const omega = 4 * Math.PI * Math.sin(rhoRad / 2) ** 2;
  return Math.max(
    0,
    (2 * Math.PI * c2 * omega) / (Math.PI * rhoRad * rhoRad) - (c2 * outer) / (Math.PI * s * s),
  );
}

/** How many ratios ρ ÷ c {@link rectangleInsideLevel} has integrated, for the tests. */
export function insideLevelStoreSize(): number {
  return outerIntegrals.size;
}

/**
 * The veil a glare source adds at `thetaRad` from its centre, per channel, in the units of its
 * `excessLuminance` (cd/m²): its excess luminance times the spread function integrated over the
 * source.
 *
 * @remarks
 * Each narrow (`poisson`) term is integrated exactly over an equal-area rectangle that faces the
 * pixel with its near edge on the limb, x ∈ [θ − ρ, θ + ρ] and |y| ≤ πρ ÷ 4
 * ({@link poissonOverRectangle}), and inside the disc takes the level that keeps the source's
 * energy ({@link rectangleInsideLevel}); the broad terms take the source as a point, L_ex Ω PSF(θ).
 * Against a brute-force quadrature over the disc, for the Sun at 1 au at 1080p across 60°, it is
 * within +7% to +13% from a quarter of a pixel to 8 px beyond the limb and 0.2% at 64 px, where the
 * point form alone is 0.19 to 0.54 of the truth near the limb (plan R07, Risks, T14.b). The broad
 * terms, taken as a point, hold to −4% for sources up to 3° across and fall to −18% at 10° and
 * −29% at 19.5°; a camera view, which has no narrow term, is the point form throughout, 0.75 of
 * the truth at the Sun's limb (science check, 2026-10-02). Far from
 * the source it is the point form, so over the sphere it integrates to L_ex Ω and the stored disc
 * and the injected veil together hold the unclamped energy. A source below
 * {@link POINT_SOURCE_FRACTION} of a term's scale is a point for that term. The tone-mapping pass
 * evaluates the same expression in `f32`.
 */
export function glareSourceVeil(
  source: GlareSource,
  thetaRad: number,
  role: ViewRole,
  eye: EyeObserver,
): Rgb {
  return glareSourceVeilOf(source, thetaRad, glareSpreadTerms(role, eye));
}

/** {@link glareSourceVeil} for a view's resolved terms. */
export function glareSourceVeilOf(
  source: GlareSource,
  thetaRad: number,
  terms: GlareSpreadTerms,
): Rgb {
  const rho = source.angularRadiusRad;
  const omega = glareSourceSolidAngleSr(source);
  let perUnit = omega * broadSpread(terms, thetaRad);
  for (const term of terms.poisson) {
    perUnit +=
      rho < POINT_SOURCE_FRACTION * term.scaleRad
        ? (omega * term.amplitude) / (1 + (thetaRad / term.scaleRad) ** 2) ** 1.5
        : thetaRad < rho
          ? term.amplitude * rectangleInsideLevel(rho, term.scaleRad)
          : poissonOverRectangle(term, thetaRad - rho, thetaRad + rho, (Math.PI * rho) / 4);
  }
  return [
    source.excessLuminance[0] * perUnit,
    source.excessLuminance[1] * perUnit,
    source.excessLuminance[2] * perUnit,
  ];
}
