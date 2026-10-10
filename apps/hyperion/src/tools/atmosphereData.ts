/**
 * `node scripts/atmosphereData.mjs`: reduces the atmosphere's fetched data to the committed values
 * the renderer reads (plan R08, R08.T4.a), since a renderer test fetches and reads no raw file.
 *
 * @remarks
 * Each input is checked against its SHA-256 and never committed by this tool; only the reduced
 * values are, with their citations in `NOTICE`'s Data section (R05.T12.d's ruling,
 * `decisions-r05.md` item 4, and `decision-r08-licences.md`). Later R08 tasks add their own
 * subcommands (R08.T4.b's cross-sections, R08.T5.b's refractive indices).
 *
 * `matching --cie <CIE_xyz_1931_2deg.csv> [--out <json>]` reduces the CIE 1931 2° standard
 * observer's colour-matching functions, 360–830 nm at 1 nm (CIE 018:2019 / ISO/CIE 11664-1:2019,
 * DOI 10.25039/CIE.DS.xvudnb9b, from https://files.cie.co.at/Publications-datasets/, CC BY-SA 4.0;
 * SHA-256 {@link CIE_CSV_SHA256}) to linear Rec. 709 r̄, ḡ and b̄ by {@link XYZ_TO_REC709}, the
 * matrix `view/photometry/toneCurve.ts` carries as `XYZ_TO_SRGB`. The CSV is the one R06 commits
 * unmodified in `crates/hyperion-fit/data/cie_cmf/` as a tool input (its `PROVENANCE.toml`), so no
 * fetch is needed; any copy with the same checksum serves. The output is
 * `src/renderer/src/view/atmosphere/colourMatching.json` unless `--out` names another file: the
 * values are derived, and offered under CC BY-SA 4.0 to the extent they are adapted material.
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { CIE_CSV_SHA256, type MatchingSample, parseCie, XYZ_TO_REC709 } from "./solarFactors";

/** The matching functions' file, relative to `apps/hyperion`. */
export const MATCHING_OUTPUT = "src/renderer/src/view/atmosphere/colourMatching.json";

/** The CIE's required citation for the 1931 2° functions (CIE's dataset page; `NOTICE`). */
export const CIE_CITATION =
  "CIE 2019, Colour-matching functions of CIE 1931 standard colorimetric observer, International " +
  "Commission on Illumination (CIE), Vienna, AT, DOI: 10.25039/CIE.DS.xvudnb9b";

/** What `colourMatching.json` holds. */
export interface ReducedMatching {
  readonly description: string;
  readonly source: string;
  readonly sha256: string;
  readonly licence: string;
  /** The matrix that took x̄, ȳ, z̄ to r̄, ḡ, b̄, as rows. */
  readonly xyzToRgb: readonly (readonly [number, number, number])[];
  /** The first row's wavelength, nm. */
  readonly firstNm: number;
  /** The rows' spacing, nm. */
  readonly stepNm: number;
  /** Each row: the wavelength, nm, and r̄, ḡ, b̄. */
  readonly rows: readonly (readonly [number, number, number, number])[];
}

/**
 * The matching functions as linear Rec. 709, row by row.
 *
 * @throws Error if the rows are not at a constant step of 1 nm.
 */
export function reduceMatching(cmf: readonly MatchingSample[]): ReducedMatching {
  const first = cmf[0];
  if (first === undefined) {
    throw new Error("the matching functions have no rows");
  }
  const rows = cmf.map((row, i) => {
    if (row.wavelengthNm !== first.wavelengthNm + i) {
      throw new Error(`row ${i + 1} is at ${row.wavelengthNm} nm, not ${first.wavelengthNm + i}`);
    }
    const [r, g, b] = XYZ_TO_REC709.map((m) => m[0] * row.x + m[1] * row.y + m[2] * row.z);
    return [row.wavelengthNm, r ?? Number.NaN, g ?? Number.NaN, b ?? Number.NaN] as const;
  });
  return {
    description:
      "The CIE 1931 2-degree colour-matching functions as linear Rec. 709 r, g and b at 1 nm, " +
      "each row [nm, r, g, b], written by apps/hyperion/scripts/atmosphereData.mjs matching " +
      "(rendering plan R08, R08.T4.a). Derived values, not the CIE's table.",
    source: `${CIE_CITATION}; CIE_xyz_1931_2deg.csv from https://files.cie.co.at/Publications-datasets/`,
    sha256: CIE_CSV_SHA256,
    licence:
      "CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/); to the extent these " +
      "values are adapted material they are offered under CC BY-SA 4.0",
    xyzToRgb: XYZ_TO_REC709,
    firstNm: first.wavelengthNm,
    stepNm: 1,
    rows,
  };
}

/** A row of numbers as JSON on one line, each in its shortest round-trip form. */
function line(values: readonly number[]): string {
  return `[${values.join(", ")}]`;
}

/** The file's text: JSON, one row a line, every number in its shortest round-trip form. */
export function matchingText(reduced: ReducedMatching): string {
  const fields = [
    `  "description": ${JSON.stringify(reduced.description)}`,
    `  "source": ${JSON.stringify(reduced.source)}`,
    `  "sha256": ${JSON.stringify(reduced.sha256)}`,
    `  "licence": ${JSON.stringify(reduced.licence)}`,
    `  "xyzToRgb": [\n${reduced.xyzToRgb.map((r) => `    ${line(r)}`).join(",\n")}\n  ]`,
    `  "firstNm": ${reduced.firstNm}`,
    `  "stepNm": ${reduced.stepNm}`,
    `  "rows": [\n${reduced.rows.map((r) => `    ${line(r)}`).join(",\n")}\n  ]`,
  ];
  return `{\n${fields.join(",\n")}\n}\n`;
}

/**
 * A file's text, if its SHA-256 is the one given.
 *
 * @throws Error naming both digests otherwise.
 */
export function readChecked(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

const USAGE = "usage: atmosphereData matching --cie <CIE_xyz_1931_2deg.csv> [--out <json>]";

/** The command line; writes the reduced file. Returns the exit code. */
export function main(args: readonly string[]): number {
  try {
    const [command, ...rest] = args;
    if (command !== "matching") {
      throw new Error(USAGE);
    }
    const { values } = parseArgs({
      args: rest,
      options: { cie: { type: "string" }, out: { type: "string" } },
    });
    if (values.cie === undefined) {
      throw new Error(USAGE);
    }
    const reduced = reduceMatching(parseCie(readChecked(values.cie, CIE_CSV_SHA256)));
    const out = values.out ?? MATCHING_OUTPUT;
    writeFileSync(out, matchingText(reduced));
    process.stdout.write(`atmosphere-data: wrote ${reduced.rows.length} rows to ${out}\n`);
    return 0;
  } catch (error: unknown) {
    console.error(`atmosphere-data: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
