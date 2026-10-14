import { beforeAll, describe, expect, it } from "vitest";

import { inlinePart } from "../../../test/kDistributionFixtures";
import { absorberTransmittance, type AbsorberCurve } from "../absorbers";
import {
  cellSigmasAt,
  cellTransmittance,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
} from "./kDistribution";
import nitrousOxide from "./exomol/lineReferencesN2O.json" with { type: "json" };
import ammonia from "./exomol/lineReferencesNH3.json" with { type: "json" };
import oxygen from "./lineReferencesO2.json" with { type: "json" };
import water from "./lineReferencesH2O.json" with { type: "json" };

/** One state of a committed cell reference, as `src/tools/lineReferences.ts` writes it. */
interface ReferenceState {
  readonly temperatureK: number;
  readonly pressurePa: number;
  readonly kind: string;
  readonly columnsCm2: ReadonlyArray<number>;
  readonly lineByLine: ReadonlyArray<number>;
  readonly f64: ReadonlyArray<number>;
}

/** A committed species' cell references. */
interface References {
  readonly species: string;
  readonly cells: ReadonlyArray<{
    readonly nm: number;
    readonly states: ReadonlyArray<ReferenceState>;
  }>;
}

/** The node tier, 10⁻³ in T, and the chain tier, 2 × 10⁻³ (`decision-r08-t4d-kdist.md`, ruling 1). */
const NODE_BOUND = 1e-3;
const CHAIN_BOUND = 2e-3;

/** The 16-bit storage's bound against the reduction's `f64` curves (its tests). */
const QUANTISATION_BOUND = 2e-4;

/**
 * The client's chain for one cell at a state, through the curves' own read: the cell's g-point σ
 * cubic in T at arrival, a row of curve at every stored pressure, and `absorberTransmittance`'s
 * read in ln p̄ on τ, at each of the state's columns.
 *
 * @param rowShift - The rows' pressures over the stored nodes': 1 for a profile's dominant line
 *   species, whose rows are its nodes; otherwise a species sharing another's rows, its σ read
 *   between its own nodes at each row (`absorbers.ts`'s `absorberCurvePressures`).
 */
function clientCurve(
  table: KDistributionTable,
  cellNm: number,
  state: ReferenceState,
  rowShift = 1,
): number[] {
  const kd = kDistributionAt(table, state.temperatureK);
  const c = kd.cells.findIndex((cell) => cell.cellNm === cellNm);
  const cell = kd.cells[c];
  if (cell === undefined) {
    throw new Error(`${table.species}'s table holds no ${cellNm} nm cell`);
  }
  const columnsM2 = state.columnsCm2.map((u) => u * 1e4);
  const rowPressuresPa = table.pressuresPa.map((p) => p * rowShift);
  const rows = rowPressuresPa.map((p) => cellSigmasAt(kd, p)[c] ?? new Float64Array(0));
  const depth = Float64Array.from(
    rows.flatMap((sigmas) =>
      columnsM2.map((u) => -Math.log(cellTransmittance(cell.weights, sigmas, u))),
    ),
  );
  // One column node a reference column, so that the read is exact in u and the cubic acts in p.
  const curve: AbsorberCurve = {
    kind: "column",
    logColumns: Float64Array.from(columnsM2, Math.log),
    logPressures: Float64Array.from(rowPressuresPa, Math.log),
    readTopLogPressure: Math.log(1e7),
    transmittance: [
      depth.map((d) => Math.exp(-d)),
      depth.map((d) => Math.exp(-d)),
      depth.map((d) => Math.exp(-d)),
    ],
    opticalDepth: [depth, depth, depth],
  };
  return columnsM2.map(
    (u) => absorberTransmittance(curve, { kind: "column", columnPerM2: u }, state.pressurePa)[0],
  );
}

const OXYGEN: References = oxygen;
const WATER: References = water;
// ExoMol's (R08.T4.g): their curves from the super-lines the tables were reduced from.
const AMMONIA: References = ammonia;
const NITROUS_OXIDE: References = nitrousOxide;

describe.each([
  ["O₂", OXYGEN],
  ["H₂O", WATER],
  ["NH₃", AMMONIA],
  ["N₂O", NITROUS_OXIDE],
])("%s's committed cells against their line-by-line curves", (_, references) => {
  let table: KDistributionTable | undefined;
  beforeAll(async () => {
    [table] = await loadKDistributions([references.species], inlinePart);
  });
  const states = references.cells.flatMap((cell) =>
    cell.states.map((state) => [cell.nm, state] as const),
  );

  /** The client's curve at a state, from the loaded table. */
  function client(nm: number, state: ReferenceState, rowShift = 1): number[] {
    if (table === undefined) {
      throw new Error(`no committed table for ${references.species}`);
    }
    return clientCurve(table, nm, state, rowShift);
  }

  it.each(states)(
    "hold the %s nm cell's curve to line by line within its tier (%o)",
    (nm, state) => {
      const errors = client(nm, state).map((t, k) =>
        Math.abs(t - (state.lineByLine[k] ?? Number.NaN)),
      );
      expect(Math.max(...errors)).toBeLessThanOrEqual(
        state.kind === "node" ? NODE_BOUND : CHAIN_BOUND,
      );
    },
  );

  it.each(states)(
    "hold the %s nm cell's curve to the reduction's f64 one within 2 × 10⁻⁴ (%o)",
    (nm, state) => {
      // The references are written to 6 significant figures, under 10⁻⁶ of T.
      const errors = client(nm, state).map((t, k) => Math.abs(t - (state.f64[k] ?? Number.NaN)));
      expect(Math.max(...errors)).toBeLessThanOrEqual(QUANTISATION_BOUND + 1e-6);
    },
  );

  it.each(states)(
    "hold the %s nm cell's curve within the chain's tier on another species' rows, half a node off its own (%o)",
    (nm, state) => {
      // A species sharing a profile's rows with a dominant one of another broadening factor: its σ
      // read between its nodes at each row, then τ between the rows. Half a node, 10^(1/8), is the
      // farthest a row falls from both of its neighbouring nodes below 1 MPa.
      const errors = client(nm, state, 10 ** (1 / 8)).map((t, k) =>
        Math.abs(t - (state.lineByLine[k] ?? Number.NaN)),
      );
      expect(Math.max(...errors)).toBeLessThanOrEqual(CHAIN_BOUND);
    },
  );
});
