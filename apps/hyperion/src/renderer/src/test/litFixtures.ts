/**
 * Fixtures for the lit-body tests of plan R07: the Solar System's measured photometry.
 *
 * @remarks
 * R07.T4.a builds {@link SOLAR_SYSTEM_PHOTOMETRY}; R07.T3 adds `aHostDisc`, R07.T5 `aLitBody`, and
 * R07.T11 {@link photometryFor}, {@link planetPhotometry} and the Moon's albedo.
 */
import type { HostDiscDto } from "@hyperion/protocol";

import type { BodyAppearance } from "../view/appearance/bodyAppearance";
import { type BodyPhotometry, PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { PhaseTemplateId } from "../view/appearance/law";
import { lawFor } from "../view/appearance/phase";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import type { Rgb } from "../view/photometry/toneCurve";

export { SUN_ABSOLUTE_V, SUN_RADIUS_M } from "../view/lighting/hostDisc";

/**
 * A host star's disc as R06 sends it (R07.T3), a Sun unless overridden: `sunLikeHostDisc`, which
 * the kept scenes share.
 */
export function aHostDisc(
  overrides: Partial<HostDiscDto> & { readonly absoluteV?: number } = {},
): HostDiscDto {
  return sunLikeHostDisc(overrides);
}

/** One value in each of Johnson B, V and R. */
export interface JohnsonBvr {
  /** B. */
  readonly b: number;
  /** V. */
  readonly v: number;
  /** R (Johnson). */
  readonly r: number;
}

/** A planet's measured photometry in Johnson B, V and R (Design note 5). */
export interface PlanetPhotometry {
  /** The planet's name. */
  readonly name: string;
  /** Geometric albedo p per band, Mallama et al. 2017, Table 7 (Johnson R, not Cousins R_C). */
  readonly geometricAlbedo: JohnsonBvr;
  /** B − V, magnitudes, from Mallama et al. 2017, Table 3's reference magnitudes. */
  readonly bMinusVMag: number;
  /** V − R, magnitudes, from the same. */
  readonly vMinusRMag: number;
  /** V(1, 0), magnitudes: Table 3's reference V at 1 au from Sun and observer, α = 0. */
  readonly v10Mag: number;
  /**
   * The zeroth-order term of the planet's phase-curve equation in Mallama and Hilton 2018 (eqs. 2,
   * 3, 5, 6, 8, 11, 15, 17), magnitudes: the V(1, 0) its template is anchored to.
   */
  readonly templateV10Mag: number;
  /**
   * The disc-equivalent radius √(a c) the albedo was computed with, km (Mallama et al. 2017, §6,
   * eq. 4: "the average Saturnian disk radius ... 57,240 km including oblateness"); for the giants
   * and Mars, the radius each Table 7 p_V implies, which matches √(a c) of their equatorial and
   * polar radii.
   */
  readonly radiusKm: number;
  /** Its phase template, which carries the law's Lommel–Seeliger share L. */
  readonly template: PhaseTemplateId;
  /**
   * The V phase integral q_V of its law at s = 1 (clamped and held, Design note 5), computed by
   * this plan's `phaseIntegral` and stated to three decimals.
   */
  readonly qV: number;
}

/**
 * The eight planets' photometry (Design note 5's table).
 *
 * @remarks
 * Mallama, Krobusek and Pavlov, "Comprehensive wide-band magnitudes and albedos for the planets,
 * with applications to exo-planets and Planet Nine", Icarus 282 (2017) 19, arXiv:1609.05048,
 * Tables 3 and 7; templates and their zeroth-order terms from Mallama and Hilton, Astronomy and
 * Computing 25 (2018) 10, arXiv:1808.01973. Mercury's Table 3 V(1, 0) of −0.69 is the 2017 value;
 * the 2018 paper's polynomial is anchored at −0.613 (its §3.1). Saturn's is the globe-only template.
 */
export const SOLAR_SYSTEM_PHOTOMETRY: ReadonlyArray<PlanetPhotometry> = [
  {
    name: "Mercury",
    geometricAlbedo: { b: 0.105, v: 0.142, r: 0.172 },
    bMinusVMag: 0.97,
    vMinusRMag: 0.75,
    v10Mag: -0.69,
    templateV10Mag: -0.613,
    radiusKm: 2439.7,
    template: "mercury",
    qV: 0.48,
  },
  {
    name: "Venus",
    geometricAlbedo: { b: 0.658, v: 0.689, r: 0.708 },
    bMinusVMag: 0.7,
    vMinusRMag: 0.57,
    v10Mag: -4.38,
    templateV10Mag: -4.384,
    radiusKm: 6051.8,
    template: "venus",
    qV: 1.344,
  },
  {
    name: "Earth",
    // Earth's measured p_V, 0.434, only happens to resemble log₁₀ e.
    // oxlint-disable-next-line approx-constant
    geometricAlbedo: { b: 0.512, v: 0.434, r: 0.418 },
    bMinusVMag: 0.47,
    vMinusRMag: 0.5,
    v10Mag: -3.99,
    templateV10Mag: -3.99,
    radiusKm: 6371.0,
    template: "earth",
    qV: 1.311,
  },
  {
    name: "Mars",
    geometricAlbedo: { b: 0.088, v: 0.17, r: 0.288 },
    bMinusVMag: 1.36,
    vMinusRMag: 1.11,
    v10Mag: -1.6,
    templateV10Mag: -1.601,
    radiusKm: 3386,
    template: "mars",
    qV: 1.085,
  },
  {
    name: "Jupiter",
    geometricAlbedo: { b: 0.443, v: 0.538, r: 0.495 },
    bMinusVMag: 0.86,
    vMinusRMag: 0.45,
    v10Mag: -9.4,
    templateV10Mag: -9.395,
    radiusKm: 69134,
    template: "jupiter",
    qV: 1.312,
  },
  {
    name: "Saturn",
    geometricAlbedo: { b: 0.339, v: 0.499, r: 0.568 },
    bMinusVMag: 1.07,
    vMinusRMag: 0.68,
    v10Mag: -8.91,
    templateV10Mag: -8.95,
    radiusKm: 57240,
    template: "saturn",
    qV: 1.357,
  },
  {
    name: "Uranus",
    geometricAlbedo: { b: 0.561, v: 0.488, r: 0.202 },
    bMinusVMag: 0.5,
    vMinusRMag: -0.42,
    v10Mag: -7.11,
    templateV10Mag: -7.11,
    radiusKm: 25264,
    template: "uranus",
    qV: 1.302,
  },
  {
    name: "Neptune",
    geometricAlbedo: { b: 0.562, v: 0.442, r: 0.181 },
    bMinusVMag: 0.39,
    vMinusRMag: -0.44,
    v10Mag: -6.94,
    templateV10Mag: -7.0,
    radiusKm: 24552,
    template: "neptune",
    qV: 1.242,
  },
];

/** The Sun's Johnson magnitudes at 1 au, Mallama et al. 2017, Table 6 (Livingston 2001). */
export const SUN_JOHNSON_MAG: JohnsonBvr = { b: -26.1, v: -26.75, r: -27.29 };

/**
 * A lit body's appearance for the shading tests (R07.T5): an Earth-sized sphere with the
 * provisional photometry, labelled, on the disc regime unless overridden.
 */
export function aLitBody(overrides: Partial<BodyAppearance> = {}): BodyAppearance {
  return {
    body: "0200080020000000.0300",
    figure: { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null },
    photometry: PROVISIONAL_PHOTOMETRY,
    regime: "disc",
    labels: ["BODY PHOTOMETRY: NOT YET MODELLED"],
    ...overrides,
  };
}

/**
 * A body's photometry from p and q per display channel (r, g, b) and its template, as plan 14's
 * section will carry it: the law `lawFor` solves, `modelled`.
 */
export function photometryFor(p: Rgb, q: Rgb, template: PhaseTemplateId): BodyPhotometry {
  return {
    geometricAlbedo: p,
    phaseIntegral: q,
    law: lawFor(p, q, template),
    bondRatioCheck: null,
    provenance: "modelled",
  };
}

/**
 * A planet's photometry from {@link SOLAR_SYSTEM_PHOTOMETRY}: its p in B, V and R as the display's
 * b, g and r, its q_V in every channel, and its template.
 *
 * @throws Error if the table has no planet of that name.
 */
export function planetPhotometry(name: string): BodyPhotometry {
  const planet = SOLAR_SYSTEM_PHOTOMETRY.find((each) => each.name === name);
  if (planet === undefined) {
    throw new Error(`no planet ${name} in SOLAR_SYSTEM_PHOTOMETRY`);
  }
  const { b, v, r } = planet.geometricAlbedo;
  return photometryFor([r, v, b], [planet.qV, planet.qV, planet.qV], planet.template);
}

/**
 * The Moon's visual geometric albedo, 0.12 (NASA GSFC, Moon Fact Sheet, D. R. Williams), in every
 * channel. The sheet's full Moon, V = −12.74 among its mean values at opposition (−12.73 in
 * Krisciunas and Schaefer 1991, PASP 103, 1033, eq. 9), taken at the mean distance of 384,400 km,
 * is a V(1, 0) of +0.21 at 1 au (derived here, as Allen's), which gives p_V = 0.121 against the
 * Sun's −26.76. Both leave out the opposition surge, about 35% at exactly full (Krisciunas and
 * Schaefer 1991, p. 1035); the sheet's own V(1, 0) of −0.08 includes it.
 */
export const MOON_GEOMETRIC_ALBEDO = 0.12;
