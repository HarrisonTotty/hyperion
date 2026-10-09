/**
 * The measured V-band phase curves that a body's phase function is fitted to (plan R07, Design
 * note 5).
 *
 * @remarks
 * Each template is a Solar System analogue's disc-integrated V phase curve, normalised to one at
 * opposition: Φ_t(α) = 10^(−0.4 [V(α) − V(0)]). The planets' curves are Mallama and Hilton,
 * "Computing apparent planetary magnitudes for The Astronomical Almanac", Astronomy and Computing
 * 25 (2018) 10, arXiv:1808.01973, eqs. 2–17, each inside the range that paper states for it,
 * except Earth's, which is Robinson, Planetary Science Journal 7 (2026) 12, arXiv:2507.22258, eq.
 * 14 (decision-r07-earth-albedo, R07.T4.d). The Moon's is Krisciunas and Schaefer, PASP 103
 * (1991) 1033, eq. 9, a fit to the table in Allen, _Astrophysical Quantities_, 3rd ed. (1973),
 * p. 143: V = −12.73 + 0.026 |α| + 4 × 10⁻⁹ α⁴, with no opposition surge. A template is defined on
 * [0, `validToRad`]; past it the law holds its phase factor f (`law.ts`), never the template's
 * fit, which is unconstrained by data there. Distance and the zeroth-order term drop out of Φ_t,
 * so Mercury's corrected V(1, 0) of −0.613 (that paper's §3.1, against the −0.694 of Mallama et
 * al. 2017's Table A-1.2) changes no template.
 *
 * Three templates borrow a curve's shape and are flagged `provisional`. Airless ice and a snowball
 * take the Moon's curve at L = 1. No disc-integrated V polynomial of an icy satellite is published
 * (Domingue and Verbiscer 1997, Icarus 128, 49, give Hapke fits only). Plan 14 solves s to the icy
 * analogue's measured q: Ganymede's 0.80 (Squyres and Veverka 1981, Icarus 46, 137) and Europa's
 * 1.01 (Grundy et al. 2007, Science 318, 234). A magma ocean under 30 kPa takes Mercury's curve;
 * above it, Venus's (decision-phase-curves, 2026-10-02).
 */
import type { PhaseTemplateId } from "./law";
import { lambertPhase } from "./shapes";

/** A measured V phase curve, normalised to one at opposition. */
export interface PhaseTemplate {
  /**
   * Φ_t at phase angle α, Φ_t(0) = 1.
   *
   * @remarks
   * Defined on [0, {@link PhaseTemplate.validToRad}]; an argument outside it is clamped to it.
   */
  readonly phaseV: (alphaRad: number) => number;
  /** The end of the curve's stated range, rad; the law holds its f beyond it (Design note 5). */
  readonly validToRad: number;
  /** The citation: paper, equations and range. */
  readonly source: string;
  /**
   * The law's Lommel–Seeliger share L that goes with this curve, by Design note 5's rule: 1 for an
   * airless surface, 0.5 for Mars's thin air, 0 under a thick atmosphere or cloud.
   */
  readonly lommelSeeligerShare: number;
  /** True for a stand-in with no measured analogue, which is labelled wherever it is shown. */
  readonly provisional: boolean;
}

const DEG_PER_RAD = 180 / Math.PI;

/** Magnitudes of dimming → flux ratio. */
function fluxFromDimming(deltaMag: number): number {
  return 10 ** (-0.4 * deltaMag);
}

/** Σ cₖ xᵏ, coefficients in ascending order (Horner). */
function polynomial(coefficients: ReadonlyArray<number>, x: number): number {
  let sum = 0;
  for (let k = coefficients.length - 1; k >= 0; k -= 1) {
    sum = sum * x + (coefficients[k] ?? 0);
  }
  return sum;
}

/**
 * A template from Δm(α°), the dimming in magnitudes against opposition with α in degrees.
 *
 * @param dimmingMag - Δm(α°); Δm(0) must be 0.
 * @param validToDeg - The end of the stated range, degrees.
 */
function template(
  dimmingMag: (alphaDeg: number) => number,
  validToDeg: number,
  lommelSeeligerShare: number,
  source: string,
  provisional: boolean,
): PhaseTemplate {
  const validToRad = validToDeg / DEG_PER_RAD;
  return {
    phaseV: (alphaRad) => {
      const clamped = Math.min(Math.max(alphaRad, 0), validToRad);
      return fluxFromDimming(dimmingMag(clamped * DEG_PER_RAD));
    },
    validToRad,
    lommelSeeligerShare,
    source,
    provisional,
  };
}

/**
 * The Moon, Krisciunas and Schaefer 1991, eq. 9 (after Allen 1973): Δm = 0.026 α + 4 × 10⁻⁹ α⁴, α
 * in degrees.
 */
function moonDimming(alphaDeg: number): number {
  return 0.026 * alphaDeg + 4e-9 * alphaDeg ** 4;
}

/** Mercury, Mallama and Hilton 2018, eq. 2 (the sixth-order polynomial, 2.1° < α < 169.5°). */
const MERCURY_COEFFICIENTS = [
  0, 6.328e-2, -1.6336e-3, 3.3644e-5, -3.4265e-7, 1.6893e-9, -3.0334e-12,
] as const;

function mercuryDimming(alphaDeg: number): number {
  return polynomial(MERCURY_COEFFICIENTS, alphaDeg);
}

/** Venus's join between eqs. 3 and 4, degrees: the forward-scattering reversal (§3.2). */
const VENUS_JOIN_DEG = 163.7;

/** Venus, eq. 3 for 0 < α ≤ 163.7°, eq. 4 for 163.7° < α < 179°, both against eq. 3's −4.384. */
function venusDimming(alphaDeg: number): number {
  if (alphaDeg <= VENUS_JOIN_DEG) {
    return polynomial([0, -1.044e-3, 3.687e-4, -2.814e-6, 8.938e-9], alphaDeg);
  }
  return polynomial([236.05828 + 4.384, -2.81914, 8.39034e-3], alphaDeg);
}

/** The asymmetry g of Earth's Henyey–Greenstein phase curve, Robinson 2026, eq. 14. */
const EARTH_HG_ASYMMETRY = -0.33;

/**
 * Earth, Robinson 2026, eq. 14: a Henyey–Greenstein function at the scattering angle 180° − α,
 * normalised at opposition, Δm = 3.75 log₁₀[(1 + g² + 2g cos α) ÷ (1 + g)²], g = −0.33.
 *
 * @remarks
 * The fit to the curated visual curve (earthshine, DSCOVR/EPIC, EPOXI, Galileo and LCROSS over
 * 5°–144°, reduced χ² 0.96), whose f = 0.23 is its geometric albedo. It replaces Mallama and
 * Hilton 2018's eq. 5, a fit to Tinetti et al. 2006's model (§4.3 there; Robinson 2026, §6.1),
 * whose Sun–observer azimuth is turned by 180° (Robinson et al. 2011, Astrobiology 11, 393, §3.4
 * and Fig. 2).
 */
function earthDimming(alphaDeg: number): number {
  const g = EARTH_HG_ASYMMETRY;
  return 3.75 * Math.log10((1 + g * g + 2 * g * Math.cos(alphaDeg / DEG_PER_RAD)) / (1 + g) ** 2);
}

/** Mars, eq. 6 (α ≤ 50°), without the longitude and season terms L(λe) and L(Ls). */
function marsDimming(alphaDeg: number): number {
  return polynomial([0, 2.267e-2, -1.302e-4], alphaDeg);
}

/** Jupiter's join between eqs. 8 and 9, degrees (§3.5). */
const JUPITER_JOIN_DEG = 12;

/** Jupiter, eq. 8 for α ≤ 12°, eq. 9 (Mayorga et al. 2016's green filter) for 12° < α < 130°. */
function jupiterDimming(alphaDeg: number): number {
  if (alphaDeg <= JUPITER_JOIN_DEG) {
    return polynomial([0, -3.7e-4, 6.16e-4], alphaDeg);
  }
  const x = alphaDeg / 180;
  // Eq. 9's −9.428 against eq. 8's −9.395: the paper's adjustment so that the two agree at 12°.
  return (
    -9.428 + 9.395 - 2.5 * Math.log10(polynomial([1, -1.507, -0.363, -0.062, 2.809, -1.876], x))
  );
}

/** Saturn's join between eqs. 11 and 12, degrees: eq. 12's −8.94 is set to agree at 6° (§3.6). */
const SATURN_JOIN_DEG = 6;

/** Saturn's globe without rings, eq. 11 for α ≤ 6°, eq. 12 for 6° < α < 150°, against −8.95. */
function saturnDimming(alphaDeg: number): number {
  if (alphaDeg <= SATURN_JOIN_DEG) {
    return polynomial([0, -3.7e-4, 6.16e-4], alphaDeg);
  }
  return -8.94 + 8.95 + polynomial([0, 2.446e-4, 2.672e-4, -1.505e-6, 4.767e-9], alphaDeg);
}

/** Uranus, eq. 15 at a sub-latitude φ′ of 0 (Pearl et al. 1990's Voyager curve, to 154°). */
function uranusDimming(alphaDeg: number): number {
  return polynomial([0, 6.587e-3, 1.045e-4], alphaDeg);
}

/** Neptune, eq. 17 (Pearl and Conrath 1991's Voyager curve, to 133°). */
function neptuneDimming(alphaDeg: number): number {
  return polynomial([0, 7.944e-3, 9.617e-5], alphaDeg);
}

const MH2018 = "Mallama and Hilton 2018, Astronomy and Computing 25, 10";
const ROBINSON2026 =
  "Robinson 2026, PSJ 7, 12, eq. 14 (Henyey–Greenstein, g = −0.33, f = 0.23; data 5°–144°); to 144°; replaces Mallama and Hilton 2018's eq. 5, from Tinetti et al. 2006's model with the Sun–observer azimuth turned by 180° (Robinson et al. 2011, Astrobiology 11, 393, Fig. 2; Robinson 2026, §6.1)";
const MOON_KS91 =
  "Krisciunas and Schaefer 1991, PASP 103, 1033, eq. 9 (a fit to Allen 1973, Astrophysical Quantities, 3rd ed., p. 143)";

/**
 * The phase-curve templates by identifier (Design note 5).
 *
 * @remarks
 * Ranges: Mercury to 169.5° (the observed 2.1°–169.5°, §4.1); Venus to 179° (eqs. 3–4); Earth to
 * 144°, the end of Robinson 2026's data (the clamp at f = 4 acts from 139.0° and takes up to 29%
 * off eq. 14 at 144°, inside the 38% spread of the weather there, his eq. 9); Mars to 50° (eq. 6;
 * eq. 7's average of Mercury and Earth beyond it is not used, as Design note 5 holds f past 50°
 * instead); Jupiter to 130° (Mayorga et al.: untrustworthy beyond); Saturn's globe to 150° (eq.
 * 12); Uranus to 154°; Neptune to 133°; the Moon to 150° (Allen's table runs to 160°, where the
 * fit gives 6.78 mag against the table's 7.5).
 */
export const PHASE_TEMPLATES: Readonly<Record<PhaseTemplateId, PhaseTemplate>> = {
  moon: template(moonDimming, 150, 1, `${MOON_KS91}; to 150°`, false),
  mercury: template(mercuryDimming, 169.5, 1, `${MH2018}, eq. 2; to 169.5°`, false),
  mars: template(marsDimming, 50, 0.5, `${MH2018}, eq. 6; to 50°`, false),
  venus: template(venusDimming, 179, 0, `${MH2018}, eqs. 3–4; to 179°`, false),
  earth: template(earthDimming, 144, 0, ROBINSON2026, false),
  jupiter: template(jupiterDimming, 130, 0, `${MH2018}, eqs. 8–9; to 130°`, false),
  saturn: template(saturnDimming, 150, 0, `${MH2018}, eqs. 11–12, globe only; to 150°`, false),
  uranus: template(uranusDimming, 154, 0, `${MH2018}, eq. 15 at φ′ = 0; to 154°`, false),
  neptune: template(neptuneDimming, 133, 0, `${MH2018}, eq. 17; to 133°`, false),
  "airless-ice": template(
    moonDimming,
    150,
    1,
    `PROVISIONAL: the Moon's curve shape (${MOON_KS91}); q from Ganymede's 0.80 (Squyres and Veverka 1981, Icarus 46, 137) through s`,
    true,
  ),
  snowball: template(
    moonDimming,
    150,
    1,
    `PROVISIONAL: the Moon's curve shape (${MOON_KS91}); q from Europa's 1.01 (Grundy et al. 2007, Science 318, 234) through s; cloud-free ice`,
    true,
  ),
  // Design note 5's provisional photometry: a Lambert sphere, Φ = Φ_L, f = 1, held past 179°
  // where Φ_L reaches 0 at 180°.
  lambert: template(
    (alphaDeg) => -2.5 * Math.log10(lambertPhase(alphaDeg / DEG_PER_RAD)),
    179,
    0,
    "PROVISIONAL: a Lambert sphere, no measured curve (Design note 5's provisional photometry)",
    true,
  ),
  magma: template(
    mercuryDimming,
    169.5,
    1,
    `PROVISIONAL: Mercury's curve stands in (${MH2018}, eq. 2); no magma-ocean analogue; thin branch only, P < 30 kPa (thicker: venus)`,
    true,
  ),
};
