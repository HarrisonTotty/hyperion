import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, describe, expect, it } from "vitest";

import {
  ATM_PER_BAR,
  bandKey,
  bandLabel,
  type BroadeningFile,
  broadWidth,
  cachedSpecies,
  binsFileName,
  type ExoMolDefinition,
  type ExoMolFilesSource,
  type LineReader,
  openBins,
  packVibration,
  parseBroadening,
  parseDefinition,
  parseDefinitionText,
  parseNumber,
  partitionSum,
  partitionSumFromPf,
  partitionSumsOf,
  readRepackLines,
  readStates,
  REPACK_RECORD_BYTES,
  type RepackSource,
  repackStrengthCm,
  splitFields,
  splitLines,
  streamPass,
  vibrationLabel,
  writeBins,
} from "./exomolLines";
import { ELECTRON_RADIUS_CM } from "./atomicLines";
import { SECOND_RADIATION_CONSTANT_CM_K, SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";
import { applyStimulatedEmission, emptyBins, type SuperLineBins } from "./superLines";

/** A field's bytes, for the parser. */
function bytesOf(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

describe("reading a number from a field's bytes", () => {
  it.each([
    "1.1858E-06",
    "13000.000056",
    "-12.5",
    "0",
    "0.000000",
    "1e-30",
    "2.6549E-07",
    "932.433472",
    "+7",
    "1.3962E+00",
    "4.6084E-03",
  ])("reads %s as Number does", (text) => {
    const bytes = bytesOf(text);
    expect(parseNumber(bytes, 0, bytes.length)).toBe(Number(text));
  });

  it("reads Inf as infinity", () => {
    const inf = bytesOf("Inf");
    expect(parseNumber(inf, 0, inf.length)).toBe(Infinity);
  });

  it("refuses what is not a number", () => {
    const bad = bytesOf("A'");
    expect(() => parseNumber(bad, 0, bad.length)).toThrow("not a number");
  });

  it("reads a thousand random wavenumbers and intensities as Number does", () => {
    let state = 12_345;
    const next = (): number => {
      state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
      return state / 2 ** 32;
    };
    const texts = Array.from({ length: 1_000 }, (_, i) =>
      i % 2 === 0
        ? (13_000 + 7_000 * next()).toFixed(6)
        : (next() * 10).toExponential(4).toUpperCase(),
    );
    const misread = texts.filter((t) => {
      const b = bytesOf(t);
      return parseNumber(b, 0, b.length) !== Number(t);
    });
    expect(misread).toEqual([]);
  });

  it("splits a line into its whitespace-separated fields", () => {
    const line = bytesOf("      820318       886319 1.1858E-06    13000.000056");
    const fields = new Int32Array(16);
    const n = splitFields(line, 0, line.length, fields);
    const text = Array.from({ length: n }, (_, i) =>
      new TextDecoder().decode(line.subarray(fields[2 * i] ?? 0, fields[2 * i + 1] ?? 0)),
    );
    expect(text).toEqual(["820318", "886319", "1.1858E-06", "13000.000056"]);
  });
});

/** Chunks of bytes, as a stream gives them. */
async function* chunksOf(parts: ReadonlyArray<string>): AsyncGenerator<Buffer> {
  for (const part of parts) {
    yield Buffer.from(part);
  }
}

/** Chunks of text, which a stream of bytes never gives. */
async function* textChunks(): AsyncGenerator<string> {
  yield "a\n";
}

/** The lines a stream of chunks splits into, as text. */
async function linesOf(parts: ReadonlyArray<string>): Promise<string[]> {
  const out: string[] = [];
  await splitLines(
    chunksOf(parts),
    (bytes, start, end) => {
      out.push(Buffer.from(bytes.subarray(start, end)).toString());
    },
    "a test",
  );
  return out;
}

describe("splitting a stream into lines", () => {
  it("joins a line split across chunks", async () => {
    await expect(linesOf(["1 2 3\n4 5", " 6\n"])).resolves.toEqual(["1 2 3", "4 5 6"]);
  });

  it("joins a line across a chunk with no newline in it", async () => {
    await expect(linesOf(["12", "34", "56\n7\n"])).resolves.toEqual(["123456", "7"]);
  });

  it("keeps a last line without its newline, and skips empty lines", async () => {
    await expect(linesOf(["a\n\nb"])).resolves.toEqual(["a", "b"]);
  });

  it("refuses a chunk that is not bytes", async () => {
    await expect(splitLines(textChunks(), () => undefined, "a test")).rejects.toThrow(
      "other than bytes",
    );
  });
});

describe("vibrational labels", () => {
  it("pack v1 v2 l2 v3 and write them as HITRAN does", () => {
    const upper = packVibration(1, 0, 0, 6);
    const lower = packVibration(0, 0, 0, 0);
    expect([vibrationLabel(upper), bandLabel(bandKey(upper, lower))]).toEqual([
      "1 0 0 6",
      "1 0 0 6 - 0 0 0 0",
    ]);
  });
});

/** A toy `.broad` file record. */
const H2_FILE: BroadeningFile = {
  gas: "H2",
  name: "toy__H2.broad",
  url: "",
  bytes: 0,
  sha256: "",
};

/** A toy list's definition, as its `def.json` would give it. */
const TOY_DEFINITION: ExoMolDefinition = {
  molecularMassDa: 44,
  airHalfWidthPerCmBar: 0.07,
  airExponent: 0.5,
  broadeners: { H2: { halfWidthPerCmBar: 0.08, exponent: 0.5 } },
};

describe("a def.json", () => {
  const text = JSON.stringify({
    isotopologue: { mass_in_Da: 30.5 },
    broad: {
      "default_Lorentzian_half-width": 0.06,
      default_temperature_exponent: 0.4,
      He: { Lorentzian_half_width: 0.02, temperature_exponent: 0.3 },
    },
  });

  it("gives the reduction its mass and default widths", () => {
    expect(parseDefinition(text, ["He"], "toy.def.json")).toEqual({
      molecularMassDa: 30.5,
      airHalfWidthPerCmBar: 0.06,
      airExponent: 0.4,
      broadeners: { He: { halfWidthPerCmBar: 0.02, exponent: 0.3 } },
    });
  });

  it("refuses a gas whose default it does not give", () => {
    expect(() => parseDefinition(text, ["H2"], "toy.def.json")).toThrow("broad.H2");
  });
});

describe("a .def text file (the archive's definitions, R08.T4.f)", () => {
  const text = [
    "EXOMOL.def                                                # ID",
    "   63.942857 1.06179671e-25                               # Isotopologue mass (Da) and (kg)",
    " 5000.00                                                  # Maximum temperature of linelist",
    "0.0700                                                    # Default value of Lorentzian half-width for all lines (in cm-1/bar)",
    "0.500                                                     # Default value of temperature exponent for all lines",
  ].join("\n");

  it("gives the reduction its mass and default width, and no .broad gas", () => {
    expect(parseDefinitionText(text, "toy.def")).toEqual({
      molecularMassDa: 63.942_857,
      airHalfWidthPerCmBar: 0.07,
      airExponent: 0.5,
      broadeners: {},
    });
  });

  /** The toy without its default width and exponent. */
  const bare = text.split("\n").slice(0, 3).join("\n");

  it("takes the assumed width where the file gives none", () => {
    const assumed = { halfWidthPerCmBar: 0.07, exponent: 0.5, why: "toy" };
    expect(parseDefinitionText(bare, "toy.def", assumed).airHalfWidthPerCmBar).toBe(0.07);
  });

  it("refuses a file that gives no width, with none assumed", () => {
    expect(() => parseDefinitionText(bare, "toy.def")).toThrow("Lorentzian half-width");
  });

  it("reads a labelled line with no value as missing, not as 0", () => {
    const empty = [
      bare,
      "0.0700  # Default value of Lorentzian half-width for all lines (in cm-1/bar)",
      "        # Default value of temperature exponent for all lines",
    ].join("\n");
    expect(() => parseDefinitionText(empty, "toy.def")).toThrow("temperature exponent");
  });
});

describe("a .pf file's Q(T)", () => {
  const pf = new Map([
    [1_000, 9_025.7336],
    [1_001, 9_040.0],
  ]);

  it("takes a tabulated temperature's value", () => {
    expect(partitionSumFromPf(pf, 1_000)).toBe(9_025.7336);
  });

  it("takes Q between two tabulated temperatures linear in T", () => {
    expect(partitionSumFromPf(pf, 1_000.5)).toBe((9_025.7336 + 9_040) / 2);
  });

  it("refuses a temperature outside its table", () => {
    expect(() => partitionSumFromPf(pf, 999)).toThrow(RangeError);
  });
});

describe("a .broad file", () => {
  const table = parseBroadening(
    ["a0 0.0908 0.583 0", "a0 0.0935 0.559 1", "m0 0.0972 0.718 1", "m1 0.1028 0.730 1 1"].join(
      "\n",
    ),
    H2_FILE,
    TOY_DEFINITION,
  );

  it("gives a line its width by |m| and K'' first, then |m|, then J'', then the default", () => {
    // R(0): m = 1; K'' 1 has an m1 row, K'' 0 only m0's.
    expect(broadWidth(table, 1, 0, 1)).toEqual([0.1028, 0.73]);
    expect(broadWidth(table, 1, 0, 0)).toEqual([0.0972, 0.718]);
    // P(1): m = -1, |m| = 1; Q(1): m = J'' = 1.
    expect(broadWidth(table, 0, 1, 0)).toEqual([0.0972, 0.718]);
    expect(broadWidth(table, 1, 1, 0)).toEqual([0.0972, 0.718]);
    // P(0) cannot be; Q(0) has no m-row at m = 0 and takes J'' = 0's a0 row.
    expect(broadWidth(table, 0, 0, 0)).toEqual([0.0908, 0.583]);
    expect(broadWidth(table, 9, 8, 0)).toEqual([0.08, 0.5]);
  });

  it("refuses a code it does not read", () => {
    expect(() => parseBroadening("a4 0.1 0.5 1 2 3 4", H2_FILE, TOY_DEFINITION)).toThrow(
      "does not read",
    );
  });

  it("converts per bar to per atm: a width per bar is 1.013 25 times as wide at 1 atm", () => {
    expect(0.1 / ATM_PER_BAR).toBeCloseTo(0.101_325, 12);
  });
});

/** A toy list: five states and four transitions, in plain text. */
const STATES = [
  "1 0.000000 1 0 0 0 0 0",
  "2 1000.000000 3 1 0 1 1 0",
  "3 14000.000000 3 1 1 0 0 6",
  "4 15000.000000 5 2 1 0 0 6",
  "5 16000.000000 3 1 2 0 0 0",
].join("\n");

/** Transitions upper lower A (no wavenumber, as TYM's): two of band 1 0 0 6, one of 2 0 0 0. */
const TRANSITIONS = ["3 1 10", "4 2 20", "5 1 5", "3 2 1"].join("\n");

/** The toy source. */
const TOY: ExoMolFilesSource = {
  kind: "exomol",
  species: "XX",
  citation: "toy",
  url: "",
  fetched: "",
  terms: "",
  def: { name: "toy.def", bytes: 0, sha256: "", format: "json" },
  states: {
    name: "toy.states",
    bytes: 0,
    sha256: "",
    layout: { energy: 2, degeneracy: 3, j: 4, vibration: [5, 6, 7, 8] },
  },
  transitions: [{ name: "toy.trans", bytes: 0, sha256: "", fromPerCm: 13_000, toPerCm: 17_000 }],
  partition: { from: "states", check: undefined },
  broadeners: [],
  abundance: 0.99,
  binRangePerCm: [13_100, 16_500],
};

/** A pass's target of bins of its own, no band apart. */
const OWN = { kind: "own", apart: undefined } as const;

/** Reads the toy files' text by name, as `streamBzip2Lines` reads a file's. */
const readToy: LineReader = async (path, onLine) => {
  const text = path.endsWith("toy.states") ? STATES : TRANSITIONS;
  const bytes = new TextEncoder().encode(text);
  let start = 0;
  let lines = 0;
  for (let i = 0; i <= bytes.length; i += 1) {
    if (i === bytes.length || bytes[i] === 10) {
      onLine(bytes, start, i);
      lines += 1;
      start = i + 1;
    }
  }
  return lines;
};

/** A reader whose stream breaks at once. */
const failingReader: LineReader = async () => {
  throw new Error("a broken stream");
};

/** The toy's band binned apart: 1 0 0 6 - 0 0 0 0. */
const MATCHED = bandKey(packVibration(1, 0, 0, 6), packVibration(0, 0, 0, 0));

/** A transition's S(T), cm a molecule, as ExoMol defines it, with the toy's abundance and Q. */
function intensity(
  gUpper: number,
  a: number,
  nu: number,
  eLower: number,
  t: number,
  q: number,
): number {
  const c = SPEED_OF_LIGHT_M_PER_S * 100;
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  return (
    (((0.99 * gUpper * a) / (8 * Math.PI * c * nu * nu)) *
      Math.exp((-c2 * eLower) / t) *
      -Math.expm1((-c2 * nu) / t)) /
    q
  );
}

/** A run of bins' total. */
function total(values: Float64Array): number {
  return values.reduce((sum, v) => sum + v, 0);
}

/**
 * `compute`'s promise, made on the first call and kept for every later one, so that the tests
 * sharing it run the work once.
 *
 * @remarks
 * `src/renderer/src/test/memo.ts`'s rule, which this file's compiler project does not reach: only
 * for work that is deterministic and whose result no test mutates, so that whichever test calls
 * first, alone or in any order, computes what any other would have.
 */
function kept<T>(compute: () => Promise<T>): () => Promise<T> {
  let promise: Promise<T> | undefined;
  return () => {
    promise ??= compute();
    return promise;
  };
}

describe("a streaming pass over a list", () => {
  const directory = mkdtempSync(join(tmpdir(), "exomol-lines-"));
  afterAll(() => {
    rmSync(directory, { recursive: true, force: true });
  });

  it("reads the states by ID, and sums Q(T) from them", async () => {
    const states = await readStates("toy.states", TOY, readToy);
    const c2 = SECOND_RADIATION_CONSTANT_CM_K;
    const expected =
      1 +
      3 * Math.exp((-c2 * 1_000) / 300) +
      3 * Math.exp((-c2 * 14_000) / 300) +
      5 * Math.exp((-c2 * 15_000) / 300) +
      3 * Math.exp((-c2 * 16_000) / 300);
    expect([states.j[4], vibrationLabel(states.vibration?.[3] ?? 0)]).toEqual([2, "1 0 0 6"]);
    expect(partitionSum(states, 300)).toBeCloseTo(expected, 12);
  });

  /** A pass over the toy at 200 and 300 K, the matched band apart, and Q(300). */
  async function toyPass(): Promise<
    Awaited<ReturnType<typeof streamPass>> & { readonly q300: number }
  > {
    const states = await readStates("toy.states", TOY, readToy);
    const pass = await streamPass(
      TOY,
      TOY_DEFINITION,
      states,
      directory,
      {
        temperaturesK: [200, 300],
        target: { kind: "own", apart: { bands: new Set([MATCHED]), rangePerCm: [13_100, 15_000] } },
        broadening: [],
        jobs: 1,
      },
      readToy,
    );
    return { ...pass, q300: partitionSum(states, 300) };
  }

  /** One {@link toyPass} for the four tests that read it, made by whichever calls first. */
  const sharedToyPass = kept(toyPass);

  // 3 → 1 (14,000 cm⁻¹) and 4 → 2 (14,000 cm⁻¹) are bands 1 0 0 6 - 0 0 0 0 and 1 0 0 6 - 0 1 1 0;
  // only the first is the matched band. 5 → 1 (16,000) is 2 0 0 0 - 0 0 0 0; 3 → 2 (13,000) lies
  // below the range.
  it("bins the matched band's lines apart, at their S(T)", async () => {
    const { bins, q300 } = await sharedToyPass();
    const apart = intensity(3, 10, 14_000, 0, 300, q300);
    expect(total(bins.apart?.[1]?.values ?? new Float64Array(0)) / apart).toBeCloseTo(1, 10);
  });

  it("bins every other line in range in the main set, at its S(T)", async () => {
    const { bins, q300 } = await sharedToyPass();
    const main = intensity(5, 20, 14_000, 1_000, 300, q300) + intensity(3, 5, 16_000, 0, 300, q300);
    expect(total(bins.main[1]?.values ?? new Float64Array(0)) / main).toBeCloseTo(1, 10);
  });

  it("reads every line and bins those in range", async () => {
    const { record } = await sharedToyPass();
    expect([record.linesRead, record.linesBinned]).toEqual([4, 3]);
  });

  it("records each band's lines over the range binned apart", async () => {
    const { record } = await sharedToyPass();
    expect(record.bands.get(MATCHED)?.lines).toBe(1);
  });

  /** One pass over the toy at 300 K into a set of bins of its own, no band apart. */
  async function ownPass(
    source: ExoMolFilesSource,
  ): Promise<Awaited<ReturnType<typeof streamPass>> & { readonly sum: number }> {
    const states = await readStates("toy.states", source, readToy);
    const pass = await streamPass(
      source,
      TOY_DEFINITION,
      states,
      directory,
      { temperaturesK: [300], target: OWN, broadening: [], jobs: 1 },
      readToy,
    );
    return { ...pass, sum: total(pass.bins.main[0]?.values ?? new Float64Array(0)) };
  }

  /** A pass over a list into shared bins at 300 K. */
  async function sharedPassInto(
    source: ExoMolFilesSource,
    into: ReadonlyArray<SuperLineBins>,
  ): Promise<void> {
    const states = await readStates("toy.states", source, readToy);
    await streamPass(
      source,
      TOY_DEFINITION,
      states,
      directory,
      { temperaturesK: [300], target: { kind: "shared", into }, broadening: [], jobs: 1 },
      readToy,
    );
  }

  it("adds a second list's lines to shared bins at its own abundance", async () => {
    const shared = [emptyBins(300, 13_100, 16_500, 0.01)];
    await sharedPassInto(TOY, shared);
    const before = total(shared[0]?.values ?? new Float64Array(0));
    await sharedPassInto({ ...TOY, abundance: 0.01 }, shared);
    const after = total(shared[0]?.values ?? new Float64Array(0));
    expect(after / before).toBeCloseTo(1 + 0.01 / 0.99, 12);
  });

  it("leaves shared bins' stimulated emission to its caller", async () => {
    const check = emptyBins(300, 13_100, 16_500, 0.01);
    await sharedPassInto(TOY, [check]);
    applyStimulatedEmission(check);
    expect(total(check.values) / (await ownPass(TOY)).sum).toBeCloseTo(1, 12);
  });

  it("refuses shared bins that are not one a temperature", async () => {
    const states = await readStates("toy.states", TOY, readToy);
    const into = [emptyBins(300, 13_100, 16_500, 0.01)];
    await expect(
      streamPass(
        TOY,
        TOY_DEFINITION,
        states,
        directory,
        { temperaturesK: [200, 300], target: { kind: "shared", into }, broadening: [], jobs: 1 },
        readToy,
      ),
    ).rejects.toThrow(RangeError);
  });

  it("takes Q(T) from the list's .pf where its source does", async () => {
    const states = await readStates("toy.states", TOY, readToy);
    const q = 2 * partitionSum(states, 300);
    writeFileSync(join(directory, "toy.pf"), `   296.0 ${q}\n   300.0 ${q}\n   301.0 ${q}\n`);
    const fromPf = await ownPass({
      ...TOY,
      partition: { from: "pf", file: { name: "toy.pf", bytes: 0, sha256: "" } },
    });
    expect([fromPf.record.partitionSums[0]?.[1], fromPf.sum / (await ownPass(TOY)).sum]).toEqual([
      q,
      0.5,
    ]);
  });

  it("scales every binned line by its source's stated factor", async () => {
    const scaled = await ownPass({ ...TOY, intensityScale: { factor: 2, why: "toy" } });
    expect(scaled.sum / (await ownPass(TOY)).sum).toBeCloseTo(2, 12);
  });

  it("reads and drops the transitions past a source's cut on v' - v''", async () => {
    // The toy's v is its first Herzberg quantum: 3 → 1 and 4 → 2 change it by 1, 5 → 1 by 2.
    const cut: ExoMolFilesSource = {
      ...TOY,
      states: { ...TOY.states, layout: { ...TOY.states.layout, v: 5 } },
      maxDeltaV: 1,
    };
    const { record } = await ownPass(cut);
    expect([record.linesRead, record.linesBinned]).toEqual([4, 2]);
  });

  it("refuses a cut on v' - v'' whose states give no v", async () => {
    await expect(ownPass({ ...TOY, maxDeltaV: 1 })).rejects.toThrow("needs its states' v");
  });

  it("refuses a number of jobs under 1", async () => {
    const states = await readStates("toy.states", TOY, readToy);
    await expect(
      streamPass(
        TOY,
        TOY_DEFINITION,
        states,
        directory,
        { temperaturesK: [300], target: OWN, broadening: [], jobs: 0 },
        readToy,
      ),
    ).rejects.toThrow("whole number of jobs");
  });

  it("stops at a reader's failure and throws it", async () => {
    const states = await readStates("toy.states", TOY, readToy);
    await expect(
      streamPass(
        TOY,
        TOY_DEFINITION,
        states,
        directory,
        { temperaturesK: [300], target: OWN, broadening: [], jobs: 2 },
        failingReader,
      ),
    ).rejects.toThrow("a broken stream");
  });

  /**
   * The toy's three lines in range as a REPACK file holds them: each one's ν, E″, g″f (as `repack`
   * writes it, g′A ÷ (8π² c ν² r_e)) and tag 16, with one line below the range between them.
   */
  function toyRepack(name: string, lastTag = 16): RepackSource {
    const c = SPEED_OF_LIGHT_M_PER_S * 100;
    const gf = (gUpper: number, a: number, nu: number): number =>
      (gUpper * a) / (8 * Math.PI ** 2 * c * nu * nu * ELECTRON_RADIUS_CM);
    const records: ReadonlyArray<readonly [number, number, number, number]> = [
      [14_000, 0, gf(3, 10, 14_000), 16],
      [13_000, 1_000, gf(3, 1, 13_000), 16],
      [16_000, 0, gf(3, 5, 16_000), 16],
      [14_000, 1_000, gf(5, 20, 14_000), lastTag],
    ];
    const bytes = Buffer.alloc(records.length * REPACK_RECORD_BYTES);
    for (const [i, [nu, eLower, g, tag]] of records.entries()) {
      bytes.writeDoubleLE(nu, i * REPACK_RECORD_BYTES);
      bytes.writeDoubleLE(eLower, i * REPACK_RECORD_BYTES + 8);
      bytes.writeDoubleLE(g, i * REPACK_RECORD_BYTES + 16);
      bytes.writeInt32LE(tag, i * REPACK_RECORD_BYTES + 24);
    }
    writeFileSync(join(directory, name), bytes);
    const { states: _states, transitions: _transitions, broadeners: _broadeners, ...rest } = TOY;
    return {
      ...rest,
      kind: "repack",
      lines: { name, bytes: bytes.length, sha256: "", md5: "", isotope: 16 },
      partition: { from: "pf", file: { name: "toy-repack.pf", bytes: 0, sha256: "" } },
    };
  }

  /** One pass over a REPACK file at 300 K into bins of its own, its Q(T) the toy states' sums. */
  async function repackPass(
    source: RepackSource,
  ): Promise<Awaited<ReturnType<typeof streamPass>> & { readonly sum: number }> {
    const states = await readStates("toy.states", TOY, readToy);
    writeFileSync(
      join(directory, "toy-repack.pf"),
      `   296.0 ${partitionSum(states, 296)}\n   300.0 ${partitionSum(states, 300)}\n`,
    );
    const pass = await streamPass(source, TOY_DEFINITION, undefined, directory, {
      temperaturesK: [300],
      target: OWN,
      broadening: [],
      jobs: 1,
    });
    return { ...pass, sum: total(pass.bins.main[0]?.values ?? new Float64Array(0)) };
  }

  it("bins a REPACK file's lines as the same lines' transitions are binned", async () => {
    const repack = await repackPass(toyRepack("toy.lbl"));
    const own = await ownPass(TOY);
    expect([
      repack.record.linesRead,
      repack.record.linesBinned,
      repack.sum / own.sum,
      repack.record.intensity296Cm / own.record.intensity296Cm,
    ]).toEqual([4, 3, expect.closeTo(1, 12), expect.closeTo(1, 12)]);
  });

  it("refuses a REPACK record of another isotopologue", async () => {
    await expect(repackPass(toyRepack("toy-other.lbl", 26))).rejects.toThrow("isotope 26");
  });

  it("refuses a pass over an ExoMol list's transitions given none of its states", async () => {
    // Q(T) from a .pf, so that the pass reaches its transitions.
    writeFileSync(join(directory, "toy-unread.pf"), "   296.0 1\n   300.0 1\n");
    const fromPf: ExoMolFilesSource = {
      ...TOY,
      partition: { from: "pf", file: { name: "toy-unread.pf", bytes: 0, sha256: "" } },
    };
    await expect(
      streamPass(fromPf, TOY_DEFINITION, undefined, directory, {
        temperaturesK: [300],
        target: OWN,
        broadening: [],
        jobs: 1,
      }),
    ).rejects.toThrow("none given");
  });

  it("refuses Q(T) summed over states where none were read", () => {
    expect(() => partitionSumsOf(TOY, undefined, directory)).toThrow("no states were read");
  });

  /** The toy list's 300 K bins, a matched band apart, written to the cache; and the set apart's peak. */
  async function cachedToy(): Promise<{
    readonly main: SuperLineBins;
    readonly apart: SuperLineBins;
    readonly peak: number;
  }> {
    const states = await readStates("toy.states", TOY, readToy);
    const { bins } = await streamPass(
      TOY,
      TOY_DEFINITION,
      states,
      directory,
      {
        temperaturesK: [300],
        target: { kind: "own", apart: { bands: new Set([MATCHED]), rangePerCm: [13_100, 15_000] } },
        broadening: [],
        jobs: 1,
      },
      readToy,
    );
    const main = bins.main[0];
    const apart = bins.apart?.[0];
    if (main === undefined || apart === undefined) {
      throw new Error("the pass bins at 300 K, a band apart");
    }
    writeBins(join(directory, binsFileName("XX", "main", 300)), main);
    writeBins(join(directory, binsFileName("XX", "apart", 300)), apart);
    let peak = 0;
    for (let k = 0; k < apart.values.length; k += 1) {
      peak = (apart.values[k] ?? 0) > (apart.values[peak] ?? 0) ? k : peak;
    }
    return { main, apart, peak };
  }

  /**
   * One {@link cachedToy} for the four tests that read the cache, made by whichever calls first.
   * Each still calls it, so that one run alone or in any order finds the files written. The test of
   * what the pass writes still reads what the work itself wrote, as `memo.ts` asks: only this pass
   * writes the files, and they stay until the block's `afterAll` removes the directory.
   */
  const sharedCachedToy = kept(cachedToy);

  it("writes a temperature's bins to the cache and reads them back a window at a time", async () => {
    const { apart, peak } = await sharedCachedToy();
    const file = openBins(join(directory, binsFileName("XX", "apart", 300)));
    const window = file.read(peak - 2, peak + 2);
    file.close();
    expect(Array.from(window.values)).toEqual(
      Array.from(apart.values.subarray(peak - 2, peak + 3)),
    );
  });

  it("reads 0 beyond a cached file's nodes", async () => {
    await sharedCachedToy();
    const file = openBins(join(directory, binsFileName("XX", "main", 300)));
    const window = file.read(-3, -1);
    file.close();
    expect(Array.from(window.values)).toEqual([0, 0, 0]);
  });

  it("adds the set apart to the main set node by node where both are read", async () => {
    const { main, apart, peak } = await sharedCachedToy();
    const species = cachedSpecies(TOY, TOY_DEFINITION, directory, "all");
    const both = species.binsAt(300, peak, peak).values[0];
    species.close();
    expect(both).toBe((main.values[peak] ?? 0) + (apart.values[peak] ?? 0));
  });

  it("reads the main set alone where only it is asked for", async () => {
    const { main, peak } = await sharedCachedToy();
    const species = cachedSpecies(TOY, TOY_DEFINITION, directory, "main");
    const alone = species.binsAt(300, peak, peak).values[0];
    species.close();
    expect(alone).toBe(main.values[peak]);
  });

  it("gives the cached lines their default width per atm, from ExoMol's per bar", () => {
    const species = cachedSpecies(TOY, TOY_DEFINITION, directory, "all");
    const width = species.airHalfWidthPerCmAtm;
    species.close();
    expect(width).toBeCloseTo(0.07 / ATM_PER_BAR, 12);
  });
});

describe("a REPACK file", () => {
  const directory = mkdtempSync(join(tmpdir(), "exomol-repack-"));
  afterAll(() => {
    rmSync(directory, { recursive: true, force: true });
  });

  it("reads each 28-byte record's ν, E″, g″f and tag, little-endian", () => {
    const bytes = Buffer.alloc(2 * REPACK_RECORD_BYTES);
    bytes.writeDoubleLE(18_241.5, 0);
    bytes.writeDoubleLE(1_234.25, 8);
    bytes.writeDoubleLE(3.5e-6, 16);
    bytes.writeInt32LE(16, 24);
    bytes.writeDoubleLE(13_100.125, 28);
    bytes.writeDoubleLE(0, 36);
    bytes.writeDoubleLE(1e-12, 44);
    bytes.writeInt32LE(16, 52);
    const path = join(directory, "two.lbl");
    writeFileSync(path, bytes);
    const read: number[][] = [];
    const count = readRepackLines(path, (nu, eLower, gf, tag) => {
      read.push([nu, eLower, gf, tag]);
    });
    expect([count, read]).toEqual([
      2,
      [
        [18_241.5, 1_234.25, 3.5e-6, 16],
        [13_100.125, 0, 1e-12, 16],
      ],
    ]);
  });

  it("refuses a file that is not a whole number of records", () => {
    const path = join(directory, "short.lbl");
    writeFileSync(path, Buffer.alloc(REPACK_RECORD_BYTES + 3));
    expect(() => readRepackLines(path, () => undefined)).toThrow("whole number");
  });

  it("gives a line's strength as g′A ÷ (8π c ν²), ExoMol's, from repack's g″f", () => {
    const c = SPEED_OF_LIGHT_M_PER_S * 100;
    const [gUpper, a, nu] = [12, 3.2e6, 17_500];
    const gf = (gUpper * a) / (8 * Math.PI ** 2 * c * nu * nu * ELECTRON_RADIUS_CM);
    expect(repackStrengthCm(gf) / ((gUpper * a) / (8 * Math.PI * c * nu * nu))).toBeCloseTo(1, 14);
  });
});
