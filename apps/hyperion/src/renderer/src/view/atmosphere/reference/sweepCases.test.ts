/**
 * R08.T13's sweep cases (plan R08, R08.T13.a; `decision-r08-t13-scope.md`, ruling 8): each
 * `ms-sweep-w<ω×1000>.case.json` is the sweep the client's writer and twin write now, through
 * `bless.ts`, and each `ms-sweep-` file, case or output, stays under the 450 kB split line.
 *
 * @remarks
 * Under a bless (`HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/sweepCases.test.ts`) or by hand under
 * `HYPERION_SLOW_TESTS=1`, every file is computed whole, the twin at all 1,100 points, the walk's
 * 750 and the floor scan's 350. Otherwise each file is checked byte for byte but for the twin's
 * values, which it takes from the committed file, and the twin is recomputed at {@link SAMPLE} and
 * held equal to them (ruling 8; main's reading 5).
 */

import { describe, expect, it } from "vitest";

import smokeCommitted from "../../../../../../../../crates/hyperion-fit/src/atmosphere/sweep_smoke.case.json" with { type: "json" };
import { blessMode, expectCommittedTable } from "../../../test/bless";
import {
  smokeSourceSpec,
  smokeSweepFile,
  smokeSweepFileComputed,
  SWEEP_AGGREGATE,
  SWEEP_ASYMMETRIES,
  SWEEP_FILES,
  SWEEP_SCAN_GROUND_ALBEDOS,
  SWEEP_TAU_CANDIDATES,
  SWEEP_TAU_COUNT,
  type SweepFileSpec,
  sweepFileJson,
  sweepGeometries,
  sweepPhase,
  sweepPointOf,
  type SweepSpot,
  sweepTwinColumn,
  scanSpot,
  walkSpot,
} from "../../../test/atmosphereSweep";
import { phaseTableAsymmetry } from "../aerosol";

/** The smoke's file, from this test. */
const SMOKE_FILE =
  "../../../../../../../../crates/hyperion-fit/src/atmosphere/sweep_smoke.case.json";

/** The committed sweep cases, by file. */
const COMMITTED = import.meta.glob<unknown>("./ms-sweep-*.case.json", {
  import: "default",
  eager: true,
});

/** Every `ms-sweep-` file, case or output, as its text. */
const SWEEP_TEXTS = import.meta.glob<string>("./ms-sweep-*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/**
 * The bytes a sweep file stays under: 450,000, the split line under the 500 kB hook (ruling 8): a
 * file that would pass it is split by halves of g.
 */
const SWEEP_SPLIT_BYTES = 450_000;

/**
 * Whether the files are checked whole by hand: `HYPERION_SLOW_TESTS=1 pnpm --filter hyperion exec
 * vitest run src/renderer/src/view/atmosphere/reference/sweepCases.test.ts`.
 */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

/** Whether every file is written or checked whole: under a bless, or by hand. */
const WHOLE = blessMode() === "bless" || slowTests();

/** The τ candidate of τ 1. */
const TAU_ONE = SWEEP_TAU_CANDIDATES.findIndex((c) => c.tau === 1);

/** One point of the default compare's sample: a file, a column's g and a spot, named. */
interface SampledPoint {
  readonly file: string;
  readonly asymmetry: number;
  readonly spot: SweepSpot;
  readonly label: string;
}

/**
 * The default compare's sample, four points (ruling 8, as "#107's twin ground" item 3 cuts it):
 * τ 1 of the ω 1 file's g 0 and g 0.75 columns and of the bright column, and the ω 1 file's g 0.75
 * floor over the scan's black ground, where the twin draws a ground of albedo 0.
 */
const SAMPLE: ReadonlyArray<SampledPoint> = [
  { file: "ms-sweep-w1000", asymmetry: 0, spot: walkSpot(TAU_ONE), label: "τ 1" },
  { file: "ms-sweep-w1000", asymmetry: 0.75, spot: walkSpot(TAU_ONE), label: "τ 1" },
  { file: "ms-sweep-w1000-a800", asymmetry: 0, spot: walkSpot(TAU_ONE), label: "τ 1" },
  { file: "ms-sweep-w1000", asymmetry: 0.75, spot: scanSpot(0), label: "the black floor" },
];

/** A file's path from this test. */
function pathOf(spec: SweepFileSpec): string {
  return `./${spec.name}.case.json`;
}

/** Whether a value is rows of numbers. */
function isRows(rows: unknown): rows is number[][] {
  return (
    Array.isArray(rows) &&
    rows.every(
      (row: unknown) =>
        Array.isArray(row) && row.every((value: unknown) => typeof value === "number"),
    )
  );
}

/**
 * A committed file's twin values of one kind, by column and τ candidate (`twin`) or scan ground
 * (`scanTwin`), or `undefined` for a placeholder.
 */
function committedTwin(
  committed: unknown,
  key: "twin" | "scanTwin",
): ReadonlyArray<ReadonlyArray<ReadonlyArray<number>>> | undefined {
  if (typeof committed !== "object" || committed === null || !("columns" in committed)) {
    return undefined;
  }
  const { columns } = committed;
  if (!Array.isArray(columns)) {
    return undefined;
  }
  const columnTwins: unknown[] = columns.map((column: unknown) => {
    if (typeof column !== "object" || column === null) {
      return undefined;
    }
    const entries: Readonly<Record<string, unknown>> = Object.fromEntries(Object.entries(column));
    return entries[key];
  });
  if (!columnTwins.every(isRows)) {
    return undefined;
  }
  return columnTwins;
}

/** A committed file's twin at a column and spot. */
function committedAt(spec: SweepFileSpec, column: number, spot: SweepSpot): ReadonlyArray<number> {
  let row: ReadonlyArray<number> | undefined;
  switch (spot.kind) {
    case "walk":
      row = committedTwin(COMMITTED[pathOf(spec)], "twin")?.[column]?.[spot.candidateIndex];
      break;
    case "scan":
      row = committedTwin(COMMITTED[pathOf(spec)], "scanTwin")?.[column]?.[spot.groundIndex];
      break;
  }
  if (row === undefined) {
    throw new Error(
      `${spec.name}.case.json holds no twin at column ${column}, ${JSON.stringify(spot)}; bless it`,
    );
  }
  return row;
}

const FILES = SWEEP_FILES.map((spec) => [spec.name, spec] as const);

/**
 * The limits of the tests, ms (`vitest.config.mts`'s `testTimeout`), from their times on a quiet
 * laptop (2026-10-26): a file whole, its twin at up to 154 points, the walk's and the scan's, took
 * up to 1,058 s on one of four processes beside each other, so 4 h; a file by its committed twin,
 * under 30 ms, so 30 s; a sampled point's twin, a twin's tables and 54 geometries' cones,
 * 3.5–5.5 s, CPU-bound, so ten times over and rounded up to 60 s; the smoke whole, its twin at 24
 * points, 145 s, so 20 min.
 */
const LIMIT_MS = {
  whole: 14_400_000,
  committed: 30_000,
  sampled: 60_000,
  smokeWhole: 1_200_000,
} as const;

describe("the sweep cases", () => {
  it.each(FILES)(
    "%s.case.json is the sweep the client writes now, whole under a bless or by hand",
    { timeout: WHOLE ? LIMIT_MS.whole : LIMIT_MS.committed },
    async (_, spec) => {
      const table = sweepFileJson(spec, (column, spot) =>
        WHOLE ? sweepTwinColumn(spec, column, spot) : committedAt(spec, column, spot),
      );
      await expect(
        expectCommittedTable(table, COMMITTED[pathOf(spec)], pathOf(spec)),
      ).resolves.toBeUndefined();
    },
  );

  const sampled = SAMPLE.map((point) => {
    const spec = SWEEP_FILES.find((f) => f.name === point.file);
    const column = spec?.asymmetries.indexOf(point.asymmetry) ?? -1;
    if (spec === undefined || column < 0) {
      throw new Error(`the sample's ${point.file} has no column of g ${point.asymmetry}`);
    }
    return [point.file, point.asymmetry, point.label, spec, column, point.spot] as const;
  });

  it.skipIf(WHOLE).each(sampled)(
    "%s's twin at g %s, %s, is the committed one",
    { timeout: LIMIT_MS.sampled },
    (_, __, ___, spec, column, spot) => {
      expect(sweepTwinColumn(spec, column, spot)).toEqual(committedAt(spec, column, spot));
    },
  );

  it.each(Object.keys(SWEEP_TEXTS))("keeps %s under the 450 kB split line", (file) => {
    expect(new TextEncoder().encode(SWEEP_TEXTS[file]).length).toBeLessThan(SWEEP_SPLIT_BYTES);
  });

  it("lists the eight case files", () => {
    expect(Object.keys(COMMITTED).toSorted()).toEqual(SWEEP_FILES.map(pathOf).toSorted());
  });

  it(
    "writes sweep_smoke.case.json, ms-sweep-w900's cut for the fit crate's smoke",
    { timeout: WHOLE ? LIMIT_MS.smokeWhole : LIMIT_MS.committed },
    async () => {
      const spec = smokeSourceSpec();
      const table = WHOLE
        ? smokeSweepFileComputed()
        : smokeSweepFile(sweepFileJson(spec, (column, spot) => committedAt(spec, column, spot)));
      await expect(
        expectCommittedTable(table, smokeCommitted, SMOKE_FILE),
      ).resolves.toBeUndefined();
    },
  );
});

describe("the sweep's grid", () => {
  it("gives each point an index of its own over the eight files, the scan's after the walk's", () => {
    const walk = SWEEP_FILES.flatMap((spec) =>
      spec.firstIndices.flatMap((first) =>
        Array.from({ length: SWEEP_TAU_COUNT }, (__, k) => first + k),
      ),
    );
    const scan = SWEEP_FILES.flatMap((spec) =>
      spec.scanFirstIndices.flatMap((first) => SWEEP_SCAN_GROUND_ALBEDOS.map((__, a) => first + a)),
    );
    expect(new Set([...walk, ...scan]).size).toBe(1_100);
    expect([Math.min(...walk), Math.max(...walk)]).toEqual([0, 749]);
    expect([Math.min(...scan), Math.max(...scan)]).toEqual([750, 1_099]);
  });

  it("holds the ruling's seven scan grounds, from the black", () => {
    expect(SWEEP_SCAN_GROUND_ALBEDOS).toEqual([0, 0.02, 0.05, 0.1, 0.2, 0.5, 0.8]);
  });

  it("puts a scan point at its column's floor τ over its own ground", () => {
    const spec = SWEEP_FILES[3];
    if (spec === undefined) {
      throw new Error("the sweep has eight files");
    }
    const points = SWEEP_SCAN_GROUND_ALBEDOS.map((_, a) => sweepPointOf(spec, 4, scanSpot(a)));
    expect(points).toEqual(
      SWEEP_SCAN_GROUND_ALBEDOS.map((groundAlbedo) => ({
        singleScatteringAlbedo: 0.9,
        asymmetry: 0.75,
        tau: 1 / 32,
        groundAlbedo,
      })),
    );
  });

  it("keeps a walk point over its file's ground", () => {
    const spec = SWEEP_FILES[3];
    if (spec === undefined) {
      throw new Error("the sweep has eight files");
    }
    expect(sweepPointOf(spec, 4, walkSpot(0)).groundAlbedo).toBe(spec.groundAlbedo);
  });

  it("refuses a scan ground past the ruling's seven", () => {
    const spec = SWEEP_FILES[0];
    if (spec === undefined) {
      throw new Error("the sweep has eight files");
    }
    expect(() => sweepPointOf(spec, 0, scanSpot(SWEEP_SCAN_GROUND_ALBEDOS.length))).toThrow(
      RangeError,
    );
  });

  it("puts the floor, then the half-octave ladder from 2^−3.5 to 8, in each column", () => {
    expect(SWEEP_TAU_CANDIDATES.map((c) => [c.tau, c.role])).toEqual([
      [1 / 32, "floor"],
      ...Array.from({ length: 14 }, (_, i) => [
        expect.closeTo(2 ** ((i - 7) / 2), 14),
        i % 2 === 1 ? "octave" : "half-octave",
      ]),
    ]);
  });

  it("holds ruling 4's 55 a point: 41 ground views, 13 from orbit and the path, and the fluxes", () => {
    const geometries = sweepGeometries();
    expect(geometries).toHaveLength(54);
    expect(geometries.filter((g) => g.kind === "ground")).toHaveLength(41);
    expect(SWEEP_AGGREGATE.sun.zenithDeg).toBe(0);
  });

  it("gives each Henyey–Greenstein table its g as its mean cosine within 10⁻⁴", () => {
    const errors = SWEEP_ASYMMETRIES.filter((g) => g > 0).map((g) => {
      const phase = sweepPhase(g);
      if (phase.kind !== "tabulated") {
        throw new Error(`g ${g} takes a table`);
      }
      return Math.abs(phaseTableAsymmetry(phase.table, 1) - g);
    });
    expect(Math.max(...errors)).toBeLessThanOrEqual(1e-4);
  });
});
