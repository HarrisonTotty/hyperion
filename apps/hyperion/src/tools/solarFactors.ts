/**
 * `node scripts/solarFactors.mjs`: computes the Sun's spectral samples and the
 * spectral-radiance-to-luminance factors that `view/atmosphere/solar.ts` commits (plan R05,
 * R05.T12.a, Design note 16).
 *
 * @remarks
 * Bruneton's method (precomputed_atmospheric_scattering, 2017, `atmosphere/model.cc`,
 * `ComputeSpectralRadianceToLuminanceFactors`): the atmosphere is computed at three wavelengths,
 * 680, 550 and 440 nm, per unit solar irradiance, and each channel c is turned into linear
 * Rec. 709 by
 *
 *   k_c = 683 lm W⁻¹ × Σ_λ c̄(λ) E(λ) ÷ E(λ_c) × (λ ÷ λ_c)^p × 1 nm,
 *
 * with c̄ the CIE 1931 matching functions taken to linear Rec. 709 (the sRGB XYZ-to-RGB
 * matrix), E the solar spectrum and p = 0 for the Sun's own light and −3 for the sky's, whose
 * spectrum between the samples Bruneton models as falling as λ⁻³. Here E is the ASTM E-490 AM0
 * spectrum rather than Bruneton's ASTM G-173 extraterrestrial column, as the plan has it.
 *
 * Inputs (not committed, per R05.T12.d's ruling of 2026-10-02; only the printed constants are):
 *
 * - `--e490`: ASTM E-490-00a (2019), the zero-air-mass solar spectral irradiance, from
 *   https://www.nlr.gov/media/docs/libraries/grid/e490_00a_amo.xls?sfvrsn=ce97914b_1
 *   (NREL, now NLR; downloaded 2026-10-02; the `.xls`'s SHA-256 is
 *   82d5de38f529fcd65d1648e397a1e2d1a8dfaff1e3e701ba5861ed4ab5bc1e7a), its sheet `NewAM0`,
 *   columns A and B (wavelength in µm and irradiance in W m⁻² µm⁻¹, 1,697 rows below the header),
 *   exported as two comma-separated columns, one row a line, each number in its shortest
 *   round-trip decimal form (Python's `repr(float)`), no header: SHA-256
 *   {@link E490_CSV_SHA256}.
 * - `--cie`: the CIE 1931 2° standard observer's colour-matching functions, 360–830 nm at 1 nm,
 *   CIE 018:2019 / ISO/CIE 11664-1:2019, DOI 10.25039/CIE.DS.xvudnb9b, from
 *   https://files.cie.co.at/Publications-datasets/CIE_xyz_1931_2deg.csv (CC BY-SA 4.0; downloaded
 *   2026-10-02), unmodified: SHA-256 {@link CIE_CSV_SHA256}.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";

/** The SHA-256 of the E-490 CSV export described in the module's header. */
export const E490_CSV_SHA256 = "f59fa5a4d5815f273c1342646e2066e5c848f7efa416ebc289d3b5729a7b3e0f";

/** The SHA-256 of the CIE's `CIE_xyz_1931_2deg.csv`. */
export const CIE_CSV_SHA256 = "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1";

/** The channels' wavelengths, nm (Bruneton's `kLambdaR`, `kLambdaG`, `kLambdaB`). */
export const CHANNEL_WAVELENGTHS_NM = [680, 550, 440] as const;

/** The maximum luminous efficacy, lm W⁻¹: 683, Bruneton's `MAX_LUMINOUS_EFFICACY` (SI's K_cd). */
export const MAX_LUMINOUS_EFFICACY_LM_PER_W = 683;

/**
 * XYZ to linear Rec. 709 (sRGB) primaries, as rows: from the sRGB primaries and D65 white XYZ
 * (0.95047, 1, 1.08883) to seven figures (Lindbloom; IEC 61966-2-1:1999 rounds them to four
 * decimals), the values of Filament's `ColorSpaceUtils.h` that `view/photometry/toneCurve.ts`
 * carries.
 */
export const XYZ_TO_REC709 = [
  [3.2404542, -1.5371385, -0.4985314],
  [-0.969266, 1.8760108, 0.041556],
  [0.0556434, -0.2040259, 1.0572252],
] as const;

/** One sample of a spectrum. */
export interface SpectralSample {
  /** The wavelength, nm. */
  readonly wavelengthNm: number;
  /** The spectral irradiance, W m⁻² nm⁻¹. */
  readonly value: number;
}

/** One row of the colour-matching functions. */
export interface MatchingSample {
  /** The wavelength, nm. */
  readonly wavelengthNm: number;
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

function numbersOf(text: string, columns: number): number[][] {
  const rows: number[][] = [];
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    if (line.trim().length === 0) {
      continue;
    }
    const cells = line.split(",").map(Number);
    if (cells.length !== columns || cells.some((v) => !Number.isFinite(v))) {
      throw new Error(`line ${index + 1}: expected ${columns} numbers, got "${line}"`);
    }
    rows.push(cells);
  }
  return rows;
}

/**
 * The E-490 export: µm and W m⁻² µm⁻¹ turned into nm and W m⁻² nm⁻¹.
 *
 * @throws Error on a malformed line or wavelengths out of order.
 */
export function parseE490(text: string): SpectralSample[] {
  const samples = numbersOf(text, 2).map(([um = Number.NaN, perUm = Number.NaN]) => ({
    wavelengthNm: um * 1_000,
    value: perUm / 1_000,
  }));
  for (let i = 1; i < samples.length; i += 1) {
    if ((samples[i]?.wavelengthNm ?? 0) <= (samples[i - 1]?.wavelengthNm ?? 0)) {
      throw new Error(`E-490 wavelengths are not increasing at row ${i + 1}`);
    }
  }
  return samples;
}

/**
 * The CIE's CSV: wavelength in nm and x̄, ȳ, z̄.
 *
 * @throws Error on a malformed line.
 */
export function parseCie(text: string): MatchingSample[] {
  return numbersOf(text, 4).map(
    ([w = Number.NaN, x = Number.NaN, y = Number.NaN, z = Number.NaN]) => ({
      wavelengthNm: w,
      x,
      y,
      z,
    }),
  );
}

/**
 * A spectrum's value at a wavelength, linearly interpolated between its samples.
 *
 * @throws Error outside the spectrum's range.
 */
export function interpolate(spectrum: readonly SpectralSample[], wavelengthNm: number): number {
  let lo = 0;
  let hi = spectrum.length - 1;
  const first = spectrum[lo];
  const last = spectrum[hi];
  if (
    first === undefined ||
    last === undefined ||
    wavelengthNm < first.wavelengthNm ||
    wavelengthNm > last.wavelengthNm
  ) {
    throw new Error(`${wavelengthNm} nm is outside the spectrum`);
  }
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if ((spectrum[mid]?.wavelengthNm ?? Number.NaN) <= wavelengthNm) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  const a = spectrum[lo];
  const b = spectrum[hi];
  if (a === undefined || b === undefined) {
    throw new Error("interpolation bracket out of range");
  }
  if (b.wavelengthNm === a.wavelengthNm) {
    return a.value;
  }
  const t = (wavelengthNm - a.wavelengthNm) / (b.wavelengthNm - a.wavelengthNm);
  return a.value + (b.value - a.value) * t;
}

/** A triple in channel order (680, 550, 440 nm). */
export type Triple = readonly [number, number, number];

/**
 * Bruneton's spectral-radiance-to-luminance factors, lm nm W⁻¹ (the module's formula), summed over
 * the matching functions' 1 nm rows.
 *
 * @param lambdaPower - p: 0 for the Sun, −3 for the sky.
 */
export function spectralToLuminanceFactors(
  solar: readonly SpectralSample[],
  cmf: readonly MatchingSample[],
  lambdaPower: number,
): Triple {
  const k = [0, 0, 0];
  const solarAt = CHANNEL_WAVELENGTHS_NM.map((l) => interpolate(solar, l));
  for (const [i, row] of cmf.entries()) {
    const next = cmf[i + 1];
    const stepNm = next === undefined ? 1 : next.wavelengthNm - row.wavelengthNm;
    const irradiance = interpolate(solar, row.wavelengthNm);
    for (let c = 0; c < 3; c += 1) {
      const m = XYZ_TO_REC709[c] ?? XYZ_TO_REC709[0];
      const bar = m[0] * row.x + m[1] * row.y + m[2] * row.z;
      const lambdaC = CHANNEL_WAVELENGTHS_NM[c] ?? Number.NaN;
      k[c] =
        (k[c] ?? 0) +
        ((bar * irradiance) / (solarAt[c] ?? Number.NaN)) *
          (row.wavelengthNm / lambdaC) ** lambdaPower *
          stepNm;
    }
  }
  return [
    (k[0] ?? 0) * MAX_LUMINOUS_EFFICACY_LM_PER_W,
    (k[1] ?? 0) * MAX_LUMINOUS_EFFICACY_LM_PER_W,
    (k[2] ?? 0) * MAX_LUMINOUS_EFFICACY_LM_PER_W,
  ];
}

/** The photopic illuminance of a spectrum, lx: 683 lm W⁻¹ × Σ ȳ(λ) E(λ) over the 1 nm rows. */
export function photopicIlluminanceLx(
  solar: readonly SpectralSample[],
  cmf: readonly MatchingSample[],
): number {
  let sum = 0;
  for (const [i, row] of cmf.entries()) {
    const next = cmf[i + 1];
    const stepNm = next === undefined ? 1 : next.wavelengthNm - row.wavelengthNm;
    sum += row.y * interpolate(solar, row.wavelengthNm) * stepNm;
  }
  return sum * MAX_LUMINOUS_EFFICACY_LM_PER_W;
}

/** What the script prints, for `solar.ts`. */
export interface SolarFactors {
  /** E-490's spectral irradiance at each channel's wavelength, W m⁻² nm⁻¹. */
  readonly spectralIrradiance: Triple;
  /** The Sun's factors (p = 0), lm nm W⁻¹. */
  readonly sunFactors: Triple;
  /** The sky's factors (p = −3), lm nm W⁻¹. */
  readonly skyFactors: Triple;
  /** E-490's photopic illuminance, lx. */
  readonly illuminanceLx: number;
}

/** Every derived constant from the two inputs. */
export function solarFactors(
  solar: readonly SpectralSample[],
  cmf: readonly MatchingSample[],
): SolarFactors {
  const [r, g, b] = CHANNEL_WAVELENGTHS_NM.map((l) => interpolate(solar, l));
  return {
    spectralIrradiance: [r ?? Number.NaN, g ?? Number.NaN, b ?? Number.NaN],
    sunFactors: spectralToLuminanceFactors(solar, cmf, 0),
    skyFactors: spectralToLuminanceFactors(solar, cmf, -3),
    illuminanceLx: photopicIlluminanceLx(solar, cmf),
  };
}

/** A triple to seven significant figures, as a TypeScript array literal. */
function literal(v: Triple): string {
  return `[${v.map((x) => x.toPrecision(7)).join(", ")}]`;
}

/** The constants as TypeScript, for pasting into `solar.ts`. */
export function describeFactors(f: SolarFactors): string {
  return [
    `SOLAR_SPECTRAL_IRRADIANCE_W_M2_NM = ${literal(f.spectralIrradiance)}`,
    `SUN_SPECTRAL_TO_LUMINANCE = ${literal(f.sunFactors)}`,
    `SKY_SPECTRAL_TO_LUMINANCE = ${literal(f.skyFactors)}`,
    `E490_ILLUMINANCE_LX = ${f.illuminanceLx.toPrecision(7)}`,
  ].join("\n");
}

function readChecked(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

/** The command line: `--e490 <csv> --cie <csv>`; prints the constants. Returns the exit code. */
export function main(args: readonly string[]): number {
  try {
    const { values } = parseArgs({
      args: [...args],
      options: { e490: { type: "string" }, cie: { type: "string" } },
    });
    if (values.e490 === undefined || values.cie === undefined) {
      throw new Error(
        "usage: solarFactors --e490 <e490_00a_amo.csv> --cie <CIE_xyz_1931_2deg.csv>",
      );
    }
    const solar = parseE490(readChecked(values.e490, E490_CSV_SHA256));
    const cmf = parseCie(readChecked(values.cie, CIE_CSV_SHA256));
    process.stdout.write(`${describeFactors(solarFactors(solar, cmf))}\n`);
    return 0;
  } catch (error: unknown) {
    console.error(`solar-factors: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
