/**
 * The curves' shared values (plan R08, R08.T12.b.1; `decision-r08-t12b-scope.md`, ruling 4): the
 * client's `absorberTransmittance` on Earth's ozone curves and on a saturated synthetic curve, at
 * columns from 0 past the last node, written through `bless.ts` to
 * `crates/hyperion-fit/src/atmosphere/absorber_curves.json`, which the tracer's `absorber` module
 * reads with `include_str!` and holds its own read to, to 10⁻¹².
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
  type AbsorberCurve,
  absorberTransmittance,
} from "../absorbers";
import { EARTH_REFERENCE_CURVES, EARTH_SLAB_REFERENCE_CURVES } from "../earth";

/** The file, from this test. */
const FILE = "../../../../../../../../crates/hyperion-fit/src/atmosphere/absorber_curves.json";

/** What the file says of itself. */
const DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/view/atmosphere/reference/absorberCurves.test.ts (plan R08, R08.T12.b.1) from the client's absorberTransmittance; read by crates/hyperion-fit/src/atmosphere/absorber.rs. Do not edit by hand.";

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
    (_, k) => SATURATED_COLUMN_PER_M2 * 10 ** (-3 + k / 40),
  );
  const depth = (law: (x: number, k: number) => number): Float64Array =>
    columns.map((u, k) => law(u / SATURATED_COLUMN_PER_M2, k));
  const opticalDepth = [
    depth((x) => 4 * x ** 0.6),
    depth((x, k) => (k < 10 ? 0 : 0.5 * x ** 0.8)),
    depth((x) => x),
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
    logPressures: Float64Array.of(Math.log(1e5)),
    readTopLogPressure: Math.log(1e5),
    transmittance: [red, green, blue],
    opticalDepth,
  };
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
    Math.exp(first - 3),
    Math.exp(first - 0.5),
    ...inside.map(Math.exp),
    ...[0.2, 1, 2, 4, 7, 11.5].map((d) => Math.exp(last + d)),
  ];
}

/** One curve's shared values: its row and the client's read at {@link columnsOf}. */
function sharedCurve(name: string, curve: AbsorberCurve): unknown {
  const columns = columnsOf(curve);
  const reads = columns.map((u) =>
    absorberTransmittance(curve, { kind: "column", columnPerM2: u }, undefined),
  );
  return {
    name,
    logColumns: Array.from(curve.logColumns),
    opticalDepth: curve.opticalDepth.map((channel) => Array.from(channel)),
    columnsPerM2: columns,
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
    expect(at(1e18)[2]).toBeCloseTo(Math.exp(-1e-5), 15);
  });
});
