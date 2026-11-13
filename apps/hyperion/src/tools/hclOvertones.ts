/**
 * `node scripts/atmosphereData.mjs hcl-overtones …` (plan R08, R08.T4.h): HCl's `overtones` pending
 * part, beside its HITRAN2024 table, written into `absorbers/bounds.json`.
 *
 * @remarks
 * R08.T4.h's science check kept HITRAN2024's 5–0 to 7–0 bands, whose intensities rest on
 * measurements (Gelfand et al. 1981, times HITRAN2024's 1.3 after Vasilchenko et al. 2023's 5–0;
 * Gordon et al. 2026, §2.15), and judged its 8–0 band an extrapolation: Li et al. 2013's dipole
 * moment function carried a quantum past its data, without the 1.3, which HITRAN codes an estimate.
 * HITRAN holds no band above 8–0 since HITRAN2016 (§2.15 there, numerical errors at Δv > 8), though
 * 9–0 to 11–0 lie in the range (12–0's lines lie above 26,316 cm⁻¹, 380 nm). So the part holds:
 *
 * - the 8–0 band: HITRAN2024's lines from {@link HCL_TABLE_BELOW_PER_CM} up, unchanged, `estimated`;
 * - 9–0 to 11–0: those lines, each moved by its band's place on H³⁵Cl's vibrational ladder,
 *   E(v) − E(8) ({@link HCL_LEVELS_PER_CM}), and divided by 10 for each quantum, keeping its
 *   lower-state energy: R08.T4.c's overtone-ladder estimate, CO's construction
 *   (`decision-r08-t4b-t12c.md`, follow-up 3, ruling 3), below the last band kept, held in a part as
 *   CO's is (follow-up 3, ruling 2).
 *
 * Each line's S(T) is HITRAN's by TIPS, its moved wavenumber in the stimulated-emission factor; its
 * cell is the 1 nm vacuum cell of its wavenumber, the sum over the cell's width in cm⁻¹, each value
 * rounded up to three figures. The lines and TIPS files are checked against their SHA-256 and not
 * committed (`decision-r08-licences.md`).
 *
 * `hcl-overtones --lines <hcl-13100-26400-broad.csv> --tips <dir> [--bounds <bounds.json>]` writes
 * the entry into the bounds file (`absorbers/bounds.json` unless given), replacing an earlier one.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { CELL_COUNT, cellOf, cellWidthPerCm, roundUpToThreeFigures } from "./atomicLines";
import { boundEntryText, type BoundEntry } from "./atomicLinesCli";
import { HCL_TABLE_BELOW_PER_CM, LINE_SOURCES } from "./kDistributionsCli";
import {
  intensityAt,
  type LineList,
  linesBeyond,
  parseHitranLines,
  parsePartitionSums,
  type PartitionSums,
  partitionSumAt,
  readCheckedText,
} from "./molecularLines";

/**
 * H³⁵Cl's vibrational levels E(v, J = 0), cm⁻¹, for v = 8 to 11: E′ = ν + E″ of each P(1) line of
 * its hot bands in HITRAN2024 (`https://hitran.org/lbl/api?iso_ids_list=52&numin=10000&numax=27000&fixwidth=0&request_params=par_line`,
 * `hcl-52-10000-27000.par`, SHA-256 `ac9e3bea…15c3d`, fetched 2026-10-19; the full hash in
 * {@link HCL_LEVELS_SOURCE}). Huber and Herzberg's three-term constants put them 66–240 cm⁻¹ higher
 * there, beyond their fit.
 */
export const HCL_LEVELS_PER_CM: Readonly<Record<8 | 9 | 10 | 11, number>> = {
  8: 20_196.056,
  9: 22_251.585,
  10: 24_197.664,
  11: 26_030.666,
};

/** The levels' source, as the entry names it. */
export const HCL_LEVELS_SOURCE =
  "H35Cl's vibrational levels E(v, J = 0) from its hot bands' P(1) lines, E' = nu + E'', in HITRAN2024's " +
  "records (https://hitran.org/lbl/api?iso_ids_list=52&numin=10000&numax=27000&fixwidth=0&request_params=par_line, " +
  "hcl-52-10000-27000.par, SHA-256 ac9e3bea0f736f89d096441e9da0e92525e782088f90890a26b6179882015c3d, fetched 2026-10-19): " +
  "v = 8 to 11 at 20196.056, 22251.585, 24197.664 and 26030.666 cm-1";

/** The part's temperature nodes, K: the table's 50 K grid with HITRAN's 296 K for its 300 K. */
export const HCL_OVERTONE_NODES_K = [50, 100, 150, 200, 250, 296, 350, 400, 450, 500] as const;

/** The 8–0 band's lines and their TIPS, from the checked HITRAN file. */
export interface HclOvertoneLines {
  readonly lines: LineList;
  readonly partitionSums: ReadonlyMap<number, PartitionSums>;
}

/**
 * The part's 1 nm cell means at a temperature, m² a molecule: the 8–0 lines where HITRAN puts them
 * and, for v = 9 to 11, moved by E(v) − E(8) and divided by 10^(v − 8).
 */
export function hclOvertoneCellMeansM2(data: HclOvertoneLines, temperatureK: number): Float64Array {
  const sumsCm = new Float64Array(CELL_COUNT);
  const { lines } = data;
  for (let i = 0; i < lines.wavenumbersPerCm.length; i += 1) {
    const sums = data.partitionSums.get(lines.isotopologues[i] ?? 0);
    if (sums === undefined) {
      throw new Error(`no TIPS for HITRAN isotopologue ${lines.isotopologues[i]}`);
    }
    const q296 = partitionSumAt(sums, 296);
    const qT = partitionSumAt(sums, temperatureK);
    for (const v of [8, 9, 10, 11] as const) {
      const nu =
        (lines.wavenumbersPerCm[i] ?? Number.NaN) + HCL_LEVELS_PER_CM[v] - HCL_LEVELS_PER_CM[8];
      const c = cellOf(1e7 / nu);
      if (c < 0) {
        continue;
      }
      const s =
        intensityAt(
          lines.intensitiesCm[i] ?? Number.NaN,
          lines.lowerEnergiesPerCm[i] ?? Number.NaN,
          nu,
          q296,
          qT,
          temperatureK,
        ) /
        10 ** (v - 8);
      sumsCm[c] = (sumsCm[c] ?? 0) + s;
    }
  }
  return Float64Array.from(sumsCm, (s, c) => (s / cellWidthPerCm(c)) * 1e-4);
}

/** HCl's 8–0 lines and TIPS from the checked files of {@link LINE_SOURCES}' HCl. */
export function readHclOvertoneLines(linesPath: string, tipsDirectory: string): HclOvertoneLines {
  const source = LINE_SOURCES["HCl"];
  if (source === undefined) {
    throw new Error("LINE_SOURCES holds no HCl");
  }
  const all = parseHitranLines(readCheckedText(linesPath, source.lines.sha256));
  return {
    lines: linesBeyond(all, HCL_TABLE_BELOW_PER_CM, "above"),
    partitionSums: new Map(
      source.tips.map((t) => [
        t.isotopologue,
        parsePartitionSums(readCheckedText(join(tipsDirectory, t.name), t.sha256)),
      ]),
    ),
  };
}

/** The part's entry, its cells at each of {@link HCL_OVERTONE_NODES_K}. */
export function hclOvertoneEntry(data: HclOvertoneLines): BoundEntry {
  const source = LINE_SOURCES["HCl"];
  if (source === undefined) {
    throw new Error("LINE_SOURCES holds no HCl");
  }
  const rows = HCL_OVERTONE_NODES_K.map((t) =>
    Array.from(hclOvertoneCellMeansM2(data, t), roundUpToThreeFigures),
  );
  let largest = { value: -1, cell: 0, temperatureK: 0 };
  for (const [n, row] of rows.entries()) {
    for (const [c, v] of row.entries()) {
      if (v > largest.value) {
        largest = { value: v, cell: c, temperatureK: HCL_OVERTONE_NODES_K[n] ?? 0 };
      }
    }
  }
  // Each band's span in cells, from its lines' extremes, and its largest cell at 296 K.
  const at296 = rows[HCL_OVERTONE_NODES_K.indexOf(296)] ?? [];
  const lowest = Math.min(...data.lines.wavenumbersPerCm);
  const highest = Math.max(...data.lines.wavenumbersPerCm);
  const bands = ([8, 9, 10, 11] as const).map((v) => {
    const shift = HCL_LEVELS_PER_CM[v] - HCL_LEVELS_PER_CM[8];
    const fromNm = Math.round(1e7 / (highest + shift));
    const toNm = Math.round(1e7 / (lowest + shift));
    const top = Math.max(0, ...at296.filter((_, c) => 380 + c >= fromNm && 380 + c <= toNm));
    return `${top.toExponential(2)} m2 (${v}-0, ${Math.max(fromNm, 380)}-${toNm} nm)`;
  });
  return {
    key: "HCl",
    kind: "species",
    task: "R08.T4.f",
    part: "overtones",
    basis: "estimated",
    temperaturesK: HCL_OVERTONE_NODES_K,
    source:
      `${source.citation}: its ${data.lines.wavenumbersPerCm.length} lines from ${HCL_TABLE_BELOW_PER_CM.toFixed(1)} cm-1 up (${source.url}, ` +
      `${source.lines.name} SHA-256 ${source.lines.sha256}; ${source.tips.map((t) => `${t.name} ${t.sha256}`).join(", ")}; fetched ${source.fetched}); ${HCL_LEVELS_SOURCE}`,
    method:
      "Stated estimate (R08.T4.h's science check): HITRAN2024's 8-0 band, never measured (Li et al. 2013's dipole moment function, J. Quant. Spectrosc. Radiat. Transfer 121, 78, " +
      "carried a quantum past its data, without HITRAN2024's factor 1.3 for 4-0 to 7-0; Gordon et al. 2026, section 2.15), unchanged, and its 9-0, 10-0 and 11-0 bands, which HITRAN " +
      "holds none of since HITRAN2016 (section 2.15 there), by the overtone ladder below 8-0 (decision-r08-t4b-t12c.md follow-up 3, ruling 3, CO's construction, a part as CO's is, ruling 2): each 8-0 line moved by " +
      "E(v) - E(8) of H35Cl's levels and divided by 10 for each quantum, keeping its lower-state energy; S(T) by TIPS, the moved wavenumber in the stimulated-emission factor, summed in " +
      "the 1 nm vacuum cell [c - 0.5, c + 0.5) nm over its width in cm-1, each value rounded up to 3 significant figures; 12-0 lies above 26316 cm-1 (380 nm). " +
      `At 296 K each band's largest cell is ${bands.join(", ")}; ` +
      `the largest at any node ${largest.value.toExponential(2)} m2 at ${380 + largest.cell} nm and ${largest.temperatureK} K.`,
    cellMeansM2: rows,
  };
}

/**
 * A bounds file's text with an entry put in: in place of the entry of its key, task and part, or
 * before the file's end.
 *
 * @throws Error if the file is not in `bounds.json`'s layout.
 */
export function withBoundEntry(text: string, entry: BoundEntry): string {
  const parsed: unknown = JSON.parse(text);
  if (typeof parsed !== "object" || parsed === null || !("bounds" in parsed)) {
    throw new Error("not a bounds file");
  }
  const entryText = boundEntryText(entry);
  const marker = `    {\n      "key": ${JSON.stringify(entry.key)},`;
  let at = text.indexOf(marker);
  while (at >= 0) {
    const end = text.indexOf("\n    }", at) + "\n    }".length;
    const block = text.substring(at, end);
    const samePart =
      entry.part === undefined
        ? !block.includes(`"part":`)
        : block.includes(`"part": ${JSON.stringify(entry.part)}`);
    if (block.includes(`"task": ${JSON.stringify(entry.task)}`) && samePart) {
      return `${text.slice(0, at)}${entryText}${text.slice(end)}`;
    }
    at = text.indexOf(marker, end);
  }
  const close = text.lastIndexOf("\n  ]\n}");
  if (close < 0) {
    throw new Error("the bounds file does not end in bounds.json's layout");
  }
  return `${text.slice(0, close)},\n${entryText}${text.slice(close)}`;
}

/** `hcl-overtones`: writes HCl's overtones part into the bounds file. */
export function runHclOvertones(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      lines: { type: "string" },
      tips: { type: "string" },
      bounds: { type: "string" },
    },
  });
  if (values.lines === undefined || values.tips === undefined) {
    throw new Error("usage: hcl-overtones --lines <csv> --tips <dir> [--bounds <bounds.json>]");
  }
  const path = values.bounds ?? "src/renderer/src/view/atmosphere/absorbers/bounds.json";
  const entry = hclOvertoneEntry(readHclOvertoneLines(values.lines, values.tips));
  writeFileSync(path, withBoundEntry(readFileSync(path, "utf8"), entry));
  process.stdout.write(
    `atmosphere-data: wrote HCl's overtones part into ${path}\n${entry.method}\n`,
  );
}
