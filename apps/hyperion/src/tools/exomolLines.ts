/**
 * ExoMol's line lists for R08.T4.g and R08.T4.f (plan R08; `decision-r08-t4d-kdist-chain.md`
 * §§9–11; decision-r08-t4ef-storage): the states and transition files of ExoMol's lists, read by
 * streaming each `.bz2` file through the system's `bzip2 -dc` (which must be on the PATH: Linux and
 * macOS ship it; on Windows, Git for Windows or MSYS2 provides it), each line's intensity at every
 * temperature asked for binned into super-lines (`superLines.ts`), and the bins cached outside the
 * repository so that a refinement pass reads only its new temperatures.
 *
 * @remarks
 * - **Intensities** (S. N. Yurchenko, A. F. Al-Refaie and J. Tennyson, Astron. Astrophys. 614
 *   (2018) A131, ExoCross's Eq. 1, on the files of J. Tennyson et al., J. Mol. Spectrosc. 327
 *   (2016) 73, the ExoMol format paper): a transition from
 *   lower state l to upper state u of wavenumber ν = E_u − E_l has
 *   S(T) = g_u A_ul ÷ (8π c ν²) × e^(−c₂E_l ÷ T) (1 − e^(−c₂ν ÷ T)) ÷ Q(T), cm a molecule, c in
 *   cm s⁻¹, per molecule of the isotopologue. Each is multiplied by the isotopologue's natural
 *   abundance, so that, as T4.d's HITRAN lines are, it is per molecule of the species. R08.T4.g's
 *   lists hold one isotopologue each, the minor ones not drawn; R08.T4.f's TiO takes Toto's five
 *   Ti isotopologues into one set of bins, each at its own Q and abundance.
 * - **Q(T)** is summed from the states file, Σ g e^(−c₂E ÷ T), where the source says so: ExoMol
 *   serves no `.pf` for C2018, and TYM's `.pf` is read only as a check of the sum. R08.T4.f's lists
 *   take the list's own `.pf` ({@link partitionSumFromPf}), since a states file may hold only the
 *   states its transitions reach (MoLLIST's FeH: its sum falls 3.1 times under the `.pf` at
 *   3,000 K).
 * - **Widths.** No list gives air widths. Every line takes the list's default from its `def.json`
 *   or `.def` (cm⁻¹ bar⁻¹ at 296 K, ExoMol's unit), converted to atm, with its exponent: C2018's
 *   0.053 cm⁻¹ bar⁻¹ (0.0537 cm⁻¹ atm⁻¹) and TYM's 0.07 (0.0709), both with n = 0.5, `estimated`.
 *   C2018's H₂ and He `.broad` files give each line's width in those gases by its quantum numbers
 *   ({@link broadWidth}), from which their ratios to the default air width and their exponents are
 *   taken as T4.d takes HITRAN's.
 * - **The archive's files.** ExoMol's own site was not reachable for R08.T4.f; its lists were
 *   fetched from the database's archive of each dataset on Zenodo (each record's DOI in its source),
 *   whose definitions are the `.def` text files ({@link parseDefinitionText}). Where a file's
 *   SHA-256 was recorded at an earlier fetch from ExoMol's site, the two are compared.
 * - **A REPACK file** (R08.T4.f's VO): a list's strong lines as P. E. Cubillos's `repack` writes
 *   them ({@link RepackFile}), each with its own ν, E″ and g″f and no states file. Its g″f is
 *   g_u A_ul ÷ (8π² c ν² r_e), so that S(T) = π r_e g″f × e^(−c₂E_l ÷ T) (1 − e^(−c₂ν ÷ T)) ÷ Q(T),
 *   the intensity above ({@link repackStrengthCm}).
 */

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, openSync, readFileSync, readSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { ELECTRON_RADIUS_CM } from "./atomicLines";
import { SECOND_RADIATION_CONSTANT_CM_K, SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";
import {
  applyStimulatedEmission,
  binStepPerCm,
  type BinsWindow,
  emptyBins,
  type SuperLineBins,
  type SuperLineSpecies,
} from "./superLines";

/** One bar in atmospheres: ExoMol's widths are per bar, HITRAN's and the tables' per atm. */
export const ATM_PER_BAR = 1e5 / 101_325;

/** A fetched file's name, size and checksum. */
export interface ExoMolFile {
  readonly name: string;
  readonly bytes: number;
  readonly sha256: string;
}

/** A transition file and the wavenumbers it covers, cm⁻¹. */
export interface TransitionFile extends ExoMolFile {
  readonly fromPerCm: number;
  readonly toPerCm: number;
}

/**
 * A list's definition file: ExoMol's `def.json` ({@link parseDefinition}), or the `.def` text the
 * database's archive holds ({@link parseDefinitionText}).
 */
export interface DefinitionFile extends ExoMolFile {
  readonly format: "json" | "text";
}

/** Where a states file keeps the fields read here, 1-based whitespace-separated field numbers. */
export interface StatesLayout {
  readonly energy: number;
  readonly degeneracy: number;
  readonly j: number;
  /** NH₃'s K, for its `.broad` files' m1 code. */
  readonly k?: number;
  /** N₂O's Herzberg v₁ v₂ l₂ v₃, for matching HITRAN's bands. */
  readonly vibration?: readonly [number, number, number, number];
  /** A diatomic's vibrational quantum number v, for its source's cut on Δv (CO's, R08.T4.f). */
  readonly v?: number;
}

/**
 * Where a list's Q(T) comes from: the states file's sum, with ExoMol's `.pf` read only as a check
 * of it where ExoMol serves one (R08.T4.g's C2018, which has none, and TYM); or the list's `.pf`
 * itself (R08.T4.f's lists, whose states file may hold only the states its transitions reach).
 */
export type PartitionSource =
  | { readonly from: "states"; readonly check: ExoMolFile | undefined }
  | { readonly from: "pf"; readonly file: ExoMolFile };

/** A list's `.pf` file, whether it gives Q(T) or checks the states' sum, or `undefined` for none. */
export function partitionFileOf(source: ExoMolSource): ExoMolFile | undefined {
  let file: ExoMolFile | undefined;
  switch (source.partition.from) {
    case "states":
      file = source.partition.check;
      break;
    case "pf":
      file = source.partition.file;
      break;
  }
  return file;
}

/** A width every line of a list takes where its definition gives none, with why it was taken. */
export interface AssumedWidth {
  /** cm⁻¹ bar⁻¹ at 296 K, ExoMol's unit. */
  readonly halfWidthPerCmBar: number;
  readonly exponent: number;
  readonly why: string;
}

/**
 * A `.broad` file of one broadening gas (ExoMol's "diet", E. J. Barton et al., J. Quant. Spectrosc.
 * Radiat. Transf. 203 (2017) 490): γ (cm⁻¹ bar⁻¹ at 296 K) and n by code; a line no code reaches
 * takes the gas's default from the list's `def.json` ({@link ExoMolDefinition}).
 */
export interface BroadeningFile extends ExoMolFile {
  readonly gas: "H2" | "He";
  readonly url: string;
}

/**
 * A REPACK file of a list's strong lines (P. E. Cubillos, Astrophys. J. 850 (2017) 32, DOI
 * 10.3847/1538-4357/aa9228; its `repack` code's `read_lbl` and `pack.py`'s
 * `struct.pack("dddi", …)`): no header, then 28-byte little-endian records,
 * each f64 ν (cm⁻¹), f64 E″ (cm⁻¹), f64 g″f and i32 isotope tag, mostly in ν's order. Its record's
 * MD5 is the archive's published checksum, beside the SHA-256 taken at the fetch.
 */
export interface RepackFile extends ExoMolFile {
  readonly md5: string;
  /** The tag every record carries: the one isotopologue the file holds. */
  readonly isotope: number;
}

/** What an ExoMol line list's source record names, whatever file holds its lines. */
interface LineListRecord {
  /** The registry key. */
  readonly species: string;
  /** The citation: the list's paper and the database's. */
  readonly citation: string;
  /** The dataset's directory, under which its files are named. */
  readonly url: string;
  readonly fetched: string;
  readonly terms: string;
  readonly def: DefinitionFile;
  /** Where Q(T) comes from ({@link PartitionSource}). */
  readonly partition: PartitionSource;
  /**
   * The isotopologue's natural abundance: HITRAN's (hitran.org/docs/iso-meta/) for a species HITRAN
   * holds, and otherwise the product of its atoms' (each source names its own).
   */
  readonly abundance: number;
  /** The wavenumbers binned, cm⁻¹: every line whose wing reaches the cells reduced. */
  readonly binRangePerCm: readonly [number, number];
  /** The width every line takes where the definition gives none, a stated estimate. */
  readonly assumedWidth?: AssumedWidth;
  /**
   * A factor every binned line's intensity takes, with why, a stated estimate (Li2015's Δv = 7
   * bands scaled to HITRAN2024's measurement-anchored 7–0, as R08.T4.c's bound is), or none.
   */
  readonly intensityScale?: { readonly factor: number; readonly why: string };
}

/**
 * An ExoMol line list as its own files hold it, as R08.T4.g and R08.T4.f read it: one
 * isotopologue's states and transition files, layout and abundance.
 */
export interface ExoMolFilesSource extends LineListRecord {
  readonly kind: "exomol";
  readonly states: ExoMolFile & { readonly layout: StatesLayout };
  readonly transitions: ReadonlyArray<TransitionFile>;
  readonly broadeners: ReadonlyArray<BroadeningFile>;
  /**
   * The largest v′ − v″ a transition keeps, by the states' `v` ({@link StatesLayout.v}), or none:
   * Li2015's CO bands from Δv = 8 are dropped (decision-r08-t4b-t12c.md follow-up 3).
   */
  readonly maxDeltaV?: number;
}

/**
 * An ExoMol line list's strong lines as a REPACK file holds them ({@link RepackFile}; R08.T4.f's
 * VO): no states file, so Q(T) is the list's `.pf`, and the mass and default width its `.def`'s.
 */
export interface RepackSource extends LineListRecord {
  readonly kind: "repack";
  readonly lines: RepackFile;
  readonly partition: { readonly from: "pf"; readonly file: ExoMolFile };
}

/** An ExoMol line list's source record: its own files, or a REPACK file of its strong lines. */
export type ExoMolSource = ExoMolFilesSource | RepackSource;

/**
 * What a list's `def.json` gives the reduction, read from the checksummed file and never typed
 * in: the isotopologue's mass, Da, for the Doppler width and the caps; the default air width,
 * cm⁻¹ bar⁻¹ at 296 K (ExoMol's unit), and its exponent; and each `.broad` gas's default.
 */
export interface ExoMolDefinition {
  readonly molecularMassDa: number;
  readonly airHalfWidthPerCmBar: number;
  readonly airExponent: number;
  readonly broadeners: Readonly<
    Record<string, { readonly halfWidthPerCmBar: number; readonly exponent: number }>
  >;
}

/** A value at a path of a parsed JSON object, or `undefined`. */
function jsonAt(value: unknown, path: ReadonlyArray<string>): unknown {
  let here = value;
  for (const key of path) {
    if (typeof here !== "object" || here === null || !(key in here)) {
      return undefined;
    }
    const next: unknown = Reflect.get(here, key);
    here = next;
  }
  return here;
}

/**
 * A `def.json`'s mass and default widths ({@link ExoMolDefinition}): `isotopologue.mass_in_Da`,
 * `broad["default_Lorentzian_half-width"]` and `broad.default_temperature_exponent`, and for each
 * gas named, `broad[gas].Lorentzian_half_width` and `.temperature_exponent`.
 *
 * @throws Error for a field that is missing or not a finite number above 0 (an exponent may be 0).
 */
export function parseDefinition(
  text: string,
  gases: ReadonlyArray<string>,
  name: string,
): ExoMolDefinition {
  const parsed: unknown = JSON.parse(text);
  const number = (path: ReadonlyArray<string>, positive: boolean): number => {
    const v = jsonAt(parsed, path);
    if (typeof v !== "number" || !Number.isFinite(v) || (positive ? v <= 0 : v < 0)) {
      throw new Error(
        `${name}'s ${path.join(".")} is not a finite number${positive ? " above 0" : ""}`,
      );
    }
    return v;
  };
  const broadeners: Record<string, { halfWidthPerCmBar: number; exponent: number }> = {};
  for (const gas of gases) {
    broadeners[gas] = {
      halfWidthPerCmBar: number(["broad", gas, "Lorentzian_half_width"], true),
      exponent: number(["broad", gas, "temperature_exponent"], false),
    };
  }
  return {
    molecularMassDa: number(["isotopologue", "mass_in_Da"], true),
    airHalfWidthPerCmBar: number(["broad", "default_Lorentzian_half-width"], true),
    airExponent: number(["broad", "default_temperature_exponent"], false),
    broadeners,
  };
}

/**
 * A `.def` text file's mass and default width ({@link ExoMolDefinition}), the ExoMol format's
 * definition file (J. Tennyson et al., J. Mol. Spectrosc. 327 (2016) 73), each value at the start
 * of its line and its label after `#`: `Isotopologue mass (Da) and (kg)`, `Default value of
 * Lorentzian half-width for all lines (in cm-1/bar)` and `Default value of temperature exponent for
 * all lines`. A file that gives no default width takes `assumed` where given. It names no `.broad`
 * gas's default, so its definition lists none.
 *
 * @throws Error for a value missing (the width's without `assumed`), or not a finite number above 0
 *   (an exponent may be 0).
 */
export function parseDefinitionText(
  text: string,
  name: string,
  assumed?: AssumedWidth,
): ExoMolDefinition {
  const labelled = (label: string): number | undefined => {
    for (const line of text.split(/\r?\n/u)) {
      const hash = line.indexOf("#");
      if (
        hash >= 0 &&
        line
          .slice(hash + 1)
          .trim()
          .startsWith(label)
      ) {
        // A label with no value before it is missing, not 0 (`Number("")`).
        const value = line.slice(0, hash).trim().split(/\s+/u)[0] ?? "";
        return value === "" ? undefined : Number(value);
      }
    }
    return undefined;
  };
  const number = (label: string, positive: boolean, fallback: number | undefined): number => {
    const v = labelled(label) ?? fallback;
    if (v === undefined || !Number.isFinite(v) || (positive ? v <= 0 : v < 0)) {
      throw new Error(
        `${name}'s "${label}" is not given as a finite number${positive ? " above 0" : ""}`,
      );
    }
    return v;
  };
  return {
    molecularMassDa: number("Isotopologue mass (Da)", true, undefined),
    airHalfWidthPerCmBar: number(
      "Default value of Lorentzian half-width",
      true,
      assumed?.halfWidthPerCmBar,
    ),
    airExponent: number("Default value of temperature exponent", false, assumed?.exponent),
    broadeners: {},
  };
}

/**
 * A source's definition from its checked file ({@link parseDefinition} or
 * {@link parseDefinitionText}, by its format), with the `.broad` gases' defaults where it is a
 * `def.json`.
 *
 * @throws Error for a file whose size or SHA-256 is not the record's, or a definition the parser
 *   refuses.
 */
export function readDefinition(dataDir: string, source: ExoMolSource): ExoMolDefinition {
  const text = readCheckedSmall(join(dataDir, source.def.name), source.def);
  return source.def.format === "json"
    ? parseDefinition(
        text,
        source.kind === "exomol" ? source.broadeners.map((file) => file.gas) : [],
        source.def.name,
      )
    : parseDefinitionText(text, source.def.name, source.assumedWidth);
}

/** A states file's fields, by state ID (index 0 unused). */
export interface ExoMolStates {
  readonly energiesPerCm: Float64Array;
  readonly degeneracies: Float64Array;
  readonly j: Int32Array;
  readonly k: Int32Array | undefined;
  /** v₁, v₂, l₂ and v₃ packed into one integer a state ({@link packVibration}), or none. */
  readonly vibration: Int32Array | undefined;
  /** A diatomic's v by state, where the layout reads it, or none. */
  readonly v: Int32Array | undefined;
}

/** v₁ v₂ l₂ v₃ as one integer, each quantum under 64. */
export function packVibration(v1: number, v2: number, l2: number, v3: number): number {
  return ((v1 * 64 + v2) * 64 + l2) * 64 + v3;
}

/** A packed vibration's quanta, as HITRAN writes them: `v1 v2 l2 v3`. */
export function vibrationLabel(packed: number): string {
  const v3 = packed % 64;
  const l2 = Math.floor(packed / 64) % 64;
  const v2 = Math.floor(packed / 4_096) % 64;
  const v1 = Math.floor(packed / 262_144);
  return `${v1} ${v2} ${l2} ${v3}`;
}

/** A band's key: its upper and lower packed vibrations. */
export function bandKey(upper: number, lower: number): number {
  return upper * 16_777_216 + lower;
}

/** A band key as HITRAN writes its two global quanta: `v1 v2 l2 v3 - v1 v2 l2 v3`. */
export function bandLabel(key: number): string {
  return `${vibrationLabel(Math.floor(key / 16_777_216))} - ${vibrationLabel(key % 16_777_216)}`;
}

/** A file's SHA-256, read in pieces so that a large file is not held whole. */
export function fileSha256(path: string): string {
  const hash = createHash("sha256");
  const fd = openSync(path, "r");
  try {
    const buffer = Buffer.allocUnsafe(1 << 24);
    for (;;) {
      const read = readSync(fd, buffer, 0, buffer.length, null);
      if (read === 0) {
        break;
      }
      hash.update(buffer.subarray(0, read));
    }
  } finally {
    closeSync(fd);
  }
  return hash.digest("hex");
}

/**
 * Checks a fetched file's size and SHA-256.
 *
 * @throws Error naming the file for a size or checksum that is not the record's.
 */
export function checkFile(path: string, file: ExoMolFile): void {
  const bytes = statSync(path).size;
  if (bytes !== file.bytes) {
    throw new Error(`${path}: ${bytes} bytes, expected ${file.bytes}`);
  }
  const digest = fileSha256(path);
  if (digest !== file.sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${file.sha256}`);
  }
}

/**
 * Reads a file's lines, each line's bytes given to `onLine` with its start and end, and resolves to
 * the count: {@link streamBzip2Lines} for the fetched files, or a test's reader of plain text. An
 * abort of `signal` stops the read.
 */
export type LineReader = (
  path: string,
  onLine: (bytes: Uint8Array, start: number, end: number) => void,
  signal: AbortSignal,
) => Promise<number>;

/**
 * Streams a `.bz2` file's lines through `bzip2 -dc` ({@link splitLines}), bzip2's own messages on
 * the process's standard error. Where reading or `onLine` fails, or `signal` aborts, the child is
 * killed and waited for before the failure is thrown.
 *
 * @throws Error if `bzip2` cannot be started, exits other than 0, or is stopped by the signal.
 */
export async function streamBzip2Lines(
  path: string,
  onLine: (bytes: Uint8Array, start: number, end: number) => void,
  signal: AbortSignal,
): Promise<number> {
  const child = spawn("bzip2", ["-dc", path], { stdio: ["ignore", "pipe", "inherit"], signal });
  const exited = new Promise<number>((resolve, reject) => {
    child.once("error", (error) => {
      reject(
        signal.aborted
          ? new Error(`bzip2 -dc ${path} stopped: the pass failed elsewhere`, { cause: error })
          : new Error(`bzip2 could not start for ${path} (it must be on the PATH)`, {
              cause: error,
            }),
      );
    });
    child.once("close", (code) => {
      resolve(code ?? -1);
    });
  });
  let lines: number;
  try {
    lines = await splitLines(child.stdout, onLine, path);
  } catch (error: unknown) {
    child.kill();
    // Its exit follows the kill; the failure that stopped the read is the one thrown.
    await exited.catch(() => -1);
    throw error;
  }
  const code = await exited;
  if (code !== 0) {
    throw new Error(`bzip2 -dc ${path} exited ${code}`);
  }
  return lines;
}

/**
 * A stream of byte chunks' lines, each line's bytes given to `onLine` with its start and end (no
 * string is made a line; a line split across chunks is joined alone), empty lines skipped and a
 * last line without its newline kept.
 *
 * @param what - Names the stream in an error.
 * @throws TypeError for a chunk that is not bytes.
 */
export async function splitLines(
  chunks: AsyncIterable<unknown>,
  onLine: (bytes: Uint8Array, start: number, end: number) => void,
  what: string,
): Promise<number> {
  let carry = Buffer.alloc(0);
  let lines = 0;
  for await (const chunk of chunks) {
    const data: unknown = chunk;
    if (!Buffer.isBuffer(data)) {
      throw new TypeError(`the output for ${what} came as other than bytes`);
    }
    let start = 0;
    // A line split across chunks: the carry and this chunk's head, joined alone.
    if (carry.length > 0) {
      const end = data.indexOf(10);
      if (end < 0) {
        carry = Buffer.concat([carry, data]);
        continue;
      }
      const joined = Buffer.concat([carry, data.subarray(0, end)]);
      if (joined.length > 0) {
        onLine(joined, 0, joined.length);
        lines += 1;
      }
      start = end + 1;
    }
    for (let i = data.indexOf(10, start); i >= 0; i = data.indexOf(10, start)) {
      if (i > start) {
        onLine(data, start, i);
        lines += 1;
      }
      start = i + 1;
    }
    carry = Buffer.from(data.subarray(start));
  }
  if (carry.length > 0) {
    onLine(carry, 0, carry.length);
    lines += 1;
  }
  return lines;
}

/** Whether a byte is a space or a tab. */
function isBlank(b: number): boolean {
  return b === 32 || b === 9 || b === 13;
}

/**
 * The whitespace-separated fields of a line as [start, end) byte offsets, into `out` (reused),
 * the count returned.
 */
export function splitFields(
  bytes: Uint8Array,
  start: number,
  end: number,
  out: Int32Array,
): number {
  let count = 0;
  let i = start;
  while (i < end && count < out.length >> 1) {
    while (i < end && isBlank(bytes[i] ?? 32)) {
      i += 1;
    }
    if (i >= end) {
      break;
    }
    const from = i;
    while (i < end && !isBlank(bytes[i] ?? 32)) {
      i += 1;
    }
    out[2 * count] = from;
    out[2 * count + 1] = i;
    count += 1;
  }
  return count;
}

/** Powers of ten exact in f64, 10⁰ to 10²². */
const EXACT_POWERS = Float64Array.from({ length: 23 }, (_, k) => 10 ** k);

/**
 * A decimal number from a field's bytes, as `Number` reads it for the forms the files use
 * (`-12.5`, `1.1858E-06`, `13000.000056`): its digits as an integer, scaled by an exact power of
 * ten, so that the result is the correctly rounded value; past 15 digits or 10^±22, by `Number`
 * itself. `Inf` and `NaN` read as such.
 *
 * @throws Error for bytes that are not a number.
 */
export function parseNumber(bytes: Uint8Array, start: number, end: number): number {
  let i = start;
  let sign = 1;
  if (bytes[i] === 45) {
    sign = -1;
    i += 1;
  } else if (bytes[i] === 43) {
    i += 1;
  }
  if (bytes[i] === 73 || bytes[i] === 105) {
    return sign * Infinity;
  }
  if (bytes[i] === 78 || bytes[i] === 110) {
    return Number.NaN;
  }
  let mantissa = 0;
  let digits = 0;
  let scale = 0;
  let seenPoint = false;
  for (; i < end; i += 1) {
    const b = bytes[i] ?? 0;
    if (b >= 48 && b <= 57) {
      mantissa = mantissa * 10 + (b - 48);
      digits += 1;
      if (seenPoint) {
        scale -= 1;
      }
    } else if (b === 46 && !seenPoint) {
      seenPoint = true;
    } else {
      break;
    }
  }
  if (i < end && (bytes[i] === 69 || bytes[i] === 101 || bytes[i] === 68 || bytes[i] === 100)) {
    i += 1;
    let expSign = 1;
    if (bytes[i] === 45) {
      expSign = -1;
      i += 1;
    } else if (bytes[i] === 43) {
      i += 1;
    }
    let exponent = 0;
    let expDigits = 0;
    for (; i < end; i += 1) {
      const b = bytes[i] ?? 0;
      if (b < 48 || b > 57) {
        break;
      }
      exponent = exponent * 10 + (b - 48);
      expDigits += 1;
    }
    if (expDigits === 0) {
      throw new Error(`${Buffer.from(bytes.subarray(start, end)).toString()} is not a number`);
    }
    scale += expSign * exponent;
  }
  if (digits === 0 || i !== end || digits > 15 || scale > 22 || scale < -22) {
    // Past 15 digits the integer may not be exact, and past 10^±22 the power; `Number` reads it
    // correctly rounded.
    const text = Buffer.from(bytes.subarray(start, end)).toString();
    const value = Number(text);
    if (digits === 0 || i !== end || !Number.isFinite(value)) {
      throw new Error(`${text} is not a number`);
    }
    return value;
  }
  if (scale === 0) {
    return sign * mantissa;
  }
  return scale > 0
    ? sign * mantissa * (EXACT_POWERS[scale] ?? Number.NaN)
    : sign * (mantissa / (EXACT_POWERS[-scale] ?? Number.NaN));
}

/** n values, NaN each: a state with no energy read. */
function nanArray(n: number): Float64Array {
  return new Float64Array(n).fill(Number.NaN);
}

/** n values, 0 each. */
function floats(n: number): Float64Array {
  return new Float64Array(n);
}

/** n whole values, 0 each. */
function ints(n: number): Int32Array {
  return new Int32Array(n);
}

/** A typed array grown to hold at least `length` values, its old values kept. */
function grown<T extends Float64Array | Int32Array>(
  array: T,
  length: number,
  make: (n: number) => T,
): T {
  if (array.length >= length) {
    return array;
  }
  const out = make(Math.max(length, 2 * array.length));
  out.set(array);
  return out;
}

/**
 * A states file's energies, degeneracies, J and the layout's other quanta, by state ID, each ID
 * from 1 up once and in order, as many as the file holds (a `def.json`'s count can be stale:
 * TYM's states 1,759,068 where its file holds 2,078,676).
 *
 * @throws Error for a line short of the layout's fields, or an ID out of order.
 */
export async function readStates(
  path: string,
  source: ExoMolFilesSource,
  readLines: LineReader = streamBzip2Lines,
): Promise<ExoMolStates> {
  const { layout } = source.states;
  let energiesPerCm = nanArray(1 << 20);
  let degeneracies = floats(1 << 20);
  let j = ints(1 << 20);
  let k = layout.k === undefined ? undefined : ints(1 << 20);
  let vibration = layout.vibration === undefined ? undefined : ints(1 << 20);
  let v = layout.v === undefined ? undefined : ints(1 << 20);
  const fields = new Int32Array(64);
  const need = Math.max(
    layout.energy,
    layout.degeneracy,
    layout.j,
    layout.k ?? 0,
    layout.v ?? 0,
    ...(layout.vibration ?? []),
  );
  const at = (n: number, bytes: Uint8Array): number =>
    parseNumber(bytes, fields[2 * (n - 1)] ?? 0, fields[2 * (n - 1) + 1] ?? 0);
  let expected = 1;
  const lines = await readLines(
    path,
    (bytes, start, end) => {
      const n = splitFields(bytes, start, end, fields);
      if (n < need) {
        throw new Error(`${path}: state ${expected} has ${n} fields, under ${need}`);
      }
      const id = at(1, bytes);
      if (id !== expected) {
        throw new Error(`${path}: state ${id} where ${expected} was expected`);
      }
      if (id >= energiesPerCm.length) {
        energiesPerCm = grown(energiesPerCm, id + 1, nanArray);
        degeneracies = grown(degeneracies, id + 1, floats);
        j = grown(j, id + 1, ints);
        k = k === undefined ? undefined : grown(k, id + 1, ints);
        vibration = vibration === undefined ? undefined : grown(vibration, id + 1, ints);
        v = v === undefined ? undefined : grown(v, id + 1, ints);
      }
      energiesPerCm[id] = at(layout.energy, bytes);
      degeneracies[id] = at(layout.degeneracy, bytes);
      j[id] = at(layout.j, bytes);
      if (k !== undefined && layout.k !== undefined) {
        k[id] = at(layout.k, bytes);
      }
      const quanta = layout.vibration;
      if (vibration !== undefined && quanta !== undefined) {
        vibration[id] = packVibration(
          at(quanta[0], bytes),
          at(quanta[1], bytes),
          at(quanta[2], bytes),
          at(quanta[3], bytes),
        );
      }
      if (v !== undefined && layout.v !== undefined) {
        v[id] = at(layout.v, bytes);
      }
      expected += 1;
    },
    new AbortController().signal,
  );
  const end = lines + 1;
  return {
    energiesPerCm: energiesPerCm.slice(0, end),
    degeneracies: degeneracies.slice(0, end),
    j: j.slice(0, end),
    k: k?.slice(0, end),
    vibration: vibration?.slice(0, end),
    v: v?.slice(0, end),
  };
}

/** Q(T) = Σ g e^(−c₂E ÷ T) over the states, summed in ID order. */
export function partitionSum(states: ExoMolStates, temperatureK: number): number {
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  let q = 0;
  for (let i = 1; i < states.energiesPerCm.length; i += 1) {
    const g = states.degeneracies[i] ?? 0;
    if (g > 0) {
      q += g * Math.exp((-c2 * (states.energiesPerCm[i] ?? 0)) / temperatureK);
    }
  }
  return q;
}

/** An ExoMol `.pf` file: temperature and Q a line. */
export function parsePartitionFunction(text: string): ReadonlyMap<number, number> {
  const out = new Map<number, number>();
  for (const line of text.split(/\r?\n/u)) {
    const [t, q] = line.trim().split(/\s+/u).map(Number);
    if (t !== undefined && q !== undefined && Number.isFinite(t) && Number.isFinite(q)) {
      out.set(t, q);
    }
  }
  return out;
}

/**
 * Q(T) from a `.pf` table: at a tabulated temperature its value, and between two, linear in T
 * (ExoMol tabulates Q every 1 K, between whose rows linear interpolation errs by under 10⁻⁵ of Q
 * above 100 K, the science check's figure).
 *
 * @throws RangeError for a temperature outside the table.
 */
export function partitionSumFromPf(pf: ReadonlyMap<number, number>, temperatureK: number): number {
  const exact = pf.get(temperatureK);
  if (exact !== undefined) {
    return exact;
  }
  let below: readonly [number, number] | undefined;
  let above: readonly [number, number] | undefined;
  for (const [t, q] of pf) {
    if (t < temperatureK && (below === undefined || t > below[0])) {
      below = [t, q];
    } else if (t > temperatureK && (above === undefined || t < above[0])) {
      above = [t, q];
    }
  }
  if (below === undefined || above === undefined) {
    throw new RangeError(`${temperatureK} K lies outside the partition function's table`);
  }
  return below[1] + ((above[1] - below[1]) * (temperatureK - below[0])) / (above[0] - below[0]);
}

/**
 * Q(T) as a list takes it ({@link PartitionSource}): the states' sum, or its `.pf`'s, the file read
 * from the data directory, checked by the caller.
 *
 * @param states - The list's states, or none for a list read without them (a REPACK file's).
 * @throws Error for a sum over states where none are given.
 */
export function partitionSumsOf(
  source: ExoMolSource,
  states: ExoMolStates | undefined,
  dataDir: string,
): (temperatureK: number) => number {
  let q: (temperatureK: number) => number;
  switch (source.partition.from) {
    case "states":
      if (states === undefined) {
        throw new Error(`${source.species}'s Q(T) is its states' sum, but no states were read`);
      }
      q = (t) => partitionSum(states, t);
      break;
    case "pf": {
      const pf = parsePartitionFunction(
        readFileSync(join(dataDir, source.partition.file.name), "utf8"),
      );
      q = (t) => partitionSumFromPf(pf, t);
      break;
    }
  }
  return q;
}

/** One `.broad` code's widths by its quanta, cm⁻¹ bar⁻¹ at 296 K, with their n. */
export interface BroadeningTable {
  readonly gas: "H2" | "He";
  /** `a0`: by J″. */
  readonly byJ: ReadonlyMap<number, readonly [number, number]>;
  /** `m0`: by |m|, m = −J″ in the P branch and J″ + 1 in the R branch. */
  readonly byM: ReadonlyMap<number, readonly [number, number]>;
  /** `m1`: by |m| and K″, keyed |m| × 1,000 + K″. */
  readonly byMK: ReadonlyMap<number, readonly [number, number]>;
  readonly defaultHalfWidthPerCmBar: number;
  readonly defaultExponent: number;
}

/**
 * A `.broad` file's codes a0 (J″), m0 (|m|) and m1 (|m|, K″), each row `code γ n` and its quanta
 * (Barton et al. 2017, Table 1), with the gas's default from the list's `def.json`.
 *
 * @throws Error for a row of a code it does not read, or one short of its quanta.
 */
export function parseBroadening(
  text: string,
  file: BroadeningFile,
  definition: ExoMolDefinition,
): BroadeningTable {
  const defaults = definition.broadeners[file.gas];
  if (defaults === undefined) {
    throw new Error(`${file.name}'s gas ${file.gas} has no default in the list's def.json`);
  }
  const byJ = new Map<number, readonly [number, number]>();
  const byM = new Map<number, readonly [number, number]>();
  const byMK = new Map<number, readonly [number, number]>();
  for (const [i, line] of text.split(/\r?\n/u).entries()) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length < 3) {
      continue;
    }
    const [code, gamma, n, q1, q2] = [fields[0], ...fields.slice(1).map(Number)];
    if (gamma === undefined || n === undefined || !Number.isFinite(gamma + n)) {
      throw new Error(`${file.name} row ${i + 1} gives no width and exponent`);
    }
    const entry = [gamma, n] as const;
    if (code === "a0" && q1 !== undefined) {
      byJ.set(q1, entry);
    } else if (code === "m0" && q1 !== undefined) {
      byM.set(q1, entry);
    } else if (code === "m1" && q1 !== undefined && q2 !== undefined) {
      byMK.set(q1 * 1_000 + q2, entry);
    } else {
      throw new Error(`${file.name} row ${i + 1} is of a code R08.T4.g does not read: ${line}`);
    }
  }
  return {
    gas: file.gas,
    byJ,
    byM,
    byMK,
    defaultHalfWidthPerCmBar: defaults.halfWidthPerCmBar,
    defaultExponent: defaults.exponent,
  };
}

/**
 * A line's width in a `.broad` gas, cm⁻¹ bar⁻¹ at 296 K, and its n: by |m| and K″ (m1) where the
 * file gives them, else by |m| (m0), else by J″ (a0), else the `def.json`'s default. m is −J″ in the
 * P branch, J″ + 1 in the R branch and J″ in the Q branch, as HITRAN takes it (J. Tennyson et al.,
 * J. Quant. Spectrosc. Radiat. Transf. 326 (2024) 109083, Table 13 and its text). The codes are
 * read from the file itself: C2018's `def.json` lists codes (a4, a1) its `.broad` files do not hold.
 */
export function broadWidth(
  table: BroadeningTable,
  upperJ: number,
  lowerJ: number,
  lowerK: number,
): readonly [number, number] {
  const m = upperJ === lowerJ + 1 ? lowerJ + 1 : lowerJ;
  return (
    table.byMK.get(m * 1_000 + Math.abs(lowerK)) ??
    table.byM.get(m) ??
    table.byJ.get(lowerJ) ?? [table.defaultHalfWidthPerCmBar, table.defaultExponent]
  );
}

/** What one streaming pass gathers besides the bins. */
export interface PassRecord {
  /** Lines read, and those inside the binned range. */
  readonly linesRead: number;
  readonly linesBinned: number;
  /** Q(T) summed from the states at each temperature, and at 296 K. */
  readonly partitionSums: ReadonlyArray<readonly [number, number]>;
  readonly partitionSum296: number;
  /**
   * Σ S(296) γ_j and Σ S(296) γ_j n_j over every binned line for each `.broad` gas (cm⁻¹ bar⁻¹),
   * with Σ S(296) for the default air width's ratio.
   */
  readonly broadening: ReadonlyArray<{
    readonly gas: string;
    readonly intensityHalfWidthPerBar: number;
    readonly intensityHalfWidthExponentPerBar: number;
  }>;
  /** Σ S(296) over every binned line, cm a molecule. */
  readonly intensity296Cm: number;
  /**
   * Each 1 nm cell's mean σ by direct sum at each temperature and at 296 K, m² a molecule: each
   * line in the cell holding its centre, ∫σ dλ = S ÷ ν² (`exomol/bounds.json`'s convention),
   * `[t][c]` for the cells 380–760 nm, the last row 296 K's.
   */
  readonly cellMeansM2: ReadonlyArray<Float64Array>;
  /** The lines binned apart's share of the 296 K row, by the same direct sum, m² a molecule. */
  readonly cellMeansApart296M2: Float64Array;
  /**
   * Per band key over the range binned apart: Σ S(296), Σ S(296) ν, the lines, and the lowest and
   * highest line's ν (cm⁻¹).
   */
  readonly bands: ReadonlyMap<
    number,
    {
      intensityCm: number;
      intensityWavenumber: number;
      lines: number;
      fromPerCm: number;
      toPerCm: number;
    }
  >;
  /** Each surveyed window's bands ({@link PassOptions.survey}), in the windows' order. */
  readonly surveyed: ReadonlyArray<ReadonlyMap<number, SurveyedBand>>;
  /** The wall time of the pass, s. */
  readonly seconds: number;
}

/** The cells' wavelengths, nm: 380 to 760. */
const CELLS_NM: readonly [number, number] = [380, 760];

/** The 1 nm cell holding a vacuum wavenumber, as an index from 380 nm, or −1 outside 380–760 nm. */
function cellIndexOf(wavenumberPerCm: number): number {
  const nm = Math.round(1e7 / wavenumberPerCm);
  return nm >= CELLS_NM[0] && nm <= CELLS_NM[1] ? nm - CELLS_NM[0] : -1;
}

/**
 * The bands a pass bins apart, by key, over a range of wavenumbers, cm⁻¹ (N₂O's matched bands,
 * R08.T4.g); every band's 296 K strength there is gathered too.
 */
export interface ApartBands {
  readonly bands: ReadonlySet<number>;
  readonly rangePerCm: readonly [number, number];
}

/**
 * Where a pass bins its lines: into bins of its own, with a set apart where bands are binned apart;
 * or into shared bins, one a temperature in the pass's order, which several isotopologues fill in
 * turn (R08.T4.f's TiO), and on which the caller applies stimulated emission once every list is in
 * ({@link applyStimulatedEmission}).
 */
export type PassTarget =
  | { readonly kind: "own"; readonly apart: ApartBands | undefined }
  | { readonly kind: "shared"; readonly into: ReadonlyArray<SuperLineBins> };

/** Options of a streaming pass. */
export interface PassOptions {
  readonly temperaturesK: ReadonlyArray<number>;
  /** Where the lines are binned. */
  readonly target: PassTarget;
  /** The `.broad` tables whose sums are gathered. */
  readonly broadening: ReadonlyArray<BroadeningTable>;
  /** How many `bzip2` processes run at once. */
  readonly jobs: number;
  /**
   * Wavenumber windows, within the range binned apart, whose lines are surveyed by band at 296 K,
   * each above its own least intensity (cm a molecule): HITRAN2024's band windows, to match TYM's
   * bands to them (R08.T4.g).
   */
  readonly survey?: ReadonlyArray<{
    readonly fromPerCm: number;
    readonly toPerCm: number;
    readonly leastIntensityCm: number;
  }>;
  /** Only the transition files overlapping these wavenumbers are read, where given (a survey's). */
  readonly filesPerCm?: readonly [number, number];
}

/** A band's lines in a surveyed window: Σ S(296), Σ S(296) ν and the count. */
export interface SurveyedBand {
  /** Σ S(296), cm a molecule. */
  intensityCm: number;
  /** Σ S(296) ν, cm × cm⁻¹. */
  intensityWavenumber: number;
  lines: number;
}

/** A pass's bins: each temperature's main set, and its set apart where bands are binned apart. */
export interface PassBins {
  readonly main: ReadonlyArray<SuperLineBins>;
  readonly apart: ReadonlyArray<SuperLineBins> | undefined;
}

/**
 * Reads a list's transition files that overlap its binned range (or `options.filesPerCm`), each
 * line given to `onLine`: up to `jobs` decompressions at once, their lines parsed in turn. The first
 * failure empties the queue and stops the others' reads, and is thrown once every reader has
 * stopped.
 *
 * @throws RangeError for a number of jobs that is not a whole number from 1.
 */
async function readTransitionFiles(
  source: ExoMolFilesSource,
  dataDir: string,
  options: PassOptions,
  readLines: LineReader,
  onLine: (bytes: Uint8Array, start: number, end: number) => void,
): Promise<void> {
  const [filesLow, filesHigh] = options.filesPerCm ?? source.binRangePerCm;
  const files = source.transitions.filter((f) => f.toPerCm > filesLow && f.fromPerCm < filesHigh);
  if (!(Number.isInteger(options.jobs) && options.jobs >= 1)) {
    throw new RangeError(`a pass takes a whole number of jobs from 1, not ${options.jobs}`);
  }
  const queue = [...files];
  const stop = new AbortController();
  let failure: unknown;
  const workers = Array.from({ length: options.jobs }, async () => {
    for (let file = queue.shift(); file !== undefined; file = queue.shift()) {
      const begun = performance.now();
      try {
        // Each worker takes its files one after another, so that `jobs` run at once and no more.
        // oxlint-disable-next-line eslint/no-await-in-loop
        const read = await readLines(join(dataDir, file.name), onLine, stop.signal);
        process.stdout.write(
          `exomol-bins: ${file.name} ${read} lines, ${((performance.now() - begun) / 1000).toFixed(0)} s\n`,
        );
      } catch (error: unknown) {
        failure ??= error;
        queue.length = 0;
        stop.abort();
      }
    }
  });
  await Promise.all(workers);
  if (failure !== undefined) {
    throw failure instanceof Error
      ? failure
      : new Error("a pass's reader failed", { cause: failure });
  }
}

/** A REPACK record's bytes: three f64 and an i32 ({@link RepackFile}). */
export const REPACK_RECORD_BYTES = 28;

/** The records a REPACK file is read by at once: 28 MiB a read. */
const REPACK_CHUNK_RECORDS = 1 << 20;

/**
 * A REPACK file's records ({@link RepackFile}), each given to `onRecord` in the file's order as its
 * ν (cm⁻¹), E″ (cm⁻¹), g″f and isotope tag; the file read in chunks, never held whole.
 *
 * @returns The records read.
 * @throws Error for a file whose size is not a whole number of records.
 */
export function readRepackLines(
  path: string,
  onRecord: (
    wavenumberPerCm: number,
    lowerEnergyPerCm: number,
    gf: number,
    isotope: number,
  ) => void,
): number {
  const bytes = statSync(path).size;
  if (bytes % REPACK_RECORD_BYTES !== 0) {
    throw new Error(
      `${path} is ${bytes} bytes, not a whole number of ${REPACK_RECORD_BYTES}-byte records`,
    );
  }
  const buffer = Buffer.allocUnsafe(REPACK_CHUNK_RECORDS * REPACK_RECORD_BYTES);
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  const fd = openSync(path, "r");
  let records = 0;
  let position = 0;
  try {
    while (position < bytes) {
      const read = readSync(fd, buffer, 0, Math.min(buffer.length, bytes - position), position);
      if (read === 0 || read % REPACK_RECORD_BYTES !== 0) {
        throw new Error(`${path}: a read at ${position} gave ${read} bytes, not whole records`);
      }
      for (let at = 0; at < read; at += REPACK_RECORD_BYTES) {
        onRecord(
          view.getFloat64(at, true),
          view.getFloat64(at + 8, true),
          view.getFloat64(at + 16, true),
          view.getInt32(at + 24, true),
        );
      }
      records += read / REPACK_RECORD_BYTES;
      position += read;
    }
  } finally {
    closeSync(fd);
  }
  return records;
}

/**
 * A REPACK line's strength before the Boltzmann factor, Q(T) and stimulated emission, cm a
 * molecule: π r_e g″f, which is g_u A_ul ÷ (8π c ν²), ExoMol's, since `repack` writes
 * g″f = g_u A_ul ÷ (8π² c ν² r_e) (`repack/utils/utilities.py`: g A C1 ÷ (8π c ν²), with
 * `repack/constants/constants.py`'s C1 = 4ε₀ mₑ c² ÷ e² = 1 ÷ (π r_e), cm⁻¹; and `pack.py`'s own
 * S = C3 g″f r_iso e^(−c₂E″ ÷ T) (1 − e^(−c₂ν ÷ T)) ÷ Q, C3 = πe² ÷ (mₑc²) = π r_e, its isotope ratio
 * r_iso applied there, not stored). r_e is CODATA 2018's ({@link ELECTRON_RADIUS_CM}).
 */
export function repackStrengthCm(gf: number): number {
  return Math.PI * ELECTRON_RADIUS_CM * gf;
}

/**
 * One streaming pass over a list's lines: each line within the binned range binned at every
 * temperature asked for (`superLines.ts`), its band's lines binned apart where asked, and the
 * pass's records ({@link PassRecord}). An ExoMol list's transition files are read against its
 * states, `jobs` files decompressed at once; a REPACK file's records are read in turn
 * ({@link readRepackLines}), each its own ν, E″ and g″f ({@link repackStrengthCm}). Q(T) is its
 * source's ({@link PartitionSource}): the states' sum ({@link partitionSum}), or the list's `.pf`,
 * read from `dataDir` ({@link partitionSumFromPf}). A transition past its source's cut on v′ − v″
 * ({@link ExoMolFilesSource.maxDeltaV}) is read and not binned.
 *
 * @param states - The list's states, or none for a REPACK file's lines.
 * @param dataDir - The directory holding the source's fetched files, each checked by the caller.
 * @throws Error for a transition whose state is unknown or has no energy, an ExoMol list given no
 *   states, or a REPACK record of another isotopologue.
 * @throws RangeError for shared bins not one a temperature.
 */
export async function streamPass(
  source: ExoMolSource,
  definition: ExoMolDefinition,
  states: ExoMolStates | undefined,
  dataDir: string,
  options: PassOptions,
  readLines: LineReader = streamBzip2Lines,
): Promise<{ readonly bins: PassBins; readonly record: PassRecord }> {
  const started = performance.now();
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  const cCmPerS = SPEED_OF_LIGHT_M_PER_S * 100;
  const [low, high] = source.binRangePerCm;
  const temps = options.temperaturesK;
  const mass = definition.molecularMassDa;
  const { target } = options;
  let main: ReadonlyArray<SuperLineBins>;
  let apartBands: ReadonlySet<number> | undefined;
  let apartRange: readonly [number, number] | undefined;
  switch (target.kind) {
    case "own":
      main = temps.map((t) => emptyBins(t, low, high, binStepPerCm(t, low, mass)));
      apartBands = target.apart?.bands;
      apartRange = target.apart?.rangePerCm;
      break;
    case "shared":
      if (target.into.length !== temps.length) {
        throw new RangeError("shared bins take one set a temperature");
      }
      main = target.into;
      break;
  }
  // The set apart on the main set's grid, from its origin to the top of its own range, so that
  // their nodes coincide.
  const apart =
    apartRange === undefined
      ? undefined
      : temps.map((t) => emptyBins(t, low, apartRange[1], binStepPerCm(t, low, mass)));
  const qOf = partitionSumsOf(source, states, dataDir);
  const partitionSums = temps.map((t) => [t, qOf(t)] as const);
  const q296 = qOf(296);
  const factors = Float64Array.from(partitionSums, ([, q]) => source.abundance / q);
  const inverseT = Float64Array.from(temps, (t) => -c2 / t);
  const factor296 = source.abundance / q296;
  const cellMeans = [...temps, 296].map(() => new Float64Array(CELLS_NM[1] - CELLS_NM[0] + 1));
  const cellMeansApart296 = new Float64Array(CELLS_NM[1] - CELLS_NM[0] + 1);
  const broadening = options.broadening.map((table) => ({
    table,
    intensityHalfWidthPerBar: 0,
    intensityHalfWidthExponentPerBar: 0,
  }));
  let intensity296Cm = 0;
  const bands = new Map<
    number,
    {
      intensityCm: number;
      intensityWavenumber: number;
      lines: number;
      fromPerCm: number;
      toPerCm: number;
    }
  >();
  const survey = options.survey ?? [];
  const surveyed = survey.map(() => new Map<number, SurveyedBand>());
  let linesRead = 0;
  let linesBinned = 0;
  const scale = source.intensityScale?.factor ?? 1;
  const cutDeltaV = source.kind === "exomol" ? source.maxDeltaV : undefined;
  const vOf = cutDeltaV === undefined ? undefined : states?.v;
  const maxDeltaV = cutDeltaV ?? Infinity;
  if (cutDeltaV !== undefined && vOf === undefined) {
    throw new Error(`${source.species}'s cut on v' - v'' needs its states' v`);
  }
  // The hot loop's arrays: each set's values, and each temperature's reciprocal step (both sets
  // share their grid's origin and step).
  const nT = temps.length;
  const mainValues = main.map((b) => b.values);
  const apartValues = apart?.map((b) => b.values);
  const inverseSteps = Float64Array.from(main, (b) => 1 / b.stepPerCm);
  const emptyValues = new Float64Array(0);
  // A line's binning at every temperature and its records, from its ν (within [low, high]), E″ and
  // strength before the Boltzmann factor and Q, cm a molecule; its states' IDs where it has them.
  const deposit = (
    nu: number,
    eLower: number,
    strength: number,
    upper: number,
    lower: number,
  ): void => {
    linesBinned += 1;
    let band = -1;
    if (states?.vibration !== undefined && apartRange !== undefined) {
      if (nu >= apartRange[0] && nu <= apartRange[1]) {
        band = bandKey(states.vibration[upper] ?? 0, states.vibration[lower] ?? 0);
      }
    }
    const into =
      band >= 0 && apartBands?.has(band) === true && apartValues !== undefined
        ? apartValues
        : mainValues;
    const cell = cellIndexOf(nu);
    // ∫σ dλ = S ÷ ν² for a line wholly in its cell, over the cell's 10⁻⁷ cm, in m².
    const perCell = 1e-4 / (nu * nu * 1e-7);
    // Each temperature's deposit, `depositLine`'s, inlined: ν lies within [low, high], so both
    // nodes lie within the bins.
    const offset = nu - low;
    for (let t = 0; t < nT; t += 1) {
      const s = strength * Math.exp((inverseT[t] ?? 0) * eLower) * (factors[t] ?? 0);
      const values = into[t] ?? emptyValues;
      const x = offset * (inverseSteps[t] ?? 0);
      const k = Math.floor(x);
      const f = x - k;
      values[k] = (values[k] ?? 0) + s * (1 - f);
      values[k + 1] = (values[k + 1] ?? 0) + s * f;
      if (cell >= 0) {
        const row = cellMeans[t] ?? emptyValues;
        row[cell] = (row[cell] ?? 0) + s * perCell;
      }
    }
    // 296 K's strength without stimulated emission's factor, under 10⁻²⁰ from 1 here.
    const s296 = strength * Math.exp((-c2 * eLower) / 296) * factor296;
    intensity296Cm += s296;
    const last = cellMeans[temps.length];
    if (cell >= 0 && last !== undefined) {
      last[cell] = (last[cell] ?? 0) + s296 * perCell;
      if (into === apartValues) {
        cellMeansApart296[cell] = (cellMeansApart296[cell] ?? 0) + s296 * perCell;
      }
    }
    if (band >= 0) {
      const b = bands.get(band) ?? {
        intensityCm: 0,
        intensityWavenumber: 0,
        lines: 0,
        fromPerCm: Infinity,
        toPerCm: -Infinity,
      };
      b.intensityCm += s296;
      b.intensityWavenumber += s296 * nu;
      b.lines += 1;
      b.fromPerCm = Math.min(b.fromPerCm, nu);
      b.toPerCm = Math.max(b.toPerCm, nu);
      bands.set(band, b);
      for (const [w, window] of survey.entries()) {
        if (nu >= window.fromPerCm && nu <= window.toPerCm && s296 >= window.leastIntensityCm) {
          const found = surveyed[w];
          const entry = found?.get(band) ?? { intensityCm: 0, intensityWavenumber: 0, lines: 0 };
          entry.intensityCm += s296;
          entry.intensityWavenumber += s296 * nu;
          entry.lines += 1;
          found?.set(band, entry);
        }
      }
    }
    if (broadening.length > 0) {
      const uJ = states?.j[upper] ?? 0;
      const lJ = states?.j[lower] ?? 0;
      const lK = states?.k?.[lower] ?? 0;
      for (const sums of broadening) {
        const [gamma, exponent] = broadWidth(sums.table, uJ, lJ, lK);
        sums.intensityHalfWidthPerBar += s296 * gamma;
        sums.intensityHalfWidthExponentPerBar += s296 * gamma * exponent;
      }
    }
  };
  const energiesPerCm = states?.energiesPerCm ?? emptyValues;
  const degeneracies = states?.degeneracies ?? emptyValues;
  const fields = new Int32Array(16);
  // An ExoMol transition: upper and lower state IDs, A, and ν where the file gives it.
  const onLine = (bytes: Uint8Array, start: number, end: number): void => {
    linesRead += 1;
    const n = splitFields(bytes, start, end, fields);
    const upper = parseNumber(bytes, fields[0] ?? 0, fields[1] ?? 0);
    const lower = parseNumber(bytes, fields[2] ?? 0, fields[3] ?? 0);
    const eUpper = energiesPerCm[upper] ?? Number.NaN;
    const eLower = energiesPerCm[lower] ?? Number.NaN;
    const nu = n >= 4 ? parseNumber(bytes, fields[6] ?? 0, fields[7] ?? 0) : eUpper - eLower;
    if (!(nu >= low && nu <= high)) {
      return;
    }
    if (!Number.isFinite(eUpper) || !Number.isFinite(eLower)) {
      throw new Error(`a transition ${upper} - ${lower} names a state the states file lacks`);
    }
    if (vOf !== undefined && (vOf[upper] ?? 0) - (vOf[lower] ?? 0) > maxDeltaV) {
      return;
    }
    const a = parseNumber(bytes, fields[4] ?? 0, fields[5] ?? 0);
    const strength = (scale * (degeneracies[upper] ?? 0) * a) / (8 * Math.PI * cCmPerS * nu * nu);
    if (strength > 0) {
      deposit(nu, eLower, strength, upper, lower);
    }
  };
  switch (source.kind) {
    case "exomol":
      if (states === undefined) {
        throw new Error(`${source.species}'s transitions are read against its states: none given`);
      }
      await readTransitionFiles(source, dataDir, options, readLines, onLine);
      break;
    case "repack": {
      const { name, isotope } = source.lines;
      readRepackLines(join(dataDir, name), (nu, eLower, gf, tag) => {
        linesRead += 1;
        if (!(nu >= low && nu <= high)) {
          return;
        }
        if (tag !== isotope) {
          throw new Error(`${name}'s line at ${nu} cm-1 is of isotope ${tag}, not ${isotope}`);
        }
        const strength = scale * repackStrengthCm(gf);
        if (strength > 0) {
          deposit(nu, eLower, strength, -1, -1);
        }
      });
      break;
    }
  }
  // Shared bins take stimulated emission from their caller, once every list is in them.
  for (const bins of target.kind === "own" ? [...main, ...(apart ?? [])] : []) {
    applyStimulatedEmission(bins);
  }
  for (const [t, temperatureK] of temps.entries()) {
    const row = cellMeans[t] ?? new Float64Array(0);
    for (let c = 0; c < row.length; c += 1) {
      const nu = 1e7 / (CELLS_NM[0] + c);
      row[c] = (row[c] ?? 0) * -Math.expm1((-c2 * nu) / temperatureK);
    }
  }
  return {
    bins: { main, apart },
    record: {
      linesRead,
      linesBinned,
      partitionSums,
      partitionSum296: q296,
      broadening: broadening.map(
        ({ table, intensityHalfWidthPerBar, intensityHalfWidthExponentPerBar }) => ({
          gas: table.gas,
          intensityHalfWidthPerBar,
          intensityHalfWidthExponentPerBar,
        }),
      ),
      intensity296Cm,
      cellMeansM2: cellMeans,
      cellMeansApart296M2: cellMeansApart296,
      bands,
      surveyed,
      seconds: (performance.now() - started) / 1000,
    },
  };
}

/** A bins cache file's magic, `HYSL`, and its format's version. */
const BINS_MAGIC = "HYSL";
const BINS_VERSION = 1;

/** A bins cache file's header bytes: the magic, the version (u32), T, origin, step (f64), count (f64). */
const BINS_HEADER_BYTES = 40;

/** A temperature's bins file in a cache: `<key>-<set>-<T>K.bins`. */
export function binsFileName(species: string, set: "main" | "apart", temperatureK: number): string {
  return `${species}-${set}-${temperatureK}K.bins`;
}

/** Writes a temperature's bins to a cache file, little-endian. */
export function writeBins(path: string, bins: SuperLineBins): void {
  const bytes = new Uint8Array(BINS_HEADER_BYTES + 8 * bins.values.length);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < BINS_MAGIC.length; i += 1) {
    view.setUint8(i, BINS_MAGIC.charCodeAt(i));
  }
  view.setUint32(4, BINS_VERSION, true);
  view.setFloat64(8, bins.temperatureK, true);
  view.setFloat64(16, bins.originPerCm, true);
  view.setFloat64(24, bins.stepPerCm, true);
  view.setFloat64(32, bins.values.length, true);
  for (let i = 0; i < bins.values.length; i += 1) {
    view.setFloat64(BINS_HEADER_BYTES + 8 * i, bins.values[i] ?? 0, true);
  }
  writeFileSync(path, bytes);
}

/** A cached temperature's bins, read a window at a time. */
export interface BinsFile {
  readonly temperatureK: number;
  readonly originPerCm: number;
  readonly stepPerCm: number;
  readonly count: number;
  /** The nodes first to last, inclusive; 0 beyond the file. */
  readonly read: (first: number, last: number) => BinsWindow;
  /** Closes the file; no read follows. */
  readonly close: () => void;
}

/**
 * Opens a cache file's bins for windowed reads.
 *
 * @throws Error for a file that is not a bins file of its version.
 */
export function openBins(path: string): BinsFile {
  const fd = openSync(path, "r");
  const header = Buffer.alloc(BINS_HEADER_BYTES);
  try {
    readSync(fd, header, 0, BINS_HEADER_BYTES, 0);
  } catch (error: unknown) {
    closeSync(fd);
    throw error;
  }
  if (header.toString("latin1", 0, 4) !== BINS_MAGIC || header.readUInt32LE(4) !== BINS_VERSION) {
    closeSync(fd);
    throw new Error(`${path} is not a bins file of version ${BINS_VERSION}`);
  }
  const temperatureK = header.readDoubleLE(8);
  const originPerCm = header.readDoubleLE(16);
  const stepPerCm = header.readDoubleLE(24);
  const count = header.readDoubleLE(32);
  const read = (first: number, last: number): BinsWindow => {
    const values = new Float64Array(last - first + 1);
    const from = Math.max(first, 0);
    const to = Math.min(last, count - 1);
    if (to >= from) {
      const buffer = Buffer.alloc(8 * (to - from + 1));
      readSync(fd, buffer, 0, buffer.length, BINS_HEADER_BYTES + 8 * from);
      for (let i = 0; i < to - from + 1; i += 1) {
        values[from - first + i] = buffer.readDoubleLE(8 * i);
      }
    }
    return { originPerCm, stepPerCm, first, values };
  };
  return {
    temperatureK,
    originPerCm,
    stepPerCm,
    count,
    read,
    close: () => {
      closeSync(fd);
    },
  };
}

/** Which of a species' cached bins a super-line source reads: its main set, or both sets. */
export type BinsSets = "main" | "all";

/** A super-line species read from a cache, with its files to close once the caller is done. */
export interface CachedSpecies extends SuperLineSpecies {
  /** Closes every bins file opened; no read follows. */
  readonly close: () => void;
}

/**
 * A super-line species read from a cache's bins (`superLines.ts`'s {@link SuperLineSpecies}): the
 * main set, or the main set with the set apart added node by node (both are on one grid where the
 * set apart's range lies within the main's, as {@link streamPass} lays them). Each temperature's
 * files open on first need and stay open until `close`.
 *
 * @throws Error when a temperature's file is missing from the cache, naming it.
 */
export function cachedSpecies(
  source: ExoMolSource,
  definition: ExoMolDefinition,
  cacheDir: string,
  sets: BinsSets,
): CachedSpecies {
  const opened = new Map<
    number,
    { readonly main: BinsFile; readonly apart: BinsFile | undefined }
  >();
  const files = (
    temperatureK: number,
  ): { readonly main: BinsFile; readonly apart: BinsFile | undefined } => {
    const found = opened.get(temperatureK);
    if (found !== undefined) {
      return found;
    }
    const main = openBins(join(cacheDir, binsFileName(source.species, "main", temperatureK)));
    const apartPath = join(cacheDir, binsFileName(source.species, "apart", temperatureK));
    let apart: BinsFile | undefined;
    if (sets === "all") {
      try {
        apart = openBins(apartPath);
      } catch (error: unknown) {
        // A list binned whole has no set apart.
        if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
          throw error;
        }
      }
    }
    const made = { main, apart };
    opened.set(temperatureK, made);
    return made;
  };
  return {
    gridAt: (temperatureK) => {
      const { main } = files(temperatureK);
      return { originPerCm: main.originPerCm, stepPerCm: main.stepPerCm, count: main.count };
    },
    binsAt: (temperatureK, first, last) => {
      const { main, apart } = files(temperatureK);
      const window = main.read(first, last);
      if (apart !== undefined) {
        const offset = Math.round((apart.originPerCm - main.originPerCm) / main.stepPerCm);
        const extra = apart.read(first - offset, last - offset);
        for (let i = 0; i < window.values.length; i += 1) {
          window.values[i] = (window.values[i] ?? 0) + (extra.values[i] ?? 0);
        }
      }
      return window;
    },
    molarMassGPerMol: definition.molecularMassDa,
    airHalfWidthPerCmAtm: definition.airHalfWidthPerCmBar / ATM_PER_BAR,
    airExponent: definition.airExponent,
    widthsEstimated: true,
    close: () => {
      for (const { main, apart } of opened.values()) {
        main.close();
        apart?.close();
      }
      opened.clear();
    },
  };
}

/** The text of a checked small file. */
export function readCheckedSmall(path: string, file: ExoMolFile): string {
  checkFile(path, file);
  return readFileSync(path, "utf8");
}
