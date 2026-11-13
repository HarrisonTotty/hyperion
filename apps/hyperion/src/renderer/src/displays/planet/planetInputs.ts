/**
 * A planet's inputs as the `PLANET VIEWER` gives them (plan R15): the generator's own, each with
 * its panel label, unit and range, and their one shareable string.
 *
 * @remarks
 * - **The inputs** are the server's `PlanetInputsDto` fields, held in its units (`h` for the
 *   rotation, which the panel shows in `d`, the guide's unit of a rotation period). Six may be
 *   left to the generator to draw (`DRAWN`), the rest are always given.
 * - **The ranges** are the server's (`planetary::viewer::InputField::range`), checked here first so
 *   that a refusal names its field before any request; the server checks again, and refuses an
 *   orbit its system cannot hold.
 * - **The string** is the sim's canonical text, which the server hashes into the system's ID:
 *   `PV1;`, then each given input as `name=value` in the inputs' order, `;`-separated, the seed in
 *   16 lower-case hexadecimal digits and every other value the shortest decimal that reads back to
 *   its bits, a drawn input left out. JavaScript's own shortest decimal is Rust's for every value
 *   the ranges hold (neither writes an exponent there), so one string names one planet on both sides.
 */
import type { PlanetInputsDto, SeedHex } from "@hyperion/protocol";

import { parseSeedHex } from "../../lib/seed";

/** The inputs that are numbers, in the order the panel and the string give them. */
export type NumericInputName =
  | "starMassMsun"
  | "feHDex"
  | "ageGyr"
  | "massMearth"
  | "smaAu"
  | "eccentricity"
  | "formedAtAu"
  | "dayHours"
  | "tiltDeg"
  | "waterRank"
  | "carbonRank"
  | "nitrogenRank";

/** An input's unit: the two drawn mass units by name, every other by its symbol. */
export type InputUnit = "msun" | "mearth" | "dex" | "Gyr" | "AU" | "d" | "°" | "";

/** A unit's accessible name, as a screen reader says it. */
export function unitWords(unit: InputUnit): string {
  let words: string;
  switch (unit) {
    case "msun":
      words = "solar masses";
      break;
    case "mearth":
      words = "Earth masses";
      break;
    case "dex":
    case "Gyr":
    case "AU":
    case "d":
    case "°":
    case "":
      words = unit;
      break;
  }
  return words;
}

/** The wire's name of each numeric input. */
type NumericWireName = Exclude<keyof PlanetInputsDto, "seed">;

/** One numeric input's label, unit, range and wire form. */
export interface NumericInputSpec {
  readonly name: NumericInputName;
  /** Its field on the wire, and in the string. */
  readonly wire: NumericWireName;
  /** Its label on the panel, upper case but for `[Fe/H]` and a formula. */
  readonly label: string;
  /**
   * Its unit as the panel shows it: `msun` and `mearth` are `M☉` and `M⊕`, whose signs are drawn
   * (`SolarMassUnit`, `EarthMassUnit`), every other its symbol, or `""` for a pure number.
   */
  readonly unit: InputUnit;
  /** The wire's units in one of the panel's: 24 for the rotation (h in a day), 1 otherwise. */
  readonly wirePerShown: number;
  /** The least value the server takes, in wire units. */
  readonly minWire: number;
  /** The greatest value the server takes, in wire units. */
  readonly maxWire: number;
  /**
   * The greatest value the panel's field takes, in wire units: the server's, but for the rotation,
   * whose field takes days up to 1,000 d, as far as the guide writes a period in days; a string may
   * still carry more.
   */
  readonly maxEntryWire: number;
  /** Whether the input may be left to the generator to draw. */
  readonly drawable: boolean;
  /** Decimals the panel shows a value it was not typed in at, in its own unit. */
  readonly decimals: number;
}

/**
 * Every numeric input, in the panel's and the string's order (the sim's `InputField` order): the
 * star's three, the orbit's three, then the planet's mass, rotation and obliquity, then the three
 * volatile ranks.
 */
export const NUMERIC_INPUTS: ReadonlyArray<NumericInputSpec> = [
  input("starMassMsun", "star_mass_msun", "INIT MASS", "msun", 0.08, 150, false, 3),
  input("feHDex", "fe_h_dex", "[Fe/H]", "dex", -3, 1, false, 2),
  input("ageGyr", "age_gyr", "AGE", "Gyr", 0.001, 13.787, false, 3),
  input("smaAu", "semi_major_axis_au", "SMA", "AU", 0.001, 100_000, false, 4),
  input("eccentricity", "eccentricity", "ECC", "", 0, 0.9, false, 4),
  input("formedAtAu", "formed_at_au", "FORMED AT", "AU", 0.001, 100_000, true, 4),
  input("massMearth", "mass_mearth", "MASS", "mearth", 0.0001, 4000, false, 4),
  {
    ...input("dayHours", "day_hours", "ROTATION", "d", 0.1, 1_000_000, true, 4),
    wirePerShown: 24,
    maxEntryWire: 24_000,
  },
  input("tiltDeg", "tilt_deg", "OBLIQUITY", "°", 0, 180, true, 1),
  input("waterRank", "water_rank", "H2O", "", 0.001, 0.999, true, 3),
  input("carbonRank", "carbon_rank", "C", "", 0.001, 0.999, true, 3),
  input("nitrogenRank", "nitrogen_rank", "N", "", 0.001, 0.999, true, 3),
];

function input(
  name: NumericInputName,
  wire: NumericWireName,
  label: string,
  unit: InputUnit,
  minWire: number,
  maxWire: number,
  drawable: boolean,
  decimals: number,
): NumericInputSpec {
  return {
    name,
    wire,
    label,
    unit,
    wirePerShown: 1,
    minWire,
    maxWire,
    maxEntryWire: maxWire,
    drawable,
    decimals,
  };
}

/** The spec of the numeric input `name`. */
export function inputSpec(name: NumericInputName): NumericInputSpec {
  const spec = NUMERIC_INPUTS.find((each) => each.name === name);
  if (spec === undefined) {
    throw new Error(`no planet input named ${name}`);
  }
  return spec;
}

/** A planet's inputs: the seed, and each numeric input's wire value, `null` for one left drawn. */
export interface PlanetInputs {
  readonly seed: SeedHex;
  readonly values: Readonly<Record<NumericInputName, number | null>>;
}

/**
 * The inputs the viewer opens on: an Earth about a Sun at 4.57 Gyr, its formation, rotation,
 * obliquity and ranks drawn, under seed 1.
 */
export const DEFAULT_INPUTS: PlanetInputs = {
  seed: "0000000000000001",
  values: {
    starMassMsun: 1,
    feHDex: 0,
    ageGyr: 4.57,
    smaAu: 1,
    eccentricity: 0.0167,
    formedAtAu: null,
    massMearth: 1,
    dayHours: null,
    tiltDeg: null,
    waterRank: null,
    carbonRank: null,
    nitrogenRank: null,
  },
};

/** What a typed entry for an input made: its wire value, or why it is refused. */
export type ParsedEntry =
  { readonly ok: true; readonly wire: number } | { readonly ok: false; readonly reason: string };

/** A decimal as the string carries one: an optional sign, digits, an optional point and digits. */
const TYPED_DECIMAL = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u;

/** A decimal as the operator types one: a string's, or its whole part grouped in threes by commas. */
const ENTERED_DECIMAL = /^[+-]?(?:\d{1,3}(?:,\d{3})+(?:\.\d*)?|\d+(?:\.\d*)?|\.\d+)$/u;

/** The shortest decimal of `value` with no exponent, as Rust's `Display` writes an `f64`. */
export function plainDecimal(value: number): string {
  const text = String(value + 0);
  if (!/e/iu.test(text)) {
    return text;
  }
  // Outside the ranges alone: expand the exponent, keeping the shortest digits.
  const [mantissa = "", exponentText = "0"] = text.toLowerCase().split("e");
  const exponent = Number(exponentText);
  const negative = mantissa.startsWith("-");
  const [whole = "", fraction = ""] = mantissa.replace("-", "").split(".");
  const digits = whole + fraction;
  const point = whole.length + exponent;
  const expanded =
    point <= 0
      ? `0.${"0".repeat(-point)}${digits}`
      : point >= digits.length
        ? digits + "0".repeat(point - digits.length)
        : `${digits.slice(0, point)}.${digits.slice(point)}`;
  return negative ? `-${expanded}` : expanded;
}

/** The range of `spec` in the panel's unit, as a refusal's hint words it: `0.08 to 150`. */
export function rangeWords(spec: NumericInputSpec): string {
  return `${shownText(spec, spec.minWire, Math.ceil)} to ${shownText(spec, spec.maxEntryWire, Math.floor)}`;
}

/**
 * A range's end in the panel's unit, to the input's decimals, rounded inward by `round` so that the
 * value it reads is in range, its digits grouped from five: the rotation's 0.1 h reads `0.0042` d.
 */
function shownText(spec: NumericInputSpec, wire: number, round: (x: number) => number): string {
  const scale = 10 ** spec.decimals;
  return grouped(plainDecimal(round((wire / spec.wirePerShown) * scale) / scale));
}

/**
 * `decimal` with its whole part grouped in threes by commas once it has five digits or more, as
 * the guide groups every number, and never its decimals.
 */
function grouped(decimal: string): string {
  const match = /^(-?)(\d+)(.*)$/u.exec(decimal);
  if (match === null) {
    return decimal;
  }
  const [, sign = "", whole = "", rest = ""] = match;
  return whole.length < 5 ? decimal : `${sign}${whole.replace(/\B(?=(\d{3})+$)/gu, ",")}${rest}`;
}

/**
 * Reads an entry the operator typed for `spec`, in the panel's unit, into its wire value: a decimal,
 * its whole part grouped by commas or not, within the field's range; the refusal says what is valid, as the guide's data-entry
 * rule asks (`MASS INVALID: enter 0.08 to 150`).
 */
export function parseEntry(spec: NumericInputSpec, text: string): ParsedEntry {
  const trimmed = text.trim();
  const refused: ParsedEntry = {
    ok: false,
    reason: `${spec.label.toUpperCase()} INVALID: enter ${rangeWords(spec)}`,
  };
  if (!ENTERED_DECIMAL.test(trimmed)) {
    return refused;
  }
  const wire = Number(trimmed.replaceAll(",", "")) * spec.wirePerShown;
  return Number.isFinite(wire) && wire >= spec.minWire && wire <= spec.maxEntryWire
    ? { ok: true, wire: wire + 0 }
    : refused;
}

/**
 * A wire value as the panel shows it in its unit: to the input's decimals, trailing zeros and
 * a bare point dropped, so that `1` reads `1` and `0.0167` reads `0.0167`, its whole part grouped
 * from five digits (`100,000`), with a `-` for a negative value and a `+` for a positive `[Fe/H]`,
 * whose sign matters.
 */
export function formatInput(spec: NumericInputSpec, wire: number): string {
  const shown = wire / spec.wirePerShown;
  const fixed = shown.toFixed(spec.decimals);
  const trimmed = fixed.includes(".") ? fixed.replace(/0+$/u, "").replace(/\.$/u, "") : fixed;
  const value = grouped(trimmed === "-0" ? "0" : trimmed);
  return spec.name === "feHDex" && shown > 0 ? `+${value}` : value;
}

/** The inputs on the wire. */
export function toInputsDto(inputs: PlanetInputs): PlanetInputsDto {
  const value = (name: NumericInputName): number | null => inputs.values[name];
  const required = (name: NumericInputName): number => {
    const given = inputs.values[name];
    if (given === null) {
      throw new Error(`the planet input ${name} is always given`);
    }
    return given;
  };
  return {
    seed: inputs.seed,
    star_mass_msun: required("starMassMsun"),
    fe_h_dex: required("feHDex"),
    age_gyr: required("ageGyr"),
    mass_mearth: required("massMearth"),
    semi_major_axis_au: required("smaAu"),
    eccentricity: required("eccentricity"),
    formed_at_au: value("formedAtAu"),
    day_hours: value("dayHours"),
    tilt_deg: value("tiltDeg"),
    water_rank: value("waterRank"),
    carbon_rank: value("carbonRank"),
    nitrogen_rank: value("nitrogenRank"),
  };
}

/** Each numeric input's value as `valueOf` gives it, in one record. */
function valuesFrom(
  valueOf: (spec: NumericInputSpec) => number | null,
): Record<NumericInputName, number | null> {
  const of = (name: NumericInputName): number | null => valueOf(inputSpec(name));
  return {
    starMassMsun: of("starMassMsun"),
    feHDex: of("feHDex"),
    ageGyr: of("ageGyr"),
    massMearth: of("massMearth"),
    smaAu: of("smaAu"),
    eccentricity: of("eccentricity"),
    formedAtAu: of("formedAtAu"),
    dayHours: of("dayHours"),
    tiltDeg: of("tiltDeg"),
    waterRank: of("waterRank"),
    carbonRank: of("carbonRank"),
    nitrogenRank: of("nitrogenRank"),
  };
}

/** The inputs of a wire answer. */
export function fromInputsDto(dto: PlanetInputsDto): PlanetInputs {
  return { seed: dto.seed, values: valuesFrom((spec) => dto[spec.wire]) };
}

/** The first line of every string, which names its form. */
const STRING_FORM = "PV1";

/**
 * The numeric inputs in the string's order, the sim's `InputField` order: the star's three, the
 * planet's mass, its orbit's three, then the drawable rotation, obliquity and ranks.
 */
const STRING_ORDER: ReadonlyArray<NumericInputName> = [
  "starMassMsun",
  "feHDex",
  "ageGyr",
  "massMearth",
  "smaAu",
  "eccentricity",
  "formedAtAu",
  "dayHours",
  "tiltDeg",
  "waterRank",
  "carbonRank",
  "nitrogenRank",
];

/** The inputs' string, the sim's canonical text (the module's remarks). */
export function formatInputsString(inputs: PlanetInputs): string {
  const parts = [STRING_FORM, `seed=${inputs.seed}`];
  for (const name of STRING_ORDER) {
    const value = inputs.values[name];
    if (value !== null) {
      parts.push(`${inputSpec(name).wire}=${plainDecimal(value)}`);
    }
  }
  return parts.join(";");
}

/** What a pasted string made: its inputs, or why it cannot be read. */
export type ParsedString =
  | { readonly ok: true; readonly inputs: PlanetInputs }
  | { readonly ok: false; readonly reason: string };

/** A pasted string refused, saying what is valid. */
function refusedString(detail: string): ParsedString {
  return { ok: false, reason: `STRING INVALID: ${detail}` };
}

/**
 * Reads a pasted string: `PV1`, then `name=value` pairs, each name once, every always-given input
 * present and every value within its range.
 */
export function parseInputsString(text: string): ParsedString {
  const parts = text.trim().split(";");
  if (parts[0] !== STRING_FORM) {
    return refusedString("enter a string that starts PV1;");
  }
  const given = new Map<string, string>();
  for (const part of parts.slice(1)) {
    const [name, value, ...rest] = part.split("=");
    if (name === undefined || value === undefined || rest.length > 0 || given.has(name)) {
      return refusedString(`enter name=value pairs, each name once (${part})`);
    }
    given.set(name, value);
  }
  const seedText = given.get("seed");
  const seed = seedText === undefined ? null : parseSeedHex(seedText);
  if (seed === null || !seed.ok) {
    return refusedString("enter seed= and 1 to 16 hexadecimal digits");
  }
  const read = new Map<NumericInputName, number | null>();
  for (const spec of NUMERIC_INPUTS) {
    const valueText = given.get(spec.wire);
    if (valueText === undefined) {
      if (!spec.drawable) {
        return refusedString(`enter ${spec.wire}=`);
      }
      read.set(spec.name, null);
      continue;
    }
    const value = TYPED_DECIMAL.test(valueText) ? Number(valueText) : Number.NaN;
    if (!(value >= spec.minWire && value <= spec.maxWire)) {
      return refusedString(
        `enter ${spec.wire}= ${plainDecimal(spec.minWire)} to ${plainDecimal(spec.maxWire)}`,
      );
    }
    read.set(spec.name, value + 0);
  }
  const known = new Set(["seed", ...NUMERIC_INPUTS.map((spec) => spec.wire)]);
  const unknown = [...given.keys()].find((name) => !known.has(name));
  if (unknown !== undefined) {
    return refusedString(`no input is named ${unknown}`);
  }
  return {
    ok: true,
    inputs: { seed: seed.seed, values: valuesFrom((spec) => read.get(spec.name) ?? null) },
  };
}

/** The numeric input a server's refusal names (`inputs.star_mass_msun`), or `null` for none. */
export function refusedInput(field: string | null): NumericInputName | "seed" | null {
  if (field === null || !field.startsWith("inputs.")) {
    return null;
  }
  const wire = field.slice("inputs.".length);
  if (wire === "seed") {
    return "seed";
  }
  return NUMERIC_INPUTS.find((spec) => spec.wire === wire)?.name ?? null;
}
