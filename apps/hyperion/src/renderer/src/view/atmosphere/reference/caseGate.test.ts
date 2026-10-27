/**
 * R08.T13.c1's case gate (plan R08, R08.T13; `decision-r08-t13-scope.md`, ruling 10): the record
 * `caseGate.json` is the gate `caseGate.ts` computes now on the four committed references, through
 * `bless.ts`, and it covers every geometry of every case, every twin value finite and not
 * negative. Every verdict is recorded, never asserted: a miss is a finding (ruling 12).
 *
 * @remarks
 * Under a bless (`HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/caseGate.test.ts`) or by hand under
 * `HYPERION_SLOW_TESTS=1`, every twin value is computed, on the seven twins the record reads (each
 * case's own, `saturn-oblate`'s of one slice and one band, `earth-ozone-slab`'s of equal steps and
 * of R05.T12.e's placement). Otherwise the record is computed from the twin values it holds and
 * checked byte for byte, and the twin is recomputed at {@link SAMPLE} and held equal to them, as
 * `sweepCases.test.ts` holds the sweep's.
 */

import { describe, expect, it } from "vitest";

import { blessMode, expectCommittedTable } from "../../../test/bless";
import committed from "./caseGate.json" with { type: "json" };
import earthReference from "./earth.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import {
  blockingValues,
  CASE_NAMES,
  caseGateRecord,
  computeTwinValues,
  crossoverDeg,
  type GateCase,
  missingTwinValues,
  recorded,
  twinValueKey,
  type TwinValues,
  type TwinValueSpec,
  twinValueSpecs,
  twinValuesOfRecord,
  uncoveredGeometries,
} from "./caseGate";
import { REFERENCE_CASES } from "./cases";
import type { ReferenceFile } from "./references";

/** The committed references, by case. */
const REFERENCES: Readonly<Record<string, ReferenceFile>> = {
  earth: earthReference,
  "earth-ozone-slab": earthOzoneSlabReference,
  mars: marsReference,
  "saturn-oblate": saturnOblateReference,
};

/** The four cases with their references. */
const CASES: ReadonlyArray<GateCase> = REFERENCE_CASES.map((referenceCase) => {
  const reference = REFERENCES[referenceCase.name];
  if (reference === undefined) {
    throw new Error(`no reference is committed for ${referenceCase.name}`);
  }
  return { referenceCase, reference };
});

/** The record's file, from this test. */
const RECORD_FILE = "./caseGate.json";

/** The record's text, for its size. */
const RECORD_TEXTS = import.meta.glob<string>("./caseGate.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The bytes the record stays under: 450,000, the split line under the 500 kB hook. */
const RECORD_SPLIT_BYTES = 450_000;

/**
 * Whether the record is computed whole by hand: `HYPERION_SLOW_TESTS=1 pnpm --filter hyperion exec
 * vitest run src/renderer/src/view/atmosphere/reference/caseGate.test.ts`.
 */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

/** Whether every twin value is computed: under a bless, or by hand. */
const WHOLE = blessMode() === "bless" || slowTests();

/**
 * The default compare's sample: on `earth-ozone-slab`, the sphere's three draws, its beam, its
 * line of sight and its drawn surface per channel; on `mars`, a ground view through its tabulated
 * dust; on `saturn-oblate`, the spheroid's ground and limb and its sliced beam. Three twins: the
 * sphere's with an absorber and layer edges, the sphere's with a tabulated phase, and the
 * spheroid's of five slices and four bands.
 */
const SAMPLE: ReadonlyArray<TwinValueSpec> = [
  ...["ground-s60-v75-a90", "disc-p30", "limb-1H", "aerial-10km"].flatMap((name) => [
    { quantity: "radiance", setting: "case", caseName: CASE_NAMES.slab, name } as const,
    { quantity: "surfaceShare", setting: "case", caseName: CASE_NAMES.slab, name } as const,
  ]),
  { quantity: "beam", setting: "case", caseName: CASE_NAMES.slab, name: "ground-s80-v0-a0" },
  { quantity: "sight", setting: "case", caseName: CASE_NAMES.slab, name: "limb-10H" },
  { quantity: "chromaticSurface", setting: "case", caseName: CASE_NAMES.slab, name: "disc-p30" },
  { quantity: "radiance", setting: "case", caseName: CASE_NAMES.mars, name: "ground-s30-v60-a180" },
  ...["lat60-s30-v60-a0", "limb-lat45-1H"].map(
    (name) =>
      ({ quantity: "radiance", setting: "case", caseName: CASE_NAMES.saturn, name }) as const,
  ),
  { quantity: "beam", setting: "case", caseName: CASE_NAMES.saturn, name: "lat60-s30-v0-a0" },
];

/**
 * The limits of the tests, ms (`vitest.config.mts`'s `testTimeout`), from their times on the
 * laptop (2026-10-27, an i7-8665U, 8 threads): the whole record, its seven twins' tables 58 s of
 * CPU and their reads some 10 s more (96–111 s a run), so ten times over and rounded up to 20 min;
 * the record from its own values, its quadratures some seconds, so 300 s; a sampled twin, its
 * tables 6–18 s, so 600 s.
 */
const LIMIT_MS = { whole: 1_200_000, recorded: 300_000, sampled: 600_000 } as const;

/** The record's writer, as its head names it. */
const HEAD = {
  writer: "caseGate.test.ts, through caseGate.ts (plan R08, R08.T13.c1)",
  radiance: "scalar",
} as const;

/** The committed record's twin values, read once. */
const RECORDED_VALUES = twinValuesOfRecord(committed);

/** The command that rewrites the record. */
const BLESS_COMMAND =
  "HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run src/renderer/src/view/atmosphere/reference/caseGate.test.ts";

/** The twin values of a run: every one computed under a bless or by hand, else the record's. */
function runValues(specs: ReadonlyArray<TwinValueSpec>): TwinValues {
  if (!WHOLE) {
    if (RECORDED_VALUES.size === 0) {
      throw new Error(`caseGate.json holds no twin values: bless it (${BLESS_COMMAND})`);
    }
    return RECORDED_VALUES;
  }
  const values = computeTwinValues(CASES, specs);
  // A blocker stops a bless before it writes (`bless.ts`: such a check runs before the write).
  const blocking = blockingValues(values);
  if (blocking.length > 0) {
    throw new Error(`the twin gives values not finite or negative: ${blocking.join(", ")}`);
  }
  return values;
}

describe("the case gate", () => {
  it("covers every geometry of every case, each traced once", () => {
    expect(uncoveredGeometries(CASES)).toEqual([]);
  });

  it.skipIf(WHOLE)("holds every twin value the record reads", () => {
    expect(missingTwinValues(RECORDED_VALUES, twinValueSpecs(CASES))).toEqual([]);
  });

  it.skipIf(WHOLE)("holds no twin value that is not finite or is negative", () => {
    expect(blockingValues(RECORDED_VALUES)).toEqual([]);
  });

  it(
    "writes caseGate.json, the gate on the four committed references, whole under a bless or by hand",
    { timeout: WHOLE ? LIMIT_MS.whole : LIMIT_MS.recorded },
    async () => {
      const record = caseGateRecord(CASES, runValues(twinValueSpecs(CASES)), HEAD);
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );

  it.skipIf(WHOLE).each([CASE_NAMES.slab, CASE_NAMES.mars, CASE_NAMES.saturn])(
    "recomputes %s's sampled twin values as recorded",
    { timeout: LIMIT_MS.sampled },
    (caseName) => {
      const specs = SAMPLE.filter((spec) => spec.caseName === caseName);
      const keys = specs.map(twinValueKey);
      const recomputed = computeTwinValues(CASES, specs);
      expect(keys.map((key) => [key, recomputed.get(key)])).toEqual(
        keys.map((key) => [key, RECORDED_VALUES.get(key)]),
      );
    },
  );

  it("finds the record as one file", () => {
    expect(Object.keys(RECORD_TEXTS)).toEqual([RECORD_FILE]);
  });

  it("keeps caseGate.json under the 450 kB split line", () => {
    const text = RECORD_TEXTS[RECORD_FILE] ?? "";
    expect(new TextEncoder().encode(text).length).toBeLessThan(RECORD_SPLIT_BYTES);
  });
});

/** One case with its reference's geometries replaced. */
function withGeometries(
  gateCase: GateCase,
  geometries: ReferenceFile["geometries"],
): ReadonlyArray<GateCase> {
  return [{ ...gateCase, reference: { ...gateCase.reference, geometries } }];
}

describe("the case gate's helpers", () => {
  const [earth] = CASES;
  if (earth === undefined) {
    throw new Error("the case gate has four cases");
  }
  const [first, ...rest] = earth.reference.geometries;
  if (first === undefined) {
    throw new Error("a reference traces its geometries");
  }

  it("keeps seven significant figures", () => {
    expect([recorded(1.234_567_89), recorded(-9.876_543_21e-12), recorded(0)]).toEqual([
      1.234_568, -9.876_543e-12, 0,
    ]);
  });

  it("finds where a ratio crosses 1, linear in angle between the bracketing points", () => {
    expect(crossoverDeg([3, 5, 8, 12], [1.2, 1.1, 0.9, 0.8])).toBeCloseTo(6.5, 12);
  });

  it("finds where a ratio crosses another level, such as the sun's own colour", () => {
    expect(crossoverDeg([3, 5, 8, 12], [1.2, 1.1, 0.9, 0.8], 0.95)).toBeCloseTo(7.25, 12);
  });

  it("takes a point where the ratio is exactly 1 as its crossover", () => {
    expect(crossoverDeg([3, 5, 8, 12], [1.2, 1, 0.9, 0.8])).toBe(5);
  });

  it("takes the last point where the ratio reaches exactly 1 there", () => {
    expect(crossoverDeg([3, 5], [1.2, 1])).toBe(5);
  });

  it("finds no crossover where the ratio stays on one side of 1", () => {
    expect(crossoverDeg([3, 5, 8, 12], [1.2, 1.1, 1.05, 1.01])).toBeNull();
  });

  it("reads no twin values from a placeholder", () => {
    expect(twinValuesOfRecord({}).size).toBe(0);
  });

  it("names a twin value that is not finite or is negative, by its channels", () => {
    expect(blockingValues(new Map([["k", [Number.NaN, -1, 0]]]))).toEqual(["k/rg"]);
  });

  it("names a twin value the record reads and the values lack", () => {
    const [spec] = twinValueSpecs(CASES);
    if (spec === undefined) {
      throw new Error("the record reads twin values");
    }
    expect(missingTwinValues(new Map(), [spec])).toEqual([twinValueKey(spec)]);
  });

  it("names a geometry its reference does not trace", () => {
    expect(uncoveredGeometries(withGeometries(earth, rest))).toEqual([`earth/${first.name}`]);
  });

  it("names a geometry its reference traces twice", () => {
    const twice = withGeometries(earth, [first, ...earth.reference.geometries]);
    expect(uncoveredGeometries(twice)).toEqual([`earth/${first.name}`]);
  });
});
