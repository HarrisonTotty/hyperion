/**
 * Molecular lines for R08.T4.d's reduction (plan R08, R08.T4.d): HITRAN's line records and
 * partition sums, each line's strength and width at a temperature and pressure, Humlíček's Voigt
 * profile, and a 1 nm cell's absorption spectrum line by line.
 *
 * @remarks
 * Every quantity is in HITRAN's units unless its name says otherwise: wavenumbers in cm⁻¹,
 * intensitiesCm in cm a molecule (cm⁻¹ ÷ (molecule cm⁻²)), half-widths in cm⁻¹ atm⁻¹, and absorption
 * cross-sections in cm² a molecule. Each line's intensity is per molecule of the species at its
 * natural isotopic abundance, as HITRAN gives it (Gordon et al. 2026, HITRAN2024; Rothman et al.
 * 1998, J. Quant. Spectrosc. Radiat. Transfer 60, 665, Appendix A), so a cell's cross-section sums
 * every isotopologue's lines.
 *
 * The profile is Voigt's, a Doppler (Gaussian) core convolved with pressure broadening's
 * Lorentzian, evaluated by Humlíček 1982's rational approximation (J. Quant. Spectrosc. Radiat.
 * Transfer 27, 437; relative error under 10⁻⁴), and cut off 25 cm⁻¹ from each line's centre, or
 * times a published far-wing factor (CO₂'s χ) out to its reach. H₂O alone also subtracts each
 * line's pedestal, CKD's convention (S. A. Clough, F. X. Kneizys and R. W. Davies, Atmos. Res. 23
 * (1989) 229; stated openly by K. P. Shine et al., J. Mol. Spectrosc. 327 (2016) 193, §1), its
 * continuum drawn apart (`waterContinuum.ts`). Pressure shifts and line mixing are not modelled: a
 * shift moves a line within its cell, which a cell's k-distribution does not see except at the
 * cell's edges.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

/** The second radiation constant c₂ = hc ÷ k, cm K (CODATA 2018: 1.438 776 877 × 10⁻² m K, exact). */
export const SECOND_RADIATION_CONSTANT_CM_K = 1.438_776_877;

/** The molar gas constant R = N_A k, J mol⁻¹ K⁻¹ (CODATA 2018, exact from N_A and k). */
export const MOLAR_GAS_CONSTANT_J_PER_MOL_K = 8.314_462_618;

/** The speed of light, m s⁻¹ (exact). */
export const SPEED_OF_LIGHT_M_PER_S = 299_792_458;

/** HITRAN's reference temperature, K: its intensitiesCm and half-widths are given at it. */
export const HITRAN_REFERENCE_K = 296;

/** One standard atmosphere, Pa (exact): HITRAN's half-widths are per atmosphere. */
export const STANDARD_ATMOSPHERE_PA = 101_325;

/** The line cut-off, cm⁻¹: 25 from the line's centre (Clough, Kneizys and Davies 1989). */
export const LINE_CUTOFF_PER_CM = 25;

/**
 * A far-wing factor that multiplies each line's profile, at a signed offset ν − ν₀ from the line's
 * centre ν₀: χ(|ν − ν₀|), and for a χ set fitted under an asymmetric line shape its asymmetry too
 * (`kDistributions.ts`'s `chiWingFactor`). Its breaks are the offsets, either side, at which it or
 * its slope jumps, where the two-grid sum takes the line exactly.
 */
export interface FarWing {
  /** The factor at a signed offset ν − ν₀, cm⁻¹, from a line centred at ν₀, cm⁻¹: 1 at 0. */
  readonly factor: (offsetPerCm: number, centrePerCm: number) => number;
  /** The offsets, cm⁻¹, at which the factor or its slope jumps, either side of the centre. */
  readonly breaksPerCm: ReadonlyArray<number>;
}

/**
 * A cell's line shape at a node (`decision-r08-t4d-kdist-chain.md`, ruling 1): Voigt within the
 * cut-off of each line's centre, times a far-wing factor where one is published, and, where a
 * continuum carries the far wing, less the line's pedestal (its profile at the cut-off).
 */
export interface LineShape {
  /** The cut-off, cm⁻¹ from each line's centre. */
  readonly cutoffPerCm: number;
  /** Whether each line's profile at the cut-off is subtracted within it, CKD's convention. */
  readonly pedestal: boolean;
  /** The far-wing factor, or none (χ = 1 to the cut-off). */
  readonly farWing?: FarWing;
}

/** The plain 25 cm⁻¹ cut-off, with the pedestal kept. */
export const STANDARD_SHAPE: LineShape = { cutoffPerCm: LINE_CUTOFF_PER_CM, pedestal: false };

/**
 * The gases whose broadening HITRAN2024 gives beside air's and the molecule's own (I. E. Gordon
 * et al. 2026, §2; J. S. Wilzewski et al., J. Quant. Spectrosc. Radiat. Transfer 168 (2016) 193;
 * Y. Tan et al., J. Geophys. Res. Atmos. 124 (2019) 11580, DOI 10.1029/2019JD030929, and
 * Astrophys. J. Suppl. Ser. 262 (2022) 40, DOI 10.3847/1538-4365/ac83a6).
 */
export const FOREIGN_BROADENERS = ["H2", "He", "CO2", "H2O"] as const;

/** A broadening gas HITRAN gives half-widths for: air, the molecule itself, or a foreign gas. */
export type Broadener = "air" | "self" | (typeof FOREIGN_BROADENERS)[number];

/** One isotopologue's HITRAN identity and its molar mass. */
export interface Isotopologue {
  /** HITRAN's global isotopologue number. */
  readonly id: number;
  /** HITRAN's molecule number. */
  readonly molecule: number;
  /** HITRAN's local isotopologue number within the molecule. */
  readonly local: number;
  /** The formula, as HITRAN writes it. */
  readonly formula: string;
  /** The molar mass, g mol⁻¹. */
  readonly molarMassGPerMol: number;
}

/**
 * The isotopologues whose lines R08.T4.d reduces, with HITRAN's molar masses
 * (https://hitran.org/docs/iso-meta/, fetched 2026-10-12, SHA-256 `6a687905…965d4`): every
 * isotopologue with a line over 13,100–26,400 cm⁻¹ in HITRAN2024 for H₂O, CO₂, O₂, CO, N₂O, HCN
 * and H₂, and HCl's four (R08.T4.h; the page fetched again 2026-10-19, unchanged).
 */
export const ISOTOPOLOGUES: ReadonlyArray<Isotopologue> = [
  { id: 1, molecule: 1, local: 1, formula: "H2(16O)", molarMassGPerMol: 18.010_565 },
  { id: 2, molecule: 1, local: 2, formula: "H2(18O)", molarMassGPerMol: 20.014_811 },
  { id: 3, molecule: 1, local: 3, formula: "H2(17O)", molarMassGPerMol: 19.01478 },
  { id: 4, molecule: 1, local: 4, formula: "HD(16O)", molarMassGPerMol: 19.01674 },
  { id: 7, molecule: 2, local: 1, formula: "(12C)(16O)2", molarMassGPerMol: 43.98983 },
  { id: 8, molecule: 2, local: 2, formula: "(13C)(16O)2", molarMassGPerMol: 44.993_185 },
  { id: 9, molecule: 2, local: 3, formula: "(16O)(12C)(18O)", molarMassGPerMol: 45.994_076 },
  { id: 21, molecule: 4, local: 1, formula: "(14N)2(16O)", molarMassGPerMol: 44.001_062 },
  { id: 26, molecule: 5, local: 1, formula: "(12C)(16O)", molarMassGPerMol: 27.994_915 },
  { id: 36, molecule: 7, local: 1, formula: "(16O)2", molarMassGPerMol: 31.98983 },
  { id: 37, molecule: 7, local: 2, formula: "(16O)(18O)", molarMassGPerMol: 33.994_076 },
  { id: 38, molecule: 7, local: 3, formula: "(16O)(17O)", molarMassGPerMol: 32.994_045 },
  { id: 52, molecule: 15, local: 1, formula: "H(35Cl)", molarMassGPerMol: 35.976_678 },
  { id: 53, molecule: 15, local: 2, formula: "H(37Cl)", molarMassGPerMol: 37.973_729 },
  { id: 107, molecule: 15, local: 3, formula: "D(35Cl)", molarMassGPerMol: 36.982_853 },
  { id: 108, molecule: 15, local: 4, formula: "D(37Cl)", molarMassGPerMol: 38.979_904 },
  { id: 70, molecule: 23, local: 1, formula: "H(12C)(14N)", molarMassGPerMol: 27.010_899 },
  { id: 103, molecule: 45, local: 1, formula: "H2", molarMassGPerMol: 2.01565 },
  { id: 115, molecule: 45, local: 2, formula: "HD", molarMassGPerMol: 3.021_825 },
  // The overtone-ladder species' anchors (R08.T4.c's estimate; their main isotopologues alone).
  { id: 76, molecule: 26, local: 1, formula: "(12C)2H2", molarMassGPerMol: 26.01565 },
  { id: 78, molecule: 27, local: 1, formula: "(12C)2H6", molarMassGPerMol: 30.04695 },
  { id: 79, molecule: 28, local: 1, formula: "(31P)H3", molarMassGPerMol: 33.997_241 },
  { id: 90, molecule: 38, local: 1, formula: "(12C)2H4", molarMassGPerMol: 28.0313 },
  { id: 92, molecule: 39, local: 1, formula: "(12C)H3(16O)H", molarMassGPerMol: 32.026_215 },
];

/** An isotopologue by its molecule and local numbers. */
export function isotopologueOf(molecule: number, local: number): Isotopologue {
  const found = ISOTOPOLOGUES.find((i) => i.molecule === molecule && i.local === local);
  if (found === undefined) {
    throw new Error(`HITRAN isotopologue ${molecule}/${local} is not one R08.T4.d reduces`);
  }
  return found;
}

/** A file's text, if its SHA-256 is the one given. */
export function readCheckedText(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

/** The parameters each HITRAN request asks for, in order (HITRAN's API names). */
export const HITRAN_PARAMETERS = [
  "molec_id",
  "local_iso_id",
  "nu",
  "sw",
  "elower",
  "gamma_air",
  "n_air",
  "delta_air",
  "gamma_self",
  "n_self",
  "gamma_H2",
  "n_H2",
  "gamma_He",
  "n_He",
  "gamma_CO2",
  "n_CO2",
  "gamma_H2O",
  "n_H2O",
] as const;

/** A species' lines as arrays, one entry a line, ascending in wavenumber. */
export interface LineList {
  readonly isotopologues: Uint16Array;
  /** ν, cm⁻¹. */
  readonly wavenumbersPerCm: Float64Array;
  /** S at 296 K, cm a molecule. */
  readonly intensitiesCm: Float64Array;
  /** E″, cm⁻¹. */
  readonly lowerEnergiesPerCm: Float64Array;
  /** γ_air at 296 K, cm⁻¹ atm⁻¹. */
  readonly airHalfWidthsPerCmAtm: Float64Array;
  /** n_air, γ ∝ (296 ÷ T)^n. */
  readonly airExponents: Float64Array;
  /** γ_self at 296 K, cm⁻¹ atm⁻¹. */
  readonly selfHalfWidthsPerCmAtm: Float64Array;
  /** γ of each foreign broadener at 296 K, cm⁻¹ atm⁻¹; NaN where HITRAN gives none. */
  readonly foreignHalfWidthsPerCmAtm: Readonly<
    Record<(typeof FOREIGN_BROADENERS)[number], Float64Array>
  >;
  /** n_self; NaN where HITRAN gives none, n_air then standing for it. */
  readonly selfExponents: Float64Array;
  /** Each foreign broadener's n; NaN where HITRAN gives none. */
  readonly foreignExponents: Readonly<Record<(typeof FOREIGN_BROADENERS)[number], Float64Array>>;
}

/** A field of a HITRAN comma-separated record: a number, or NaN for HITRAN's `#` (not given). */
function field(fields: ReadonlyArray<string>, index: number, what: string): number {
  const text = (fields[index] ?? "").trim();
  if (text === "#") {
    return Number.NaN;
  }
  const value = Number(text);
  if (text.length === 0 || !Number.isFinite(value)) {
    throw new Error(`HITRAN record field ${what} reads ${JSON.stringify(text)}`);
  }
  return value;
}

/**
 * A HITRAN API response of {@link HITRAN_PARAMETERS}, comma-separated, one line a record, as a
 * {@link LineList} sorted by wavenumber.
 *
 * @throws Error for a record of another field count, an unread required field, or an
 *   isotopologue {@link ISOTOPOLOGUES} does not hold.
 */
export function parseHitranLines(text: string): LineList {
  const records = text.split(/\r?\n/u).filter((line) => line.trim().length > 0);
  const n = records.length;
  const list = {
    isotopologues: new Uint16Array(n),
    wavenumbersPerCm: new Float64Array(n),
    intensitiesCm: new Float64Array(n),
    lowerEnergiesPerCm: new Float64Array(n),
    airHalfWidthsPerCmAtm: new Float64Array(n),
    airExponents: new Float64Array(n),
    selfHalfWidthsPerCmAtm: new Float64Array(n),
    foreignHalfWidthsPerCmAtm: {
      H2: new Float64Array(n),
      He: new Float64Array(n),
      CO2: new Float64Array(n),
      H2O: new Float64Array(n),
    },
    selfExponents: new Float64Array(n),
    foreignExponents: {
      H2: new Float64Array(n),
      He: new Float64Array(n),
      CO2: new Float64Array(n),
      H2O: new Float64Array(n),
    },
  };
  const rows = records.map((record, i) => {
    const fields = record.split(",");
    if (fields.length !== HITRAN_PARAMETERS.length) {
      throw new Error(`HITRAN record ${i + 1} has ${fields.length} fields`);
    }
    return fields;
  });
  rows.sort((a, b) => field(a, 2, "nu") - field(b, 2, "nu"));
  for (const [i, fields] of rows.entries()) {
    const iso = isotopologueOf(field(fields, 0, "molec_id"), field(fields, 1, "local_iso_id"));
    list.isotopologues[i] = iso.id;
    list.wavenumbersPerCm[i] = field(fields, 2, "nu");
    list.intensitiesCm[i] = field(fields, 3, "sw");
    list.lowerEnergiesPerCm[i] = field(fields, 4, "elower");
    list.airHalfWidthsPerCmAtm[i] = field(fields, 5, "gamma_air");
    list.airExponents[i] = field(fields, 6, "n_air");
    list.selfHalfWidthsPerCmAtm[i] = field(fields, 8, "gamma_self");
    list.foreignHalfWidthsPerCmAtm.H2[i] = field(fields, 10, "gamma_H2");
    list.foreignHalfWidthsPerCmAtm.He[i] = field(fields, 12, "gamma_He");
    list.foreignHalfWidthsPerCmAtm.CO2[i] = field(fields, 14, "gamma_CO2");
    list.foreignHalfWidthsPerCmAtm.H2O[i] = field(fields, 16, "gamma_H2O");
    list.selfExponents[i] = field(fields, 9, "n_self");
    list.foreignExponents.H2[i] = field(fields, 11, "n_H2");
    list.foreignExponents.He[i] = field(fields, 13, "n_He");
    list.foreignExponents.CO2[i] = field(fields, 15, "n_CO2");
    list.foreignExponents.H2O[i] = field(fields, 17, "n_H2O");
    for (const [what, value] of [
      ["sw", list.intensitiesCm[i]],
      ["elower", list.lowerEnergiesPerCm[i]],
      ["gamma_air", list.airHalfWidthsPerCmAtm[i]],
      ["n_air", list.airExponents[i]],
      ["gamma_self", list.selfHalfWidthsPerCmAtm[i]],
    ] as const) {
      if (!Number.isFinite(value)) {
        throw new Error(`HITRAN record ${i + 1}'s ${what} is not given`);
      }
    }
  }
  return list;
}

/**
 * The lines of a list whose wavenumber is below a limit, or at or above it, in the list's order:
 * a species' table takes one side and its overtones part the other (HCl, R08.T4.h).
 */
export function linesBeyond(list: LineList, limitPerCm: number, side: "below" | "above"): LineList {
  const keep = Array.from(list.wavenumbersPerCm, (nu, i) =>
    (side === "below" ? nu < limitPerCm : nu >= limitPerCm) ? i : -1,
  ).filter((i) => i >= 0);
  const pick = <A extends Float64Array | Uint16Array>(from: A, make: (n: number) => A): A => {
    const out = make(keep.length);
    for (const [j, i] of keep.entries()) {
      out[j] = from[i] ?? Number.NaN;
    }
    return out;
  };
  const f64 = (from: Float64Array): Float64Array => pick(from, (n) => new Float64Array(n));
  const foreign = (
    of: Readonly<Record<(typeof FOREIGN_BROADENERS)[number], Float64Array>>,
  ): Record<(typeof FOREIGN_BROADENERS)[number], Float64Array> => ({
    H2: f64(of.H2),
    He: f64(of.He),
    CO2: f64(of.CO2),
    H2O: f64(of.H2O),
  });
  return {
    isotopologues: pick(list.isotopologues, (n) => new Uint16Array(n)),
    wavenumbersPerCm: f64(list.wavenumbersPerCm),
    intensitiesCm: f64(list.intensitiesCm),
    lowerEnergiesPerCm: f64(list.lowerEnergiesPerCm),
    airHalfWidthsPerCmAtm: f64(list.airHalfWidthsPerCmAtm),
    airExponents: f64(list.airExponents),
    selfHalfWidthsPerCmAtm: f64(list.selfHalfWidthsPerCmAtm),
    foreignHalfWidthsPerCmAtm: foreign(list.foreignHalfWidthsPerCmAtm),
    selfExponents: f64(list.selfExponents),
    foreignExponents: foreign(list.foreignExponents),
  };
}

/** One band of an overtone-ladder estimate: its anchor's lines and where they are moved. */
export interface LadderBand {
  /** The anchor's global isotopologue number. */
  readonly isotopologue: number;
  /** HITRAN's global upper and lower vibrational quanta of the anchor band, as its file gives them. */
  readonly upper: string;
  readonly lower: string;
  /** The target polyad's intensity-weighted centre, cm⁻¹. */
  readonly targetPerCm: number;
  /** The vibrational quanta from the anchor to the target: each scales the intensity by 10⁻¹. */
  readonly quanta: number;
}

/**
 * An overtone-ladder estimate's lines (R08.T4.c's estimate, `decision-r08-t4b-t12c.md`, follow-up 3,
 * ruling 3): the anchor band's lines from a HITRAN `.par` file (160-character records; Rothman et
 * al. 1998, Appendix A), each moved by its offset from the band's intensity-weighted centroid to the
 * target, and its intensity divided by 10 for each quantum. Each keeps its lower-state energy and
 * its widths, so that S(T) and the line shape follow the anchor's.
 *
 * @throws Error if the file holds no line of the band.
 */
export function ladderLines(parText: string, band: LadderBand): LineList {
  const iso = ISOTOPOLOGUES.find((i) => i.id === band.isotopologue);
  if (iso === undefined) {
    throw new Error(`isotopologue ${band.isotopologue} is not one R08.T4.d reduces`);
  }
  const rows = parText
    .split(/\r?\n/u)
    .filter(
      (r) =>
        r.length >= 97 &&
        Number(r.slice(0, 2)) === iso.molecule &&
        Number(r.slice(2, 3)) === iso.local &&
        r.slice(67, 82).trim() === band.upper &&
        r.slice(82, 97).trim() === band.lower,
    )
    .map((r) => ({
      nu: Number(r.slice(3, 15)),
      s: Number(r.slice(15, 25)),
      air: Number(r.slice(35, 40)),
      self: Number(r.slice(40, 45)),
      e: Number(r.slice(45, 55)),
      n: Number(r.slice(55, 59)),
    }));
  if (rows.length === 0) {
    throw new Error(`no line of the band ${band.upper} ← ${band.lower} in the anchor's file`);
  }
  const total = rows.reduce((sum, r) => sum + r.s, 0);
  const centroid = rows.reduce((sum, r) => sum + r.s * r.nu, 0) / total;
  const scale = 10 ** -band.quanta;
  const moved = rows
    .map((r) => Object.assign({}, r, { nu: band.targetPerCm + (r.nu - centroid), s: r.s * scale }))
    .toSorted((a, b) => a.nu - b.nu);
  const n = moved.length;
  const none = (): Float64Array => new Float64Array(n).fill(Number.NaN);
  return {
    isotopologues: new Uint16Array(n).fill(band.isotopologue),
    wavenumbersPerCm: Float64Array.from(moved, (r) => r.nu),
    intensitiesCm: Float64Array.from(moved, (r) => r.s),
    lowerEnergiesPerCm: Float64Array.from(moved, (r) => r.e),
    airHalfWidthsPerCmAtm: Float64Array.from(moved, (r) => r.air),
    airExponents: Float64Array.from(moved, (r) => r.n),
    selfHalfWidthsPerCmAtm: Float64Array.from(moved, (r) => r.self),
    foreignHalfWidthsPerCmAtm: { H2: none(), He: none(), CO2: none(), H2O: none() },
    selfExponents: none(),
    foreignExponents: { H2: none(), He: none(), CO2: none(), H2O: none() },
  };
}

/** A partition sum tabulated at whole kelvins, as HITRAN's TIPS files give it. */
export interface PartitionSums {
  /** The first temperature, K. */
  readonly firstK: number;
  /** Q at firstK, firstK + 1, …. */
  readonly values: Float64Array;
}

/**
 * A HITRAN TIPS file (`q<N>.txt`, R. R. Gamache et al., J. Quant. Spectrosc. Radiat. Transfer 271
 * (2021) 107713), one temperature and Q a line, at every kelvin from 1 K.
 *
 * @throws Error for a line that is not two numbers, or temperatures not in steps of 1 K.
 */
export function parsePartitionSums(text: string): PartitionSums {
  const pairs = text
    .split(/\r?\n/u)
    .filter((line) => line.trim().length > 0)
    .map((line, i) => {
      const [t, q] = line.trim().split(/\s+/u).map(Number);
      if (t === undefined || q === undefined || !Number.isFinite(t + q)) {
        throw new Error(`TIPS line ${i + 1} is not a temperature and a partition sum`);
      }
      return [t, q] as const;
    });
  const firstK = pairs[0]?.[0] ?? Number.NaN;
  if (!pairs.every(([t], i) => t === firstK + i)) {
    throw new Error("TIPS temperatures are not in steps of 1 K");
  }
  return { firstK, values: Float64Array.from(pairs, ([, q]) => q) };
}

/**
 * Q at a temperature, linear between the file's whole kelvins.
 *
 * @throws RangeError outside the file's temperatures.
 */
export function partitionSumAt(sums: PartitionSums, temperatureK: number): number {
  const x = temperatureK - sums.firstK;
  const k = Math.floor(x);
  if (!(k >= 0 && k < sums.values.length - 1)) {
    throw new RangeError(`no partition sum at ${temperatureK} K`);
  }
  const a = sums.values[k] ?? Number.NaN;
  const b = sums.values[k + 1] ?? Number.NaN;
  return a + (b - a) * (x - k);
}

/**
 * A line's intensity at a temperature from its 296 K value (HITRAN's definitions, Rothman et al.
 * 1998, Appendix A, Eq. A11): S(T) = S(296) Q(296) ÷ Q(T) × e^(−c₂E″ ÷ T) ÷ e^(−c₂E″ ÷ 296) ×
 * (1 − e^(−c₂ν ÷ T)) ÷ (1 − e^(−c₂ν ÷ 296)).
 */
export function intensityAt(
  intensity296: number,
  lowerEnergyPerCm: number,
  wavenumberPerCm: number,
  q296: number,
  qT: number,
  temperatureK: number,
): number {
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  const t0 = HITRAN_REFERENCE_K;
  const boltzmann = Math.exp(-c2 * lowerEnergyPerCm * (1 / temperatureK - 1 / t0));
  const stimulated =
    -Math.expm1(-(c2 * wavenumberPerCm) / temperatureK) / -Math.expm1(-(c2 * wavenumberPerCm) / t0);
  return ((intensity296 * q296) / qT) * boltzmann * stimulated;
}

/** A line's Doppler half-width at half maximum, cm⁻¹: (ν ÷ c) √(2 ln 2 R T ÷ M). */
export function dopplerHalfWidthPerCm(
  wavenumberPerCm: number,
  temperatureK: number,
  molarMassGPerMol: number,
): number {
  const massKgPerMol = molarMassGPerMol * 1e-3;
  return (
    (wavenumberPerCm / SPEED_OF_LIGHT_M_PER_S) *
    Math.sqrt((2 * Math.LN2 * MOLAR_GAS_CONSTANT_J_PER_MOL_K * temperatureK) / massKgPerMol)
  );
}

/**
 * The Voigt profile's half-width at half maximum from its Lorentzian and Gaussian ones, cm⁻¹, to
 * about 0.02% (J. J. Olivero and R. L. Longbothum, J. Quant. Spectrosc. Radiat. Transfer 17
 * (1977) 233, Eq. 5): f_V ≈ 0.5346 f_L + √(0.2166 f_L² + f_G²).
 */
export function voigtHalfWidthPerCm(lorentzPerCm: number, dopplerPerCm: number): number {
  return 0.5346 * lorentzPerCm + Math.sqrt(0.2166 * lorentzPerCm ** 2 + dopplerPerCm ** 2);
}

/** √(ln 2 ÷ π), the Gaussian profile's peak times its half-width. */
const DOPPLER_PEAK = Math.sqrt(Math.LN2 / Math.PI);

/** √(ln 2). */
const SQRT_LN2 = Math.sqrt(Math.LN2);

/**
 * The real part of the Faddeeva function, K(x, y) = Re w(x + iy), y ≥ 0, by Humlíček 1982's W4
 * algorithm (J. Quant. Spectrosc. Radiat. Transfer 27, 437, its four regions and their rational
 * approximations; relative error under 10⁻⁴ everywhere).
 */
export function voigtK(x: number, y: number): number {
  const s = Math.abs(x) + y;
  // t = y − i x.
  const tr = y;
  const ti = -x;
  if (s >= 15) {
    // Region I: w = t × 0.5641896 ÷ (0.5 + t²).
    const ur = tr * tr - ti * ti + 0.5;
    const ui = 2 * tr * ti;
    const d = ur * ur + ui * ui;
    return (0.564_189_6 * (tr * ur + ti * ui)) / d;
  }
  if (s >= 5.5) {
    // Region II: w = t (1.410474 + u × 0.5641896) ÷ (0.75 + u (3 + u)), u = t².
    const ur = tr * tr - ti * ti;
    const ui = 2 * tr * ti;
    const nr = 1.410_474 + 0.564_189_6 * ur;
    const ni = 0.564_189_6 * ui;
    const numR = tr * nr - ti * ni;
    const numI = tr * ni + ti * nr;
    // 0.75 + u (3 + u) = 0.75 + 3u + u².
    const u2r = ur * ur - ui * ui;
    const u2i = 2 * ur * ui;
    const denR = 0.75 + 3 * ur + u2r;
    const denI = 3 * ui + u2i;
    const d = denR * denR + denI * denI;
    return (numR * denR + numI * denI) / d;
  }
  if (y >= 0.195 * Math.abs(x) - 0.176) {
    // Region III: a quotient of quintic polynomials in t.
    let nr = 0.564_223_6;
    let ni = 0;
    for (const c of [3.778_987, 11.964_82, 20.209_33, 16.4955]) {
      const r = nr * tr - ni * ti + c;
      const i = nr * ti + ni * tr;
      nr = r;
      ni = i;
    }
    let dr = 1;
    let di = 0;
    for (const c of [6.699_398, 21.692_74, 39.271_21, 38.823_63, 16.4955]) {
      const r = dr * tr - di * ti + c;
      const i = dr * ti + di * tr;
      dr = r;
      di = i;
    }
    const d = dr * dr + di * di;
    return (nr * dr + ni * di) / d;
  }
  // Region IV: w = exp(u) − t (…) ÷ (…), u = t², polynomials in u.
  const ur = tr * tr - ti * ti;
  const ui = 2 * tr * ti;
  // Numerator 36183.31 − u(3321.9905 − u(1540.787 − u(219.0313 − u(35.76683 − u(1.320522 − u 0.56419))))).
  let pr = 0.564_19;
  let pi = 0;
  for (const c of [1.320_522, 35.766_83, 219.0313, 1_540.787, 3_321.9905, 36_183.31]) {
    // p = c − u p.
    const r = c - (ur * pr - ui * pi);
    const i = -(ur * pi + ui * pr);
    pr = r;
    pi = i;
  }
  // Denominator 32066.6 − u(24322.84 − u(9022.228 − u(2186.181 − u(364.2191 − u(61.57037 − u(1.841439 − u)))))).
  let qr = 1;
  let qi = 0;
  for (const c of [1.841_439, 61.570_37, 364.2191, 2_186.181, 9_022.228, 24_322.84, 32_066.6]) {
    const r = c - (ur * qr - ui * qi);
    const i = -(ur * qi + ui * qr);
    qr = r;
    qi = i;
  }
  // t p ÷ q.
  const tpR = tr * pr - ti * pi;
  const tpI = tr * pi + ti * pr;
  const d = qr * qr + qi * qi;
  const fracR = (tpR * qr + tpI * qi) / d;
  // Re exp(u) = e^(ur) cos(ui).
  return Math.exp(ur) * Math.cos(ui) - fracR;
}

/**
 * The Voigt profile at a distance from the line's centre, cm (its integral over wavenumber is 1),
 * from its Doppler and Lorentzian half-widths at half maximum, cm⁻¹.
 */
export function voigtProfile(
  offsetPerCm: number,
  dopplerPerCm: number,
  lorentzPerCm: number,
): number {
  const x = (SQRT_LN2 * offsetPerCm) / dopplerPerCm;
  const y = (SQRT_LN2 * lorentzPerCm) / dopplerPerCm;
  return (DOPPLER_PEAK / dopplerPerCm) * voigtK(x, y);
}

/** The vacuum wavenumber range of a 1 nm cell [λ − 0.5, λ + 0.5) nm, cm⁻¹. */
export function cellWavenumbersPerCm(cellNm: number): readonly [number, number] {
  return [1e7 / (cellNm + 0.5), 1e7 / (cellNm - 0.5)];
}

/** A line's state at one temperature and broadening pressure: its strength and its widths. */
export interface LineState {
  /** S(T), cm a molecule. */
  readonly intensityCm: number;
  /** The Doppler half-width, cm⁻¹. */
  readonly dopplerPerCm: number;
  /** The Lorentzian half-width, cm⁻¹. */
  readonly lorentzPerCm: number;
}

/** What a species' lines need besides the list: each isotopologue's partition sums. */
export interface LineSpecies {
  readonly lines: LineList;
  /** TIPS by global isotopologue number. */
  readonly partitionSums: ReadonlyMap<number, PartitionSums>;
}

/**
 * Each line's strength and widths at a temperature and an air-equivalent broadening pressure: the
 * pressure at which air would give the lines their Lorentzian widths, γ_air (296 ÷ T)^n_air p ÷
 * 1 atm.
 */
export function lineStates(
  species: LineSpecies,
  indices: ReadonlyArray<number>,
  temperatureK: number,
  broadeningPa: number,
): LineState[] {
  const { lines } = species;
  const atm = broadeningPa / STANDARD_ATMOSPHERE_PA;
  const q = new Map<number, readonly [number, number]>();
  for (const [id, sums] of species.partitionSums) {
    q.set(id, [partitionSumAt(sums, HITRAN_REFERENCE_K), partitionSumAt(sums, temperatureK)]);
  }
  return indices.map((i) => {
    const id = lines.isotopologues[i] ?? 0;
    const [q296, qT] = q.get(id) ?? [Number.NaN, Number.NaN];
    const iso = ISOTOPOLOGUES.find((x) => x.id === id);
    const nu = lines.wavenumbersPerCm[i] ?? Number.NaN;
    return {
      intensityCm: intensityAt(
        lines.intensitiesCm[i] ?? Number.NaN,
        lines.lowerEnergiesPerCm[i] ?? Number.NaN,
        nu,
        q296,
        qT,
        temperatureK,
      ),
      dopplerPerCm: dopplerHalfWidthPerCm(nu, temperatureK, iso?.molarMassGPerMol ?? Number.NaN),
      lorentzPerCm:
        (lines.airHalfWidthsPerCmAtm[i] ?? Number.NaN) *
        (HITRAN_REFERENCE_K / temperatureK) ** (lines.airExponents[i] ?? Number.NaN) *
        atm,
    };
  });
}

/** The indices of the lines whose centres lie within a cut-off of a wavenumber range. */
export function linesNear(
  lines: LineList,
  lowPerCm: number,
  highPerCm: number,
  cutoffPerCm = LINE_CUTOFF_PER_CM,
): number[] {
  const nu = lines.wavenumbersPerCm;
  let lo = 0;
  let hi = nu.length;
  const from = lowPerCm - cutoffPerCm;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((nu[mid] ?? Number.NaN) < from) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  const out: number[] = [];
  for (let i = lo; i < nu.length && (nu[i] ?? Number.NaN) <= highPerCm + cutoffPerCm; i += 1) {
    out.push(i);
  }
  return out;
}

/**
 * The near zone of a line, in its Voigt half-widths: inside it the profile is summed on the fine
 * grid; beyond it, out to the cut-off, on the coarse grid, where it is Lorentzian to 10⁻³ (the
 * Voigt function's asymptotic series, K ≈ y ÷ (√π x²) (1 + 3 ÷ (2x²)), at x ≥ 50 √ln 2).
 */
export const NEAR_ZONE_HALF_WIDTHS = 50;

/** Fine points per smallest Voigt half-width among a cell's lines. */
export const FINE_POINTS_PER_HALF_WIDTH = 4;

/** The fewest fine points a cell takes, however broad its lines. */
export const MIN_CELL_POINTS = 512;

/**
 * Coarse steps per near zone, for the far wings: 16, so that the linear interpolation of a
 * Lorentzian wing A ÷ Δν² errs by at most ¾ (step ÷ Δν)², 2.3 × 10⁻³ of the wing where it is first
 * interpolated (Δν = 18 steps) and less farther out.
 */
export const COARSE_STEPS_PER_NEAR_ZONE = 16;

/**
 * The margin, in coarse steps, about a near zone's edge, the cut-off and a far-wing factor's breaks
 * within which a line is summed exactly.
 */
const EXACT_MARGIN_STEPS = 2;

/**
 * The widest coarse step under a far-wing factor, cm⁻¹: the linear interpolation of a wing
 * L(Δν) e^(−B Δν) errs by at most h² ÷ 8 × (6 ÷ Δν² + 4B ÷ Δν + B²) of itself, the χ terms adding
 * 7.7 × 10⁻⁴ at Δν = 3 cm⁻¹ with h = 0.1 cm⁻¹ and Perrin and Hartmann's largest B over 100–500 K,
 * 0.365 cm (CO₂–N₂'s B₁ at 500 K), and the total within the two-grid scheme's 2.3 × 10⁻³.
 */
export const FAR_WING_COARSE_STEP_PER_CM = 0.1;

/** A cell's absorption cross-section sampled at equal steps of vacuum wavelength. */
export interface CellSpectrum {
  /** The cell's centre, nm. */
  readonly cellNm: number;
  /** σ at each point, cm² a molecule: the points at λ − 0.5 + (j + ½) ÷ N nm. */
  readonly crossSectionsCm2: Float64Array;
}

/**
 * The fine step a cell takes for a set of line states: a quarter of the least Voigt half-width
 * among them, and at least {@link MIN_CELL_POINTS} points, as a point count.
 */
export function cellPointCount(cellNm: number, states: ReadonlyArray<LineState>): number {
  const [low, high] = cellWavenumbersPerCm(cellNm);
  let least = Infinity;
  for (const s of states) {
    least = Math.min(least, voigtHalfWidthPerCm(s.lorentzPerCm, s.dopplerPerCm));
  }
  const step = least / FINE_POINTS_PER_HALF_WIDTH;
  return Math.max(MIN_CELL_POINTS, Number.isFinite(step) ? Math.ceil((high - low) / step) : 0);
}

/**
 * A cell's absorption cross-section, line by line, on `points` equal steps of vacuum wavelength
 * over [λ − 0.5, λ + 0.5) nm, each point its step's midpoint: Σᵢ Sᵢ Vᵢ(ν − νᵢ) χ(|ν − νᵢ|), each
 * line's Voigt profile cut off `shape.cutoffPerCm` from its centre, less its value there if
 * `shape.pedestal`, and times the far-wing factor at its signed offset if `shape.farWing` (else 1).
 *
 * @remarks
 * A line far from a point adds only its smooth wing there, so the wings are summed on a coarse
 * grid of the cell and interpolated linearly to the fine points, the near zones on the fine grid
 * (a two-grid scheme, after LBLRTM's; S. A. Clough and M. J. Iacono, J. Geophys. Res. 100 (1995)
 * 16519):
 *
 * - each line's far part is its profile beyond its near zone W ({@link NEAR_ZONE_HALF_WIDTHS} of its
 *   Voigt half-width) and 0 beyond the cut-off; inside W it is continued as F (2 − Δν² ÷ W²), F the
 *   profile at W, which keeps the coarse array bounded and smooth there;
 * - at each fine point within W, or within {@link EXACT_MARGIN_STEPS} coarse steps of W, of the
 *   cut-off or of a break in χ's slope, the line adds its exact profile less its own far part as the
 *   coarse grid interpolates it there, so that its contribution at that point is exact;
 * - every other point takes the line's wing from the coarse grid, whose step is a
 *   {@link COARSE_STEPS_PER_NEAR_ZONE}th of the narrowest near zone, and at most
 *   {@link FAR_WING_COARSE_STEP_PER_CM} under a far-wing factor: the wing errs by at most
 *   ¾ (step ÷ Δν)², 2.3 × 10⁻³ of itself at 18 steps from the line and less farther out, and χ's
 *   exponentials by at most 1.7 × 10⁻⁴.
 */
export function cellSpectrum(
  cellNm: number,
  species: LineSpecies,
  indices: ReadonlyArray<number>,
  states: ReadonlyArray<LineState>,
  points: number,
  shape: LineShape = STANDARD_SHAPE,
): CellSpectrum {
  const { lines } = species;
  const cutoff = shape.cutoffPerCm;
  const wing = shape.farWing;
  const chi = (offset: number, centre: number): number =>
    wing === undefined ? 1 : wing.factor(offset, centre);
  const sigma = new Float64Array(points);
  const lowNm = cellNm - 0.5;
  const stepNm = 1 / points;
  const nuAt = Float64Array.from({ length: points }, (_, j) => 1e7 / (lowNm + (j + 0.5) * stepNm));
  // The coarse grid of the far wings, its first node half a coarse step beyond the cell's lower
  // wavelength edge and its last beyond the upper one, so that every fine point lies between two.
  let narrowestZone = Infinity;
  for (const s of states) {
    narrowestZone = Math.min(
      narrowestZone,
      NEAR_ZONE_HALF_WIDTHS * voigtHalfWidthPerCm(s.lorentzPerCm, s.dopplerPerCm),
    );
  }
  const [cellLowPerCm, cellHighPerCm] = cellWavenumbersPerCm(cellNm);
  const cellWidthPerCm = cellHighPerCm - cellLowPerCm;
  const coarseStepPerCm = Math.min(
    narrowestZone / COARSE_STEPS_PER_NEAR_ZONE,
    cellWidthPerCm / 64,
    wing === undefined ? Infinity : FAR_WING_COARSE_STEP_PER_CM,
  );
  const coarseCount = Math.ceil(1 / ((coarseStepPerCm * (cellNm * cellNm)) / 1e7)) + 2;
  const coarseStepNm = 1 / (coarseCount - 2);
  const coarseNu = Float64Array.from(
    { length: coarseCount },
    (_, m) => 1e7 / (lowNm + (m - 0.5) * coarseStepNm),
  );
  // A fine point's place on the coarse grid: its lower node and the fraction towards the next.
  const coarseOf = (j: number): readonly [number, number] => {
    const x = ((j + 0.5) * stepNm) / coarseStepNm + 0.5;
    const m = Math.min(Math.floor(x), coarseCount - 2);
    return [m, x - m];
  };
  // The widest coarse step in wavenumber, at the cell's short-wavelength edge, for the margins.
  const marginPerCm = EXACT_MARGIN_STEPS * ((coarseStepNm * 1e7) / (lowNm * lowNm));
  const breaks = [cutoff, ...(wing?.breaksPerCm ?? []).filter((b) => b < cutoff)];
  const far = new Float64Array(coarseCount);
  for (const [k, i] of indices.entries()) {
    const s = states[k];
    if (s === undefined || !(s.intensityCm > 0)) {
      continue;
    }
    const centre = lines.wavenumbersPerCm[i] ?? Number.NaN;
    const zone = NEAR_ZONE_HALF_WIDTHS * voigtHalfWidthPerCm(s.lorentzPerCm, s.dopplerPerCm);
    const pedestal = shape.pedestal ? voigtProfile(cutoff, s.dopplerPerCm, s.lorentzPerCm) : 0;
    const exact = (offset: number): number => {
      const a = Math.abs(offset);
      return a <= cutoff
        ? (voigtProfile(offset, s.dopplerPerCm, s.lorentzPerCm) - pedestal) * chi(offset, centre)
        : 0;
    };
    // The fine points whose wavenumber lies within `reach` of the line's centre.
    const fineRange = (reach: number): readonly [number, number] => [
      Math.max(0, Math.floor((1e7 / (centre + reach) - lowNm) / stepNm - 0.5)),
      Math.min(points - 1, Math.ceil((1e7 / (centre - reach) - lowNm) / stepNm - 0.5)),
    ];
    if (zone + marginPerCm >= cutoff) {
      // A broad line: its whole profile on the fine grid.
      const [from, to] = fineRange(cutoff);
      for (let j = from; j <= to; j += 1) {
        sigma[j] = (sigma[j] ?? 0) + s.intensityCm * exact((nuAt[j] ?? Number.NaN) - centre);
      }
      continue;
    }
    // The far part's continuation inside the near zone meets the exact profile at both of its
    // edges, which an asymmetric far-wing factor makes unequal: their mean, and half their
    // difference linear across the zone.
    const edgeAbove = exact(zone);
    const edgeBelow = exact(-zone);
    const farPart = (offset: number): number => {
      const a = Math.abs(offset);
      if (a > cutoff) {
        return 0;
      }
      if (a >= zone) {
        return exact(offset);
      }
      return (
        0.5 * (edgeAbove + edgeBelow) * (2 - (a * a) / (zone * zone)) +
        0.5 * (edgeAbove - edgeBelow) * (offset / zone)
      );
    };
    const [mFrom, mTo] = [
      Math.max(0, Math.floor((1e7 / (centre + cutoff) - lowNm) / coarseStepNm + 0.5) - 1),
      Math.min(
        coarseCount - 1,
        Math.ceil((1e7 / (centre - cutoff) - lowNm) / coarseStepNm + 0.5) + 1,
      ),
    ];
    for (let m = mFrom; m <= mTo; m += 1) {
      far[m] = (far[m] ?? 0) + s.intensityCm * farPart((coarseNu[m] ?? Number.NaN) - centre);
    }
    // The points where the line is summed exactly: its near zone with its margin, and the bands
    // about its cut-off and χ's breaks, each point once.
    const own = (j: number): number => {
      const [m, t] = coarseOf(j);
      const a = farPart((coarseNu[m] ?? Number.NaN) - centre);
      const b = farPart((coarseNu[m + 1] ?? Number.NaN) - centre);
      return a + (b - a) * t;
    };
    const ranges: (readonly [number, number])[] = [fineRange(zone + marginPerCm)];
    for (const b of breaks) {
      if (b + marginPerCm <= zone) {
        continue;
      }
      const [outerFrom, outerTo] = fineRange(b + marginPerCm);
      const [innerFrom, innerTo] = fineRange(Math.max(b - marginPerCm, 0));
      ranges.push([outerFrom, innerFrom - 1], [innerTo + 1, outerTo]);
    }
    ranges.sort((x, y) => x[0] - y[0]);
    let next = 0;
    for (const [from, to] of ranges) {
      for (let j = Math.max(from, next); j <= to; j += 1) {
        sigma[j] =
          (sigma[j] ?? 0) + s.intensityCm * (exact((nuAt[j] ?? Number.NaN) - centre) - own(j));
      }
      next = Math.max(next, to + 1);
    }
  }
  for (let j = 0; j < points; j += 1) {
    const [m, t] = coarseOf(j);
    const a = far[m] ?? Number.NaN;
    const b = far[m + 1] ?? Number.NaN;
    // The exact bands cancel a line's interpolated far part, which can leave a rounding error of
    // either sign, of order 10⁻¹⁶ of the line's wing there; a cross-section is never below 0.
    sigma[j] = Math.max((sigma[j] ?? 0) + a + (b - a) * t, 0);
  }
  return { cellNm, crossSectionsCm2: sigma };
}
