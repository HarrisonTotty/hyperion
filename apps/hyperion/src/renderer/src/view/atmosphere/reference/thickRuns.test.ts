/**
 * R08.T14.c's recorded runs (plan R08, R08.T14.c; `decision-r08-t13c2-classes.md` §1 and §6), by
 * hand: `HYPERION_THICK_RUNS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/thickRuns.test.ts`, with `HYPERION_BLESS=1` to write
 * `thickRuns.json`. Recorded, never gated: every verdict is the record's, and a run asserts only
 * that its values are finite and not negative and that the record is as committed.
 *
 * @remarks
 * - **c1's scalar case gate through the bake.** Each of the four committed cases (`earth`,
 *   `earth-ozone-slab`, `mars`, `saturn-oblate`) is baked from its medium's sources under its one
 *   light, the reference Sun (`thick/keptBake.ts`, `cases.ts`'s `referenceCaseBakeLights`), on the
 *   twin's own bands (`twin.ts`'s `twinBakeBody`), and its twin reads the table in place of Ψ_ms
 *   from the table's first sun cosine cell up (`TwinOptions.thick`). Its radiance and drawn-surface
 *   share replace the case twin's in the committed `caseGate.json`'s values, and R08.T13.c1's
 *   record (`caseGate.ts`'s `caseGateRecord`) is formed on them against the same references,
 *   scalar against scalar: nothing is re-traced. Each image's tally is written beside c1's.
 * - **The half floats** (T14.b's deferred finding): each bake's E⁰ and E¹ in m⁻¹ sr⁻¹, the packed
 *   J⁰ and G in sr⁻¹, and the half's largest relative rounding over the entries at least 10⁻³ of
 *   their table's largest J⁰.
 * - **What the label covered** (`decision-r08-t6b-floor.md` §9.4.3): on R08.T6.c's Venus-class
 *   medium (`venusClassMedium`), the analytic multiple-scattering table's texel of the largest
 *   1 ÷ mean(escape), its Ψ_ms there, and the bake's J⁰ there averaged over the travel cosines, the
 *   same per-steradian quantity.
 */

import { describe, expect, it } from "vitest";

import { venusClassColumn, venusClassMedium } from "../../../test/atmosphereFixtures";
import {
  SWEEP_FIGURE,
  SWEEP_FILES,
  SWEEP_TAU_CANDIDATES,
  sweepGeometries,
  sweepMedium,
  sweepPointOf,
  walkSpot,
} from "../../../test/atmosphereSweep";
import { expectCommittedTable } from "../../../test/bless";
import { R06_SOLAR_BAKE_SPECTRUM } from "../earth";
import { TABLE_SIZES, type TableSizes } from "../hillaire";
import { keptBakeLightsOf } from "../opticsMessages";
import { kernelMedium, MULTI_SCATTERING_SIZE, type TableSize } from "../tables";
import { multiScatteringSeriesTwin, planetTwin } from "../tablesCpu";
import type { MsSourceTable } from "../thick/bake";
import { bakeKeptCase, sphereBakeBody } from "../thick/keptBake";
import {
  addThickLayer,
  packThickTable,
  type PackedThickTable,
  THICK_TRAVEL_COSINES,
  thickRowAt,
  thickSourceScratch,
} from "../thick/read";
import { channelSource, type SpectralSource } from "../thick/spectralColumn";
import committedGate from "./caseGate.json" with { type: "json" };
import {
  blockingValues,
  caseGateRecord,
  caseTwin,
  type GateCase,
  recorded,
  recordedRgb,
  twinValueKey,
  twinValueOf,
  twinValueSpecs,
  twinValuesOfRecord,
} from "./caseGate";
import { EARTH_CASE, REFERENCE_CASES, type ReferenceCase, referenceCaseBakeLights } from "./cases";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import type { ReferenceFile } from "./references";
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import { readSweepCase, readSweepOutput, recomputePoint } from "./sweepBoundary";
import committed from "./thickRuns.json" with { type: "json" };
import { referenceTwin, twinBakeBody, type TwinOptions } from "./twin";

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

/** Whether the runs are made: by hand, `HYPERION_THICK_RUNS=1`. */
function runs(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_THICK_RUNS"] === "1";
}

const RUN = runs();

/**
 * The limits, ms, from the runs on the desktop (2026-10-27, 67 s in all): a case's bake 1–3 s a
 * band and its twin's values some 10 s, the Venus-class table's 1,024 texels some 15 s, and the
 * record, c1's own, as `caseGate.test.ts` limits it; each well over ten times its run.
 */
const LIMIT_MS = { whole: 3_600_000 } as const;

/** The record's file, from this test. */
const RECORD_FILE = "./thickRuns.json";

/** The record's format. */
const THICK_RUNS_FORMAT = 1;

/** The twin's values of c1's record, which a case's thick values replace. */
const C1_VALUES = twinValuesOfRecord(committedGate);

/** A case's bake: its tables, one a band of the twin's, and the solves it took. */
function bakeOf(referenceCase: ReferenceCase): ReadonlyArray<MsSourceTable> {
  return bakeKeptCase(
    referenceCase,
    referenceCaseBakeLights(referenceCase),
    twinBakeBody(referenceCase.figure),
  );
}

/** A number of the record, to its significant figures; null for none. */
function figure(value: number): number {
  return recorded(value);
}

/** The ranges of a bake's values and their packing (the module's remarks, the half floats). */
function halfFloatsOf(
  bands: ReadonlyArray<MsSourceTable>,
  packed: PackedThickTable,
): Record<string, unknown> {
  let e0Least = Number.POSITIVE_INFINITY;
  let e0Most = 0;
  let e1Most = 0;
  for (const band of bands) {
    for (const slot of band.slots) {
      for (let i = 0; i < slot.values.length; i += 6) {
        for (let c = 0; c < 3; c += 1) {
          const e0 = slot.values[i + c] ?? Number.NaN;
          const e1 = Math.abs(slot.values[i + 3 + c] ?? Number.NaN);
          if (e0 > 0) {
            e0Least = Math.min(e0Least, e0);
          }
          e0Most = Math.max(e0Most, e0);
          e1Most = Math.max(e1Most, e1);
        }
      }
    }
  }
  // The packed table's J⁰ and G against their f64 values, read back at the cells' centres.
  let j0Least = Number.POSITIVE_INFINITY;
  let j0Most = 0;
  let slopeMost = 0;
  const halves: number[] = [];
  const scratch = thickSourceScratch();
  for (let layer = 0; layer < packed.layers; layer += 1) {
    for (let a = 0; a < packed.altitudesM.length; a += 1) {
      const row = thickRowAt(packed, packed.altitudesM[a] ?? Number.NaN);
      for (let s = 0; s < 32; s += 1) {
        for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
          scratch.level.fill(0);
          scratch.slope.fill(0);
          addThickLayer(packed, layer, row, (s + 0.5) / 32, (v + 0.5) / 8 - 1, 1, scratch);
          for (let c = 0; c < 3; c += 1) {
            const j0 = scratch.level[c] ?? Number.NaN;
            if (j0 > 0) {
              j0Least = Math.min(j0Least, j0);
            }
            j0Most = Math.max(j0Most, j0);
            slopeMost = Math.max(slopeMost, Math.abs(scratch.slope[c] ?? Number.NaN));
            halves.push(j0);
          }
        }
      }
    }
  }
  return {
    e0PerMSr: [figure(e0Least), figure(e0Most)],
    e1MostPerMSr: figure(e1Most),
    j0PerSr: [figure(j0Least), figure(j0Most)],
    slopeMostPerSr: figure(slopeMost),
    j0BelowLeastNormal: halves.filter((j) => j > 0 && j < 2 ** -14).length,
    entries: halves.length,
  };
}

/** One case's run: its thick values and its bake's record. */
interface CaseRun {
  readonly values: ReadonlyMap<string, readonly [number, number, number]>;
  readonly bake: Record<string, unknown>;
}

/** The kernels' sky-view size at four times its rows, the near-horizon rows among them. */
const FINE_SKY_VIEW: TableSize = {
  widthTexels: TABLE_SIZES.high.skyView.widthTexels,
  heightTexels: 4 * TABLE_SIZES.high.skyView.heightTexels,
};

/** The control's analytic twin: Ψ_ms at four times its μ_s, and the sky view's rows four times. */
const FINE_TWIN: Partial<TwinOptions> = {
  sizes: { ...TABLE_SIZES.high, skyView: FINE_SKY_VIEW } satisfies TableSizes,
  multiScatteringSize: {
    widthTexels: 4 * MULTI_SCATTERING_SIZE.widthTexels,
    heightTexels: MULTI_SCATTERING_SIZE.heightTexels,
  },
};

/** The sweep's committed files, by name: its outputs and cases. */
const SWEEP_OUTPUTS = import.meta.glob<unknown>(["./ms-sweep-w1000.json"], {
  import: "default",
  eager: true,
});
const SWEEP_CASES = import.meta.glob<unknown>(["./ms-sweep-w1000.case.json"], {
  import: "default",
  eager: true,
});

/** The record's case section's images, as c1's record writes them. */
function imagesOf(record: unknown, caseName: string): unknown {
  if (typeof record !== "object" || record === null) {
    return null;
  }
  const cases: unknown = Reflect.get(record, "cases");
  if (typeof cases !== "object" || cases === null) {
    return null;
  }
  const section: unknown = Reflect.get(cases, caseName);
  if (typeof section !== "object" || section === null) {
    return null;
  }
  return { all: Reflect.get(section, "all"), images: Reflect.get(section, "images") };
}

/**
 * Per image of a record's case section, the least and largest twin ÷ reference over its
 * geometries and channels, each with where it lies (`name/channel`).
 */
function ratiosOf(record: unknown, caseName: string): unknown[] {
  const cases: unknown =
    typeof record === "object" && record !== null ? Reflect.get(record, "cases") : null;
  const section: unknown =
    typeof cases === "object" && cases !== null ? Reflect.get(cases, caseName) : null;
  const rows: unknown =
    typeof section === "object" && section !== null ? Reflect.get(section, "geometries") : null;
  if (!Array.isArray(rows)) {
    return [];
  }
  const letters = ["r", "g", "b"];
  const byImage = new Map<
    string,
    { least: number; leastAt: string; most: number; mostAt: string }
  >();
  for (const row of rows) {
    if (!Array.isArray(row) || typeof row[0] !== "string" || typeof row[1] !== "string") {
      continue;
    }
    for (let c = 0; c < 3; c += 1) {
      const twin: unknown = row[4 + c];
      const reference: unknown = row[10 + c];
      if (typeof twin === "number" && typeof reference === "number" && reference > 0) {
        const ratio = twin / reference;
        const at = `${row[0]}/${letters[c] ?? "?"}`;
        const held = byImage.get(row[1]) ?? {
          least: Number.POSITIVE_INFINITY,
          leastAt: "",
          most: 0,
          mostAt: "",
        };
        byImage.set(row[1], {
          least: ratio < held.least ? ratio : held.least,
          leastAt: ratio < held.least ? at : held.leastAt,
          most: ratio > held.most ? ratio : held.most,
          mostAt: ratio > held.most ? at : held.mostAt,
        });
      }
    }
  }
  return [...byImage].map(([image, held]) => [
    image,
    figure(held.least),
    held.leastAt,
    figure(held.most),
    held.mostAt,
  ]);
}

/** The twin's radiance and drawn-surface share of `referenceCase`'s own setting, as recorded. */
function caseValuesOf(
  twin: ReturnType<typeof caseTwin>,
  referenceCase: ReferenceCase,
): Map<string, readonly [number, number, number]> {
  const values = new Map<string, readonly [number, number, number]>();
  for (const spec of twinValueSpecs(CASES)) {
    if (
      spec.caseName === referenceCase.name &&
      spec.setting === "case" &&
      (spec.quantity === "radiance" || spec.quantity === "surfaceShare")
    ) {
      values.set(twinValueKey(spec), recordedRgb(twinValueOf(twin, referenceCase, spec)));
    }
  }
  return values;
}

/** A case baked and its twin read through the table (the module's remarks). */
function runCase({ referenceCase }: GateCase): CaseRun {
  const bands = bakeOf(referenceCase);
  const packed = packThickTable(bands, kernelMedium(referenceCase.medium));
  const values = caseValuesOf(caseTwin(referenceCase, { thick: bands }), referenceCase);
  return {
    values,
    bake: {
      bands: bands.length,
      slots: bands[0]?.slots.length ?? 0,
      solves: bands.reduce((sum, band) => sum + (band.slots[0]?.solves ?? 0), 0),
      doubleForwardShare: figure(
        Math.max(...bands.map((band) => band.slots[0]?.doubleForwardShare ?? 0)),
      ),
      halfFloats: halfFloatsOf(bands, packed),
    },
  };
}

/**
 * R08.T6.c's Venus-class analytic table at its largest 1 ÷ mean(escape), over every texel and over
 * those whose sun is above the horizontal, where the bake holds the sun (μ₀ > 0), beside the
 * bake's J⁰ there averaged over the travel cosines; and the bake's half floats.
 */
function runVenus(): Record<string, unknown> {
  const { medium, radiusM } = venusClassMedium();
  const column = venusClassColumn();
  const sources: ReadonlyArray<SpectralSource> = [
    {
      kind: "molecular",
      fractions: [{ species: "CO2", moleFraction: 1 }],
      numberDensityPerM3: column.surfaceNumberDensityPerM3,
    },
    ...medium.terms.slice(1).map(channelSource),
  ];
  const bands = bakeKeptCase(
    { medium, sources },
    keptBakeLightsOf(medium, [R06_SOLAR_BAKE_SPECTRUM]),
    sphereBakeBody(radiusM, column.referenceGravityMS2),
  );
  const drawn = kernelMedium(medium);
  const packed = packThickTable(bands, drawn);
  const planet = planetTwin(drawn, radiusM);
  const n = planet.multiScattering.widthTexels;
  const texels = Array.from({ length: n * n }, (_, i) => [i % n, Math.floor(i / n)] as const);
  const series = multiScatteringSeriesTwin(drawn, radiusM, planet, texels);
  if (!series.every((one) => one.escape.every((e) => Number.isFinite(e)))) {
    throw new Error("the Venus-class table's escape is not finite");
  }
  const largestOf = (daylit: boolean): Record<string, unknown> => {
    let best = { texel: [0, 0] as readonly [number, number], channel: 0, inverse: 0 };
    for (const [k, one] of series.entries()) {
      const texel = texels[k] ?? [0, 0];
      if (daylit && !((2 * texel[0]) / (n - 1) - 1 > 0)) {
        continue;
      }
      for (let c = 0; c < 3; c += 1) {
        const escape = one.escape[c] ?? Number.NaN;
        if (escape > 0 && 1 / escape > best.inverse) {
          best = { texel, channel: c, inverse: 1 / escape };
        }
      }
    }
    const [x, y] = best.texel;
    const muSun = (2 * x) / (n - 1) - 1;
    const heightM = 10 + (y / (n - 1)) * (medium.topHeightM - 10);
    const psi = planet.multiScattering.texels[(y * n + x) * 4 + best.channel] ?? Number.NaN;
    let meanJ: number | null = null;
    if (muSun > 0) {
      const row = thickRowAt(packed, heightM);
      let sum = 0;
      for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
        const scratch = thickSourceScratch();
        addThickLayer(packed, 0, row, muSun, (v + 0.5) / 8 - 1, 1, scratch);
        sum += scratch.level[best.channel] ?? Number.NaN;
      }
      meanJ = sum / THICK_TRAVEL_COSINES;
    }
    return {
      texel: [x, y],
      channel: best.channel,
      heightM: figure(heightM),
      muSun: figure(muSun),
      inverseMeanEscape: figure(best.inverse),
      psiMsPerSr: figure(psi),
      bakeMeanJ0PerSr: meanJ === null ? null : figure(meanJ),
    };
  };
  return {
    everyTexel: largestOf(false),
    sunAboveTheHorizontal: largestOf(true),
    halfFloats: halfFloatsOf(bands, packed),
  };
}

/** The control on `earth`: c1's analytic twin at {@link FINE_TWIN}'s sizes, gated as c1 is. */
function runEarthControl(): Record<string, unknown> {
  const values = new Map(C1_VALUES);
  for (const [key, value] of caseValuesOf(caseTwin(EARTH_CASE, FINE_TWIN), EARTH_CASE)) {
    values.set(key, value);
  }
  if (blockingValues(values).length > 0) {
    throw new Error(`the control's twin gives values not finite or negative`);
  }
  const record = caseGateRecord(CASES, values, {
    writer: "thickRuns.test.ts, through caseGate.ts (plan R08, R08.T14.c's control)",
    radiance: "scalar",
  });
  return {
    twin: "c1's analytic twin, Psi_ms 128 x 32 (four times its mu_s), the sky view 192 x 432 (four times its rows)",
    fine: imagesOf(record, EARTH_CASE.name),
    fineRatios: ratiosOf(record, EARTH_CASE.name),
  };
}

/**
 * The control on the (1, 0) points at τ 1/8 and ¼: each point's failing geometries with the
 * analytic twin at its kernels' sizes and at {@link FINE_TWIN}'s, by `sweepBoundary.ts`'s
 * `recomputePoint` against the committed traces.
 */
function runSweepControl(): unknown[] {
  const file = SWEEP_FILES.find((f) => f.name === "ms-sweep-w1000");
  const output = SWEEP_OUTPUTS["./ms-sweep-w1000.json"];
  const caseJson = SWEEP_CASES["./ms-sweep-w1000.case.json"];
  if (file === undefined || output === undefined || caseJson === undefined) {
    throw new Error("the (1, 0) column's sweep files are committed");
  }
  const sweepCase = readSweepCase(caseJson, "ms-sweep-w1000.case.json");
  const sweepOutput = readSweepOutput(output, "ms-sweep-w1000.json");
  const column = file.asymmetries.indexOf(0);
  const geometries = new Map(sweepGeometries().map((g) => [g.name, g]));
  return [1 / 8, 1 / 4].map((tau) => {
    const candidate = SWEEP_TAU_CANDIDATES.findIndex((c) => Math.abs(c.tau - tau) < 1e-12);
    const pointIndex = sweepCase.columns[column]?.points[candidate];
    const point = sweepOutput.points.find((p) => p.index === pointIndex);
    if (candidate < 0 || point?.traced === null || point === undefined) {
      throw new Error(`the (1, 0) column has no traced point at τ ${tau}`);
    }
    const traced = point.traced;
    const medium = sweepMedium(sweepPointOf(file, column, walkSpot(candidate)));
    const failingOf = (options: Partial<TwinOptions>): ReadonlyArray<string> => {
      const twin = referenceTwin(medium, SWEEP_FIGURE, options);
      const row = sweepCase.geometries.map((g) => {
        const geometry = geometries.get(g.name);
        if (geometry === undefined) {
          throw new Error(`the sweep has no geometry ${g.name}`);
        }
        return twin.radiance(geometry)[1];
      });
      const columns = sweepCase.columns.map((c, k) =>
        k === column ? { ...c, twin: c.twin.map((r, i) => (i === candidate ? row : r)) } : c,
      );
      const recomputed = recomputePoint(
        { ...sweepCase, columns },
        "ms-sweep-w1000",
        column,
        { kind: "walk", candidate },
        point,
        traced,
      );
      return recomputed.failing.map((g) => sweepCase.geometries[g]?.name ?? `#${g}`);
    };
    return {
      tau,
      comparisons: sweepCase.geometries.length,
      kernels: failingOf({}),
      fine: failingOf(FINE_TWIN),
    };
  });
}

describe.skipIf(!RUN)("R08.T14.c's recorded runs (by hand)", () => {
  it(
    "bakes the four cases, gates their twins through the tables beside c1's, records R08.T6.c's Venus-class table and the control, and writes thickRuns.json",
    { timeout: LIMIT_MS.whole },
    async () => {
      const caseRuns = new Map(
        CASES.map((gateCase) => [gateCase.referenceCase.name, runCase(gateCase)]),
      );
      const values = new Map(C1_VALUES);
      for (const { values: thick } of caseRuns.values()) {
        for (const [key, value] of thick) {
          values.set(key, value);
        }
      }
      expect(blockingValues(values)).toEqual([]);
      const thickRecord = caseGateRecord(CASES, values, {
        writer: "thickRuns.test.ts, through caseGate.ts (plan R08, R08.T14.c)",
        radiance: "scalar",
      });
      const record = {
        description:
          "R08.T14.c's recorded runs: R08.T13.c1's scalar case gate with each case's twin reading its thick bake in place of Psi_ms from the table's first sun cosine cell up, beside c1's (caseGate.json), against the same committed references; the bakes' half floats; R08.T6.c's Venus-class analytic table at its largest 1 / mean(escape); the control at four times Psi_ms's mu_s and the sky view's rows. Recorded, never gated. Written by thickRuns.test.ts; do not edit by hand.",
        format: THICK_RUNS_FORMAT,
        cases: Object.fromEntries(
          CASES.map(({ referenceCase: { name } }) => [
            name,
            {
              bake: caseRuns.get(name)?.bake ?? null,
              thick: imagesOf(thickRecord, name),
              c1: imagesOf(committedGate, name),
              thickRatios: ratiosOf(thickRecord, name),
              c1Ratios: ratiosOf(committedGate, name),
            },
          ]),
        ),
        venusClass: runVenus(),
        control: { earth: runEarthControl(), sweep: runSweepControl() },
      };
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );
});

describe("R08.T14.c's recorded runs' record", () => {
  it("is committed, in its format or as its placeholder", () => {
    const format: unknown = Reflect.get(committed, "format");
    expect(format === undefined || format === THICK_RUNS_FORMAT).toBe(true);
  });
});
