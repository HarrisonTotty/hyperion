/**
 * The atoms' and ions' k-distributions of R08.T4.e (plan R08, R08.T4.e; decision-r08-t4ef-storage
 * and its addendum of 2026-10-19): their lines from R. L. Kurucz's lists (`atomicLines.ts`'s
 * parser), each line's strength in LTE and its Voigt profile at a node of temperature and
 * broadening pressure, as a {@link NodeSpectra} that R08.T4.d's reduction reads, on E1's or E1′'s
 * temperature ladder.
 *
 * @remarks
 * - **Strengths.** A line's strength per absorber of its stage is π r_e gf e^(−c₂E_low ÷ T)
 *   (1 − e^(−c₂ν ÷ T)) ÷ Q(T), cm, Boltzmann's populations with Barklem and Collet 2016's partition
 *   functions (`atomicLines.ts`'s {@link partitionFunctionAt}), stimulated emission counted. The
 *   column is the record's: ionisation is the generator's, and no Saha factor is applied.
 * - **The reference broadener** is H₂ (decision-r08-t4ef-storage §3, "the broadening axis"): a
 *   table's pressure is the pressure of pure H₂ that gives the lines their van der Waals widths, at
 *   the node's temperature. Each line's width per perturber is Barklem, Piskunov and O'Mara 2000's
 *   ABO value for neutral hydrogen where their table holds the line ({@link parseAboTable}), else
 *   Kurucz's own van der Waals field, else Unsöld's per perturber (`atomicLines.ts`'s
 *   {@link vanDerWaalsWidthPerS}, which bounds a neutral atom's widths from Rydberg levels,
 *   decision-r08-t4e-unsold); the H-perturber widths are taken to H₂ and He by Unsöld's scaling
 *   ({@link perturberScale}), C₆ ∝ α_p, so σ ∝ α_p^(2/5), at each perturber's reduced mass.
 * - **Radiative damping** is Kurucz's Γ_rad where the record gives it, else the classical γ_cl.
 *   Stark broadening of the metals' lines is left out: at 4,600 K and 0.1 bar in the reference
 *   column's balance their quadratic Stark widths are about 1% of the van der Waals widths.
 * - **The shape** is T4.d's: Voigt within 25 cm⁻¹ of each line, no pedestal, `estimated` where the
 *   cell's S-weighted γ_L passes 0.25 cm⁻¹. Na D's and K's far wings beyond 25 cm⁻¹ are their pairs'
 *   (`Na+H2`, `Na+He`, `K+H2`, `K+He`; follow-up 5, ruling 2), so nothing is counted twice.
 */

import { gunzipSync } from "node:zlib";

import {
  type AtomicStage,
  BOLTZMANN_J_PER_K,
  classicalDampingPerS,
  ELECTRON_RADIUS_CM,
  type KuruczLine,
  linesInWindow,
  PERTURBER_POLARISABILITIES_A03,
  vanDerWaalsWidthPerS,
} from "./atomicLines";
import { cellNodeOf, lineShape, type NodeSpectra, type ShapePolicy } from "./kDistributions";
import {
  cellPointCount,
  cellSpectrum,
  cellWavenumbersPerCm,
  dopplerHalfWidthPerCm,
  LINE_CUTOFF_PER_CM,
  type LineState,
  linesNear,
  SECOND_RADIATION_CONSTANT_CM_K,
  SPEED_OF_LIGHT_M_PER_S,
} from "./molecularLines";

// --- The temperature ladders: E1 (decision-r08-t4ef-storage §5.3) and E1′ (decision-r08-t4e-budget §3) ---

/** The top of R08.T4.e's range, K: above it the 5,000 K node is held, `estimated`. */
export const LADDER_TOP_K = 5_000;

/**
 * The temperature ladders a table may take: E1's (50 K steps to 1,000 K, 100 K to 2,000 K and
 * 200 K beyond) or E1′'s, the 10% ladder (decision-r08-t4e-budget §3, amending E1), whose steps from
 * 1,000 K are each the largest multiple of 50 K at most 10% of its lower node. Each stage takes the
 * one whose projected bytes after refinement are fewer.
 */
export type TemperatureLadder = "E1" | "E1prime";

/** A ladder's step from a node, K: below 1,000 K, 50 K on either; no step passes 10% of its node. */
export function ladderStepK(lowerK: number, ladder: TemperatureLadder = "E1"): number {
  if (lowerK < 1_000) {
    return 50;
  }
  if (ladder === "E1prime") {
    return Math.max(50, Math.floor((0.1 * lowerK) / 50 + 1e-9) * 50);
  }
  return lowerK < 2_000 ? 100 : 200;
}

/**
 * A ladder's nodes from a first node to the range's top, K: every node at or above `fromK`, the
 * ladder running from 50 K in its steps ({@link ladderStepK}), and the top the last node (on E1′,
 * 4,700 K is followed by 5,000 K).
 */
export function ladderTemperatures(
  fromK: number,
  toK = LADDER_TOP_K,
  ladder: TemperatureLadder = "E1",
): number[] {
  const nodes: number[] = [];
  for (let t = 50; t < toK - 1e-9; t += ladderStepK(t, ladder)) {
    if (t >= fromK - 1e-9) {
      nodes.push(t);
    }
  }
  nodes.push(toK);
  return nodes;
}

/**
 * The finest step a failing interval refines to, K: a quarter of E1's step there, 12.5 K below
 * 1,000 K, 25 K to 2,000 K and 50 K beyond. E1 reaches it by bisecting twice, E1′ by bisecting up to
 * three times (decision-r08-t4e-budget §3); an interval still failing then is `estimated`.
 */
export function ladderFinestStepK(lowerK: number): number {
  return ladderStepK(lowerK, "E1") / 4;
}

// --- Barklem, Piskunov and O'Mara 2000's ABO widths ---

/**
 * One line of P. S. Barklem, N. Piskunov and B. J. O'Mara, Astron. Astrophys. Suppl. Ser. 142
 * (2000) 467, Table 1 (CDS J/A+AS/142/467, `table1.dat.gz`): the ABO theory's broadening of a
 * metallic line by neutral hydrogen.
 */
export interface AboLine {
  /** The element and stage, Z + charge ÷ 100 (26.00 for Fe I, 20.01 for Ca II). */
  readonly spectrumCode: number;
  /** The air wavelength, nm. */
  readonly airNm: number;
  readonly lowerJ: number;
  readonly upperJ: number;
  /** log₁₀ of the Lorentzian full width per hydrogen atom at 10,000 K, rad s⁻¹ cm³. */
  readonly logGammaPerHydrogen: number;
  /** The width's temperature exponent, (1 − α) ÷ 2, α the cross-section's velocity exponent. */
  readonly temperatureExponent: number;
}

/**
 * The ABO table's lines (its ReadMe's byte-by-byte description: spec 1–5, λ 7–16 Å, J 18–21 and
 * 23–26, logGam 105–110, Texp 112–116).
 *
 * @throws Error for a record whose fields are not numbers.
 */
export function parseAboTable(text: string): ReadonlyArray<AboLine> {
  const lines: AboLine[] = [];
  for (const [i, raw] of text.split(/\r?\n/u).entries()) {
    if (raw.trim().length === 0) {
      continue;
    }
    const number = (from: number, to: number, what: string): number => {
      const value = Number(raw.slice(from, to).trim());
      if (!Number.isFinite(value) || raw.slice(from, to).trim().length === 0) {
        throw new Error(`ABO table record ${i + 1}: its ${what} is not a number`);
      }
      return value;
    };
    lines.push({
      spectrumCode: number(0, 5, "spectrum"),
      airNm: number(6, 16, "wavelength") / 10,
      lowerJ: number(17, 21, "lower J"),
      upperJ: number(22, 26, "upper J"),
      logGammaPerHydrogen: number(104, 110, "log Gamma"),
      temperatureExponent: number(111, 116, "temperature exponent"),
    });
  }
  return lines;
}

/** The ABO table's gzipped bytes as its text. */
export function aboText(gzipped: Uint8Array): string {
  return gunzipSync(gzipped).toString("ascii");
}

/** How far a Kurucz line's air wavelength may lie from an ABO line's and match it, nm (0.02 Å). */
export const ABO_MATCH_NM = 0.002;

/** A line's van der Waals width per perturber: where it comes from, and its parameters. */
export type VanDerWaalsSource =
  /** Barklem et al. 2000's ABO width per H atom at 10,000 K and its exponent. */
  | { readonly kind: "abo"; readonly logGammaPerHydrogen: number; readonly exponent: number }
  /** Kurucz's own field, per H atom at 10,000 K, with the classical exponent 0.3. */
  | { readonly kind: "kurucz"; readonly logGammaPerHydrogen: number }
  /** Unsöld's hydrogenic C₆ (`atomicLines.ts`'s {@link vanDerWaalsWidthPerS}). */
  | { readonly kind: "unsold" }
  /**
   * A Balmer line's self-broadening by H atoms (decision-r08-t4e-budget §8): Barklem, Piskunov and
   * O'Mara 2000b's p–d component as a width per H atom at 10,000 K and its exponent
   * ({@link selfBroadeningSource}); by H₂ and He the line keeps Unsöld's width, since the
   * self-broadening holds the resonance interaction between like atoms.
   */
  | {
      readonly kind: "selfBroadening";
      readonly logGammaPerHydrogen: number;
      readonly exponent: number;
    }
  /**
   * Na D's impact width by H₂ from Allard et al. 2019's ab initio tables ({@link NA_D_H2_WIDTHS}),
   * which the ABO width scaled to H₂ by Unsöld's rule passes 1.25–1.37 times for D2 and 1.64–1.74
   * times for D1 over 500–3,000 K; by H the line's own ABO width, ab initio for H atoms; by He and
   * the air gases Allard's H₂ width scaled by Unsöld's rule, `estimated` (no open Na–He set).
   */
  | {
      readonly kind: "allardH2";
      /** Each of {@link NA_D_H2_WIDTHS}' half widths, cm⁻¹, at its reference density. */
      readonly hwhmPerCm: ReadonlyArray<number>;
      readonly otherwise: AboSource;
    };

/** An ABO width source alone: {@link VanDerWaalsSource}'s `abo` variant. */
export type AboSource = Extract<VanDerWaalsSource, { readonly kind: "abo" }>;

/**
 * Na D's impact widths by H₂: N. F. Allard et al., Astron. Astrophys. 628 (2019) A120 (CDS
 * J/A+A/628/A120, `opacity.tar.gz`, SHA-256 b0c73fe0…fdf791; tables 2017), each table's data header
 * (its README's "Line 12", the impact width and shift, at the table's n_H2 = 10²¹ cm⁻³): D2's
 * `tableD2_NaH2_<T>_1e21_FS17.omg` and D1's `table_nearwing_<T>.omg` (at 1,000 K
 * `tableD1_NaH2_1000_1e21_FS17_nearwing.omg`), read 2026-10-19 as data. Half widths at half
 * maximum, cm⁻¹: the deposit's `lect_sig.f` (read as data, not run) builds its impact core as a
 * Lorentzian of this half width, and their Fig. 6's caption names it the half width (the text
 * before their Eq. 7 says full width). D1's match the paper's Eq. 7, w = 0.169 × 10⁻²⁰ n_H2 T^0.33,
 * within 3–5%; D2's lie within 2.5% of Fig. 6's label for it, 0.242 × 10⁻²⁰ n_H2 T^0.32, and
 * 1.5–1.7 times under Eq. 8 as printed (T^0.39), so the tables' own are taken. Their shifts (D2
 * +1.3 to +2.7, D1 −8.2 to −12.7 cm⁻¹ at 10²¹ cm⁻³) are left out: at 10¹⁹ cm⁻³ they are under
 * 0.13 cm⁻¹.
 */
export const NA_D_H2_WIDTHS = {
  referencePerCm3: 1e21,
  temperaturesK: [500, 600, 725, 1_000, 1_500, 2_000, 2_500, 3_000],
  d2HwhmPerCm: [18.13, 18.905, 19.961, 21.982, 24.566, 27.841, 29.728, 31.794],
  d1HwhmPerCm: [13.788, 14.502, 15.331, 16.984, 19.443, 21.513, 23.258, 24.931],
} as const;

/**
 * A full width per H₂ from {@link NA_D_H2_WIDTHS}' half widths, rad s⁻¹ m³: ln w linear in ln T
 * between its temperatures, and so from its end intervals beyond (a power law, T^0.37 above
 * 3,000 K for D2 and T^0.38 for D1, extrapolated to the table's 5,000 K and recorded), per
 * perturber at its reference density.
 *
 * @throws Error for half widths that are not one a temperature of the table.
 */
export function allardH2WidthM3PerS(
  hwhmPerCm: ReadonlyArray<number>,
  temperatureK: number,
): number {
  const temps = NA_D_H2_WIDTHS.temperaturesK;
  if (hwhmPerCm.length !== temps.length) {
    throw new Error(
      `Allard et al.'s widths are one a temperature (${temps.length}), not ${hwhmPerCm.length}`,
    );
  }
  let i = 0;
  while (i < temps.length - 2 && (temps[i + 1] ?? Infinity) < temperatureK) {
    i += 1;
  }
  const [t0, t1, w0, w1] = [temps[i], temps[i + 1], hwhmPerCm[i], hwhmPerCm[i + 1]];
  if (t0 === undefined || t1 === undefined || w0 === undefined || w1 === undefined) {
    throw new Error(`Allard et al.'s widths hold no interval ${i}`);
  }
  const x0 = Math.log(t0);
  const x1 = Math.log(t1);
  const y0 = Math.log(w0);
  const y1 = Math.log(w1);
  const hwhm = Math.exp(y0 + ((y1 - y0) * (Math.log(temperatureK) - x0)) / (x1 - x0));
  // A half width in cm⁻¹ per cm⁻³ as an angular full width per m⁻³: twice 2πc, cm s⁻¹, and 10⁻⁶ m³
  // a cm³.
  return (
    ((hwhm / NA_D_H2_WIDTHS.referencePerCm3) * 4 * Math.PI * SPEED_OF_LIGHT_M_PER_S * 100) /
    CM3_PER_M3
  );
}

/** The Bohr radius, m (CODATA 2018). */
const BOHR_RADIUS_M = 5.291_772_109_03e-11;

/** cm³ in a m³: a width per perturber in cm³ s⁻¹ over this is one in m³ s⁻¹. */
export const CM3_PER_M3 = 1e6;

/** The atomic mass unit, kg (CODATA 2018). */
const ATOMIC_MASS_KG = 1.660_539_066_60e-27;

/** The reference speed of the ABO cross-sections, m s⁻¹ (Anstee and O'Mara 1995's v₀). */
const ABO_REFERENCE_SPEED_M_PER_S = 1e4;

/**
 * Γ(x) for x > 0 by Lanczos's series (C. Lanczos, J. SIAM Numer. Anal. B 1 (1964) 86; g = 7 and
 * nine coefficients, as P. Godfrey gives them, relative error near 10⁻¹⁵), with Γ(x) = Γ(x + 1) ÷ x
 * below 0.5.
 */
export function gammaFunction(x: number): number {
  if (x < 0.5) {
    return gammaFunction(x + 1) / x;
  }
  const c = [
    0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1,
    -176.615_029_162_140_6, 12.507_343_278_686_905, -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6, 1.505_632_735_149_311_6e-7,
  ];
  const z = x - 1;
  let sum = c[0] ?? 1;
  for (let k = 1; k < c.length; k += 1) {
    sum += (c[k] ?? 0) / (z + k);
  }
  const t = z + 7.5;
  return Math.sqrt(2 * Math.PI) * t ** (z + 0.5) * Math.exp(-t) * sum;
}

/**
 * An ABO cross-section σ(v₀) and velocity parameter α as a Lorentzian full width per perturber at
 * 10,000 K, log₁₀ rad s⁻¹ cm³, and its temperature exponent (1 − α) ÷ 2: Γ = 2 (4/π)^(α/2)
 * Γ((4 − α)/2) v̄ σ(v₀) (v̄ ÷ v₀)^(−α), v̄ = √(8kT ÷ πμ) (P. S. Anstee and B. J. O'Mara, Mon. Not. R.
 * Astron. Soc. 276 (1995) 859, Eq. 3, as Barklem, Piskunov and O'Mara 2000's Table 1 gives its
 * widths: from its σ and α this reproduces its log Γ within 0.0015 dex over its 4,028 lines of the
 * twelve elements R08.T4.e draws, Na, Mg, Al, Si, K, Ca, Ti, V, Cr, Mn, Fe and Ni, R08.T4.e's
 * check).
 *
 * @param sigmaA02 - σ at v₀ = 10⁴ m s⁻¹, a₀².
 * @param reducedMassDa - The pair's reduced mass, Da.
 */
export function aboWidth(
  sigmaA02: number,
  alpha: number,
  reducedMassDa: number,
): { readonly logGammaPerHydrogen: number; readonly exponent: number } {
  const meanSpeed = Math.sqrt(
    (8 * BOLTZMANN_J_PER_K * 1e4) / (Math.PI * reducedMassDa * ATOMIC_MASS_KG),
  );
  const halfWidthM3PerS =
    (4 / Math.PI) ** (alpha / 2) *
    gammaFunction((4 - alpha) / 2) *
    meanSpeed *
    sigmaA02 *
    BOHR_RADIUS_M ** 2 *
    (meanSpeed / ABO_REFERENCE_SPEED_M_PER_S) ** -alpha;
  return {
    logGammaPerHydrogen: Math.log10(2 * halfWidthM3PerS * CM3_PER_M3),
    exponent: (1 - alpha) / 2,
  };
}

/**
 * Barklem, Piskunov and O'Mara 2000b's p–d component of the Balmer lines' self-broadening (Astron.
 * Astrophys. 363 (2000) 1091, Table 3; open copy arXiv astro-ph/0010022, read 2026-10-19): σ at
 * 10⁴ m s⁻¹, a₀², and α, by the upper level n. Their impact limits, 35.0, 13.2 and 7.7 Å, each lie
 * beyond the 25 cm⁻¹ shape (10.8, 5.9 and 4.7 Å), but their text holds the impact approximation
 * secure only to about a fifth of the limit, 7.0 Å for Hα and 2.6 Å for Hβ near 5,000 K, so the
 * shape's outer part lies past it, as H's `estimated` table allows. The members above Hγ take Hγ's,
 * a lower bound on their widths, since σ grows with the upper level (decision-r08-t4e-budget §8).
 */
export const BALMER_SELF_BROADENING: ReadonlyArray<{
  readonly upperN: number;
  readonly sigmaA02: number;
  readonly alpha: number;
}> = [
  { upperN: 3, sigmaA02: 1_180, alpha: 0.677 },
  { upperN: 4, sigmaA02: 2_320, alpha: 0.455 },
  { upperN: 5, sigmaA02: 4_208, alpha: 0.38 },
];

/** H's mass, Da (standard atomic weight), whose self-broadening pair takes half of it. */
const HYDROGEN_MASS_DA = 1.008;

/**
 * A Balmer line's self-broadening source ({@link BALMER_SELF_BROADENING}): its upper level's σ and
 * α, Hγ's for the members above, at the H–H reduced mass.
 */
export function selfBroadeningSource(upperN: number): VanDerWaalsSource {
  const row =
    BALMER_SELF_BROADENING.find((r) => r.upperN === upperN) ?? BALMER_SELF_BROADENING.at(-1);
  if (row === undefined) {
    throw new Error("no Balmer self-broadening row");
  }
  return {
    kind: "selfBroadening",
    ...aboWidth(row.sigmaA02, row.alpha, HYDROGEN_MASS_DA / 2),
  };
}

/** The perturbers whose widths the tables carry: the reference H₂, and He and H by their ratios. */
export type Perturber = "H2" | "He" | "H";

/**
 * Every gas a table states a width ratio for (its sidecar's `broadening`): the {@link Perturber}s,
 * and N₂, O₂ and Ar, which the client would otherwise take at ratio 1, as it takes air for a
 * molecular table, unlabelled: for an atom's table, whose reference broadener is H₂, at H₂'s width
 * (`absorbers/kDistribution.ts`'s `broadeningFactor`). Any other gas a table gives no ratio for
 * (CO₂, H₂O, …) the client takes at ratio 1 too, `estimated`.
 */
export type BroadeningGas = Perturber | "N2" | "O2" | "Ar";

/** The broadening gases' masses, Da (standard atomic weights). */
const PERTURBER_MASSES_DA: Readonly<Record<BroadeningGas, number>> = {
  H: 1.008,
  H2: 2.016,
  He: 4.0026,
  N2: 28.0134,
  O2: 31.9988,
  Ar: 39.948,
};

/**
 * The broadening gases' static dipole polarisabilities, a₀³: T4.h's for H, H₂ and He
 * (`atomicLines.ts`'s {@link PERTURBER_POLARISABILITIES_A03}), and N₂'s 1.710, O₂'s 1.562 and Ar's
 * 1.664 Å³, NIST CCCBDB's experimental list (read 2026-10-20), from T. N. Olney, N. M. Cann, G.
 * Cooper and C. E. Brion, Chem. Phys. 223 (1997) 59, at 0.148 184 7 Å³ a₀⁻³.
 *
 * @remarks
 * Two methods meet here: H₂'s and He's are Lorentz–Lorenz on the client's dispersions, the air
 * gases' Olney et al.'s. Lorentz–Lorenz on the client's own N₂, O₂ and Ar dispersions gives 1.7%,
 * 0.4% and −1.3% from Olney's, and Olney's H₂ is 2.2% under T4.h's (the science check's figures),
 * which move the width ratios by 0.7% or less through α^(2/5).
 */
const POLARISABILITIES_A03: Readonly<Record<BroadeningGas, number>> = {
  ...PERTURBER_POLARISABILITIES_A03,
  N2: 1.71 / 0.148_184_7,
  O2: 1.562 / 0.148_184_7,
  Ar: 1.664 / 0.148_184_7,
};

/**
 * Dry air's broadening gases by volume, normalised over them (US Standard Atmosphere 1976's Table
 * 3: 0.780 84 N₂, 0.209 476 O₂ and 0.009 34 Ar), for the ratio the client takes for `air`.
 */
export const AIR_GASES_BY_VOLUME: Readonly<Partial<Record<BroadeningGas, number>>> = {
  N2: 0.780_84 / 0.999_656,
  O2: 0.209_476 / 0.999_656,
  Ar: 0.009_34 / 0.999_656,
};

/** The classical van der Waals width's temperature exponent, Γ₆ ∝ v̄^(3/5) ∝ T^0.3 per perturber. */
export const UNSOLD_TEMPERATURE_EXPONENT = 0.3;

/**
 * A perturber's width over an H atom's for the same line, by Unsöld's scaling: C₆ ∝ α_p, so the
 * impact cross-section goes as α_p^(2/5), and Γ ∝ v̄^(2n) σ at the pair's mean speed, μ^(−n), n the
 * width's temperature exponent: (α_p ÷ α_H)^(2/5) (μ_H ÷ μ_p)^n.
 */
export function perturberScale(
  perturber: BroadeningGas,
  atomMassDa: number,
  exponent: number,
): number {
  const reduced = (m: number): number => (atomMassDa * m) / (atomMassDa + m);
  return (
    (POLARISABILITIES_A03[perturber] / POLARISABILITIES_A03.H) ** 0.4 *
    (reduced(PERTURBER_MASSES_DA.H) / reduced(PERTURBER_MASSES_DA[perturber])) ** exponent
  );
}

// --- A stage's lines ---

/** A drawn stage's lines over the window, ready for its spectra. */
export interface AtomicLineSet {
  readonly stage: AtomicStage;
  /** The atom's or ion's mass, Da. */
  readonly massDa: number;
  /** The stage's ionisation limit, eV. */
  readonly limitEv: number;
  /** Its lines over 360–780 nm (vacuum) from levels below the limit, ascending in wavenumber. */
  readonly lines: ReadonlyArray<KuruczLine>;
  /** Their vacuum wavenumbers, cm⁻¹, ascending. */
  readonly wavenumbersPerCm: Float64Array;
  /** Each line's van der Waals source. */
  readonly vanDerWaals: ReadonlyArray<VanDerWaalsSource>;
  /** Q(T) of the stage. */
  readonly partitionFunction: (temperatureK: number) => number;
  /**
   * A line's Stark half width at half maximum at a node, cm⁻¹, added to its Lorentzian: H I's
   * Balmer lines' (`balmerStark.ts`, at the node's `estimated` electron density); none for the
   * metals, whose quadratic Stark widths are about 1% of their van der Waals widths.
   */
  readonly starkHalfWidthPerCm?: (
    index: number,
    temperatureK: number,
    pressurePa: number,
  ) => number;
}

/** A stage's code in the ABO table: Z + charge ÷ 100. */
function aboCode(atomicNumber: number, stage: 0 | 1): number {
  return atomicNumber + stage / 100;
}

/**
 * A stage's lines for its tables: every line of its list over 360–780 nm by vacuum wavelength from a
 * level below the ionisation limit (`atomicLines.ts`'s {@link linesInWindow}), ascending in
 * wavenumber, each with its van der Waals source: the ABO table's line of the same stage, J's and air
 * wavelength within {@link ABO_MATCH_NM}, nearest first; else Kurucz's field where the record gives
 * one; else Unsöld's.
 */
export function atomicLineSet(
  stage: AtomicStage,
  atomicNumber: number,
  massDa: number,
  limitEv: number,
  lines: ReadonlyArray<KuruczLine>,
  abo: ReadonlyArray<AboLine>,
  partitionFunction: (temperatureK: number) => number,
): AtomicLineSet {
  const code = aboCode(atomicNumber, stage.stage);
  const own = abo
    .filter((a) => Math.abs(a.spectrumCode - code) < 1e-6)
    .toSorted((a, b) => a.airNm - b.airNm);
  const sorted = linesInWindow(lines, limitEv).toSorted((a, b) => b.vacuumNm - a.vacuumNm);
  const vanDerWaals = sorted.map((line): VanDerWaalsSource => {
    let best: AboLine | undefined;
    if (line.listedNm >= 200) {
      for (const a of own) {
        if (
          Math.abs(a.airNm - line.listedNm) <= ABO_MATCH_NM &&
          a.lowerJ === line.lowerJ &&
          a.upperJ === line.upperJ &&
          (best === undefined ||
            Math.abs(a.airNm - line.listedNm) < Math.abs(best.airNm - line.listedNm))
        ) {
          best = a;
        }
      }
    }
    if (best !== undefined) {
      return {
        kind: "abo",
        logGammaPerHydrogen: best.logGammaPerHydrogen,
        exponent: best.temperatureExponent,
      };
    }
    return line.logGammaVanDerWaalsPerHydrogen !== 0
      ? { kind: "kurucz", logGammaPerHydrogen: line.logGammaVanDerWaalsPerHydrogen }
      : { kind: "unsold" };
  });
  return {
    stage,
    massDa,
    limitEv,
    lines: sorted,
    wavenumbersPerCm: Float64Array.from(sorted, (l) => 1e7 / l.vacuumNm),
    vanDerWaals,
    partitionFunction,
  };
}

/**
 * A line's van der Waals width per perturber at a temperature, rad s⁻¹ m³ (the Lorentzian's full
 * width in angular frequency over the perturber's number density).
 */
export function vanDerWaalsWidthM3PerS(
  set: AtomicLineSet,
  index: number,
  perturber: Perturber,
  temperatureK: number,
): number {
  const source = set.vanDerWaals[index];
  const line = set.lines[index];
  if (source === undefined || line === undefined) {
    throw new Error(`no line ${index} in ${set.stage.key}'s set`);
  }
  return sourceWidth(set, line, source, perturber, temperatureK);
}

/** A van der Waals source's width per perturber for a line, rad s⁻¹ m³. */
function sourceWidth(
  set: AtomicLineSet,
  line: KuruczLine,
  source: VanDerWaalsSource,
  perturber: Perturber,
  temperatureK: number,
): number {
  let width: number;
  switch (source.kind) {
    case "allardH2":
      // H takes the line's own ABO width, an ab initio one for H atoms; He is scaled from the ab
      // initio H₂ width by Unsöld's rule, not from H's (which scaled to H₂ is 1.25–1.74 times
      // Allard's).
      width =
        perturber === "H"
          ? sourceWidth(set, line, source.otherwise, perturber, temperatureK)
          : allardH2WidthM3PerS(source.hwhmPerCm, temperatureK) *
            (perturber === "H2"
              ? 1
              : perturberScale(perturber, set.massDa, source.otherwise.exponent) /
                perturberScale("H2", set.massDa, source.otherwise.exponent));
      break;
    case "abo":
      width =
        ((10 ** source.logGammaPerHydrogen * (temperatureK / 1e4) ** source.exponent) /
          CM3_PER_M3) *
        perturberScale(perturber, set.massDa, source.exponent);
      break;
    case "kurucz":
      width =
        ((10 ** source.logGammaPerHydrogen * (temperatureK / 1e4) ** UNSOLD_TEMPERATURE_EXPONENT) /
          CM3_PER_M3) *
        perturberScale(perturber, set.massDa, UNSOLD_TEMPERATURE_EXPONENT);
      break;
    case "unsold":
      width = unsoldPerPerturber(set, line, perturber, temperatureK);
      break;
    case "selfBroadening":
      width =
        perturber === "H"
          ? (10 ** source.logGammaPerHydrogen * (temperatureK / 1e4) ** source.exponent) /
            CM3_PER_M3
          : unsoldPerPerturber(set, line, perturber, temperatureK);
      break;
  }
  return width;
}

/** A line's Unsöld width per perturber, rad s⁻¹ m³ (`atomicLines.ts`'s {@link vanDerWaalsWidthPerS}). */
function unsoldPerPerturber(
  set: AtomicLineSet,
  line: KuruczLine,
  perturber: Perturber,
  temperatureK: number,
): number {
  return vanDerWaalsWidthPerS(
    line,
    set.limitEv,
    set.stage.stage + 1,
    set.massDa,
    temperatureK,
    {
      H: perturber === "H" ? 1 : 0,
      H2: perturber === "H2" ? 1 : 0,
      He: perturber === "He" ? 1 : 0,
    },
    set.stage.symbol === "H",
  );
}

/**
 * A line's van der Waals width per H atom as N₂'s, O₂'s and Ar's are scaled from it, rad s⁻¹ m³:
 * its own, but Unsöld's for a Balmer line's self-broadening, which holds the resonance interaction
 * between like atoms alone.
 */
function hydrogenBaseWidth(set: AtomicLineSet, index: number, temperatureK: number): number {
  const line = set.lines[index];
  if (line === undefined) {
    throw new Error(`no line ${index} in ${set.stage.key}'s set`);
  }
  return set.vanDerWaals[index]?.kind === "selfBroadening"
    ? unsoldPerPerturber(set, line, "H", temperatureK)
    : vanDerWaalsWidthM3PerS(set, index, "H", temperatureK);
}

/** A line's radiative damping, s⁻¹: Kurucz's Γ_rad where given, else the classical γ_cl. */
export function radiativeDampingPerS(line: KuruczLine): number {
  return line.logGammaRadPerS !== 0
    ? 10 ** line.logGammaRadPerS
    : classicalDampingPerS(line.vacuumNm);
}

/** A Lorentzian full width in angular frequency, s⁻¹, as a half-width at half maximum, cm⁻¹. */
function halfWidthPerCm(gammaPerS: number): number {
  return gammaPerS / (4 * Math.PI * SPEED_OF_LIGHT_M_PER_S * 100);
}

/**
 * A line's strength per absorber of its stage in LTE at a temperature, cm:
 * π r_e gf e^(−c₂E_low ÷ T)(1 − e^(−c₂ν ÷ T)) ÷ Q(T).
 */
export function atomicLineStrengthCm(
  line: KuruczLine,
  temperatureK: number,
  partitionFunction: number,
): number {
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  const nu = 1e7 / line.vacuumNm;
  return (
    (Math.PI *
      ELECTRON_RADIUS_CM *
      10 ** line.logGf *
      Math.exp((-c2 * line.lowerPerCm) / temperatureK) *
      -Math.expm1((-c2 * nu) / temperatureK)) /
    partitionFunction
  );
}

/**
 * The lines' states at a temperature and a broadening pressure of the reference H₂, Pa: each line's
 * strength ({@link atomicLineStrengthCm}), its thermal Doppler half-width, and its Lorentzian
 * half-width from its radiative damping and its van der Waals width by H₂ at n = p ÷ kT.
 */
export function atomicLineStates(
  set: AtomicLineSet,
  indices: ReadonlyArray<number>,
  temperatureK: number,
  pressurePa: number,
): LineState[] {
  const q = set.partitionFunction(temperatureK);
  const molecules = pressurePa / (BOLTZMANN_J_PER_K * temperatureK);
  return indices.map((i) => {
    const line = set.lines[i];
    if (line === undefined) {
      throw new Error(`no line ${i} in ${set.stage.key}'s set`);
    }
    const nu = 1e7 / line.vacuumNm;
    return {
      intensityCm: atomicLineStrengthCm(line, temperatureK, q),
      dopplerPerCm: dopplerHalfWidthPerCm(nu, temperatureK, set.massDa),
      lorentzPerCm:
        halfWidthPerCm(
          radiativeDampingPerS(line) +
            molecules * vanDerWaalsWidthM3PerS(set, i, "H2", temperatureK),
        ) + (set.starkHalfWidthPerCm?.(i, temperatureK, pressurePa) ?? 0),
    };
  });
}

/**
 * The atoms' shape (T4.d's, decision-r08-t4d-kdist-chain.md ruling 1): Voigt within 25 cm⁻¹ of each
 * line, no pedestal, `estimated` past γ_L = 0.25 cm⁻¹; one variant, the reference broadener H₂
 * standing for every gas by its ratio (the sidecar's `air` slot).
 */
export const ATOMIC_SHAPE: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: LINE_CUTOFF_PER_CM,
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/** The intensity-weighted Lorentzian half-width of a set of line states, cm⁻¹. */
function meanLorentzPerCm(states: ReadonlyArray<LineState>): number {
  let weighted = 0;
  let total = 0;
  for (const s of states) {
    weighted += s.intensityCm * s.lorentzPerCm;
    total += s.intensityCm;
  }
  return total > 0 ? weighted / total : 0;
}

/**
 * The pressure whose lines set a cell's fine grid at every pressure above it, Pa: the first stored
 * node ({@link atomicSpectra}).
 */
export const FIXED_GRID_PA = 10;

/**
 * A stage's lines as a {@link NodeSpectra}, line by line under the shape ({@link ATOMIC_SHAPE}):
 * a cell's spectrum at a temperature and an H₂ broadening pressure, or `undefined` where no line
 * lies within the cut-off of the cell.
 *
 * @remarks
 * The fine grid of a cell at a temperature is the one its lines take at {@link FIXED_GRID_PA}
 * (their own below it), a quarter of the least Voigt half-width there, the same at every pressure
 * above, where the lines only broaden. On each pressure's own grid, as T4.d's line-by-line takes it,
 * a saturated wing's 25 cm⁻¹ cut-off lands on a grid that moves with pressure, by up to a step, and
 * the depth jitters: by up to 1% at 20–45 kPa in Mg I's 385 nm cell at 3,600 K and u = 2 × 10²⁵
 * cm⁻², where the chain test failed at a node's temperature between two standard rows (R08.T4.e's
 * re-projection). Fixed, the cut-off keeps its place and the depth is smooth in pressure.
 */
export function atomicSpectra(set: AtomicLineSet, policy: ShapePolicy = ATOMIC_SHAPE): NodeSpectra {
  const lines = { wavenumbersPerCm: set.wavenumbersPerCm };
  return (cellNm, temperatureK, pressurePa, weights) => {
    const shape = lineShape(policy, temperatureK, weights);
    const [low, high] = cellWavenumbersPerCm(cellNm);
    const indices = linesNear(lines, low, high, shape.cutoffPerCm);
    if (indices.length === 0) {
      return undefined;
    }
    const states = atomicLineStates(set, indices, temperatureK, pressurePa);
    const points = cellPointCount(
      cellNm,
      pressurePa > FIXED_GRID_PA
        ? atomicLineStates(set, indices, temperatureK, FIXED_GRID_PA)
        : states,
    );
    return cellNodeOf(
      cellNm,
      cellSpectrum(cellNm, { lines }, indices, states, points, shape).crossSectionsCm2,
      meanLorentzPerCm(states),
      shape,
      policy,
    );
  };
}

/**
 * The cells of the bake range a stage's lines reach, nm: those with a line within the cut-off.
 */
export function cellsWithLines(set: AtomicLineSet): number[] {
  const cells: number[] = [];
  for (let cellNm = 380; cellNm <= 760; cellNm += 1) {
    const [low, high] = cellWavenumbersPerCm(cellNm);
    if (linesNear({ wavenumbersPerCm: set.wavenumbersPerCm }, low, high).length > 0) {
      cells.push(cellNm);
    }
  }
  return cells;
}

/** What a line's N₂, O₂ and Ar widths are scaled from by Unsöld's rule ({@link perturberScale}). */
interface AirGasBasis {
  /** The width's temperature exponent. */
  readonly exponent: number;
  /** The width per perturber of the gas they are scaled from, rad s⁻¹ m³. */
  readonly anchor: number;
  readonly from: BroadeningGas;
}

/**
 * A line's {@link AirGasBasis}: its H width ({@link hydrogenBaseWidth}) at its source's exponent, or
 * for Na D its ab initio H₂ one.
 */
function airGasBasis(set: AtomicLineSet, index: number, temperatureK: number): AirGasBasis {
  const source = set.vanDerWaals[index];
  if (source === undefined) {
    throw new Error(`no line ${index} in ${set.stage.key}'s set`);
  }
  const fromHydrogen = (exponent: number): AirGasBasis => ({
    exponent,
    anchor: hydrogenBaseWidth(set, index, temperatureK),
    from: "H",
  });
  let basis: AirGasBasis;
  switch (source.kind) {
    case "abo":
      basis = fromHydrogen(source.exponent);
      break;
    case "allardH2":
      basis = {
        exponent: source.otherwise.exponent,
        anchor: vanDerWaalsWidthM3PerS(set, index, "H2", temperatureK),
        from: "H2",
      };
      break;
    case "kurucz":
    case "unsold":
    case "selfBroadening":
      basis = fromHydrogen(UNSOLD_TEMPERATURE_EXPONENT);
      break;
  }
  return basis;
}

/**
 * The perturbers' width ratios to H₂'s over a stage's lines (the sidecar's `broadening`): r_p =
 * Σ S γ_p ÷ Σ S γ_H2 at a temperature, H₂'s 1; N₂'s, O₂'s and Ar's from each line's H width by
 * Unsöld's scaling ({@link perturberScale}), and air's their volume-weighted mean. Every gas's width
 * shares each line's temperature exponent, so the ratios hold at every temperature and their
 * exponent offsets are 0.
 */
export function atomicBroadeningRatios(
  set: AtomicLineSet,
  temperatureK: number,
): Readonly<Record<BroadeningGas | "air", number>> {
  const q = set.partitionFunction(temperatureK);
  const sums: Record<BroadeningGas, number> = { H2: 0, He: 0, H: 0, N2: 0, O2: 0, Ar: 0 };
  for (const [i, line] of set.lines.entries()) {
    const s = atomicLineStrengthCm(line, temperatureK, q);
    for (const p of ["H2", "He", "H"] as const) {
      sums[p] += s * vanDerWaalsWidthM3PerS(set, i, p, temperatureK);
    }
    const { exponent, anchor, from } = airGasBasis(set, i, temperatureK);
    for (const p of ["N2", "O2", "Ar"] as const) {
      sums[p] +=
        (s * anchor * perturberScale(p, set.massDa, exponent)) /
        perturberScale(from, set.massDa, exponent);
    }
  }
  const ratio = (p: BroadeningGas): number => sums[p] / sums.H2;
  const air = (["N2", "O2", "Ar"] as const).reduce(
    (sum, p) => sum + (AIR_GASES_BY_VOLUME[p] ?? 0) * ratio(p),
    0,
  );
  return {
    H2: 1,
    He: ratio("He"),
    H: ratio("H"),
    N2: ratio("N2"),
    O2: ratio("O2"),
    Ar: ratio("Ar"),
    air,
  };
}

/** What the van der Waals sources come to over a stage's lines: counts by kind. */
export function vanDerWaalsCounts(
  set: AtomicLineSet,
): Readonly<Record<VanDerWaalsSource["kind"], number>> {
  const counts = { abo: 0, kurucz: 0, unsold: 0, selfBroadening: 0, allardH2: 0 };
  for (const s of set.vanDerWaals) {
    counts[s.kind] += 1;
  }
  return counts;
}
