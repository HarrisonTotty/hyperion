/**
 * The line-by-line references R08.T4.d's tests read (`decision-r08-t4d-kdist.md`, its tests;
 * `decision-r08-t4d-kdist-chain.md`): `node scripts/atmosphereData.mjs earth-reference …` and
 * `cell-references …`. Each is computed in `f64` from the same lines as the k-distributions and with
 * the same line shapes, and only the reduced curves are committed.
 *
 * @remarks
 * - `earth-reference --paths <earthLinePaths.json> --o2-lines <csv> --h2o-lines <csv> --tips <dir>
 *   --out <json>`: Earth's O₂ and H₂O lines in one spectrum through each layer of each of the paths'
 *   fixture's paths, every 1 nm cell's mean transmittance per path.
 * - `cell-references --species <key> --lines <file> --tips <dir> --table <sidecar.json> --cells <nm,…>
 *   --out <json>`: each named cell's line-by-line curve of growth at three node pressures and three
 *   midpoints, beside the reduction's `f64` curves there.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import {
  broadeningExponentOffsets,
  broadeningRatios,
  CHAIN_COLUMNS_PER_DECADE,
  chainCurve,
  chainColumnCm2,
  gatedColumnCm2,
  errorColumns,
  intervalTransmittance,
  lineByLine,
  lineShape,
  type NodeSpectra,
  REDUCTION_TARGET,
  reduceGridCell,
  type ReductionGrid,
  type ShapePolicy,
  chainRowsPa,
  latticePressurePa,
  meetsPressureRule,
  sortedNode,
} from "./kDistributions";
import { readLineSpecies, sourceOf } from "./kDistributionsCli";
import {
  cellPointCount,
  cellSpectrum,
  cellWavenumbersPerCm,
  type LineSpecies,
  linesNear,
  lineStates,
} from "./molecularLines";

/** One layer of a start height's paths, as `earthLinePaths.json` holds it. */
interface PathsLayer {
  readonly pressurePa: number;
  readonly temperatureK: number;
  readonly o2Fraction: number;
  readonly h2oFraction: number;
  readonly o2PerM2: ReadonlyArray<number>;
  readonly h2oPerM2: ReadonlyArray<number>;
}

/** One start height's paths, as `earthLinePaths.json` holds them. */
interface PathsStart {
  readonly startHeightM: number;
  readonly zenithDeg: ReadonlyArray<number>;
  readonly layers: ReadonlyArray<PathsLayer>;
}

/** Whether a value read from the paths' file has its starts: a trust boundary of our own writing. */
function isPaths(value: unknown): value is { readonly starts: ReadonlyArray<PathsStart> } {
  return (
    typeof value === "object" && value !== null && "starts" in value && Array.isArray(value.starts)
  );
}

/** A number to six significant figures. */
function six(value: number): number {
  return value === 0 ? 0 : Number(value.toPrecision(6));
}

/** One cm² in m²: a column per m² times it is the column per cm². */
const M2_PER_CM2 = 1e-4;

/**
 * A line species' self-broadening ratio to air at a temperature, r_self (296 ÷ T)^(Δn_self), as
 * the client takes it (air's 1 and 0 if HITRAN gives none; `decision-r08-t4d-kdist-chain.md` §13,
 * ruling 4).
 */
function selfRatio(species: LineSpecies): (temperatureK: number) => number {
  const ratio = broadeningRatios(species.lines)["self"] ?? 1;
  const offset = broadeningExponentOffsets(species.lines)["self"] ?? 0;
  return (temperatureK) => ratio * (296 / temperatureK) ** offset;
}

/**
 * Each cell's mean transmittance through each path with O₂ and H₂O in one spectrum: per layer, each
 * species' cross-section line by line at the layer's temperature and air-equivalent broadening
 * pressure on one fine grid a cell (the finest any layer needs), τ summed over the layers with each
 * path's columns, and T the cell's mean of e^(−τ).
 */
export function earthReference(
  start: PathsStart,
  o2: LineSpecies,
  h2o: LineSpecies,
  cellsNm: ReadonlyArray<number>,
): ReadonlyArray<{
  readonly cellNm: number;
  readonly transmittance: ReadonlyArray<number>;
  readonly homogeneous: ReadonlyArray<number>;
}> {
  const o2Policy = sourceOf("O2")?.policy;
  const h2oPolicy = sourceOf("H2O")?.policy;
  if (o2Policy === undefined || h2oPolicy === undefined) {
    throw new Error("the k-distribution tool lists no O2 or H2O shape");
  }
  // Each layer's air-equivalent pressure is p (x r_self(T) + 1 − x), as the client takes a bulk of
  // air.
  const o2Self = selfRatio(o2);
  const h2oSelf = selfRatio(h2o);
  const paths = start.zenithDeg.length;
  const out: { cellNm: number; transmittance: number[]; homogeneous: number[] }[] = [];
  for (const cellNm of cellsNm) {
    const [low, high] = cellWavenumbersPerCm(cellNm);
    const parts = [
      {
        species: o2,
        policy: o2Policy,
        self: o2Self,
        fraction: (l: PathsLayer) => l.o2Fraction,
        column: (l: PathsLayer) => l.o2PerM2,
      },
      {
        species: h2o,
        policy: h2oPolicy,
        self: h2oSelf,
        fraction: (l: PathsLayer) => l.h2oFraction,
        column: (l: PathsLayer) => l.h2oPerM2,
      },
    ].flatMap((part) => {
      const indices = linesNear(part.species.lines, low, high, part.policy.cutoffPerCm);
      return indices.length === 0 ? [] : [{ ...part, indices }];
    });
    if (parts.length === 0) {
      continue;
    }
    // One grid for every layer, the finest any needs, so that the layers' τ add point by point.
    const layerStates = parts.map((part) =>
      start.layers.map((l) => {
        const x = part.fraction(l);
        return x > 0
          ? lineStates(
              part.species,
              part.indices,
              l.temperatureK,
              l.pressurePa * (x * part.self(l.temperatureK) + (1 - x)),
            )
          : undefined;
      }),
    );
    let points = 0;
    for (const states of layerStates) {
      for (const s of states) {
        points = s === undefined ? points : Math.max(points, cellPointCount(cellNm, s));
      }
    }
    const tau = Array.from({ length: paths }, () => new Float64Array(points));
    for (const [k, part] of parts.entries()) {
      for (const [i, l] of start.layers.entries()) {
        const states = layerStates[k]?.[i];
        if (states === undefined) {
          continue;
        }
        const shape = lineShape(part.policy, l.temperatureK, [1]);
        const sigma = cellSpectrum(
          cellNm,
          part.species,
          part.indices,
          states,
          points,
          shape,
        ).crossSectionsCm2;
        const columns = part.column(l);
        for (let p = 0; p < paths; p += 1) {
          const u = (columns[p] ?? 0) * M2_PER_CM2;
          const row = tau[p];
          if (row === undefined || !(u > 0)) {
            continue;
          }
          for (let j = 0; j < points; j += 1) {
            row[j] = (row[j] ?? 0) + (sigma[j] ?? 0) * u;
          }
        }
      }
    }
    // Each path as the client reads it: each species homogeneous at its vertical column-weighted
    // temperature and mole fraction and the path's column-weighted (Curtis–Godson) pressure.
    const homogeneous = Array.from({ length: paths }, () => new Float64Array(points));
    for (const part of parts) {
      let weight = 0;
      let tSum = 0;
      let xSum = 0;
      for (const l of start.layers) {
        const u = part.column(l)[0] ?? 0;
        weight += u;
        tSum += u * l.temperatureK;
        xSum += u * part.fraction(l);
      }
      const temperatureK = tSum / weight;
      const x = xSum / weight;
      const shape = lineShape(part.policy, temperatureK, [1]);
      for (let p = 0; p < paths; p += 1) {
        let u = 0;
        let pu = 0;
        for (const l of start.layers) {
          const c = part.column(l)[p] ?? 0;
          u += c;
          pu += c * l.pressurePa;
        }
        if (!(u > 0)) {
          continue;
        }
        const states = lineStates(
          part.species,
          part.indices,
          temperatureK,
          (pu / u) * (x * part.self(temperatureK) + (1 - x)),
        );
        const sigma = cellSpectrum(
          cellNm,
          part.species,
          part.indices,
          states,
          points,
          shape,
        ).crossSectionsCm2;
        const row = homogeneous[p];
        if (row === undefined) {
          continue;
        }
        for (let j = 0; j < points; j += 1) {
          row[j] = (row[j] ?? 0) + (sigma[j] ?? 0) * u * M2_PER_CM2;
        }
      }
    }
    const mean = (row: Float64Array): number => {
      let sum = 0;
      for (const t of row) {
        sum += Math.exp(-t);
      }
      return six(sum / points);
    };
    out.push({ cellNm, transmittance: tau.map(mean), homogeneous: homogeneous.map(mean) });
  }
  return out;
}

/** `earth-reference`: writes the Earth paths' line-by-line cell transmittances. */
export function runEarthReference(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      paths: { type: "string" },
      "o2-lines": { type: "string" },
      "h2o-lines": { type: "string" },
      tips: { type: "string" },
      out: { type: "string" },
    },
  });
  const o2Source = sourceOf("O2");
  const h2oSource = sourceOf("H2O");
  const o2Lines = values["o2-lines"];
  const h2oLines = values["h2o-lines"];
  if (
    values.paths === undefined ||
    o2Lines === undefined ||
    h2oLines === undefined ||
    values.tips === undefined ||
    values.out === undefined ||
    o2Source === undefined ||
    h2oSource === undefined
  ) {
    throw new Error(
      "usage: earth-reference --paths <json> --o2-lines <csv> --h2o-lines <csv> --tips <dir> --out <json>",
    );
  }
  const parsed: unknown = JSON.parse(readFileSync(values.paths, "utf8"));
  if (!isPaths(parsed)) {
    throw new Error(`${values.paths} holds no paths`);
  }
  const o2 = readLineSpecies(o2Source, o2Lines, values.tips);
  const h2o = readLineSpecies(h2oSource, h2oLines, values.tips);
  const cells = Array.from({ length: 381 }, (_, k) => 380 + k);
  const starts = parsed.starts.map((start) => {
    const started = performance.now();
    const result = earthReference(start, o2, h2o, cells);
    process.stdout.write(
      `earth-reference: ${start.startHeightM} m, ${result.length} cells, ${((performance.now() - started) / 1000).toFixed(0)} s\n`,
    );
    return { startHeightM: start.startHeightM, zenithDeg: start.zenithDeg, cells: result };
  });
  const description =
    "Earth's O2 and H2O lines in one spectrum, line by line in f64, through each path of " +
    "earthLinePaths.json (R08.T4.d's end-to-end test; decision-r08-t4d-kdist.md, its tests), " +
    "written by apps/hyperion/scripts/atmosphereData.mjs earth-reference from the HITRAN2024 lines " +
    "the k-distributions are reduced from, with the same line shapes and broadening (air-equivalent " +
    "pressure x r_self + (1 - x) at each layer): each 1 nm cell's mean transmittance per path, in " +
    "zenithDeg's order (t); and the same with each species homogeneous along the path at its " +
    "vertical column-weighted temperature and mole fraction and the path's column-weighted " +
    "(Curtis-Godson) pressure, as the client reads a path (h). The lines are not committed.";
  const lines = starts.map(
    (s) =>
      `    {"startHeightM": ${s.startHeightM}, "zenithDeg": [${s.zenithDeg.join(", ")}], "cells": [\n` +
      s.cells
        .map(
          (c) =>
            `      {"nm": ${c.cellNm}, "t": [${c.transmittance.join(", ")}], "h": [${c.homogeneous.join(", ")}]}`,
        )
        .join(",\n") +
      "\n    ]}",
  );
  writeFileSync(
    values.out,
    `{\n  "description": ${JSON.stringify(description)},\n  "starts": [\n${lines.join(",\n")}\n  ]\n}\n`,
  );
}

/** One state a cell's reference is taken at. */
interface ReferenceState {
  readonly temperatureK: number;
  readonly pressurePa: number;
  readonly kind: "node" | "midpoint";
}

/**
 * The states of a cell's reference on a grid: three node pressures (1 kPa, 0.1 and 1 MPa) at the
 * node nearest `nearK`, and three midpoints, half way in T to the next node and in ln p to the next
 * stored pressure above each.
 *
 * @param nearK - The temperature whose nearest node the states take, K: 250 for T4.d's and T4.g's
 *   temperate tables, an atom's middle node for R08.T4.e's.
 */
export function referenceStates(grid: ReductionGrid, nearK = 250): ReadonlyArray<ReferenceState> {
  const t = grid.temperaturesK.reduce((best, x) =>
    Math.abs(x - nearK) < Math.abs(best - nearK) ? x : best,
  );
  const ti = grid.temperaturesK.indexOf(t);
  const next = grid.temperaturesK[ti + 1] ?? t;
  return [1e3, 1e5, 1e6].flatMap((p): ReferenceState[] => {
    const pi = grid.pressuresPa.findIndex((x) => Math.abs(x / p - 1) < 1e-9);
    const above = grid.pressuresPa[pi + 1] ?? p;
    return [
      { temperatureK: t, pressurePa: p, kind: "node" },
      { temperatureK: 0.5 * (t + next), pressurePa: Math.sqrt(p * above), kind: "midpoint" },
    ];
  });
}

/** Whether a value read from a k-distribution part has its grid: a trust boundary of our own writing. */
function isPart(value: unknown): value is {
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    "temperaturesK" in value &&
    "pressuresPa" in value
  );
}

/** `cell-references`: writes named cells' line-by-line curves and the reduction's f64 curves. */
export function runCellReferences(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      lines: { type: "string" },
      tips: { type: "string" },
      table: { type: "string" },
      cells: { type: "string" },
      out: { type: "string" },
    },
  });
  const source = sourceOf(values.species);
  if (
    source === undefined ||
    values.lines === undefined ||
    values.tips === undefined ||
    values.table === undefined ||
    values.cells === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: cell-references --species <key> --lines <file> --tips <dir> --table <sidecar.json> --cells <nm,...> --out <json>",
    );
  }
  const part: unknown = JSON.parse(readFileSync(values.table, "utf8"));
  if (!isPart(part)) {
    throw new Error(`${values.table} is not a k-distribution part`);
  }
  // The part's pressures on the lattice, as the midpoint rule lets a table hold them
  // (decision-r08-t4e-unsold's follow-up): the standard 30, or E3's with its midpoints.
  const pressuresPa = part.pressuresPa.map(latticePressurePa);
  if (!meetsPressureRule(pressuresPa)) {
    throw new Error(`${values.table}'s pressures do not meet the lattice rule`);
  }
  const grid: ReductionGrid = {
    temperaturesK: part.temperaturesK,
    pressuresPa,
    rowsPa: chainRowsPa(pressuresPa),
  };
  const species = readLineSpecies(source, values.lines, values.tips);
  const spectra = lineByLine(species, source.policy);
  const entries = values.cells
    .split(",")
    .map(Number)
    .map((cellNm) => {
      const cap = (t: number, p: number): number =>
        gatedColumnCm2(source.saturation, cellNm, t, p, source.molecularMassDa);
      // T4.d's sources take helium's Rayleigh column, so their chain cap is the gated column.
      const chainCap = (t: number, p: number): number =>
        chainColumnCm2(source.saturation, cellNm, t, p, source.molecularMassDa, undefined);
      const entry = cellReference(cellNm, spectra, grid, source.policy, cap, chainCap);
      process.stdout.write(`cell-references: ${source.species} ${cellNm} nm\n`);
      return entry;
    });
  writeFileSync(values.out, `${JSON.stringify({ species: source.species, cells: entries })}\n`);
}

/** One state of a cell's committed reference: its columns, line by line and the reduction's `f64` curve. */
export interface CellReferenceState extends ReferenceState {
  readonly columnsCm2: ReadonlyArray<number>;
  readonly lineByLine: ReadonlyArray<number>;
  readonly f64: ReadonlyArray<number>;
}

/**
 * A cell's reference: the cell reduced on its grid ({@link reduceGridCell}), and at each of its
 * {@link referenceStates} its curve of growth from the source's own spectrum (line by line, or the
 * super-lines', R08.T4.g) beside the reduction's `f64` values through the client's chain, on the
 * spectrum's columns to the state's cap (a node's the gated column, a midpoint's the chain test's,
 * which also stops at the species' own Rayleigh column from R08.T4.g on), each to six figures.
 *
 * @param cap - The node tier's column cap ({@link gatedColumnCm2}).
 * @param chainCap - The chain test's, at most `cap` ({@link chainColumnCm2}).
 * @param nearK - The temperature whose nearest node the states take ({@link referenceStates}).
 * @throws Error for a cell with no lines within the cut-off.
 */
export function cellReference(
  cellNm: number,
  spectra: NodeSpectra,
  grid: ReductionGrid,
  policy: ShapePolicy,
  cap: (temperatureK: number, pressurePa: number) => number,
  chainCap: (temperatureK: number, pressurePa: number) => number,
  nearK = 250,
): { readonly nm: number; readonly states: ReadonlyArray<CellReferenceState> } {
  const nP = grid.pressuresPa.length;
  const cell = reduceGridCell(cellNm, spectra, grid, policy, cap, chainCap);
  if (cell === undefined) {
    throw new Error(`no lines within the cut-off of ${cellNm} nm`);
  }
  const g = cell.weights.length;
  const values64 = Array.from({ length: grid.temperaturesK.length * nP }, (_, n) =>
    cell.values.subarray(n * g, (n + 1) * g).map((v) => v / M2_PER_CM2),
  );
  const states = referenceStates(grid, nearK).map((state): CellReferenceState => {
    const exact = spectra(cellNm, state.temperatureK, state.pressurePa, [1])?.node;
    if (exact === undefined) {
      throw new Error(`no lines within the cut-off of ${cellNm} nm at a reference state`);
    }
    const columns = errorColumns(
      exact,
      REDUCTION_TARGET,
      CHAIN_COLUMNS_PER_DECADE,
      (state.kind === "node" ? cap : chainCap)(state.temperatureK, state.pressurePa),
    );
    // The reduction's f64 values through the client's chain at the state (exact at a node).
    const f64 = chainCurve(grid, cell.weights, values64, state.temperatureK, state.pressurePa);
    return {
      temperatureK: state.temperatureK,
      pressurePa: six(state.pressurePa),
      kind: state.kind,
      columnsCm2: Array.from(columns, six),
      lineByLine: Array.from(columns, (u) => six(intervalTransmittance(exact, 0, 1, u))),
      f64: Array.from(columns, (u) => six(f64(u))),
    };
  });
  return { nm: cellNm, states };
}

/**
 * A species' cell curve on a foreign background, line by line with HITRAN2024's own widths for
 * that gas (γ_air where it gives none), against the same curve at the air-equivalent pressure
 * p r_gas with γ_air, as the tables take it (`decision-r08-t4d-kdist.md`, its tests): the largest
 * |ΔT| over the cell's columns, at 296 K, where the widths' temperature exponents do not act.
 */
export function backgroundError(
  species: LineSpecies,
  cellNm: number,
  pressurePa: number,
  gas: "CO2" | "H2" | "He" | "H2O",
  policy: NonNullable<ReturnType<typeof sourceOf>>["policy"],
): number {
  const ratio = broadeningRatios(species.lines)[gas] ?? 1;
  const own = species.lines.foreignHalfWidthsPerCmAtm[gas];
  const foreign: LineSpecies = {
    ...species,
    lines: {
      ...species.lines,
      airHalfWidthsPerCmAtm: species.lines.airHalfWidthsPerCmAtm.map((air, i) => {
        const g = own[i] ?? Number.NaN;
        return Number.isFinite(g) ? g : air;
      }),
    },
  };
  const shape = lineShape(policy, 296, [1]);
  const [low, high] = cellWavenumbersPerCm(cellNm);
  const indices = linesNear(species.lines, low, high, shape.cutoffPerCm);
  const exactStates = lineStates(foreign, indices, 296, pressurePa);
  const tableStates = lineStates(species, indices, 296, pressurePa * ratio);
  const points = Math.max(cellPointCount(cellNm, exactStates), cellPointCount(cellNm, tableStates));
  const exact = sortedNode(
    cellSpectrum(cellNm, foreign, indices, exactStates, points, shape).crossSectionsCm2,
  );
  const table = sortedNode(
    cellSpectrum(cellNm, species, indices, tableStates, points, shape).crossSectionsCm2,
  );
  let worst = 0;
  for (const u of errorColumns(exact, REDUCTION_TARGET, CHAIN_COLUMNS_PER_DECADE, Infinity)) {
    worst = Math.max(
      worst,
      Math.abs(intervalTransmittance(exact, 0, 1, u) - intervalTransmittance(table, 0, 1, u)),
    );
  }
  return worst;
}

/** `background-error`: prints a species' cells' errors on a foreign background at p_eff. */
export function runBackgroundError(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      lines: { type: "string" },
      tips: { type: "string" },
      cells: { type: "string" },
      gas: { type: "string" },
      pressures: { type: "string" },
    },
  });
  const source = sourceOf(values.species);
  const gas = values.gas;
  if (
    source === undefined ||
    values.lines === undefined ||
    values.tips === undefined ||
    values.cells === undefined ||
    (gas !== "CO2" && gas !== "H2" && gas !== "He" && gas !== "H2O")
  ) {
    throw new Error(
      "usage: background-error --species <key> --lines <file> --tips <dir> --cells <nm,...> --gas CO2|H2|He|H2O [--pressures <Pa,...>]",
    );
  }
  const species = readLineSpecies(source, values.lines, values.tips);
  const ratio = broadeningRatios(species.lines)[gas];
  for (const p of (values.pressures ?? "1e4,1e5,1e6").split(",").map(Number)) {
    for (const cellNm of values.cells.split(",").map(Number)) {
      const error = backgroundError(species, cellNm, p, gas, source.policy);
      process.stdout.write(
        `${source.species} on ${gas} (r ${ratio?.toFixed(3) ?? "none"}), ${cellNm} nm, ${p} Pa, 296 K: |dT| ${error.toExponential(2)}\n`,
      );
    }
  }
}
