/**
 * R08.T14.f's vector gates (plan R08, R08.T14.f; Design note 10, _Polarisation_ and _The gates_;
 * `decision-r08-t13c2-classes.md` §6): R08.T13.c1's case gate (`caseGate.ts`) re-run against the
 * four committed references' vector radiance I with the polarisation correction, the record
 * `vectorGate.json`. Every verdict is recorded, never asserted (`decision-r08-t13-scope.md`,
 * ruling 12): a miss is a finding against R08.T14.d's preconditions, recorded with its figures.
 *
 * @remarks
 * - **The route.** Each case is read through the path `thick/regime.ts`'s `classifyRegime` gives it
 *   under a clear sky (`CLEAR_COVER`, how the cases were traced): a `thickScattering` case's twin
 *   reads its thick bake with the correction summed into it (`thick/read.ts`'s `thick` kind), a
 *   `thin` one's the correction alone beside Ψ_ms. All four classify `thickScattering`
 *   (R08.T13.c2). Each is baked from its medium's sources under its one light, the reference Sun,
 *   on the twin's own bands (`thick/keptBake.ts`'s `bakeKeptCase` and `bakeKeptCorrection`,
 *   `twin.ts`'s `twinBakeBody`).
 * - **The diagnostic** beside it: the analytic path with the correction, the twin reading the
 *   correction alone beside Ψ_ms. It cannot pass: c1 measured that path outside the gate.
 * - **Uncorrected**, recorded beside: c1's own record, the uncorrected twin against the scalar I
 *   (`caseGate.json`), and the same twin against the vector I.
 * - **R08.T13's surface irradiance** (check 6) against `earth`'s vector flux aggregate, to 2% by the
 *   tolerance test, uncorrected: the sky's irradiance reads Ψ_ms alone, as the kernels' does.
 * - **The tables.** Each case's packed table's bytes, and its correction's largest |ΔJ_m| and the
 *   correction alone's half floats under the least normal (R08.T14.e's deferred _Half floats_).
 *
 * The twin's values on the route and the diagnostic, each case setting's radiance and drawn-surface
 * share (the values `thickRuns.test.ts` replaces), are the record's; every other value is c1's.
 * By hand (`HYPERION_VECTOR_GATE=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/vectorGate.test.ts`, with `HYPERION_BLESS=1` to write
 * the record) every case is baked and its twin's values computed on both paths. Otherwise the
 * route's verdicts are formed from the values the record holds against the same references and held
 * to the record's, and `earth`'s route is baked and its twin's values recomputed at
 * {@link SAMPLED} and held equal to them; under `HYPERION_SLOW_TESTS=1` the whole record is formed
 * from its values and held to the committed file byte for byte (some 90 s on a pod's 4 vCPU, the
 * route's alone 30 s).
 */

import { describe, expect, it } from "vitest";

import type { Rgb } from "../../photometry/toneCurve";
import { expectCommittedTable } from "../../../test/bless";
import { kernelMedium } from "../tables";
import type { MsSourceTable } from "../thick/bake";
import { bakeKeptCase, bakeKeptCorrection } from "../thick/keptBake";
import type { PolarisationTable } from "../thick/polarisation";
import {
  type PackedThickTable,
  packThickTable,
  type ThickTableKind,
  thickTableBytes,
} from "../thick/read";
import { type AtmosphereRegime, CLEAR_COVER, classifyRegime } from "../thick/regime";
import committedGate from "./caseGate.json" with { type: "json" };
import {
  blockingValues,
  caseGateRecord,
  caseTwin,
  type GateCase,
  recorded,
  recordedRgb,
  recordImagesOf,
  recordRatiosOf,
  type TwinValues,
  twinValueKey,
  twinValueOf,
  type TwinValueSpec,
  twinValueSpecs,
  twinValuesOfRecord,
} from "./caseGate";
import { EARTH_CASE, REFERENCE_CASES, type ReferenceCase, referenceCaseBakeLights } from "./cases";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import type { ReferenceFile } from "./references";
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import { twinBakeBody, type TwinOptions } from "./twin";
import committed from "./vectorGate.json" with { type: "json" };

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

/** Whether the bakes are made: by hand, `HYPERION_VECTOR_GATE=1`. */
function bakesRun(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_VECTOR_GATE"] === "1";
}

const BAKES = bakesRun();

/** Whether the whole record is formed from its values: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/**
 * The limits, ms: by hand, the four cases' bakes and both paths' twins, some 4 minutes on a pod's
 * 4 vCPU (2026-10-29), each well over ten times its run; and the record formed three times from its
 * values with `earth`'s route baked again, some 20 s there.
 */
const LIMIT_MS = { whole: 3_600_000, record: 600_000 } as const;

/** The record's file, from this test. */
const RECORD_FILE = "./vectorGate.json";

/** The record's format. */
const VECTOR_GATE_FORMAT = 1;

/** Who writes the record, as each of its records' heads says. */
const WRITER = "vectorGate.test.ts, through caseGate.ts (plan R08, R08.T14.f)";

/** The twin's values of c1's record, which a path's values replace. */
const C1_VALUES = twinValuesOfRecord(committedGate);

/** The values a path replaces: each case setting's radiance and drawn-surface share. */
const PATH_SPECS: ReadonlyArray<TwinValueSpec> = twinValueSpecs(CASES).filter(
  (spec) =>
    spec.setting === "case" && (spec.quantity === "radiance" || spec.quantity === "surfaceShare"),
);

/** The two paths whose values the record holds. */
type PathName = "routed" | "diagnostic";

/** `earth`'s route's values recomputed in every run, by spec key: its first ground views. */
const SAMPLED: ReadonlyArray<string> = PATH_SPECS.filter(
  (spec) => spec.caseName === EARTH_CASE.name && spec.quantity === "radiance",
)
  .slice(0, 3)
  .map(twinValueKey);

/**
 * A case's regime under a clear sky, the table its route reads (`thick/read.ts`'s kinds), and how
 * the record says it.
 *
 * @throws Error for a case routed `cloudDeck`, whose deck split is R08.T15's: none is.
 */
function routeOf(referenceCase: ReferenceCase): {
  readonly regime: AtmosphereRegime;
  readonly table: ThickTableKind;
  readonly path: string;
} {
  const regime = classifyRegime(referenceCase.medium, CLEAR_COVER);
  let table: ThickTableKind;
  let path: string;
  switch (regime.kind) {
    case "thin":
      table = "correction";
      path = "the correction alone beside Psi_ms";
      break;
    case "thickScattering":
      table = "thick";
      path = "the thick bake with the correction summed into it";
      break;
    case "cloudDeck":
      throw new Error(`${referenceCase.name} routes cloudDeck, whose deck split is R08.T15's`);
  }
  return { regime, table, path };
}

/** A case's bake and correction, one table a band of its twin's. */
interface CaseBakes {
  readonly bands: ReadonlyArray<MsSourceTable>;
  readonly correction: ReadonlyArray<PolarisationTable>;
}

/** A case baked from its medium's sources under the reference Sun, on its twin's bands. */
function bakesOf(referenceCase: ReferenceCase): CaseBakes {
  const lights = referenceCaseBakeLights(referenceCase);
  const body = twinBakeBody(referenceCase.figure);
  return {
    bands: bakeKeptCase(referenceCase, lights, body),
    correction: bakeKeptCorrection(referenceCase, lights, body),
  };
}

/** A case's twin on `path`, over its bakes. */
function twinOn(
  path: PathName,
  referenceCase: ReferenceCase,
  bakes: CaseBakes,
): ReturnType<typeof caseTwin> {
  const table = path === "routed" ? routeOf(referenceCase).table : "correction";
  let options: Partial<TwinOptions>;
  switch (table) {
    case "thick":
      options = { thick: bakes.bands, polarisation: bakes.correction };
      break;
    case "correction":
      options = { polarisation: bakes.correction };
      break;
  }
  return caseTwin(referenceCase, options);
}

/** A twin's values of `specs`, as recorded. */
function valuesOf(
  twin: ReturnType<typeof caseTwin>,
  referenceCase: ReferenceCase,
  specs: ReadonlyArray<TwinValueSpec>,
): Map<string, Rgb> {
  const values = new Map<string, Rgb>();
  for (const spec of specs) {
    if (spec.caseName === referenceCase.name) {
      values.set(twinValueKey(spec), recordedRgb(twinValueOf(twin, referenceCase, spec)));
    }
  }
  return values;
}

/** A case's tables as packed: their bytes, and the correction's largest values and half floats. */
function tablesOf(referenceCase: ReferenceCase, bakes: CaseBakes): Record<string, unknown> {
  const drawn = kernelMedium(referenceCase.medium);
  const correction = { bands: bakes.correction, medium: referenceCase.medium };
  const alone = packThickTable(null, drawn, correction);
  let routed: PackedThickTable;
  switch (routeOf(referenceCase).table) {
    case "thick":
      routed = packThickTable(bakes.bands, drawn, correction);
      break;
    case "correction":
      routed = alone;
      break;
  }
  const largest = [0, 0, 0];
  for (const band of bakes.correction) {
    for (const slot of band.slots) {
      for (let i = 0; i < slot.values.length; i += 1) {
        const m = Math.floor(i / 3) % 3;
        largest[m] = Math.max(largest[m] ?? 0, Math.abs(slot.values[i] ?? Number.NaN));
      }
    }
  }
  // The correction alone's J⁰ halves under the half's least normal, 2⁻¹⁴, and above 0.
  let subnormal = 0;
  let halves = 0;
  for (let texel = 0; texel < alone.texels.length / 4; texel += 1) {
    const x = alone.texels[texel * 4] ?? 0;
    const y = alone.texels[texel * 4 + 1] ?? 0;
    for (const bits of [x & 0xffff, x >>> 16, y & 0xffff]) {
      halves += 1;
      const exponent = (bits >>> 10) & 0x1f;
      if (exponent === 0 && (bits & 0x3ff) !== 0) {
        subnormal += 1;
      }
    }
  }
  const [first] = bakes.correction;
  return {
    bands: bakes.correction.length,
    slots: first?.slots.length ?? 0,
    kind: routed.kind,
    bytes: thickTableBytes(routed),
    solves: bakes.correction.reduce((sum, band) => sum + (band.slots[0]?.solves ?? 0), 0),
    complexPairs: bakes.correction.reduce(
      (sum, band) => sum + (band.slots[0]?.complexPairs ?? 0),
      0,
    ),
    largestCorrectionPerSr: largest.map((value) => recorded(value)),
    correctionHalvesBelowLeastNormal: subnormal,
    correctionHalves: halves,
  };
}

/** c1's values with `path`'s in their place. */
function withPath(path: TwinValues): TwinValues {
  const values = new Map(C1_VALUES);
  for (const [key, value] of path) {
    values.set(key, value);
  }
  return values;
}

/** Rows of a path's values, `[key, r, g, b]`, in {@link PATH_SPECS}' order. */
function rowsOf(values: TwinValues): ReadonlyArray<readonly [string, number, number, number]> {
  return PATH_SPECS.map((spec) => {
    const key = twinValueKey(spec);
    const value = values.get(key);
    if (value === undefined) {
      throw new Error(`the path holds no value ${key}`);
    }
    return [key, value[0], value[1], value[2]] as const;
  });
}

/**
 * A path's values as the committed record holds them.
 *
 * @throws Error for a record without them.
 */
function committedValues(path: PathName): Map<string, Rgb> {
  const twinValues: unknown = Reflect.get(committed, "twinValues");
  const rows: unknown =
    typeof twinValues === "object" && twinValues !== null ? Reflect.get(twinValues, path) : null;
  if (!Array.isArray(rows)) {
    throw new Error(`vectorGate.json holds no ${path} values`);
  }
  const values = new Map<string, Rgb>();
  for (const row of rows) {
    if (
      Array.isArray(row) &&
      typeof row[0] === "string" &&
      typeof row[1] === "number" &&
      typeof row[2] === "number" &&
      typeof row[3] === "number"
    ) {
      values.set(row[0], [row[1], row[2], row[3]]);
    }
  }
  return values;
}

/** The committed record's tables section of `caseName`, read back as it stands. */
function committedTables(caseName: string): unknown {
  const cases: unknown = Reflect.get(committed, "cases");
  const section: unknown =
    typeof cases === "object" && cases !== null ? Reflect.get(cases, caseName) : null;
  return typeof section === "object" && section !== null ? Reflect.get(section, "tables") : null;
}

/** A record's section `key`, or `null`. */
function sectionOf(record: unknown, key: string): unknown {
  return typeof record === "object" && record !== null ? Reflect.get(record, key) : null;
}

/**
 * c1's case gate on a path's values against the vector I.
 *
 * @throws Error for a value not finite or negative.
 */
function vectorRecordOf(path: TwinValues): unknown {
  const values = withPath(path);
  if (blockingValues(values).length > 0) {
    throw new Error("a path's twin gives values not finite or negative");
  }
  return caseGateRecord(CASES, values, { writer: WRITER, radiance: "vector" });
}

/** The routed path's sections of the record, from its case gate's record `routed`. */
function routedSections(routed: unknown): Record<string, unknown> {
  return {
    cases: Object.fromEntries(
      CASES.map(({ referenceCase: { name } }) => [
        name,
        { routed: recordImagesOf(routed, name), routedRatios: recordRatiosOf(routed, name) },
      ]),
    ),
    routedMarsAureole: sectionOf(routed, "marsAureole"),
    routedSaturnOblateness: sectionOf(routed, "saturnOblateness"),
  };
}

/** The committed record's routed sections, as {@link routedSections} forms them. */
function committedRoutedSections(): Record<string, unknown> {
  const cases: unknown = Reflect.get(committed, "cases");
  return {
    cases: Object.fromEntries(
      CASES.map(({ referenceCase: { name } }) => {
        const section = sectionOf(cases, name);
        return [
          name,
          {
            routed: sectionOf(section, "routed"),
            routedRatios: sectionOf(section, "routedRatios"),
          },
        ];
      }),
    ),
    routedMarsAureole: sectionOf(committed, "routedMarsAureole"),
    routedSaturnOblateness: sectionOf(committed, "routedSaturnOblateness"),
  };
}

/** The record of both paths' values and each case's tables (the module's remarks). */
function vectorGateRecord(
  paths: Readonly<Record<PathName, TwinValues>>,
  tables: (caseName: string) => unknown,
): Record<string, unknown> {
  const routed = vectorRecordOf(paths.routed);
  const diagnostic = vectorRecordOf(paths.diagnostic);
  const uncorrected = caseGateRecord(CASES, C1_VALUES, { writer: WRITER, radiance: "vector" });
  return {
    description:
      "R08.T14.f's vector gates: R08.T13.c1's case gate re-run against the four committed references' vector radiance I with the polarisation correction, each case through the path classifyRegime gives it under a clear sky (routed), beside the analytic path with the correction (diagnostic), the uncorrected twin against the vector I, and c1's own record, the uncorrected twin against the scalar I (caseGate.json); R08.T13's surface irradiance against the vector flux aggregate, uncorrected; and each case's tables. Verdicts recorded, never gated. Written by vectorGate.test.ts; do not edit by hand.",
    format: VECTOR_GATE_FORMAT,
    cases: Object.fromEntries(
      CASES.map(({ referenceCase }) => {
        const { name } = referenceCase;
        const { regime, path } = routeOf(referenceCase);
        return [
          name,
          {
            regime: regime.kind,
            route: path,
            tables: tables(name),
            routed: recordImagesOf(routed, name),
            diagnostic: recordImagesOf(diagnostic, name),
            uncorrectedVector: recordImagesOf(uncorrected, name),
            c1: recordImagesOf(committedGate, name),
            routedRatios: recordRatiosOf(routed, name),
            diagnosticRatios: recordRatiosOf(diagnostic, name),
          },
        ];
      }),
    ),
    irradiance: sectionOf(uncorrected, "irradiance"),
    routedMarsAureole: sectionOf(routed, "marsAureole"),
    routedSaturnOblateness: sectionOf(routed, "saturnOblateness"),
    twinValues: { routed: rowsOf(paths.routed), diagnostic: rowsOf(paths.diagnostic) },
  };
}

describe.skipIf(!BAKES)("R08.T14.f's vector gates, baked (by hand)", () => {
  it(
    "bakes the four cases and their corrections, gates both paths' twins against the vector I, and writes vectorGate.json",
    { timeout: LIMIT_MS.whole },
    async () => {
      const paths: Record<PathName, Map<string, Rgb>> = {
        routed: new Map(),
        diagnostic: new Map(),
      };
      const tables = new Map<string, unknown>();
      for (const { referenceCase } of CASES) {
        const bakes = bakesOf(referenceCase);
        for (const path of ["routed", "diagnostic"] as const) {
          for (const [key, value] of valuesOf(
            twinOn(path, referenceCase, bakes),
            referenceCase,
            PATH_SPECS,
          )) {
            paths[path].set(key, value);
          }
        }
        tables.set(referenceCase.name, tablesOf(referenceCase, bakes));
      }
      const record = vectorGateRecord(paths, (name) => tables.get(name) ?? null);
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );
});

describe.skipIf(BAKES)("R08.T14.f's vector gates' record", () => {
  it(
    "holds the route's verdicts its own values give against the committed references",
    { timeout: LIMIT_MS.record },
    () => {
      // Through JSON, as the record is committed.
      const formed: unknown = JSON.parse(
        JSON.stringify(routedSections(vectorRecordOf(committedValues("routed")))),
      );
      expect(formed).toEqual(committedRoutedSections());
    },
  );

  it.skipIf(!SLOW)(
    "is the record its own values give against the committed references, byte for byte",
    { timeout: LIMIT_MS.record },
    async () => {
      const record = vectorGateRecord(
        { routed: committedValues("routed"), diagnostic: committedValues("diagnostic") },
        committedTables,
      );
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );

  it(
    "holds the values earth's route gives now, at its first ground views",
    { timeout: LIMIT_MS.record },
    () => {
      const twin = twinOn("routed", EARTH_CASE, bakesOf(EARTH_CASE));
      const now = valuesOf(
        twin,
        EARTH_CASE,
        PATH_SPECS.filter((spec) => SAMPLED.includes(twinValueKey(spec))),
      );
      const held = committedValues("routed");
      expect(SAMPLED.length).toBe(3);
      expect(SAMPLED.map((key) => now.get(key))).toEqual(SAMPLED.map((key) => held.get(key)));
    },
  );
});
