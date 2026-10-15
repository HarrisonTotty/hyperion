import { describe, expect, it } from "vitest";

import simInputsSource from "../../../../../../../../crates/hyperion-sim/src/planetary/surface/inputs.rs?raw";
import simReferenceSource from "../../../../../../../../crates/hyperion-sim/src/planetary/surface/reference.rs?raw";
import surfaceLiquidsSource from "../../../../../../../../crates/hyperion-surface/src/testing/liquids.rs?raw";
import { BAKE_BIN_COUNT, BAKE_BIN_WIDTH_NM, BAKE_RANGE_NM } from "../medium";
import basalticGlassFile from "./basaltic-glass.json" with { type: "json" };
import { type MaterialIndexFile, parseMaterialFile } from "./materials";
import methaneLiquidFile from "./methane-liquid.json" with { type: "json" };
import waterFile from "./water.json" with { type: "json" };

// Read from the Rust sources and the material files beside them, so that a re-reduced file or a
// hand edit to a record fails here rather than leaving the surface payload's copy stale
// (decision-r11-palette-liquids §1.2 and §1.8; R09.T2.e).

/** The decimal places to which the records enter n. */
const INDEX_DECIMALS = 6;

/** The significant figures to which the records enter a. */
const ABSORPTION_FIGURES = 6;

/**
 * A rounding's slack beyond half a unit in its last place, relative: far above the float error of
 * a mean of a few points, far below any change a reduction makes.
 */
const ROUNDING_SLACK = 1e-6;

/** How a record's index follows its file's bin means. */
type IndexRule =
  /** The bin means themselves. */
  | { readonly kind: "file" }
  /**
   * The bin means carried by Lorentz–Lorenz at constant molar refraction from the liquid's density
   * at the file's temperature, kg m⁻³ and K, to the record's density.
   */
  | {
      readonly kind: "lorentzLorenz";
      readonly fileDensityKgM3: number;
      readonly fileTemperatureK: number;
    };

/**
 * The methane records' carry: from liquid methane's density at 90 K, `methane-liquid.json`'s
 * temperature, 452.4 kg m⁻³, the saturated liquid's line extrapolated 0.69 K below the triple point
 * (`liquid_methane_titan`'s documentation; plan 09's "Deviations in T2.e, as built").
 */
const METHANE_CARRY: IndexRule = {
  kind: "lorentzLorenz",
  fileDensityKgM3: 452.4,
  fileTemperatureK: 90,
};

/** One hand-entered record and the material file it was reduced from. */
interface LiquidRecordCase {
  /** The record's Rust item, as the tests name it. */
  readonly name: string;
  /** The Rust source that holds it. */
  readonly source: string;
  /** The text that opens the record's item in that source, once. */
  readonly opens: string;
  /** The material file's name. */
  readonly fileName: string;
  /** The material file. */
  readonly file: MaterialIndexFile;
  /** How the record's index follows the file. */
  readonly index: IndexRule;
  /** Whether the record carries the file's absorption, or leaves it not modelled. */
  readonly absorption: "file" | "absent";
}

const WATER = parseMaterialFile(waterFile, "water.json");
const METHANE = parseMaterialFile(methaneLiquidFile, "methane-liquid.json");
const BASALTIC_GLASS = parseMaterialFile(basalticGlassFile, "basaltic-glass.json");

/**
 * Every hand-entered liquid record of the surface payload, with the file it was reduced from.
 *
 * @remarks
 * The surface crate's synthetic records (`hyperion_surface::testing`'s `liquid_water`,
 * `liquid_methane_titan` and `basaltic_melt`) and the sim's copies of two of them (the reference
 * Earth's `WATER_RECORD` and the surface tests' `methane_sea`) are entered by hand, since neither
 * crate reads a material file. Each by-bin member is the uniform mean, over each of R08's fifteen
 * bake bins, of its file's 5 nm grid interpolated linearly in n and in a = 4πk ÷ λ, n to
 * {@link INDEX_DECIMALS} decimal places and a to {@link ABSORPTION_FIGURES} significant figures.
 * R08.T5.h moved `water.json` after R09.T2.e had entered its bins, and no test saw the copies go
 * stale; these tests read the records from their Rust sources and the files beside them, so that
 * a change to either side fails here until the other follows (and the surface crate's
 * `wire/payloads.golden` is re-blessed).
 */
const RECORDS: ReadonlyArray<LiquidRecordCase> = [
  {
    name: "hyperion_surface::testing::liquid_water",
    source: surfaceLiquidsSource,
    opens: "pub fn liquid_water() -> LiquidProperties {",
    fileName: "water.json",
    file: WATER,
    index: { kind: "file" },
    absorption: "file",
  },
  {
    name: "hyperion_sim::planetary::surface::reference's WATER_RECORD",
    source: simReferenceSource,
    opens: "const WATER_RECORD: LiquidProperties = LiquidProperties {",
    fileName: "water.json",
    file: WATER,
    index: { kind: "file" },
    absorption: "file",
  },
  {
    name: "hyperion_surface::testing::liquid_methane_titan",
    source: surfaceLiquidsSource,
    opens: "pub fn liquid_methane_titan() -> LiquidProperties {",
    fileName: "methane-liquid.json",
    file: METHANE,
    index: METHANE_CARRY,
    absorption: "absent",
  },
  {
    name: "hyperion_sim::planetary::surface::inputs's methane_sea",
    source: simInputsSource,
    opens: "fn methane_sea() -> LiquidProperties {",
    fileName: "methane-liquid.json",
    file: METHANE,
    index: METHANE_CARRY,
    absorption: "absent",
  },
  {
    name: "hyperion_surface::testing::basaltic_melt",
    source: surfaceLiquidsSource,
    opens: "pub fn basaltic_melt() -> LiquidProperties {",
    fileName: "basaltic-glass.json",
    file: BASALTIC_GLASS,
    index: { kind: "file" },
    absorption: "file",
  },
];

/** The bake bins' edges, nm: 380 + 25.33 k, k = 0..15. */
const BIN_EDGES_NM = Array.from(
  { length: BAKE_BIN_COUNT + 1 },
  (_, k) => BAKE_RANGE_NM[0] + k * BAKE_BIN_WIDTH_NM,
);

/** The value at `index`, which the caller knows to be in range. */
function at(values: ArrayLike<number>, index: number): number {
  const value = values[index];
  if (value === undefined) {
    throw new Error(`index ${index} is outside a list of ${values.length}`);
  }
  return value;
}

/** The linear interpolant of `values` on `gridNm` at `wavelengthNm`, inside the grid. */
function linearAt(gridNm: Float64Array, values: Float64Array, wavelengthNm: number): number {
  for (let i = 1; i < gridNm.length; i += 1) {
    const high = at(gridNm, i);
    if (wavelengthNm <= high) {
      const low = at(gridNm, i - 1);
      const t = (wavelengthNm - low) / (high - low);
      return at(values, i - 1) + t * (at(values, i) - at(values, i - 1));
    }
  }
  throw new Error(`${wavelengthNm} nm lies beyond the file's grid`);
}

/**
 * The uniform mean of the linear interpolant over [lowNm, highNm]: exact, the trapezium rule
 * between the bin's edges and the grid's points inside it.
 */
function binMean(
  gridNm: Float64Array,
  values: Float64Array,
  lowNm: number,
  highNm: number,
): number {
  const points = [lowNm, ...Array.from(gridNm).filter((nm) => nm > lowNm && nm < highNm), highNm];
  let integral = 0;
  for (let i = 1; i < points.length; i += 1) {
    const from = at(points, i - 1);
    const to = at(points, i);
    integral += 0.5 * (linearAt(gridNm, values, from) + linearAt(gridNm, values, to)) * (to - from);
  }
  return integral / (highNm - lowNm);
}

/** Each bake bin's mean of `values` on the file's grid. */
function binMeans(file: MaterialIndexFile, values: Float64Array): ReadonlyArray<number> {
  return Array.from({ length: BAKE_BIN_COUNT }, (_, k) =>
    binMean(file.wavelengthsNm, values, at(BIN_EDGES_NM, k), at(BIN_EDGES_NM, k + 1)),
  );
}

/** The file's absorption coefficient a = 4πk ÷ λ, m⁻¹, at each point of its grid. */
function absorptionPerM(file: MaterialIndexFile): Float64Array {
  return file.k.map((k, i) => (4 * Math.PI * k) / (at(file.wavelengthsNm, i) * 1e-9));
}

/** `n` carried by Lorentz–Lorenz at constant molar refraction by the density ratio `ratio`. */
function lorentzLorenz(n: number, ratio: number): number {
  const refraction = ((n * n - 1) / (n * n + 2)) * ratio;
  return Math.sqrt((1 + 2 * refraction) / (1 - refraction));
}

/** A Rust float literal's value (`1_473.15`, `8.900e-4`). */
function rustFloat(literal: string): number {
  const value = Number(literal.trim().replaceAll("_", ""));
  if (!Number.isFinite(value)) {
    throw new Error(`"${literal}" is not a Rust float literal`);
  }
  return value;
}

/** A Rust array literal's values, from the text between its brackets. */
function rustFloats(list: string): ReadonlyArray<number> {
  return list
    .split(",")
    .filter((item) => item.trim().length > 0)
    .map(rustFloat);
}

/** The text of the record that `opens` opens, up to its last member, `viscosity_pa_s`. */
function recordText(source: string, opens: string): string {
  const start = source.indexOf(opens);
  if (start < 0 || source.includes(opens, start + 1)) {
    throw new Error(`no single record opens with "${opens}"`);
  }
  const end = source.indexOf("viscosity_pa_s:", start);
  if (end < 0) {
    throw new Error(`the record "${opens}" opens has no viscosity_pa_s member`);
  }
  return source.slice(start, end);
}

/** A member's literal: `text` itself, or the value of the `const` of `kind` it names in `source`. */
function constOrLiteral(source: string, text: string, kind: "f64" | "bins"): string {
  if (!/^[A-Z][A-Z0-9_]*$/.test(text)) {
    return text;
  }
  const pattern =
    kind === "f64"
      ? new RegExp(`const ${text}: f64 = ([^;]+);`)
      : new RegExp(`const ${text}: \\[f64; LIQUID_BINS\\] = \\[([^\\]]*)\\];`);
  const found = pattern.exec(source)?.[1];
  if (found === undefined) {
    throw new Error(`the source defines no ${kind} const ${text}`);
  }
  return found;
}

/** A record's members that its file gives, as its Rust literal holds them. */
interface HeldRecord {
  /** The record's density, kg m⁻³. */
  readonly densityKgM3: number;
  /** Its index by bin. */
  readonly index: ReadonlyArray<number>;
  /** Its absorption by bin, m⁻¹, or `undefined` where it is `None`. */
  readonly absorptionPerM: ReadonlyArray<number> | undefined;
  /** Whether its scattering is `None`. */
  readonly scatteringAbsent: boolean;
}

/** The record `opens` opens in `source`. */
function heldRecord(source: string, opens: string): HeldRecord {
  const text = recordText(source, opens);
  const density = /density_kg_m3:\s*([^,]+),/.exec(text)?.[1];
  const index = /refractive_index:\s*(?:\[([^\]]*)\]|([A-Z][A-Z0-9_]*))\s*,/.exec(text);
  const absorption =
    /absorption_per_m:\s*(None|Some\(\s*(?:\[([^\]]*)\]|([A-Z][A-Z0-9_]*))\s*\))/.exec(text);
  if (density === undefined || index === null || absorption === null) {
    throw new Error(`the record "${opens}" opens lacks its density, index or absorption`);
  }
  const bins = (inline: string | undefined, named: string | undefined): string | undefined =>
    inline ?? (named === undefined ? undefined : constOrLiteral(source, named, "bins"));
  const indexBins = bins(index[1], index[2]);
  if (indexBins === undefined) {
    throw new Error(`the record "${opens}" opens has no index by bin`);
  }
  const absorptionBins = bins(absorption[2], absorption[3]);
  return {
    densityKgM3: rustFloat(constOrLiteral(source, density.trim(), "f64")),
    index: rustFloats(indexBins),
    absorptionPerM:
      absorption[1] === "None" || absorptionBins === undefined
        ? undefined
        : rustFloats(absorptionBins),
    scatteringAbsent: /scattering_per_m:\s*None\s*,/.test(text),
  };
}

/** How far a value may lie from its mean when it is the mean rounded to `unit`. */
function roundingTolerance(unit: number): number {
  return 0.5 * unit * (1 + ROUNDING_SLACK);
}

/** The unit in the last place of `value` rounded to `figures` significant figures. */
function figuresUnit(value: number, figures: number): number {
  return value === 0 ? 0 : 10 ** (Math.floor(Math.log10(Math.abs(value))) - (figures - 1));
}

/**
 * The bins where `held` is not `expected` rounded to the unit `unitOf` gives for the larger of
 * the two, each described; none where every bin is.
 */
function offBins(
  held: ReadonlyArray<number>,
  expected: ReadonlyArray<number>,
  unitOf: (value: number) => number,
): ReadonlyArray<string> {
  return held.flatMap((value, k) => {
    const mean = at(expected, k);
    const unit = unitOf(Math.max(Math.abs(value), Math.abs(mean)));
    return Math.abs(value - mean) <= roundingTolerance(unit)
      ? []
      : [`bin ${k}: ${value} held, ${mean} from the file`];
  });
}

/** The unit in the last place of n as the records enter it. */
const indexUnit = (): number => 10 ** -INDEX_DECIMALS;

/** The unit in the last place of a value of a as the records enter it. */
const absorptionUnit = (value: number): number => figuresUnit(value, ABSORPTION_FIGURES);

/** The records whose index is their file's bin means. */
const UNCARRIED = RECORDS.filter((record) => record.index.kind === "file");

/** The records whose index is their file's carried by Lorentz–Lorenz, each with its carry. */
const CARRIED = RECORDS.flatMap((record) =>
  record.index.kind === "lorentzLorenz" ? [{ ...record, carry: record.index }] : [],
);

describe("the surface palettes' liquid records", () => {
  it.each(UNCARRIED)("$name's index is $fileName's bin means", (record) => {
    const held = heldRecord(record.source, record.opens).index;
    expect(held).toHaveLength(BAKE_BIN_COUNT);
    expect(offBins(held, binMeans(record.file, record.file.n), indexUnit)).toEqual([]);
  });

  it.each(CARRIED)(
    "$name's index is $fileName's bin means carried to its density by Lorentz–Lorenz",
    (record) => {
      const held = heldRecord(record.source, record.opens);
      const ratio = held.densityKgM3 / record.carry.fileDensityKgM3;
      const expected = binMeans(record.file, record.file.n).map((n) => lorentzLorenz(n, ratio));
      expect(held.index).toHaveLength(BAKE_BIN_COUNT);
      expect(offBins(held.index, expected, indexUnit)).toEqual([]);
    },
  );

  it.each(CARRIED)("$fileName is at the temperature $name's carry starts from", (record) => {
    expect(record.file.temperatureK).toBe(record.carry.fileTemperatureK);
  });

  it.each(RECORDS.filter((record) => record.absorption === "file"))(
    "$name's absorption is $fileName's bin means",
    (record) => {
      const held = heldRecord(record.source, record.opens).absorptionPerM;
      expect(held).toHaveLength(BAKE_BIN_COUNT);
      const expected = binMeans(record.file, absorptionPerM(record.file));
      expect(offBins(held ?? [], expected, absorptionUnit)).toEqual([]);
    },
  );

  it.each(RECORDS.filter((record) => record.absorption === "absent"))(
    "$name leaves its absorption not modelled",
    (record) => {
      expect(heldRecord(record.source, record.opens).absorptionPerM).toBeUndefined();
    },
  );

  // No material file holds a scattering coefficient, so no record takes one from a file.
  it.each(RECORDS)("$name leaves its scattering not modelled", (record) => {
    expect(heldRecord(record.source, record.opens).scatteringAbsent).toBe(true);
  });

  // A record's `basis` is its one required member that names no array, so it counts the records
  // whichever way their arrays are written.
  it("covers every record its sources hold", () => {
    const sources = [surfaceLiquidsSource, simReferenceSource, simInputsSource];
    const counted = sources.map((source) => source.split("basis: LiquidBasis::").length - 1);
    const listed = sources.map((source) => RECORDS.filter((r) => r.source === source).length);
    expect(counted).toEqual(listed);
  });
});
