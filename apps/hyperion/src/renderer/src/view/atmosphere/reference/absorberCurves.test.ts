/**
 * The curves' shared values (plan R08, R08.T12.b.1 and b.2; `decision-r08-t12b-scope.md`, ruling
 * 4): the client's `absorberTransmittance` on Earth's ozone curves, a saturated synthetic curve, a
 * Saturn-class air's pair curve, and synthetic curves of six, three and two pressure rows, at
 * amounts from 0 past the last node and, on the curves with rows, at pressures on, between and past
 * them, written through `bless.ts` to `crates/hyperion-fit/src/atmosphere/absorber_curves.json`,
 * which the tracer's `absorber` module reads with `include_str!` and holds its own read to, to
 * 10⁻¹².
 *
 * @remarks
 * Rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/absorberCurves.test.ts`.
 */

import { describe, expect, it } from "vitest";

import committed from "../../../../../../../../crates/hyperion-fit/src/atmosphere/absorber_curves.json" with { type: "json" };
import committedText from "../../../../../../../../crates/hyperion-fit/src/atmosphere/absorber_curves.json?raw";
import { ADDED_FILE_LIMIT_BYTES } from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberAmount,
  type AbsorberCurve,
  absorberCurve,
  absorberSpecs,
  absorberTransmittance,
} from "../absorbers";
import { hydrostaticColumn } from "../column";
import {
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE_CURVES,
  R06_SOLAR_BAKE_SPECTRUM,
} from "../earth";
import { exp, log, pow } from "../../../lib/pinnedMath";
import { columnLengthM } from "../medium";

/** The file, from this test. */
const FILE = "../../../../../../../../crates/hyperion-fit/src/atmosphere/absorber_curves.json";

/** What the file says of itself. */
const DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/view/atmosphere/reference/absorberCurves.test.ts (plan R08, R08.T12.b.1 and b.2) from the client's absorberTransmittance; read by crates/hyperion-fit/src/atmosphere/absorber.rs. Do not edit by hand.";

/** The synthetic curve's vertical column, molecules m⁻². */
const SATURATED_COLUMN_PER_M2 = 1e23;

/**
 * A saturated curve on the client's grid, 241 nodes over 10⁻³ to 10³ of its column U
 * (`absorbers.ts`'s `ABSORBER_CURVE_DECADES`): red τ = 4 (u ÷ U)^0.6, which passes −ln 10⁻⁶ at 7.9 U and reaches the
 * floor; green 0 at the first ten nodes, then 0.5 (u ÷ U)^0.8, so that the read is linear in τ
 * across the step from 0; blue τ = u ÷ U, the linear regime.
 */
function saturatedCurve(): AbsorberCurve {
  const columns = Float64Array.from(
    { length: 241 },
    (_, k) => SATURATED_COLUMN_PER_M2 * pow(10, -3 + k / 40),
  );
  const depth = (law: (x: number, k: number) => number): Float64Array =>
    columns.map((u, k) => law(u / SATURATED_COLUMN_PER_M2, k));
  const opticalDepth = [
    depth((x) => 4 * pow(x, 0.6)),
    depth((x, k) => (k < 10 ? 0 : 0.5 * pow(x, 0.8))),
    depth((x) => x),
  ] as const;
  const transmittance = opticalDepth.map((channel) =>
    channel.map((tau) => Math.max(exp(-tau), ABSORBER_TRANSMITTANCE_FLOOR)),
  );
  const [red, green, blue] = transmittance;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a curve has three channels");
  }
  return {
    kind: "column",
    logColumns: columns.map(log),
    logPressures: Float64Array.of(log(1e5)),
    readTopLogPressure: log(1e5),
    transmittance: [red, green, blue],
    opticalDepth,
  };
}

/**
 * A synthetic curve of pressure rows at `pressuresPa`, on 61 nodes over the client's six decades
 * of U (ten to a decade, a quarter of the client's, so that the file stays small: the read is the
 * same on any nodes), its read's top at `readTopPa`: red τ = 4 (u ÷ U)^0.6 (1 + ½ sin y), y = ln(p ÷ 1 Pa), which passes −ln 10⁻⁶ and
 * makes the rows' Lagrange read pass the ceiling; green 0 at the first three nodes, then
 * 0.5 (u ÷ U)^0.8 (1 + y ÷ 20), so that the read is linear in τ across the step from 0; blue
 * τ = (u ÷ U) e^(−y ÷ 8), falling with pressure, as a line's transmittance at a fixed u may rise.
 */
function rowsCurve(pressuresPa: ReadonlyArray<number>, readTopPa: number): AbsorberCurve {
  const columns = Float64Array.from(
    { length: 61 },
    (_, k) => SATURATED_COLUMN_PER_M2 * 10 ** (-3 + k / 10),
  );
  const logPressures = Float64Array.from(pressuresPa, (p) => Math.log(p));
  const depth = (law: (x: number, k: number, y: number) => number): Float64Array =>
    Float64Array.from(
      Array.from(logPressures).flatMap((y) =>
        Array.from(columns, (u, k) => law(u / SATURATED_COLUMN_PER_M2, k, y)),
      ),
    );
  const opticalDepth = [
    depth((x, _, y) => 4 * x ** 0.6 * (1 + 0.5 * Math.sin(y))),
    depth((x, k, y) => (k < 3 ? 0 : 0.5 * x ** 0.8 * (1 + y / 20))),
    depth((x, _, y) => x * Math.exp(-y / 8)),
  ] as const;
  const transmittance = opticalDepth.map((channel) =>
    channel.map((tau) => Math.max(Math.exp(-tau), ABSORBER_TRANSMITTANCE_FLOOR)),
  );
  const [red, green, blue] = transmittance;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a curve has three channels");
  }
  return {
    kind: "column",
    logColumns: columns.map(Math.log),
    logPressures,
    readTopLogPressure: Math.log(readTopPa),
    transmittance: [red, green, blue],
    opticalDepth,
  };
}

/**
 * The pair curve of a Saturn-class air (H₂ 96.3%, He 3.25%, CH₄ 0.45% by volume, `reference/
 * cases.ts`'s `SATURN_OBLATE_AIR`) on an isothermal 2-bar column at 134 K and Saturn's g_ref,
 * under the reference Sun: `absorberSpecs`' `pair:H2+He,H2+H2`, its u a pair column, molecule
 * pairs m⁻⁵.
 */
function pairCurve(): AbsorberCurve {
  const column = hydrostaticColumn({
    surfacePa: 2e5,
    temperature: { kind: "isothermal", temperatureK: 134 },
    meanMolarMassGPerMol: 2.07,
    referenceGravityMS2: 10.4,
    referenceRadiusM: 58_300e3,
  });
  const molecules =
    column.surfaceNumberDensityPerM3 * columnLengthM(column.density, column.topHeightM);
  const { specs } = absorberSpecs(
    [
      { species: "H2", columnPerM2: 0.963 * molecules, basePa: 2e5, topPa: 0 },
      { species: "He", columnPerM2: 0.0325 * molecules, basePa: 2e5, topPa: 0 },
    ],
    column,
  );
  const pair = specs.find((spec) => spec.kind === "pair");
  if (pair === undefined) {
    throw new Error("H2 and He form their pairs");
  }
  return absorberCurve(pair, R06_SOLAR_BAKE_SPECTRUM);
}

/** Slot 0's one curve of a medium's curves. */
function onlyCurve(slots: ReadonlyArray<ReadonlyArray<AbsorberCurve>>): AbsorberCurve {
  const curve = slots[0]?.[0];
  if (curve === undefined) {
    throw new Error("the medium has one absorber under its sun");
  }
  return curve;
}

/**
 * The columns a curve is read at: 0, two below its first node, every fourth node and the middle of
 * every fourth interval in ln u, and six past its last node, up to 10⁵ times it.
 */
function columnsOf(curve: AbsorberCurve): number[] {
  const x = Array.from(curve.logColumns);
  const first = x[0] ?? Number.NaN;
  const last = x.at(-1) ?? Number.NaN;
  const inside = x.flatMap((xk, k) =>
    k % 4 === 0 && k + 1 < x.length ? [xk, 0.5 * (xk + (x[k + 1] ?? Number.NaN))] : [],
  );
  return [
    0,
    exp(first - 3),
    exp(first - 0.5),
    ...inside.map(exp),
    ...[0.2, 1, 2, 4, 7, 11.5].map((d) => exp(last + d)),
  ];
}

/**
 * The pressures a curve with rows is read at, Pa: two below its first row, each row, the middle of
 * each two in ln p, its read's top, and two past its last row; none for a curve of one row.
 */
function pressuresOf(curve: AbsorberCurve): number[] {
  const y = Array.from(curve.logPressures);
  if (y.length < 2) {
    return [];
  }
  const first = y[0] ?? Number.NaN;
  const last = y.at(-1) ?? Number.NaN;
  const middles = y.slice(1).map((yk, k) => 0.5 * (yk + (y[k] ?? Number.NaN)));
  return [
    first - 3,
    first - 0.2,
    ...y,
    ...middles,
    curve.readTopLogPressure,
    last + 0.5,
    last + 4,
  ].map(Math.exp);
}

/** An amount of a curve's kind. */
function amountOf(curve: AbsorberCurve, u: number): AbsorberAmount {
  return curve.kind === "column"
    ? { kind: "column", columnPerM2: u }
    : { kind: "pair", pairColumnPerM5: u };
}

/**
 * One curve's shared values: its rows and the client's read at {@link columnsOf}, and for a curve
 * with rows at each of {@link pressuresOf} too.
 */
function sharedCurve(name: string, curve: AbsorberCurve): unknown {
  const pressures = pressuresOf(curve);
  const pairs =
    pressures.length === 0
      ? columnsOf(curve).map((u) => ({ u, p: undefined }))
      : pressures.flatMap((p) => columnsOf(curve).map((u) => ({ u, p })));
  const reads = pairs.map(({ u, p }) => absorberTransmittance(curve, amountOf(curve, u), p));
  return {
    name,
    kind: curve.kind,
    logColumns: Array.from(curve.logColumns),
    logPressures: Array.from(curve.logPressures),
    readTopLogPressure: curve.readTopLogPressure,
    opticalDepth: curve.opticalDepth.map((channel) => Array.from(channel)),
    amounts: pairs.map(({ u }) => u),
    ...(pressures.length === 0 ? {} : { pressuresPa: pairs.map(({ p }) => p) }),
    transmittance: [0, 1, 2].map((c) => reads.map((t) => t[c] ?? Number.NaN)),
  };
}

describe("the curves' shared values", () => {
  it("are written to absorber_curves.json, which the tracer holds its read to", async () => {
    const table = {
      description: DESCRIPTION,
      curves: [
        sharedCurve("earth-ozone", onlyCurve(EARTH_REFERENCE_CURVES)),
        sharedCurve("earth-ozone-slab", onlyCurve(EARTH_SLAB_REFERENCE_CURVES)),
        sharedCurve("saturated", saturatedCurve()),
        sharedCurve("saturn-class-pair", pairCurve()),
        sharedCurve("six-rows", rowsCurve([10, 100, 1e3, 1e4, 1e5, 1e6], 3e5)),
        sharedCurve("three-rows", rowsCurve([1e3, 3e4, 1e6], 2e6)),
        sharedCurve("two-rows", rowsCurve([1e4, 1e5], 1e5)),
      ],
    };
    await expect(expectCommittedTable(table, committed, FILE)).resolves.toBeUndefined();
  });

  it("keep the file under the added-file hook", () => {
    expect(new TextEncoder().encode(committedText).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });
});

describe("the saturated synthetic curve", () => {
  const at = (u: number): readonly number[] =>
    absorberTransmittance(saturatedCurve(), { kind: "column", columnPerM2: u }, undefined);

  it("passes all light through no column", () => {
    expect(at(0)).toEqual([1, 1, 1]);
  });

  it("reaches the floor past its last node", () => {
    expect(at(1e30)[0]).toBe(ABSORBER_TRANSMITTANCE_FLOOR);
  });

  it("is proportional to u below its first node", () => {
    // Blue's τ = u ÷ U there.
    expect(at(1e18)[2]).toBeCloseTo(exp(-1e-5), 15);
  });
});
