/**
 * Super-lines for R08.T4.g (plan R08, R08.T4.g): a large line list's intensities binned on a fine
 * wavenumber grid at each temperature, and each node's cell spectrum those bins convolved with the
 * node's Voigt profile by FFT, cut off 25 cm⁻¹ from each bin as R08.T4.d's lines are, then sampled
 * at the cell's equal steps of vacuum wavelength as T4.d's line-by-line spectrum is
 * (`molecularLines.ts`'s `cellSpectrum`), so that the same reduction and tests read it.
 *
 * @remarks
 * - **The bins.** Each line's S(T) is shared between the two grid nodes about its centre in
 *   proportion to its nearness (linear, "cloud-in-cell" binning: R. W. Hockney and J. W. Eastwood,
 *   _Computer Simulation Using Particles_, 1981, §5-2), which keeps each node's total and its first
 *   moment: a super-line at a node stands for the lines within a step of it. The bins depend on T
 *   alone, since every line of a list here takes one width (its default, `exomolLines.ts`), so one
 *   streaming pass over the list fills every temperature's bins, and each (T, p) node is then one
 *   convolution. The step is a twelfth of the Doppler half-width at the grid's lowest wavenumber,
 *   finer than the plan's tenth of the narrowest Voigt half-width, which is never below it.
 * - **The convolution.** At a node the bins are taken over the cell and 25 cm⁻¹ each side,
 *   coarsened by an integer factor m where the node's narrowest Voigt half-width allows (each fine
 *   node shared between the coarse nodes about it, as a line is), so that the step stays under a
 *   tenth of it and under a 600th of the cut-off, and convolved with the profile cut off at
 *   25 cm⁻¹ by a radix-2 FFT (J. W. Cooley and J. W. Tukey, Math. Comput. 19 (1965) 297), written
 *   here. A circular convolution of N points is exact on the cell where N is at least the window's
 *   length, since the cell lies a cut-off inside the window at both ends.
 * - **The cut-off's steps.** Each line's wing stops at the cut-off, at 7–29% of its peak at
 *   5.6–13.3 MPa (N₂O at 100 K; 15% at 8.66 MPa), so the summed spectrum steps there, line by
 *   line; the grid resolves those steps only
 *   to its own step. Bounded by the half-width alone, the step grew with p (1.04 cm⁻¹ at 8.66 MPa
 *   and 100 K for N₂O), and the spectra's error moved with it from one pressure node to the next:
 *   N₂O's 668 nm cell at 100 K erred against a converged step by 3.9 × 10⁻³ to 2.1 × 10⁻² in T at
 *   5.62–13.3 MPa and 4.5 × 10²⁹ cm⁻², and its curve at 8.66 MPa was not cubic in ln p across
 *   them (7.4 × 10⁻³). The bound in cm⁻¹, {@link SUPER_LINE_STEPS_PER_CUTOFF}, holds the step at
 *   0.042 cm⁻¹ or under, the same at every pressure where it binds (above about 0.35–0.8 MPa
 *   over both tables' temperatures; below, the half-width's bound is the finer), where the four
 *   rows agree with a step four times finer to 2.3–5.8 × 10⁻⁵ at that column, the spectra's own
 *   floor there (R08.T4.g's second ruling). A 150th of the cut-off,
 *   0.17 cm⁻¹, left them 4.2–8.7 × 10⁻⁴ apart there (2 × 10⁻³ at 3 × 10³⁰ cm⁻²), though smooth in
 *   p; at 300 K, where helium's cap gates such columns, 1 × 10⁻⁴.
 * - **The binning's smoothing.** Linear binning spreads each line over a triangle of one step,
 *   which broadens a Doppler core by about 10⁻³ of its width at a twelfth of it. The bins'
 *   transform is divided by the binning's own response ({@link assignmentResponse}), as
 *   particle-mesh codes divide by their assignment function (Hockney and Eastwood 1981, §5-6), so
 *   that only the error that depends on where a line falls between two nodes is left, which
 *   averages to nothing over a cell's lines.
 * - **Doppler across the cell.** The Doppler width grows with ν, 0.1–0.2% across a cell. The
 *   profile is convolved at the Doppler widths of the cell's two edges and the results blended
 *   linearly in ν at each point, so that a line's core takes the width at its own wavenumber to
 *   first order (the blend errs by about ¼(Δα ÷ α)² of a Doppler peak, Δα ÷ α the width's
 *   growth across the cell: under 1.5 × 10⁻⁶). Both kernels share one complex FFT, as do both
 *   outputs (two real sequences in one complex transform, E. O. Brigham, _The Fast Fourier
 *   Transform_, 1974, §10-10).
 * - **Sampling.** The convolved spectrum is read at the cell's points by 4-point Lagrange
 *   interpolation on its own grid, at least ten steps a half-width, where the cubic errs by under
 *   1.4 × 10⁻⁵ of a Doppler line's peak and 5.5 × 10⁻⁵ of a Lorentz line's (the remainder's
 *   (9 ÷ 16) h⁴ max|f⁗| ÷ 24). The points are T4.d's rule, a quarter of the narrowest Voigt
 *   half-width or finer and at least 512.
 */

import { type CellNode, cellNodeOf, type NodeSpectra, type ShapePolicy } from "./kDistributions";
import {
  cellPointCount,
  cellSpectrum,
  cellWavenumbersPerCm,
  dopplerHalfWidthPerCm,
  FINE_POINTS_PER_HALF_WIDTH,
  HITRAN_REFERENCE_K,
  type LineShape,
  type LineSpecies,
  linesNear,
  lineStates,
  MIN_CELL_POINTS,
  SECOND_RADIATION_CONSTANT_CM_K,
  STANDARD_ATMOSPHERE_PA,
  voigtHalfWidthPerCm,
  voigtProfile,
} from "./molecularLines";

/** Bin steps a Doppler half-width at the grid's lowest wavenumber: 12, finer than a tenth. */
export const BIN_STEPS_PER_DOPPLER_HALF_WIDTH = 12;

/** The coarsest step a node's bins take, in its narrowest Voigt half-width: under a tenth. */
export const SUPER_LINE_STEPS_PER_HALF_WIDTH = 10;

/**
 * The coarsest step a node's bins take, in its cut-off: under a 600th, 0.042 cm⁻¹ at 25 cm⁻¹, so
 * that the cut-off's steps in the summed spectrum are resolved alike at every pressure and to the
 * spectrum's own convergence. R08.T4.g's second ruling bounds it at 0.17 cm⁻¹ and asks the rows to
 * lie within about 3 × 10⁻⁵ of a converged step at N₂O's worst state: a 600th's lie 2.3–5.8 ×
 * 10⁻⁵ from a step four times finer, at the spectra's own floor there (finer steps scatter by 7 ×
 * 10⁻⁵ among themselves); a 150th's lay 4.2–8.7 × 10⁻⁴ away.
 */
export const SUPER_LINE_STEPS_PER_CUTOFF = 600;

/** One temperature's super-lines: S(T) binned on a uniform wavenumber grid. */
export interface SuperLineBins {
  readonly temperatureK: number;
  /** The wavenumber of bin 0, cm⁻¹. */
  readonly originPerCm: number;
  /** The grid's step, cm⁻¹. */
  readonly stepPerCm: number;
  /** S(T) at each node, cm a molecule (cm⁻¹ ÷ (molecule cm⁻²)), summed over the lines it stands for. */
  readonly values: Float64Array;
}

/**
 * The bin step at a temperature, cm⁻¹: {@link BIN_STEPS_PER_DOPPLER_HALF_WIDTH} a Doppler
 * half-width at the grid's lowest wavenumber, for a molecule of a molar mass, g mol⁻¹.
 */
export function binStepPerCm(
  temperatureK: number,
  lowestPerCm: number,
  molarMassGPerMol: number,
): number {
  return (
    dopplerHalfWidthPerCm(lowestPerCm, temperatureK, molarMassGPerMol) /
    BIN_STEPS_PER_DOPPLER_HALF_WIDTH
  );
}

/** Empty bins over [lowPerCm, highPerCm] at a temperature, the last node at or past the top. */
export function emptyBins(
  temperatureK: number,
  lowPerCm: number,
  highPerCm: number,
  stepPerCm: number,
): SuperLineBins {
  const count = Math.ceil((highPerCm - lowPerCm) / stepPerCm) + 2;
  return {
    temperatureK,
    originPerCm: lowPerCm,
    stepPerCm,
    values: new Float64Array(count),
  };
}

/**
 * Adds a line of intensity S (cm a molecule) at a wavenumber (cm⁻¹) to the bins, shared linearly
 * between the two nodes about it. A line outside the grid is refused.
 *
 * @throws RangeError for a line outside the bins.
 */
export function depositLine(
  bins: SuperLineBins,
  wavenumberPerCm: number,
  intensityCm: number,
): void {
  const x = (wavenumberPerCm - bins.originPerCm) / bins.stepPerCm;
  const k = Math.floor(x);
  if (!(k >= 0 && k + 1 < bins.values.length)) {
    throw new RangeError(`a line at ${wavenumberPerCm} cm-1 lies outside the bins`);
  }
  const f = x - k;
  bins.values[k] = (bins.values[k] ?? 0) + intensityCm * (1 - f);
  bins.values[k + 1] = (bins.values[k + 1] ?? 0) + intensityCm * f;
}

/**
 * The stimulated-emission factor 1 − e^(−c₂ν ÷ T) applied to every node at its own wavenumber, after
 * binning: within a step of a line's own, which moves a node's share by under 3 × 10⁻¹⁰ of itself
 * below 2,000 K at the visible wavenumbers, and the line's total, its first moment kept, at second
 * order, under 10⁻¹⁵ (under 10⁻²¹ to the tables' 500 K).
 */
export function applyStimulatedEmission(bins: SuperLineBins): void {
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  for (let k = 0; k < bins.values.length; k += 1) {
    const nu = bins.originPerCm + k * bins.stepPerCm;
    bins.values[k] = (bins.values[k] ?? 0) * -Math.expm1((-c2 * nu) / bins.temperatureK);
  }
}

/**
 * Bins over a range of node indices, coarsened by an integer factor: coarse node j lies at fine
 * node j m, and takes each fine node within m of it by 1 − |i − j m| ÷ m, as a line is shared
 * (every fine node's total and first moment kept).
 *
 * @param read - The fine bins' value at a node index, 0 beyond the grid.
 * @returns The coarse nodes `first` to `last`, inclusive.
 */
export function coarsenBins(
  read: (index: number) => number,
  factor: number,
  first: number,
  last: number,
): Float64Array {
  const out = new Float64Array(last - first + 1);
  if (factor === 1) {
    for (let j = first; j <= last; j += 1) {
      out[j - first] = read(j);
    }
    return out;
  }
  for (let j = first; j <= last; j += 1) {
    let sum = 0;
    const centre = j * factor;
    for (let i = centre - factor + 1; i < centre + factor; i += 1) {
      sum += read(i) * (1 - Math.abs(i - centre) / factor);
    }
    out[j - first] = sum;
  }
  return out;
}

/** The twiddle factors of an N-point FFT, cos and sin of 2πk ÷ N for k < N ÷ 2, by size. */
const TWIDDLES = new Map<number, { readonly cos: Float64Array; readonly sin: Float64Array }>();

/** The twiddle factors of an N-point transform, computed once a size. */
function twiddles(n: number): { readonly cos: Float64Array; readonly sin: Float64Array } {
  const found = TWIDDLES.get(n);
  if (found !== undefined) {
    return found;
  }
  const half = n >> 1;
  const made = {
    cos: Float64Array.from({ length: half }, (_, k) => Math.cos((2 * Math.PI * k) / n)),
    sin: Float64Array.from({ length: half }, (_, k) => Math.sin((2 * Math.PI * k) / n)),
  };
  TWIDDLES.set(n, made);
  return made;
}

/**
 * An in-place complex discrete Fourier transform of a power-of-two length, X_k = Σ_j x_j
 * e^(∓2πi jk ÷ N): radix-2 decimation in time (Cooley and Tukey 1965), its input in bit-reversed
 * order, then log₂ N butterfly passes. The inverse takes e^(+2πi jk ÷ N) and divides by N.
 *
 * @throws RangeError for a length that is not a power of two, or real and imaginary parts of
 *   different lengths.
 */
export function fft(re: Float64Array, im: Float64Array, inverse = false): void {
  const n = re.length;
  if (im.length !== n || n === 0 || (n & (n - 1)) !== 0) {
    throw new RangeError(`an FFT takes two arrays of one power-of-two length, not ${n}`);
  }
  for (let i = 1, j = 0; i < n; i += 1) {
    let bit = n >> 1;
    for (; (j & bit) !== 0; bit >>= 1) {
      j ^= bit;
    }
    j ^= bit;
    if (i < j) {
      const tr = re[i] ?? 0;
      re[i] = re[j] ?? 0;
      re[j] = tr;
      const ti = im[i] ?? 0;
      im[i] = im[j] ?? 0;
      im[j] = ti;
    }
  }
  const { cos, sin } = twiddles(n);
  const sign = inverse ? 1 : -1;
  for (let size = 2; size <= n; size <<= 1) {
    const half = size >> 1;
    const stride = n / size;
    for (let start = 0; start < n; start += size) {
      for (let k = 0; k < half; k += 1) {
        const wr = cos[k * stride] ?? 1;
        const wi = sign * (sin[k * stride] ?? 0);
        const a = start + k;
        const b = a + half;
        const br = re[b] ?? 0;
        const bi = im[b] ?? 0;
        const xr = br * wr - bi * wi;
        const xi = br * wi + bi * wr;
        const ar = re[a] ?? 0;
        const ai = im[a] ?? 0;
        re[b] = ar - xr;
        im[b] = ai - xi;
        re[a] = ar + xr;
        im[a] = ai + xi;
      }
    }
  }
  if (inverse) {
    for (let i = 0; i < n; i += 1) {
      re[i] = (re[i] ?? 0) / n;
      im[i] = (im[i] ?? 0) / n;
    }
  }
}

/** The least power of two at or above a length. */
export function powerOfTwoAtLeast(length: number): number {
  let n = 1;
  while (n < length) {
    n <<= 1;
  }
  return n;
}

/**
 * A Voigt profile cut off at a distance, on a uniform grid laid circularly for an N-point
 * transform: entry j and N − j hold V(j h) times the share of the step about j h, [j h − h ÷ 2,
 * j h + h ÷ 2], that lies within the cut-off (1 inside it, 0 beyond, linear across it), so that the
 * cut-off falls at the given distance in the profile's integral whatever the step. A cut-off
 * rounded to a whole step would move it by up to a step, which at 10 MPa (a step near 0.8 cm⁻¹, a
 * line's wing at 25 cm⁻¹ a tenth of its peak) moves a cell's least cross-sections unevenly from one
 * pressure node to the next.
 */
export function circularKernel(
  n: number,
  stepPerCm: number,
  cutoffPerCm: number,
  dopplerPerCm: number,
  lorentzPerCm: number,
): Float64Array {
  const kernel = new Float64Array(n);
  const reach = Math.min(Math.ceil(cutoffPerCm / stepPerCm + 0.5), (n >> 1) - 1);
  kernel[0] = voigtProfile(0, dopplerPerCm, lorentzPerCm);
  for (let j = 1; j <= reach; j += 1) {
    const share = Math.min(Math.max(cutoffPerCm / stepPerCm - j + 0.5, 0), 1);
    const v = share * voigtProfile(j * stepPerCm, dopplerPerCm, lorentzPerCm);
    kernel[j] = v;
    kernel[n - j] = v;
  }
  return kernel;
}

/**
 * The weights of 4-point Lagrange interpolation at a fraction t ∈ [0, 1) between nodes 0 and 1,
 * on nodes −1, 0, 1 and 2.
 */
function cubicWeights(t: number): readonly [number, number, number, number] {
  return [
    (-t * (t - 1) * (t - 2)) / 6,
    ((t + 1) * (t - 1) * (t - 2)) / 2,
    (-(t + 1) * t * (t - 2)) / 2,
    ((t + 1) * t * (t - 1)) / 6,
  ];
}

/** What a super-line species gives its cells besides the bins: its one line width and mass. */
export interface SuperLineSpecies {
  /** The bins' grid at a temperature: its origin and step, cm⁻¹, and its node count. */
  readonly gridAt: (temperatureK: number) => {
    readonly originPerCm: number;
    readonly stepPerCm: number;
    readonly count: number;
  };
  /**
   * The bins at a temperature over a run of node indices, inclusive, read on demand
   * (`exomolLines.ts`); nodes beyond the grid read 0.
   */
  readonly binsAt: (temperatureK: number, first: number, last: number) => BinsWindow;
  /** The molar mass, g mol⁻¹, for the Doppler width. */
  readonly molarMassGPerMol: number;
  /** The lines' one air half-width at 296 K and 1 atm, cm⁻¹ atm⁻¹. */
  readonly airHalfWidthPerCmAtm: number;
  /** Its temperature exponent: γ ∝ (296 ÷ T)^n. */
  readonly airExponent: number;
  /** Whether the width is a stated estimate (ExoMol's default), its nodes then `estimated`. */
  readonly widthsEstimated: boolean;
}

/** A run of fine bins: the node indices `first` to `first + values.length − 1` on a grid. */
export interface BinsWindow {
  readonly originPerCm: number;
  readonly stepPerCm: number;
  readonly first: number;
  readonly values: Float64Array;
}

/** A cell's super-line spectrum at a node, with the S-weighted width and the bins' total. */
export interface SuperLineCell {
  /** σ at the cell's points, cm² a molecule, as `cellSpectrum` samples them. */
  readonly crossSectionsCm2: Float64Array;
  /** The lines' Lorentz half-width at the node, cm⁻¹: one for every line. */
  readonly lorentzPerCm: number;
  /** The total S(T) of the bins within the cut-off of the cell, cm a molecule. */
  readonly windowIntensityCm: number;
}

/**
 * The point count a cell's super-line spectrum needs at a node: T4.d's rule
 * (`cellPointCount`) for the narrowest Voigt half-width a line in the window can have, the Doppler
 * width at the window's lowest wavenumber, the cut-off below the cell.
 */
export function superLinePointCount(
  cellNm: number,
  temperatureK: number,
  lorentzPerCm: number,
  molarMassGPerMol: number,
  cutoffPerCm: number,
): number {
  const [low, high] = cellWavenumbersPerCm(cellNm);
  const least = voigtHalfWidthPerCm(
    lorentzPerCm,
    dopplerHalfWidthPerCm(low - cutoffPerCm, temperatureK, molarMassGPerMol),
  );
  return Math.max(MIN_CELL_POINTS, Math.ceil((high - low) / (least / FINE_POINTS_PER_HALF_WIDTH)));
}

/** The line width at a node: γ_air (296 ÷ T)^n p ÷ 1 atm, cm⁻¹. */
export function superLineLorentzPerCm(
  species: SuperLineSpecies,
  temperatureK: number,
  pressurePa: number,
): number {
  return (
    species.airHalfWidthPerCmAtm *
    (HITRAN_REFERENCE_K / temperatureK) ** species.airExponent *
    (pressurePa / STANDARD_ATMOSPHERE_PA)
  );
}

/** One cell's bins at one temperature and coarsening, transformed, kept across its pressures: a cache entry its callers own. */
export interface TransformedWindow {
  readonly key: string;
  readonly n: number;
  readonly stepPerCm: number;
  /** The wavenumber of the window's first coarse node, cm⁻¹. */
  readonly firstPerCm: number;
  readonly re: Float64Array;
  readonly im: Float64Array;
  readonly totalCm: number;
}

/**
 * A cell's super-line spectrum at a node (the module's remarks), or `undefined` where no bin
 * within the cut-off of the cell holds a line.
 *
 * @param points - The cell's point count ({@link superLinePointCount}, or more).
 * @param cache - Keeps one window's transform between calls at the same cell, temperature and
 *   coarsening; a map the caller owns, cleared between cells.
 * @throws RangeError where the bins' own step is not under a tenth of the node's narrowest Voigt
 *   half-width and {@link SUPER_LINE_STEPS_PER_CUTOFF} times under the cut-off.
 */
export function superLineCell(
  cellNm: number,
  species: SuperLineSpecies,
  temperatureK: number,
  pressurePa: number,
  cutoffPerCm: number,
  points: number,
  cache: Map<string, TransformedWindow>,
): SuperLineCell | undefined {
  const [low, high] = cellWavenumbersPerCm(cellNm);
  const lorentz = superLineLorentzPerCm(species, temperatureK, pressurePa);
  const mass = species.molarMassGPerMol;
  const dopplerLow = dopplerHalfWidthPerCm(low, temperatureK, mass);
  const dopplerHigh = dopplerHalfWidthPerCm(high, temperatureK, mass);
  const narrowest = voigtHalfWidthPerCm(
    lorentz,
    dopplerHalfWidthPerCm(low - cutoffPerCm, temperatureK, mass),
  );
  const grid = species.gridAt(temperatureK);
  const limit = Math.min(
    narrowest / SUPER_LINE_STEPS_PER_HALF_WIDTH,
    cutoffPerCm / SUPER_LINE_STEPS_PER_CUTOFF,
  );
  if (!(grid.stepPerCm < limit)) {
    throw new RangeError(
      `bins of ${grid.stepPerCm} cm-1 at ${temperatureK} K are not under a tenth of ${cellNm} nm's narrowest half-width and a ${SUPER_LINE_STEPS_PER_CUTOFF}th of the cut-off`,
    );
  }
  const factor = Math.max(1, Math.ceil(limit / grid.stepPerCm) - 1);
  const window = transformedWindow(
    cellNm,
    temperatureK,
    species,
    grid,
    factor,
    [low, high],
    cutoffPerCm,
    cache,
  );
  if (!(window.totalCm > 0)) {
    return undefined;
  }
  const { n, stepPerCm, re: binsRe, im: binsIm } = window;
  // Both kernels in one transform: real and even, so each one's transform is real.
  const kernelRe = circularKernel(n, stepPerCm, cutoffPerCm, dopplerLow, lorentz);
  const kernelIm = circularKernel(n, stepPerCm, cutoffPerCm, dopplerHigh, lorentz);
  fft(kernelRe, kernelIm);
  // B K_low + i B K_high: its inverse is y_low + i y_high, both real.
  const outRe = new Float64Array(n);
  const outIm = new Float64Array(n);
  for (let k = 0; k < n; k += 1) {
    const kl = kernelRe[k] ?? 0;
    const kh = kernelIm[k] ?? 0;
    const br = binsRe[k] ?? 0;
    const bi = binsIm[k] ?? 0;
    // (br + i bi) kl + i (br + i bi) kh.
    outRe[k] = br * kl - bi * kh;
    outIm[k] = bi * kl + br * kh;
  }
  fft(outRe, outIm, true);
  const sigma = new Float64Array(points);
  const lowNm = cellNm - 0.5;
  for (let j = 0; j < points; j += 1) {
    const nu = 1e7 / (lowNm + (j + 0.5) / points);
    const x = (nu - window.firstPerCm) / stepPerCm;
    const i = Math.floor(x);
    const w = cubicWeights(x - i);
    let a = 0;
    let b = 0;
    for (let q = 0; q < 4; q += 1) {
      a += (w[q] ?? 0) * (outRe[i - 1 + q] ?? 0);
      b += (w[q] ?? 0) * (outIm[i - 1 + q] ?? 0);
    }
    const t = (nu - low) / (high - low);
    // The transforms' rounding, about 10⁻¹⁶ of the peak, of either sign; a cross-section is never
    // below 0.
    sigma[j] = Math.max(a + (b - a) * t, 0);
  }
  return { crossSectionsCm2: sigma, lorentzPerCm: lorentz, windowIntensityCm: window.totalCm };
}

/**
 * The binning's response at a frequency f (cm): linear binning onto the fine grid, a triangle of
 * one step whose transform is sinc²(π f h) (Hockney and Eastwood 1981, §5-6-1), then each fine node
 * shared between the coarse nodes about it, a sampled triangle of m steps, whose transform is
 * [sin(π f m h) ÷ (m sin(π f h))]² (Fejér's kernel). Dividing the bins' transform by it removes the
 * smoothing binning adds to every line, about 10⁻³ of a Doppler core's width at a twelfth of it;
 * what is left depends on where a line falls between nodes and averages to nothing.
 */
export function assignmentResponse(
  frequencyCm: number,
  fineStepPerCm: number,
  factor: number,
): number {
  const x = Math.PI * frequencyCm * fineStepPerCm;
  if (x === 0) {
    return 1;
  }
  const fine = (Math.sin(x) / x) ** 2;
  const coarse = factor === 1 ? 1 : (Math.sin(factor * x) / (factor * Math.sin(x))) ** 2;
  return fine * coarse;
}

/**
 * The cell's window of bins at a coarsening, transformed: the coarse nodes from a cut-off below the
 * cell to a cut-off above it, laid from index 0 of an N-point transform, N the least power of two
 * holding them (on which the circular convolution is exact over the cell, the module's remarks);
 * kept in the cache for the cell's other pressures at the same temperature and coarsening.
 */
function transformedWindow(
  cellNm: number,
  temperatureK: number,
  species: SuperLineSpecies,
  grid: { readonly originPerCm: number; readonly stepPerCm: number },
  factor: number,
  [lowPerCm, highPerCm]: readonly [number, number],
  cutoffPerCm: number,
  cache: Map<string, TransformedWindow>,
): TransformedWindow {
  const key = `${cellNm}:${temperatureK}:${factor}`;
  const found = cache.get(key);
  if (found !== undefined) {
    return found;
  }
  const stepPerCm = grid.stepPerCm * factor;
  const first = Math.floor((lowPerCm - cutoffPerCm - grid.originPerCm) / stepPerCm) - 1;
  const last = Math.ceil((highPerCm + cutoffPerCm - grid.originPerCm) / stepPerCm) + 1;
  // Every fine node a coarse node of the window takes.
  const fine = species.binsAt(
    temperatureK,
    first * factor - factor + 1,
    last * factor + factor - 1,
  );
  const read = (index: number): number => fine.values[index - fine.first] ?? 0;
  const coarse = coarsenBins(read, factor, first, last);
  // Two more than the window, for the kernel's half-step past the cut-off (its ramp), so that no
  // offset from the cell into the window wraps.
  const n = powerOfTwoAtLeast(coarse.length + 2);
  const re = new Float64Array(n);
  re.set(coarse);
  const im = new Float64Array(n);
  let totalCm = 0;
  for (const v of coarse) {
    totalCm += v;
  }
  fft(re, im);
  // The binning's own smoothing, undone at every frequency (the module's remarks).
  for (let k = 0; k < n; k += 1) {
    const response = assignmentResponse(
      (k <= n >> 1 ? k : k - n) / (n * stepPerCm),
      grid.stepPerCm,
      factor,
    );
    re[k] = (re[k] ?? 0) / response;
    im[k] = (im[k] ?? 0) / response;
  }
  const made: TransformedWindow = {
    key,
    n,
    stepPerCm,
    firstPerCm: grid.originPerCm + first * stepPerCm,
    re,
    im,
    totalCm,
  };
  cache.set(key, made);
  return made;
}

/** One merged cell's spectrum at a node, with its two parts' means on the same points. */
export interface MergedCell {
  /** σ at the cell's points, cm² a molecule: the lines' and the super-lines' summed. */
  readonly crossSectionsCm2: Float64Array;
  /** The intensity-weighted γ_L over both parts, cm⁻¹. */
  readonly lorentzPerCm: number;
  /** Each part's mean σ over the points, cm² a molecule: the lines', then the super-lines'. */
  readonly linesMeanCm2: number;
  readonly superLinesMeanCm2: number;
  /** Whether the super-lines add anything within the cut-off of the cell. */
  readonly withSuperLines: boolean;
}

/**
 * A cell's spectrum at a node from a line list drawn line by line (HITRAN2024's, T4.d's) and a
 * super-line list holding only the bands the first lacks (`decision-r08-t4d-kdist-chain.md` §11),
 * summed on the same points: the finer of the two parts' point counts. Its γ_L is weighted by each
 * line's S and by the super-lines' S within the cut-off; or `undefined` where neither part has a
 * line within the cut-off.
 */
export function mergedCell(
  cellNm: number,
  lines: LineSpecies,
  superLines: SuperLineSpecies,
  temperatureK: number,
  pressurePa: number,
  shape: LineShape,
  cache: Map<string, TransformedWindow>,
): MergedCell | undefined {
  const [low, high] = cellWavenumbersPerCm(cellNm);
  const indices = linesNear(lines.lines, low, high, shape.cutoffPerCm);
  const states = lineStates(lines, indices, temperatureK, pressurePa);
  const lorentz = superLineLorentzPerCm(superLines, temperatureK, pressurePa);
  const points = Math.max(
    indices.length > 0 ? cellPointCount(cellNm, states) : 0,
    superLinePointCount(
      cellNm,
      temperatureK,
      lorentz,
      superLines.molarMassGPerMol,
      shape.cutoffPerCm,
    ),
  );
  const fromLines =
    indices.length > 0
      ? cellSpectrum(cellNm, lines, indices, states, points, shape).crossSectionsCm2
      : new Float64Array(points);
  const fromSuperLines = superLineCell(
    cellNm,
    superLines,
    temperatureK,
    pressurePa,
    shape.cutoffPerCm,
    points,
    cache,
  );
  if (indices.length === 0 && fromSuperLines === undefined) {
    return undefined;
  }
  const sigma = new Float64Array(points);
  let linesSum = 0;
  let superSum = 0;
  for (let j = 0; j < points; j += 1) {
    const a = fromLines[j] ?? 0;
    const b = fromSuperLines?.crossSectionsCm2[j] ?? 0;
    linesSum += a;
    superSum += b;
    sigma[j] = a + b;
  }
  let weighted = 0;
  let total = 0;
  for (const s of states) {
    weighted += s.intensityCm * s.lorentzPerCm;
    total += s.intensityCm;
  }
  const superIntensity = fromSuperLines?.windowIntensityCm ?? 0;
  weighted += superIntensity * lorentz;
  total += superIntensity;
  return {
    crossSectionsCm2: sigma,
    lorentzPerCm: total > 0 ? weighted / total : 0,
    linesMeanCm2: linesSum / points,
    superLinesMeanCm2: superSum / points,
    withSuperLines: fromSuperLines !== undefined,
  };
}

/**
 * N₂O's merged spectra (R08.T4.g; `decision-r08-t4d-kdist-chain.md` §11) as a {@link NodeSpectra}:
 * below `splitNm`, every band of the super-line list (`all`); from it, the line list's lines line by
 * line with the super-line list's bands the line list lacks (`apart` filtered out, `lacking`), each
 * band from one list only ({@link mergedCell}). A node with super-lines is `estimated` where their
 * widths are a stated estimate.
 *
 * @throws Error for a policy of more than one variant, a far-wing factor or a pedestal.
 */
export function mergedSpectra(
  lines: LineSpecies,
  all: SuperLineSpecies,
  lacking: SuperLineSpecies,
  policy: ShapePolicy,
  splitNm: number,
): NodeSpectra {
  const below = superLineSpectra(all, policy);
  const shape: LineShape = { cutoffPerCm: policy.cutoffPerCm, pedestal: false };
  const cache = new Map<string, TransformedWindow>();
  let cacheCell = Number.NaN;
  return (cellNm, temperatureK, pressurePa, weights): CellNode | undefined => {
    if (cellNm < splitNm) {
      return below(cellNm, temperatureK, pressurePa, weights);
    }
    if (cellNm !== cacheCell) {
      cache.clear();
      cacheCell = cellNm;
    }
    const cell = mergedCell(cellNm, lines, lacking, temperatureK, pressurePa, shape, cache);
    return cell === undefined
      ? undefined
      : cellNodeOf(
          cellNm,
          cell.crossSectionsCm2,
          cell.lorentzPerCm,
          shape,
          policy,
          cell.withSuperLines && lacking.widthsEstimated,
        );
  };
}

/**
 * A super-line species as a {@link NodeSpectra}, under a one-variant policy without a far-wing
 * factor: each node's spectrum from its temperature's bins ({@link superLineCell}), its node
 * `estimated` where the policy's half-width is passed or the width is a stated estimate.
 *
 * @throws Error for a policy of more than one variant or a far-wing factor, which super-lines do
 *   not take.
 */
export function superLineSpectra(species: SuperLineSpecies, policy: ShapePolicy): NodeSpectra {
  if (policy.variants.length !== 1 || policy.variants[0]?.chi !== undefined || policy.pedestal) {
    throw new Error("super-lines take one variant, without a far-wing factor or a pedestal");
  }
  const shape: LineShape = { cutoffPerCm: policy.cutoffPerCm, pedestal: false };
  const cache = new Map<string, TransformedWindow>();
  let cacheCell = Number.NaN;
  return (cellNm, temperatureK, pressurePa): CellNode | undefined => {
    if (cellNm !== cacheCell) {
      cache.clear();
      cacheCell = cellNm;
    }
    const lorentz = superLineLorentzPerCm(species, temperatureK, pressurePa);
    const points = superLinePointCount(
      cellNm,
      temperatureK,
      lorentz,
      species.molarMassGPerMol,
      shape.cutoffPerCm,
    );
    const cell = superLineCell(
      cellNm,
      species,
      temperatureK,
      pressurePa,
      shape.cutoffPerCm,
      points,
      cache,
    );
    return cell === undefined
      ? undefined
      : cellNodeOf(
          cellNm,
          cell.crossSectionsCm2,
          cell.lorentzPerCm,
          shape,
          policy,
          species.widthsEstimated,
        );
  };
}
