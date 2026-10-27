/**
 * Line absorbers' k-distributions (plan R08, R08.T4.d; Design note 5; `decision-r08-t4d-kdist.md`):
 * each 1 nm cell's absorption, reduced offline from its molecular lines
 * (`src/tools/kDistributions.ts`), as a few g-points whose cross-sections reproduce the cell's
 * curve of growth, on a grid of temperature and broadening pressure.
 *
 * @remarks
 * A cell's mean transmittance through a column u at a node is Σ_g w_g e^(−σ_g u) (correlated-k;
 * A. A. Lacis and V. Oinas, J. Geophys. Res. 96 (1991) 9027). The weights are the cell's, the same
 * at every node, so that a g-point's σ is interpolated between nodes:
 *
 * - in temperature, once a body, by 4-point Lagrange in T on ln σ over the four nodes nearest
 *   ({@link kDistributionAt}), held beyond the grid (`estimated`);
 * - in broadening pressure, where a species is read off the stored nodes, by 4-point Lagrange in
 *   ln p on ln σ ({@link cellSigmasAt}), held beyond the grid: below 10 Pa a cell's lines are
 *   Doppler-broadened, and above 10 MPa its node is `estimated`.
 *
 * Where any of the four values is 0 the interpolation is linear in σ between the two nodes about
 * it. The broadening pressure is the pressure at which air would give the lines their widths: a
 * mixture's pressure times {@link broadeningFactor}, the mole-fraction-weighted mean of HITRAN's
 * half-width ratios to air's, each taken to the profile's temperature by its own exponent over
 * air's (`decision-r08-t4d-kdist-chain.md` §13, ruling 4).
 *
 * CO₂'s table holds two variants on shared g-points, its far wing under CO₂–CO₂'s χ (`self`) and
 * under CO₂–N₂'s (`air`), mixed at arrival as σ(g) = w σ_self(g) + (1 − w) σ_air(g), w the self
 * share of the Lorentz width, x_CO₂ r_self ÷ Σ_j x_j r_j (`decision-r08-t4d-kdist-chain.md` §7).
 *
 * The values are stored as 16-bit ln σ, each cell with its own offset and step, in little-endian
 * binary parts beside a JSON sidecar of the table's grid and records
 * (`decision-r08-t4d-kdist-chain.md` §9), each part under the 500 kB file hook, which
 * {@link readKDistribution} joins; {@link loadKDistributions} imports them per species, for the
 * optics worker, never the render thread's bundle: the parts by Vite's `?url`, as the height worker
 * loads its wasm, or, in tests, by a reader of `?inline` assets.
 */

import { BAKE_RANGE_NM } from "../medium";

/** Whether a table's values rest on measured line data or on a stated estimate. */
export type KDistributionBasis = "measured" | "estimated";

/** One 1 nm cell's k-distribution over the grid's nodes, its values still as stored. */
export interface KDistributionCell {
  /** The cell's centre, nm: the cell is [λ − 0.5, λ + 0.5) on vacuum wavelengths. */
  readonly cellNm: number;
  /** The g-points' weights, summing to 1. */
  readonly weights: Float64Array;
  /** ln(σ ÷ 1 m²) of code 1. */
  readonly lnOffset: number;
  /** The step in ln σ between codes. */
  readonly lnStep: number;
  /** The codes at each variant, node and g-point, `[((v × T + t) × P + p) × G + g]`: 0 for σ = 0. */
  readonly codes: Uint16Array;
}

/** A species' k-distributions on its grid of temperature and broadening pressure. */
export interface KDistributionTable {
  /** The registry key. */
  readonly species: string;
  /** The lines' citation and how they were reduced. */
  readonly source: string;
  readonly basis: KDistributionBasis;
  /** The grid's temperatures, K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** The temperatures over which the values are `measured`, K, within the grid. */
  readonly measuredRangeK: readonly [number, number];
  /** The grid's air-equivalent broadening pressures, Pa, ascending. */
  readonly pressuresPa: ReadonlyArray<number>;
  /** Whether the held 10 Pa table stays `measured` below the grid, by its recorded 1 Pa check. */
  readonly measuredBelowGrid: boolean;
  /**
   * At each temperature node, the highest pressure node whose line shape is measured, or −1:
   * beyond it the cell's S-weighted Lorentz half-width passes the shape's measured limit and the
   * node is `estimated`.
   */
  readonly measuredShapeTo: ReadonlyArray<number>;
  /**
   * The temperature intervals where the chain test still failed at 12.5 K steps, K: the species is
   * `estimated` within them (`decision-r08-t4d-kdist-chain.md`, ruling 4).
   */
  readonly estimatedTemperaturesK: ReadonlyArray<readonly [number, number]>;
  /**
   * The variants' broadeners, in the values' order: `["air"]`, or CO₂'s `["self", "air"]`, its two
   * far-wing shapes.
   */
  readonly variants: ReadonlyArray<"self" | "air">;
  /**
   * `[t][v]`: whether variant v's far-wing χ is held at temperature node t, beyond its set's fit
   * (`decision-r08-t4d-kdist-chain.md` §14). R08.T10.a bounds a held χ's effect on the body's own
   * column; the table marks no such node `estimated` itself.
   */
  readonly chiHeld: ReadonlyArray<ReadonlyArray<boolean>>;
  /**
   * Each broadening gas's half-widths over air's, by registry key, with `self` for the species
   * itself (HITRAN2024's γ_self, γ_H2, γ_He, γ_CO2 and γ_H2O where it gives them). A gas not listed
   * broadens as air does. Unless it is N₂, O₂ or Ar it is then `estimated` where it has a permanent
   * dipole ({@link DIPOLAR_BROADENERS}) or the table has two variants, and is otherwise weighed by
   * R08.T10.c's effect bound ({@link Broadening.widthlessShare}).
   */
  readonly broadening: ReadonlyMap<string, number>;
  /**
   * Each listed gas's temperature exponent over air's, n̄_j − n̄_air, by the same keys: its ratio
   * at T is r_j (296 ÷ T)^(n̄_j − n̄_air). A gas not listed takes 0.
   */
  readonly broadeningExponents: ReadonlyMap<string, number>;
  /** The cells with lines, ascending, each once. */
  readonly cells: ReadonlyArray<KDistributionCell>;
  /**
   * `[t × P + p]`: at each node the largest share over the cells and variants of a Lorentz line's
   * strength 3–25 cm⁻¹ from its centre, (2 ÷ π)[arctan(25 ÷ γ_L) − arctan(3 ÷ γ_L)], γ_L the cell's
   * intensity-weighted Lorentz half-width, to two significant figures, as the reduction records it
   * (`src/tools/kDistributions.ts`'s `nearWingShare`); absent for a sidecar that records none.
   * R08.T10.a recovers γ_L from it for the held-χ bound (`negligible.ts`).
   */
  readonly nearWingShare?: Float64Array;
  /**
   * A two-variant table's fixed-g mix record ({@link MixLimits}; `decision-r08-t4d-kdist-chain.md`
   * §16), absent for a table of one variant or one reduced before §16.
   */
  readonly mixLimits?: MixLimits;
}

/**
 * Where a two-variant table's fixed-g mix of its variants, σ(g) = w σ_self(g) + (1 − w) σ_air(g),
 * first errs past the chain tier, 2 × 10⁻³, against line by line (`decision-r08-t4d-kdist-chain.md`
 * §16): recorded by the reduction, not gated, for R08.T10.a to label a body by its own column.
 */
export interface MixLimits {
  /** The self shares w the mix was tested at, ascending in (0, 1). */
  readonly shares: Float64Array;
  /** The temperatures it was tested at, K, ascending: the grid's nodes and the midpoints. */
  readonly temperaturesK: Float64Array;
  /** Each share's worst error within the column cap, over the cells and every tested state. */
  readonly worstErrors: Float64Array;
  /**
   * `[cell][share]`, the cells in their order: u_mix(w, T) at each tested temperature, m⁻²; ∞
   * where the mix never errs past the tier there, or the sample lies inside one of the table's
   * `estimated` temperature intervals (§17).
   */
  readonly columnsM2: ReadonlyArray<ReadonlyArray<Float64Array>>;
}

/** A binary part as the sidecar lists it. */
export interface KDistributionPartListing {
  /** Its file, beside the sidecar. */
  readonly file: string;
  readonly cells: number;
  readonly bytes: number;
  readonly sha256: string;
}

/** A species' k-distribution sidecar, as its JSON import reads it. */
export interface KDistributionFile {
  readonly species: string;
  readonly source: string;
  readonly reduction: string;
  readonly basis: string;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly measuredRangeK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
  readonly measuredBelowGrid: boolean;
  readonly measuredShapeTo: ReadonlyArray<number>;
  readonly estimatedTemperaturesK: ReadonlyArray<ReadonlyArray<number>>;
  readonly broadening: Readonly<Record<string, number>>;
  readonly broadeningExponents: Readonly<Record<string, number>>;
  readonly variants: ReadonlyArray<string>;
  readonly chiHeld: ReadonlyArray<ReadonlyArray<boolean>>;
  /** Each node's largest near-wing share, `[t × P + p]` ({@link KDistributionTable.nearWingShare}). */
  readonly nearWingShare?: ReadonlyArray<number>;
  /** A two-variant table's fixed-g mix record, its columns cm⁻² and null for none (§16). */
  readonly mixing?: {
    readonly shares: ReadonlyArray<number>;
    readonly temperaturesK: ReadonlyArray<number>;
    readonly worstErrors: ReadonlyArray<number>;
    readonly limitColumnsCm2: ReadonlyArray<ReadonlyArray<ReadonlyArray<number | null>>>;
  };
  readonly parts: ReadonlyArray<KDistributionPartListing>;
}

/** A binary part's magic, `HYKD`, and the format's version (`src/tools/kDistributions.ts`). */
const PART_MAGIC = "HYKD";
const PART_VERSION = 1;

/** A binary part's header bytes: the magic, then six u16. */
const PART_HEADER_BYTES = 16;

/** One cell as a binary part holds it, before its checks. */
interface PartCell {
  readonly nm: number;
  readonly weights: Float64Array;
  readonly lnOffset: number;
  readonly lnStep: number;
  readonly codes: Uint16Array;
}

/**
 * A binary part's cells, little-endian (`src/tools/kDistributions.ts`'s `kDistributionPart`):
 * `HYKD`, u16 version, cell count, variants, temperatures, pressures and 0; then per cell u16 nm,
 * u16 G, f64 ln σ of code 1, f64 its step, G f64 weights, and V × T × P × G u16 codes.
 *
 * @throws Error naming the part for a wrong magic, version or grid, or bytes that do not hold its
 *   cells exactly.
 */
function partCells(
  buffer: ArrayBuffer,
  where: string,
  expected: {
    readonly cells: number;
    readonly variants: number;
    readonly temperatures: number;
    readonly pressures: number;
  },
): PartCell[] {
  const view = new DataView(buffer);
  const magic = String.fromCharCode(...new Uint8Array(buffer, 0, Math.min(4, buffer.byteLength)));
  if (buffer.byteLength < PART_HEADER_BYTES || magic !== PART_MAGIC) {
    throw new Error(`${where} is not a k-distribution part`);
  }
  const header = [1, 2, 3, 4, 5].map((k) => view.getUint16(2 + 2 * k, true));
  const [version, count, variants, temperatures, pressures] = header;
  if (
    version !== PART_VERSION ||
    count !== expected.cells ||
    variants !== expected.variants ||
    temperatures !== expected.temperatures ||
    pressures !== expected.pressures
  ) {
    throw new Error(
      `${where}'s header [${header.join(", ")}] is not version ${PART_VERSION} of its sidecar's grid`,
    );
  }
  const nodes = expected.variants * expected.temperatures * expected.pressures;
  const cells: PartCell[] = [];
  let at = PART_HEADER_BYTES;
  for (let c = 0; c < expected.cells; c += 1) {
    if (at + 20 > buffer.byteLength) {
      throw new Error(`${where} ends inside its cell ${c + 1}`);
    }
    const nm = view.getUint16(at, true);
    const g = view.getUint16(at + 2, true);
    const lnOffset = view.getFloat64(at + 4, true);
    const lnStep = view.getFloat64(at + 12, true);
    at += 20;
    if (at + 8 * g + 2 * nodes * g > buffer.byteLength) {
      throw new Error(`${where} ends inside its ${nm} nm cell`);
    }
    const weights = Float64Array.from({ length: g }, (_, k) => view.getFloat64(at + 8 * k, true));
    at += 8 * g;
    const codes = Uint16Array.from({ length: nodes * g }, (_, k) =>
      view.getUint16(at + 2 * k, true),
    );
    at += 2 * nodes * g;
    cells.push({ nm, weights, lnOffset, lnStep, codes });
  }
  if (at !== buffer.byteLength) {
    throw new Error(`${where} holds ${buffer.byteLength - at} bytes past its cells`);
  }
  return cells;
}

/** Whether values ascend strictly from above 0, all finite. */
function ascendingPositive(values: ReadonlyArray<number>): boolean {
  return (
    values.length > 0 &&
    values.every((v, i) => Number.isFinite(v) && v > 0 && v > (values[i - 1] ?? 0))
  );
}

/** The tolerance a cell's weights are held to summing to 1. */
const WEIGHT_SUM_TOLERANCE = 1e-6;

/**
 * A species' k-distribution from its sidecar and its binary parts in the sidecar's order, checked:
 * a known basis; ascending temperatures and pressures; a measured range within the grid's
 * temperatures; finite broadening ratios above 0, and a finite exponent offset for listed gases
 * alone; each part's header its sidecar's; each cell
 * inside the bake range, once, ascending, with weights above 0 that sum to 1, a finite step above 0
 * and a code at every node and g-point; and near-wing shares, where recorded, one a node in [0, 1].
 *
 * @throws Error naming the file and the check it fails.
 */
export function readKDistribution(
  first: KDistributionFile,
  buffers: ReadonlyArray<ArrayBuffer>,
  name: string,
): KDistributionTable {
  const where = `${name}'s ${first.species}`;
  if (buffers.length !== first.parts.length) {
    throw new Error(`${where} lists ${first.parts.length} parts and ${buffers.length} were read`);
  }
  const basis = first.basis;
  if (basis !== "measured" && basis !== "estimated") {
    throw new Error(`${where}'s basis ${basis} is neither measured nor estimated`);
  }
  if (!ascendingPositive(first.temperaturesK) || !ascendingPositive(first.pressuresPa)) {
    throw new Error(`${where}'s temperatures or pressures do not ascend from above 0`);
  }
  const [lowK, highK] = first.measuredRangeK;
  const coldest = first.temperaturesK[0] ?? Number.NaN;
  const hottest = first.temperaturesK.at(-1) ?? Number.NaN;
  if (
    lowK === undefined ||
    highK === undefined ||
    first.measuredRangeK.length !== 2 ||
    !(lowK >= coldest && highK <= hottest && highK >= lowK)
  ) {
    throw new Error(`${where}'s measured range does not lie within its temperatures`);
  }
  if (
    typeof first.measuredBelowGrid !== "boolean" ||
    !first.chiHeld.every((row) => row.every((flag) => typeof flag === "boolean")) ||
    !first.measuredShapeTo.every(
      (p) => Number.isInteger(p) && p >= -1 && p < first.pressuresPa.length,
    )
  ) {
    throw new Error(
      `${where}'s measuredBelowGrid, held-χ flags or line-shape limits are not of their kinds`,
    );
  }
  if (first.measuredShapeTo.length !== first.temperaturesK.length) {
    throw new Error(`${where}'s shape record does not give each temperature node`);
  }
  const estimatedTemperaturesK = first.estimatedTemperaturesK.map(
    (interval): readonly [number, number] => {
      const [a, b] = interval;
      if (
        interval.length !== 2 ||
        a === undefined ||
        b === undefined ||
        !first.temperaturesK.includes(a) ||
        !(b > a)
      ) {
        throw new Error(
          `${where}'s estimated interval [${interval.join(", ")}] is not two of its temperatures`,
        );
      }
      return [a, b];
    },
  );
  const variants = first.variants;
  const oneVariant = variants.length === 1 && variants[0] === "air";
  const twoVariants = variants.length === 2 && variants[0] === "self" && variants[1] === "air";
  if (!oneVariant && !twoVariants) {
    throw new Error(
      `${where}'s variants [${variants.join(", ")}] are neither ["air"] nor ["self", "air"]`,
    );
  }
  const broadening = new Map<string, number>();
  for (const [gas, ratio] of Object.entries(first.broadening)) {
    if (!(Number.isFinite(ratio) && ratio > 0)) {
      throw new Error(`${where}'s broadening by ${gas} is ${ratio}, not finite and above 0`);
    }
    broadening.set(gas, ratio);
  }
  if (
    first.chiHeld.length !== first.temperaturesK.length ||
    !first.chiHeld.every((row) => row.length === variants.length)
  ) {
    throw new Error(`${where}'s held-χ flags are not one a variant at each temperature`);
  }
  const broadeningExponents = new Map<string, number>();
  for (const [gas, offset] of Object.entries(first.broadeningExponents)) {
    if (!Number.isFinite(offset) || !broadening.has(gas)) {
      throw new Error(`${where}'s exponent offset for ${gas} is ${offset}, or for no listed gas`);
    }
    broadeningExponents.set(gas, offset);
  }
  const nodes = variants.length * first.temperaturesK.length * first.pressuresPa.length;
  const [low, high] = BAKE_RANGE_NM;
  const cells = buffers.flatMap((buffer, k) =>
    partCells(buffer, `${where}'s part ${first.parts[k]?.file ?? k + 1}`, {
      cells: first.parts[k]?.cells ?? 0,
      variants: variants.length,
      temperatures: first.temperaturesK.length,
      pressures: first.pressuresPa.length,
    }),
  );
  const read = cells.map((cell, i): KDistributionCell => {
    const previous = cells[i - 1]?.nm ?? -Infinity;
    const { weights, codes } = cell;
    const sum = weights.reduce((total, w) => total + w, 0);
    if (
      !Number.isInteger(cell.nm) ||
      !(cell.nm >= low && cell.nm <= high && cell.nm > previous) ||
      weights.length === 0 ||
      codes.length !== nodes * weights.length
    ) {
      throw new Error(
        `${where}'s cell ${cell.nm} nm is not a cell of the range with a code at each variant, node and g-point`,
      );
    }
    if (!weights.every((w) => w > 0) || !(Math.abs(sum - 1) <= WEIGHT_SUM_TOLERANCE)) {
      throw new Error(
        `${where}'s cell ${cell.nm} nm has weights that are not above 0 summing to 1`,
      );
    }
    if (!(Number.isFinite(cell.lnOffset) && Number.isFinite(cell.lnStep) && cell.lnStep > 0)) {
      throw new Error(`${where}'s cell ${cell.nm} nm has no finite offset and step above 0`);
    }
    return { cellNm: cell.nm, weights, lnOffset: cell.lnOffset, lnStep: cell.lnStep, codes };
  });
  const mixLimits = readMixLimits(first, read.length, twoVariants, where);
  const nearWingShare = first.nearWingShare;
  if (
    nearWingShare !== undefined &&
    (nearWingShare.length !== first.temperaturesK.length * first.pressuresPa.length ||
      !nearWingShare.every((s) => Number.isFinite(s) && s >= 0 && s <= 1))
  ) {
    throw new Error(`${where}'s near-wing shares are not one a node, each in [0, 1]`);
  }
  return {
    species: first.species,
    source: `${first.source}. ${first.reduction}`,
    basis,
    temperaturesK: first.temperaturesK,
    measuredRangeK: [lowK, highK],
    pressuresPa: first.pressuresPa,
    measuredBelowGrid: first.measuredBelowGrid,
    measuredShapeTo: first.measuredShapeTo,
    estimatedTemperaturesK,
    variants: twoVariants ? ["self", "air"] : ["air"],
    chiHeld: first.chiHeld,
    broadening,
    broadeningExponents,
    cells: read,
    ...(nearWingShare === undefined ? {} : { nearWingShare: Float64Array.from(nearWingShare) }),
    ...(mixLimits === undefined ? {} : { mixLimits }),
  };
}

/** cm² in a m²: a column in cm⁻² times this is in m⁻². */
const CM2_PER_M2 = 1e4;

/**
 * A sidecar's fixed-g mix record, checked (§16): on a two-variant table alone; shares ascending in
 * (0, 1), a worst error each, finite and at least 0; and a row a cell, a column a share, each null
 * or finite and above 0.
 *
 * @throws Error naming the file and the check it fails.
 */
function readMixLimits(
  first: KDistributionFile,
  cellCount: number,
  twoVariants: boolean,
  where: string,
): MixLimits | undefined {
  const mixing = first.mixing;
  if (mixing === undefined) {
    return undefined;
  }
  const { shares, temperaturesK, worstErrors, limitColumnsCm2 } = mixing;
  const sharesHeld =
    shares.length > 0 &&
    shares.every((w, i) => w > 0 && w < 1 && w > (shares[i - 1] ?? 0)) &&
    ascendingPositive(temperaturesK) &&
    worstErrors.length === shares.length &&
    worstErrors.every((e) => Number.isFinite(e) && e >= 0);
  const rowsHeld =
    limitColumnsCm2.length === cellCount &&
    limitColumnsCm2.every(
      (cell) =>
        cell.length === shares.length &&
        cell.every(
          (row) =>
            row.length === temperaturesK.length &&
            row.every((u) => u === null || (Number.isFinite(u) && u > 0)),
        ),
    );
  if (!twoVariants || !sharesHeld || !rowsHeld) {
    throw new Error(
      `${where}'s mix record is not a two-variant table's shares, temperatures, worst errors and a column a cell at each share and temperature`,
    );
  }
  return {
    shares: Float64Array.from(shares),
    temperaturesK: Float64Array.from(temperaturesK),
    worstErrors: Float64Array.from(worstErrors),
    columnsM2: limitColumnsCm2.map((cell) =>
      cell.map((row) => Float64Array.from(row, (u) => (u === null ? Infinity : u * CM2_PER_M2))),
    ),
  };
}

/**
 * A share's u_mix at a temperature: the least over the tested temperatures bracketing it, the end
 * one's beyond them, where the temperature is held (§17), m⁻².
 */
function mixLimitAtTemperature(
  temperaturesK: Float64Array,
  row: Float64Array | undefined,
  temperatureK: number,
): number {
  if (row === undefined) {
    return Infinity;
  }
  const last = temperaturesK.length - 1;
  if (temperatureK <= (temperaturesK[0] ?? Infinity)) {
    return row[0] ?? Infinity;
  }
  if (temperatureK >= (temperaturesK[last] ?? -Infinity)) {
    return row[last] ?? Infinity;
  }
  let k = 0;
  while (k < last - 1 && (temperaturesK[k + 1] ?? Infinity) < temperatureK) {
    k += 1;
  }
  return Math.min(row[k] ?? Infinity, row[k + 1] ?? Infinity);
}

/**
 * u_mix(w, T), the column at which a two-variant table's fixed-g mix first errs past 2 × 10⁻³ in a
 * cell, at a body's self share w and temperature (`decision-r08-t4d-kdist-chain.md` §§16–17;
 * R08.T10.a labels CO₂ `estimated` where the body's own largest column reaches it).
 *
 * @remarks
 * At each share the least over the tested temperatures bracketing T, the end sample's beyond them,
 * where T is held; then ln u linear in w between the recorded shares, held at the end shares'
 * values beyond them, and between a share that records a column and one that records none, the
 * recorded column, the lower and so the conservative one.
 *
 * @param cellIndex - The cell's index in `table.cells`.
 * @param selfShare - w, the self share of the Lorentz width ({@link Broadening.selfShare}).
 * @param temperatureK - The profile's temperature, K.
 * @returns m⁻²; ∞ where the mix never errs past the tier, or the table records no mix.
 */
export function mixLimitColumnM2(
  table: KDistributionTable,
  cellIndex: number,
  selfShare: number,
  temperatureK: number,
): number {
  const limits = table.mixLimits;
  const cell = limits?.columnsM2[cellIndex];
  if (limits === undefined || cell === undefined) {
    return Infinity;
  }
  const row = cell.map((byT) => mixLimitAtTemperature(limits.temperaturesK, byT, temperatureK));
  const { shares } = limits;
  const last = shares.length - 1;
  if (selfShare <= (shares[0] ?? 0)) {
    return row[0] ?? Infinity;
  }
  if (selfShare >= (shares[last] ?? 1)) {
    return row[last] ?? Infinity;
  }
  let k = 0;
  while (k < last - 1 && (shares[k + 1] ?? 1) < selfShare) {
    k += 1;
  }
  const a = row[k] ?? Infinity;
  const b = row[k + 1] ?? Infinity;
  if (!Number.isFinite(a) || !Number.isFinite(b)) {
    return Math.min(a, b);
  }
  const t = (selfShare - (shares[k] ?? 0)) / ((shares[k + 1] ?? 1) - (shares[k] ?? 0));
  return Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * t);
}

/**
 * The standard broadening pressures a table's nodes are drawn from, Pa (`decision-r08-t4d-kdist.md`,
 * ruling 2; `decision-r08-t4d-kdist-chain.md` §8): 4 a decade from 10 Pa to 1 MPa, 8 a decade to
 * 10 MPa, and 13.3 MPa, 30 in all, as `src/tools/kDistributions.ts`'s `STORED_PRESSURES_PA`.
 */
export const STANDARD_PRESSURES_PA: ReadonlyArray<number> = [
  ...Array.from({ length: 21 }, (_, k) => 10 * 10 ** (k / 4)),
  ...Array.from({ length: 9 }, (_, k) => 1e6 * 10 ** ((k + 1) / 8)),
];

/**
 * How near a table's node lies to a standard or lattice pressure and is it, relatively: 10⁻⁵, since
 * a sidecar writes its pressures to six figures (`src/tools/kDistributions.ts`'s
 * `kDistributionMetaText`).
 */
const STANDARD_TOLERANCE = 1e-5;

/**
 * The pressure lattice a table's nodes are drawn from, Pa (decision-r08-t4e-unsold's follow-up, the
 * 581 nm residual), as `src/tools/kDistributions.ts`'s `PRESSURE_LATTICE_PA`: 10 × 10^(j/8) for
 * j = 0…49, its even j below 1 MPa and every j from it {@link STANDARD_PRESSURES_PA}, by the same
 * values, its odd j below 1 MPa the 8-a-decade midpoints a table may add.
 */
export const PRESSURE_LATTICE_PA: ReadonlyArray<number> = Array.from({ length: 50 }, (_, j) => {
  if (j >= 40) {
    return STANDARD_PRESSURES_PA[20 + (j - 40)] ?? Number.NaN;
  }
  return j % 2 === 0 ? (STANDARD_PRESSURES_PA[j / 2] ?? Number.NaN) : 10 * 10 ** (j / 8);
});

/** Whether a table's node is a lattice midpoint: an odd j below 1 MPa. */
export function isLatticeMidpoint(pressurePa: number): boolean {
  const j = PRESSURE_LATTICE_PA.findIndex((p) => Math.abs(p / pressurePa - 1) < STANDARD_TOLERANCE);
  return j >= 0 && j < 40 && j % 2 === 1;
}

/**
 * The pressure rows E3's rule gives a table's curves, Pa, ascending (R08.T4.e's economy E3,
 * decision-r08-t4ef-storage §5.3, ruled 2026-10-16): its nodes, or where its nodes other than the
 * midpoints it holds ({@link isLatticeMidpoint}) are a strict subset of
 * {@link STANDARD_PRESSURES_PA} (alternates dropped below 1 MPa), the standard pressures from its
 * first node to its last, each table then read between its own nodes ({@link cellSigmasAt}), as a
 * non-dominant species' already is. The midpoints a slot's tables hold are added to these
 * (`absorbers.ts`'s `absorberCurvePressures`). No stored layout changes.
 */
export function curveRowsPa(pressuresPa: ReadonlyArray<number>): ReadonlyArray<number> {
  const standard = (p: number): boolean =>
    STANDARD_PRESSURES_PA.some((s) => Math.abs(s / p - 1) < STANDARD_TOLERANCE);
  const base = pressuresPa.filter((p) => !isLatticeMidpoint(p));
  const first = base[0] ?? Number.NaN;
  const last = base.at(-1) ?? Number.NaN;
  const span = STANDARD_PRESSURES_PA.filter(
    (s) => s >= first * (1 - STANDARD_TOLERANCE) && s <= last * (1 + STANDARD_TOLERANCE),
  );
  return base.every(standard) && span.length > base.length ? span : base;
}

/** The gases that broaden as air does by definition: air's own (HITRAN's γ_air). */
const AIR_GASES: ReadonlySet<string> = new Set(["air", "N2", "O2", "Ar"]);

/**
 * The five broadening gases with a permanent dipole that `decision-r08-approximate-plate.md`, item
 * 1a (2026-10-27), names, H₂O, NH₃, SO₂, HCN and H₂S, whose foreign widths can pass twice air's:
 * beyond the range the trace broadeners' effect bound takes for a gas a table holds no width for
 * (`negligible.ts`'s `BroadenerVerdict`). A table that holds no width for one of them is
 * `estimated` at any share, as before the bound, until a cited range of the gas's own exists.
 *
 * @remarks
 * The registry holds other gases whose dipoles are as strong (HCl, CH₃OH, SiO, H₂SO₄, TiO, VO,
 * FeH), which the bound weighs as the ruling stands; recorded for the owner (R08.T10.c's deferred
 * findings).
 */
export const DIPOLAR_BROADENERS: ReadonlySet<string> = new Set(["H2O", "NH3", "SO2", "HCN", "H2S"]);

/** A species' broadening in a mixture ({@link broadeningFactor}). */
export interface Broadening {
  /** Σ_b x_b r_b (296 ÷ T)^(n̄_b − n̄_air), the broadening pressure over the pressure. */
  readonly factor: number;
  /** The self share of the Lorentz width, x_self r_self ÷ Σ_b x_b r_b: a two-variant table's mix. */
  readonly selfShare: number;
  /**
   * Whether it rests on a gas of {@link DIPOLAR_BROADENERS} the table holds no width for, or, for a
   * two-variant table, on any gas but the species, N₂, O₂ and Ar, whose far wing takes CO₂–N₂'s χ,
   * `estimated`.
   */
  readonly estimated: boolean;
  /**
   * ε, the share of the broadening pressure that rests on the other gases the table holds no width
   * for, each taken as air (r 1): Σ_j x_j ÷ Σ_b x_b r_b (296 ÷ T)^(Δn_b), j over those gases and b
   * over the mixture, so that a gas whose own ratio lies anywhere in [0, 2], the range item 1a
   * assumes, moves the broadening pressure by at most this share of itself
   * (`decision-r08-approximate-plate.md`, item 1a). 0 where
   * there are none, and on a two-variant table, where every such gas is `estimated`. R08.T10.c's
   * effect bound weighs it in place of a label (`negligible.ts`'s `BroadenerVerdict`).
   */
  readonly widthlessShare: number;
}

/**
 * The broadening of a species' lines in a mixture at a temperature: Σ_b x_b r_b (296 ÷ T)^(Δn_b)
 * over its gases, r_b and Δn_b the table's ratio and exponent offset for gas b (air's 1 and 0 for a
 * gas it does not list) and r_self's for the species' own mole fraction, and the self share of it
 * (`decision-r08-t4d-kdist.md` §5.3; `decision-r08-t4d-kdist-chain.md` §§7, 13). The mole
 * fractions are normalised to sum to 1.
 *
 * @remarks
 * A gas the table holds no width for, other than N₂, O₂ and Ar, is taken as air. It makes the
 * species `estimated` where it is one of {@link DIPOLAR_BROADENERS} or the table has two variants,
 * and otherwise adds its share to {@link Broadening.widthlessShare}, for the effect bound
 * (R08.T10.c).
 *
 * @param moleFractions - The mixture's gases by registry key, the species' own among them.
 * @param temperatureK - The profile's temperature, K.
 * @throws RangeError for mole fractions that are not finite and at least 0, or none above 0, or a
 *   temperature that is not finite and above 0 K.
 */
export function broadeningFactor(
  table: KDistributionTable,
  moleFractions: ReadonlyMap<string, number>,
  temperatureK: number,
): Broadening {
  if (!(Number.isFinite(temperatureK) && temperatureK > 0)) {
    throw new RangeError(`a broadening temperature is finite and above 0 K, not ${temperatureK}`);
  }
  const twoVariant = table.variants.length > 1;
  let total = 0;
  let weighted = 0;
  let self = 0;
  let widthless = 0;
  let estimated = false;
  for (const [gas, x] of moleFractions) {
    if (!(Number.isFinite(x) && x >= 0)) {
      throw new RangeError(`a broadening gas's mole fraction is ${x}, not finite and at least 0`);
    }
    const own = gas === table.species;
    const key = own ? "self" : gas;
    const ratio = table.broadening.get(key);
    const foreign = x > 0 && !own && !AIR_GASES.has(gas);
    const noWidth = foreign && ratio === undefined;
    const bounded = noWidth && !twoVariant && !DIPOLAR_BROADENERS.has(gas);
    estimated ||= (foreign && twoVariant) || (noWidth && !bounded);
    const atT = (ratio ?? 1) * (296 / temperatureK) ** (table.broadeningExponents.get(key) ?? 0);
    total += x;
    weighted += x * atT;
    self += own ? x * atT : 0;
    widthless += bounded ? x * atT : 0;
  }
  if (!(total > 0)) {
    throw new RangeError("a broadening mixture needs a gas with a mole fraction above 0");
  }
  return {
    factor: weighted / total,
    selfShare: self / weighted,
    estimated,
    widthlessShare: widthless / weighted,
  };
}

/** A species' k-distributions at one temperature, on its table's pressures. */
export interface SpeciesKDistribution {
  readonly species: string;
  /** The table's basis: an `estimated` table is estimated at every temperature. */
  readonly basis: KDistributionBasis;
  /**
   * `measured` within the table's measured range and basis and outside its estimated intervals,
   * `estimated` otherwise.
   */
  readonly provenance: KDistributionBasis;
  /** Whether the temperature lies in one of the table's estimated intervals. */
  readonly inEstimatedInterval: boolean;
  /**
   * Whether a variant the mix takes holds its χ at the temperature's node, or at either node about
   * it ({@link KDistributionTable.chiHeld}): R08.T10.a's held-χ bound applies.
   */
  readonly chiHeld: boolean;
  /** The temperature asked for, K. */
  readonly temperatureK: number;
  /** The table's broadening pressures, Pa, ascending. */
  readonly pressuresPa: ReadonlyArray<number>;
  /**
   * The highest broadening pressure whose line shape is measured at this temperature, Pa: the
   * lower of the two temperature nodes' about it, or 0 if neither has one.
   */
  readonly measuredShapeToPa: number;
  /** Whether the held 10 Pa table stays `measured` below the grid. */
  readonly measuredBelowGrid: boolean;
  /** Each cell's weights and σ at each pressure and g-point, m²: `sigmas[p × G + g]`. */
  readonly cells: ReadonlyArray<{
    readonly cellNm: number;
    readonly weights: Float64Array;
    readonly sigmas: Float64Array;
  }>;
  readonly source: string;
}

/** The weights of 4-point Lagrange interpolation at x from four abscissae. */
function lagrangeWeights(xs: readonly [number, number, number, number], x: number): number[] {
  return xs.map((xi, i) => {
    let w = 1;
    for (const [j, xj] of xs.entries()) {
      if (j !== i) {
        w *= (x - xj) / (xi - xj);
      }
    }
    return w;
  });
}

/**
 * The interval of an ascending grid that holds x, held to the grid's ends, the fraction of x
 * across it by `along`, and the four nodes nearest the interval, held inside the grid.
 */
function bracket(
  grid: ReadonlyArray<number>,
  x: number,
  along: (v: number) => number,
): { readonly index: number; readonly fraction: number; readonly stencil: number[] } {
  const n = grid.length;
  let i = 0;
  while (i < n - 2 && (grid[i + 1] ?? Infinity) < x) {
    i += 1;
  }
  const lo = along(grid[i] ?? Number.NaN);
  const hi = along(grid[Math.min(i + 1, n - 1)] ?? Number.NaN);
  const clamped = Math.min(Math.max(along(x), Math.min(lo, hi)), Math.max(lo, hi));
  const fraction = hi === lo ? 0 : (clamped - lo) / (hi - lo);
  const first = Math.min(Math.max(i - 1, 0), Math.max(n - 4, 0));
  return {
    index: i,
    fraction,
    stencil: Array.from({ length: Math.min(n, 4) }, (_, k) => first + k),
  };
}

/**
 * Values at x from their nodes' values: 4-point Lagrange on ln v where every one of the four is
 * above 0, linear in v between the two about x otherwise or on a grid of under four nodes, each
 * held at the grid's ends.
 */
function cubicOnLog(
  grid: ReadonlyArray<number>,
  x: number,
  along: (v: number) => number,
  valueAt: (node: number) => number,
): number {
  const { index, fraction, stencil } = bracket(grid, x, along);
  const a = valueAt(index);
  const b = valueAt(Math.min(index + 1, grid.length - 1));
  const values = stencil.map(valueAt);
  if (stencil.length < 4 || !values.every((v) => v > 0)) {
    return a + (b - a) * fraction;
  }
  const at = Math.min(
    Math.max(along(x), along(grid[0] ?? Number.NaN)),
    along(grid.at(-1) ?? Number.NaN),
  );
  const w = lagrangeWeights(
    [
      along(grid[stencil[0] ?? 0] ?? 0),
      along(grid[stencil[1] ?? 0] ?? 0),
      along(grid[stencil[2] ?? 0] ?? 0),
      along(grid[stencil[3] ?? 0] ?? 0),
    ],
    at,
  );
  return Math.exp(w.reduce((sum, wk, k) => sum + wk * Math.log(values[k] ?? 1), 0));
}

/** A cell's σ at its codes' index ((v × T + t) × P + p) × G + g, m². */
function storedSigma(cell: KDistributionCell, index: number): number {
  const code = cell.codes[index] ?? 0;
  return code === 0 ? 0 : Math.exp(cell.lnOffset + cell.lnStep * (code - 1));
}

/**
 * A species' k-distributions at a temperature: each g-point's σ by 4-point Lagrange in T on ln σ
 * over the four nodes nearest (`decision-r08-t4d-kdist.md`, ruling 2: cubic in 1/T is worse, linear
 * in T 4–8 times worse), held beyond the grid; a two-variant table's two σ then mixed at fixed g by
 * the self share, w σ_self + (1 − w) σ_air.
 *
 * @param selfShare - The self share of the Lorentz width ({@link Broadening.selfShare}), in [0, 1];
 *   read by a two-variant table only.
 * @throws RangeError for a temperature that is not finite and above 0 K, or a share outside [0, 1].
 */
export function kDistributionAt(
  table: KDistributionTable,
  temperatureK: number,
  selfShare = 0,
): SpeciesKDistribution {
  if (!(Number.isFinite(temperatureK) && temperatureK > 0)) {
    throw new RangeError(
      `a k-distribution's temperature is finite and above 0 K, not ${temperatureK}`,
    );
  }
  if (!(selfShare >= 0 && selfShare <= 1)) {
    throw new RangeError(`a self share lies in [0, 1], not ${selfShare}`);
  }
  const pressures = table.pressuresPa.length;
  const perVariant = table.temperaturesK.length * pressures;
  const shares = table.variants.length === 2 ? [selfShare, 1 - selfShare] : [1];
  const cells = table.cells.map((cell) => {
    const g = cell.weights.length;
    const sigmas = new Float64Array(pressures * g);
    for (const [v, share] of shares.entries()) {
      if (!(share > 0)) {
        continue;
      }
      for (let k = 0; k < pressures * g; k += 1) {
        sigmas[k] =
          (sigmas[k] ?? 0) +
          share *
            cubicOnLog(
              table.temperaturesK,
              temperatureK,
              (t) => t,
              (t) => storedSigma(cell, (v * perVariant + t * pressures) * g + k),
            );
      }
    }
    return { cellNm: cell.cellNm, weights: cell.weights, sigmas };
  });
  const [lowK, highK] = table.measuredRangeK;
  const inEstimatedInterval = table.estimatedTemperaturesK.some(
    ([a, b]) => temperatureK >= a && temperatureK <= b,
  );
  const { index } = bracket(table.temperaturesK, temperatureK, (t) => t);
  const next = Math.min(index + 1, table.temperaturesK.length - 1);
  const shapeTo = Math.min(table.measuredShapeTo[index] ?? -1, table.measuredShapeTo[next] ?? -1);
  // At a node the read is that node's alone; between two, either's flag holds.
  const atNode = table.temperaturesK.indexOf(temperatureK);
  const about = atNode >= 0 ? [atNode] : [index, next];
  const chiHeld = shares.some(
    (share, v) => share > 0 && about.some((t) => table.chiHeld[t]?.[v] === true),
  );
  return {
    species: table.species,
    basis: table.basis,
    provenance:
      table.basis === "measured" &&
      temperatureK >= lowK &&
      temperatureK <= highK &&
      !inEstimatedInterval
        ? "measured"
        : "estimated",
    inEstimatedInterval,
    chiHeld,
    temperatureK,
    pressuresPa: table.pressuresPa,
    measuredShapeToPa: shapeTo >= 0 ? (table.pressuresPa[shapeTo] ?? 0) : 0,
    measuredBelowGrid: table.measuredBelowGrid,
    cells,
    source: table.source,
  };
}

/**
 * Each cell's g-point σ at a broadening pressure, m²: 4-point Lagrange in ln p on ln σ over the
 * four stored nodes nearest, exact at a node, held beyond the grid. `σ[c][g]` for the
 * distribution's cell c.
 *
 * @throws RangeError for a pressure that is not finite and above 0 Pa.
 */
export function cellSigmasAt(kd: SpeciesKDistribution, broadeningPa: number): Float64Array[] {
  if (!(Number.isFinite(broadeningPa) && broadeningPa > 0)) {
    throw new RangeError(`a broadening pressure is finite and above 0 Pa, not ${broadeningPa}`);
  }
  const node = kd.pressuresPa.indexOf(broadeningPa);
  return kd.cells.map((cell) => {
    const g = cell.weights.length;
    if (node >= 0) {
      return cell.sigmas.slice(node * g, (node + 1) * g);
    }
    return Float64Array.from({ length: g }, (_, k) =>
      cubicOnLog(kd.pressuresPa, broadeningPa, Math.log, (p) => cell.sigmas[p * g + k] ?? 0),
    );
  });
}

/** A cell's mean transmittance through a column u, m⁻², from its weights and g-point σ, m². */
export function cellTransmittance(
  weights: Float64Array,
  sigmasM2: Float64Array,
  columnPerM2: number,
): number {
  let t = 0;
  for (let g = 0; g < weights.length; g += 1) {
    t += (weights[g] ?? 0) * Math.exp(-(sigmasM2[g] ?? 0) * columnPerM2);
  }
  return t;
}

/** A cell's mean cross-section, Σ_g w_g σ_g, m²: its curve of growth's slope at u = 0. */
export function cellMeanSigma(weights: Float64Array, sigmasM2: Float64Array): number {
  let s = 0;
  for (let g = 0; g < weights.length; g += 1) {
    s += (weights[g] ?? 0) * (sigmasM2[g] ?? 0);
  }
  return s;
}

/**
 * The committed k-distribution sidecars, by file, imported on demand. Typing them as
 * {@link KDistributionFile} is a cast at the committed-data boundary: the files are written by
 * `src/tools/kDistributions.ts`'s `kDistributionMetaText`, and {@link readKDistribution} checks
 * the grid, the records it acts on and the parts before use (the text fields are only carried).
 */
const SIDECAR_LOADERS = import.meta.glob<KDistributionFile>(
  ["./kDistributions/*.json", "./exomol/kDistributions/*.json"],
  { import: "default" },
);

/** The committed binary parts' URLs, by file, as Vite serves and bundles them (`?url`). */
const PART_URLS = import.meta.glob<string>(
  ["./kDistributions/*.bin", "./exomol/kDistributions/*.bin"],
  {
    query: "?url",
    import: "default",
  },
);

/**
 * Reads a committed binary part by its path relative to `absorbers/` (`kDistributions/O2-1.bin`):
 * the worker's {@link fetchPart}, or a test's reader of `?inline` assets.
 */
export type PartReader = (path: string) => Promise<ArrayBuffer>;

/**
 * Reads a committed part through its `?url`, as the height worker reads its wasm.
 *
 * @throws Error for a part the client does not hold, or a fetch that fails.
 */
export const fetchPart: PartReader = async (path) => {
  const url = PART_URLS[`./${path}`];
  if (url === undefined) {
    throw new Error(`no committed k-distribution part ${path}`);
  }
  const response = await fetch(await url());
  if (!response.ok) {
    throw new Error(`fetching ${path} failed: ${response.status}`);
  }
  return response.arrayBuffer();
};

/** A sidecar's species key, from its file name `<key>.json`, and its directory. */
function sidecarSpecies(
  path: string,
): { readonly key: string; readonly directory: string } | undefined {
  const match = /^\.\/(.+\/)([^/]+)\.json$/u.exec(path);
  return match?.[1] === undefined || match[2] === undefined
    ? undefined
    : { directory: match[1], key: match[2] };
}

/** The species the client holds committed k-distribution tables for, ascending. */
export function committedLineSpecies(): ReadonlyArray<string> {
  return Object.keys(SIDECAR_LOADERS)
    .flatMap((path) => {
      const found = sidecarSpecies(path);
      return found === undefined ? [] : [found.key];
    })
    .toSorted();
}

/**
 * The committed k-distribution tables of the species asked for that the client holds, each
 * species' sidecar and parts read on first need (`decision-r08-t4d-kdist.md` §4: per species, by
 * the optics worker). A species with no table is left out.
 *
 * @param readPart - How a part's bytes are read: {@link fetchPart} in the worker.
 */
export async function loadKDistributions(
  species: ReadonlyArray<string>,
  readPart: PartReader = fetchPart,
): Promise<ReadonlyArray<KDistributionTable>> {
  const tables = await Promise.all(
    species.map(async (key) => {
      const path = Object.keys(SIDECAR_LOADERS).find((p) => sidecarSpecies(p)?.key === key);
      const load = path === undefined ? undefined : SIDECAR_LOADERS[path];
      const directory = path === undefined ? undefined : sidecarSpecies(path)?.directory;
      if (load === undefined || directory === undefined) {
        return [];
      }
      const sidecar = await load();
      const buffers = await Promise.all(
        sidecar.parts.map(async (part) => readPart(`${directory}${part.file}`)),
      );
      return [readKDistribution(sidecar, buffers, `${key}'s k-distribution`)];
    }),
  );
  return tables.flat();
}
