/**
 * Toy k-distribution tables for the line absorbers' tests and the smoke (plan R08, R08.T4.d): a
 * sidecar and its binary part as `src/tools/kDistributions.ts` writes them. Apart from
 * `kDistributionFixtures.ts`'s reader of the committed parts, so that the smoke's bundle takes no
 * committed table.
 */

import type { KDistributionFile } from "../view/atmosphere/absorbers/kDistribution";

/** One toy cell: its centre, weights and σ at every variant, node and g-point, m². */
export interface ToyCell {
  readonly nm: number;
  readonly weights: ReadonlyArray<number>;
  /** `[((v × T + t) × P + p) × G + g]`, m², 0 allowed. */
  readonly sigmasM2: ReadonlyArray<number>;
}

/** The codes of a 16-bit ln σ: 0 for σ = 0, and 1 to 65,535 above. */
const CODE_MAX = 65_535;

/**
 * A binary part of cells as the tool writes it: `HYKD`, u16 version 1, cells, variants,
 * temperatures, pressures and 0; then per cell u16 nm, u16 G, f64 ln σ of code 1, f64 its step,
 * G f64 weights and the u16 codes, each cell's 16-bit ln σ over its own range.
 */
export function toyPart(
  cells: ReadonlyArray<ToyCell>,
  grid: { readonly variants: number; readonly temperatures: number; readonly pressures: number },
): ArrayBuffer {
  const size =
    16 + cells.reduce((sum, c) => sum + 20 + 8 * c.weights.length + 2 * c.sigmasM2.length, 0);
  const view = new DataView(new ArrayBuffer(size));
  for (let i = 0; i < 4; i += 1) {
    view.setUint8(i, "HYKD".charCodeAt(i));
  }
  for (const [i, v] of [
    1,
    cells.length,
    grid.variants,
    grid.temperatures,
    grid.pressures,
    0,
  ].entries()) {
    view.setUint16(4 + 2 * i, v, true);
  }
  let at = 16;
  for (const cell of cells) {
    const logs = cell.sigmasM2.filter((v) => v > 0).map(Math.log);
    const lo = logs.length > 0 ? Math.min(...logs) : 0;
    const hi = logs.length > 0 ? Math.max(...logs) : 0;
    const step = hi > lo ? (hi - lo) / (CODE_MAX - 1) : 1;
    view.setUint16(at, cell.nm, true);
    view.setUint16(at + 2, cell.weights.length, true);
    view.setFloat64(at + 4, lo, true);
    view.setFloat64(at + 12, step, true);
    at += 20;
    for (const w of cell.weights) {
      view.setFloat64(at, w, true);
      at += 8;
    }
    for (const v of cell.sigmasM2) {
      view.setUint16(at, v > 0 ? 1 + Math.round((Math.log(v) - lo) / step) : 0, true);
      at += 2;
    }
  }
  return view.buffer;
}

/** A toy sidecar of one part, its fields overridable. */
export function toySidecar(
  temperaturesK: ReadonlyArray<number>,
  pressuresPa: ReadonlyArray<number>,
  cells: number,
  overrides: Partial<KDistributionFile> = {},
): KDistributionFile {
  return {
    species: "O2",
    source: "toy",
    reduction: "toy",
    basis: "measured",
    temperaturesK,
    measuredRangeK: [temperaturesK[0] ?? 0, Math.min(296, temperaturesK.at(-1) ?? 0)],
    pressuresPa,
    measuredBelowGrid: true,
    measuredShapeTo: temperaturesK.map(() => pressuresPa.length - 1),
    estimatedTemperaturesK: [],
    broadening: { self: 1 },
    broadeningExponents: {},
    chiHeld: temperaturesK.map(() => (overrides.variants ?? ["air"]).map(() => false)),
    variants: ["air"],
    parts: [{ file: "toy-1.bin", cells, bytes: 0, sha256: "" }],
    ...overrides,
  };
}
