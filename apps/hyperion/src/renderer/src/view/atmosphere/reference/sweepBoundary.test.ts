/**
 * R08.T13.c2's recompute of the sweep (plan R08, R08.T13; `decision-r08-t13-scope.md`, rulings 5, 6
 * and 9, "#107's twin ground", "#107's floor misses" and "#112's floor refusals"):
 * `THICK_MS_BOUNDARY` is the boundary `gate.ts` itself sets from the eight committed outputs, and
 * the record `sweepBoundary.json` is the recompute, the attribution and the classes computed now,
 * through `bless.ts`. Every verdict and class is recorded, never asserted to pass or to be `thin`.
 *
 * @remarks
 * Under a bless (`HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/sweepBoundary.test.ts`) or by hand under
 * `HYPERION_SLOW_TESTS=1`, every twin value is computed: the drawn surface's part at each drift
 * cell's first failing geometries, and the surface and the zeroed multiple scattering at every
 * failing geometry of the raises' points, one twin of each sweep point drawn and one zeroed.
 * Otherwise the record is computed from the twin values it holds and checked byte for byte, and
 * {@link SAMPLE}'s values are recomputed and held equal to them. The recompute itself needs no
 * twin and runs every time.
 */

import { describe, expect, it } from "vitest";

import { titanClassMethaneMedium, venusClassMedium } from "../../../test/atmosphereFixtures";
import {
  detailOf,
  recordInput,
  venus58Detail,
  venus92Detail,
} from "../../../test/atmosphereRecords";
import {
  SWEEP_FIGURE,
  SWEEP_FILES,
  sweepGeometries,
  sweepMedium,
  sweepPointOf,
  walkSpot,
} from "../../../test/atmosphereSweep";
import { blessMode, expectCommittedTable } from "../../../test/bless";
import { earthDetail } from "../../../test/planetaryFixture";
import { assembleMedium } from "../assemble";
import { hydrostaticColumn } from "../column";
import { EARTH_DRY_AIR, EARTH_REFERENCE, EARTH_SLAB_REFERENCE } from "../earth";
import type { AtmosphereMedium } from "../medium";
import { molecularTerm } from "../rayleigh";
import {
  CLEAR_COVER,
  regimeChannels,
  THICK_MS_BOUNDARY,
  verticalOpticalDepth,
} from "../thick/regime";
import caseGateCommitted from "./caseGate.json" with { type: "json" };
import { MARS_REFERENCE, SATURN_OBLATE_REFERENCE } from "./cases";
import committed from "./sweepBoundary.json" with { type: "json" };
import {
  attributionOf,
  blockingSweepTwinValues,
  boundaryOfRecompute,
  caseGateSummary,
  type ClassifiedMedium,
  driftAttributions,
  gridCells,
  missingSweepTwinValues,
  raisePoints,
  readSweepCase,
  readSweepOutput,
  recomputeSweep,
  recordedTwinValue,
  sortSweepFloor,
  type SweepFilePair,
  type SweepOutcome,
  sweepBoundaryRecord,
  type SweepTauRole,
  sweepTwinKey,
  type SweepTwinSpec,
  sweepTwinSpecs,
  type SweepTwinValues,
  sweepTwinValuesOfRecord,
  walkSweepColumn,
} from "./sweepBoundary";
import type { ReferenceGeometry } from "./geometries";
import { referenceTwin, type ReferenceTwin, type TwinMultipleScattering } from "./twin";

/** The committed outputs, by file. */
const OUTPUTS = import.meta.glob<unknown>(["./ms-sweep-w*.json", "!./ms-sweep-w*.case.json"], {
  import: "default",
  eager: true,
});

/** The committed cases, by file. */
const CASES = import.meta.glob<unknown>("./ms-sweep-w*.case.json", {
  import: "default",
  eager: true,
});

/** The record's file, from this test. */
const RECORD_FILE = "./sweepBoundary.json";

/** The record's text, for its size. */
const RECORD_TEXTS = import.meta.glob<string>("./sweepBoundary.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The bytes the record stays under: 450,000, the split line under the 500 kB hook. */
const RECORD_SPLIT_BYTES = 450_000;

/**
 * Whether the record is computed whole by hand: `HYPERION_SLOW_TESTS=1 pnpm --filter hyperion exec
 * vitest run src/renderer/src/view/atmosphere/reference/sweepBoundary.test.ts`.
 */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

/** Whether every twin value is computed: under a bless, or by hand. */
const WHOLE = blessMode() === "bless" || slowTests();

/**
 * The limits of the tests, ms (`vitest.config.mts`'s `testTimeout`): the whole record, some 90 of
 * the sweep's twins at 4–9 s each, so an hour; the record from its own values, its recompute a
 * second or two, so 120 s; the sample, two twins, so 300 s.
 */
const LIMIT_MS = { whole: 3_600_000, recorded: 120_000, sampled: 300_000 } as const;

/** The record's head. */
const HEAD = {
  writer: "sweepBoundary.test.ts, through sweepBoundary.ts (plan R08, R08.T13.c2)",
  twin: "reference/twin.ts's referenceTwin of test/atmosphereSweep.ts's sweepMedium on SWEEP_FIGURE at TABLE_SIZES.high, the sweep's channel 1: drawn (multipleScattering drawn) for the surface's part, and zeroed for the multiple scattering's",
} as const;

/** The command that rewrites the record. */
const BLESS_COMMAND =
  "HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run src/renderer/src/view/atmosphere/reference/sweepBoundary.test.ts";

/** The eight files, in `SWEEP_FILES`' order, each output with its case. */
const FILES: ReadonlyArray<SweepFilePair> = SWEEP_FILES.map((spec) => {
  const output = OUTPUTS[`./${spec.name}.json`];
  const sweepCase = CASES[`./${spec.name}.case.json`];
  if (output === undefined || sweepCase === undefined) {
    throw new Error(`${spec.name} has no committed output or case`);
  }
  return {
    name: spec.name,
    output: readSweepOutput(output, `${spec.name}.json`),
    sweepCase: readSweepCase(sweepCase, `${spec.name}.case.json`),
  };
});

/** The recompute, once a test file. */
const RECOMPUTE = recomputeSweep(FILES);

/** The committed record's twin values, read once. */
const RECORDED_VALUES = sweepTwinValuesOfRecord(committed);

/**
 * The default compare's sample: the zeroed multiple scattering at (1, 0)'s τ 1/8 drift view under
 * the sun at 80°, and the drawn surface's part at (1, 0.5)'s first geometry that is not a floor
 * miss, its limb at 0.3 H, whose cone meets the ground (share 0.48). Two twins.
 */
const SAMPLE: ReadonlyArray<SweepTwinSpec> = [
  {
    quantity: "zeroed",
    file: "ms-sweep-w1000",
    column: 0,
    candidate: 2,
    geometry: "ground-s80-v75-a90",
  },
  { quantity: "surface", file: "ms-sweep-w1000", column: 2, candidate: 1, geometry: "limb-0.3H" },
];

/** The sweep's geometries, by name. */
const GEOMETRIES = new Map(sweepGeometries().map((g) => [g.name, g]));

/** A geometry's name in a file's case, by index. */
function geometryName(file: string, g: number): string {
  const name = FILES.find((f) => f.name === file)?.sweepCase.geometries[g]?.name;
  if (name === undefined) {
    throw new Error(`${file} has no geometry ${g}`);
  }
  return name;
}

/** The twin a quantity is read from: drawn for the surface's part, zeroed for the zeroed radiance. */
function multipleScatteringOf(quantity: SweepTwinSpec["quantity"]): TwinMultipleScattering {
  let setting: TwinMultipleScattering;
  switch (quantity) {
    case "surface":
      setting = "drawn";
      break;
    case "zeroed":
      setting = "zeroed";
      break;
  }
  return setting;
}

/** A twin's value of one quantity at a geometry, in the sweep's channel. */
function quantityOf(
  twin: ReferenceTwin,
  quantity: SweepTwinSpec["quantity"],
  geometry: ReferenceGeometry,
): number {
  let value: number;
  switch (quantity) {
    case "surface":
      value = twin.surfaceRadiance(geometry)[1];
      break;
    case "zeroed":
      value = twin.radiance(geometry)[1];
      break;
  }
  return value;
}

/**
 * The twin's values of `specs`, each at seven significant figures: one twin a sweep point, drawn
 * for the surface's part and zeroed for the multiple scattering's, built and read in turn.
 */
function computeTwinValues(specs: ReadonlyArray<SweepTwinSpec>): Map<string, number> {
  const groups = new Map<string, SweepTwinSpec[]>();
  for (const spec of specs) {
    const key = `${spec.file}/${spec.column}/${spec.candidate}/${spec.quantity}`;
    groups.set(key, [...(groups.get(key) ?? []), spec]);
  }
  const values = new Map<string, number>();
  for (const group of groups.values()) {
    const [first] = group;
    const file = SWEEP_FILES.find((f) => f.name === first?.file);
    if (first === undefined || file === undefined) {
      throw new Error(`no sweep file is named ${first?.file ?? "(none)"}`);
    }
    const medium = sweepMedium(sweepPointOf(file, first.column, walkSpot(first.candidate)));
    const twin = referenceTwin(medium, SWEEP_FIGURE, {
      multipleScattering: multipleScatteringOf(first.quantity),
    });
    for (const spec of group) {
      const geometry = GEOMETRIES.get(spec.geometry);
      if (geometry === undefined) {
        throw new Error(`the sweep has no geometry ${spec.geometry}`);
      }
      values.set(sweepTwinKey(spec), recordedTwinValue(quantityOf(twin, spec.quantity, geometry)));
    }
  }
  return values;
}

/** The twin values of a run: every one computed under a bless or by hand, else the record's. */
function runValues(): SweepTwinValues {
  if (!WHOLE) {
    if (RECORDED_VALUES.size === 0) {
      throw new Error(`sweepBoundary.json holds no twin values: bless it (${BLESS_COMMAND})`);
    }
    return RECORDED_VALUES;
  }
  const values = computeTwinValues(sweepTwinSpecs(RECOMPUTE));
  // A blocker stops a bless before it writes (`bless.ts`: such a check runs before the write).
  const blocking = blockingSweepTwinValues(values);
  if (blocking.length > 0) {
    throw new Error(`the twin gives values not finite or negative: ${blocking.join(", ")}`);
  }
  return values;
}

/**
 * An Earth-class column of dry air at 288.15 K and `surfacePa` under its molecular term alone, as
 * `assembleThick.test.ts` builds it: the U.S. Standard Atmosphere 1976's M₀, 28.9644 g mol⁻¹, and
 * g₀, 9.80665 m s⁻².
 */
function earthClassAir(surfacePa: number): AtmosphereMedium {
  const column = hydrostaticColumn({
    surfacePa,
    temperature: { kind: "isothermal", temperatureK: 288.15 },
    meanMolarMassGPerMol: 28.9644,
    referenceGravityMS2: 9.806_65,
    referenceRadiusM: 6_371_000,
  });
  return {
    name: "Earth-class air",
    topHeightM: column.topHeightM,
    groundAlbedo: [0.1, 0.1, 0.1],
    terms: [molecularTerm(column, EARTH_DRY_AIR).term],
  };
}

/** A record's assembled medium, or a thrown error for one that draws none. */
function assembledMedium(detail: Parameters<typeof recordInput>[0]): AtmosphereMedium {
  const { medium } = assembleMedium(recordInput(detail));
  if (medium === undefined) {
    throw new Error("the record draws no medium");
  }
  return medium;
}

/** The kept bodies, each as traced: a clear sky over its ground. */
const BODIES: ReadonlyArray<ClassifiedMedium> = [
  { name: "earth", medium: EARTH_REFERENCE, cover: CLEAR_COVER },
  { name: "earth-ozone-slab", medium: EARTH_SLAB_REFERENCE, cover: CLEAR_COVER },
  { name: "mars", medium: MARS_REFERENCE, cover: CLEAR_COVER },
  { name: "saturn-oblate", medium: SATURN_OBLATE_REFERENCE, cover: CLEAR_COVER },
];

/** A medium classified under a clear sky over its ground. */
function clear(name: string, medium: AtmosphereMedium): ClassifiedMedium {
  return { name, medium, cover: CLEAR_COVER };
}

/** A traced point of the walk's tests: these geometries fail, these are refused (listed or not). */
function traced(failing: number[], refused: Array<[number, boolean]> = []): SweepOutcome {
  return {
    kind: "traced",
    failing,
    refused: refused.map(([geometry, listed]) => ({ geometry, refusal: "noisy", listed })),
  };
}

/** Every medium `assembleThick.test.ts` classifies, the cache's kept fixtures among them. */
function labelMedia(): ClassifiedMedium[] {
  const atOne = 101_325 / Math.max(...verticalOpticalDepth(earthClassAir(101_325)));
  return [
    ...BODIES.map((b) => ({ ...b, name: `${b.name} (reference case, kept fixture)` })),
    clear("Venus-class medium (fixture)", venusClassMedium().medium),
    clear("Titan-class methane column (fixture)", titanClassMethaneMedium().medium),
    clear("Venus at 92 bar (record, assembled)", assembledMedium(venus92Detail())),
    clear("Venus at 58 bar (record, assembled)", assembledMedium(venus58Detail())),
    clear("the generated Earth (record, assembled)", assembledMedium(detailOf(earthDetail()))),
    clear("Earth-class air at 1.001 of τ 1", earthClassAir(atOne * 1.001)),
    clear("Earth-class air at 0.999 of τ 1", earthClassAir(atOne * 0.999)),
  ];
}

describe("the sweep's recompute", () => {
  it("reads the eight files, 50 cells: the grid's 49 over the walk's ground and the bright column", () => {
    expect([FILES.length, RECOMPUTE.cells.length, gridCells(RECOMPUTE).length]).toEqual([
      8, 50, 49,
    ]);
  });

  it("holds THICK_MS_BOUNDARY equal to the boundary gate.ts sets from the files, cell by cell with its kind", () => {
    const boundary = boundaryOfRecompute(RECOMPUTE);
    expect({
      omega: Array.from(THICK_MS_BOUNDARY.omega),
      g: Array.from(THICK_MS_BOUNDARY.g),
      tauStar: Array.from(THICK_MS_BOUNDARY.tauStar),
      albedoStar: Array.from(THICK_MS_BOUNDARY.albedoStar),
      kind: [...THICK_MS_BOUNDARY.kind],
    }).toEqual({
      omega: Array.from(boundary.omega),
      g: Array.from(boundary.g),
      tauStar: Array.from(boundary.tauStar),
      albedoStar: Array.from(boundary.albedoStar),
      kind: [...boundary.kind],
    });
  });

  it("reads every traced point's τ, ω, g and ground as its column's", () => {
    const misplaced = RECOMPUTE.cells.flatMap((cell) => {
      const sweepCase = FILES.find((f) => f.name === cell.file)?.sweepCase;
      return cell.points.flatMap((p) => {
        const tau = p.spot.kind === "walk" ? sweepCase?.tau[p.spot.candidate] : sweepCase?.tau[0];
        const ground =
          p.spot.kind === "walk" ? cell.groundAlbedo : sweepCase?.scanGroundAlbedos[p.spot.ground];
        const agrees =
          tau !== undefined &&
          Math.abs(p.point.tau / tau - 1) < 1e-6 &&
          p.point.asymmetry === cell.g &&
          p.point.singleScatteringAlbedo === cell.omega &&
          p.point.groundAlbedo === ground;
        return agrees ? [] : [p.point.index];
      });
    });
    expect(misplaced).toEqual([]);
  });
});

describe("the sweep's walk, sweep.rs's rules", () => {
  /** The ladder: the floor, then the half-octaves from 2^(−3.5) to 8, the octaves at whole powers. */
  const roles: SweepTauRole[] = [
    "floor",
    ...Array.from({ length: 14 }, (_, i): SweepTauRole =>
      (i - 7) % 2 === 0 ? "octave" : "half-octave",
    ),
  ];
  const taus = [1 / 32, ...Array.from({ length: 14 }, (_, i) => 2 ** ((i - 7) / 2))];
  const walk = (
    outcome: (candidate: number) => SweepOutcome,
    past: Parameters<typeof walkSweepColumn>[2] = { kind: "no" },
  ) => walkSweepColumn(taus, roles, past, outcome);

  it("climbs the octaves to the top where every point passes", () => {
    const result = walk(() => traced([]));
    expect([result.kind, result.tauStar, result.visited.map((v) => v.candidate)]).toEqual([
      "top",
      8,
      [0, 2, 4, 6, 8, 10, 12, 14],
    ]);
  });

  it("takes the half-octave below a drift, and its τ* where it drifts too", () => {
    const result = walk((k) => traced(k >= 3 ? [5] : []));
    expect([result.kind, result.tauStar, result.visited.map((v) => v.candidate)]).toEqual([
      "drift",
      taus[3],
      [0, 2, 4, 3],
    ]);
  });

  it("lists a floor miss and sets no τ* by it", () => {
    const result = walk(() => traced([7]));
    expect([result.kind, [...result.floorMisses]]).toEqual(["top", [7]]);
  });

  it("stops as reach at a refusal that is not listed, at the last pass", () => {
    const result = walk((k) => traced([], k === 4 ? [[2, false]] : []));
    expect([result.kind, result.tauStar]).toEqual(["reach", taus[3]]);
  });

  it("lists a near-sun refusal at a VROOM point and walks on", () => {
    const result = walk((k) => traced([], k === 4 ? [[2, true]] : []));
    expect([result.kind, result.listed]).toEqual(["top", [{ candidate: 4, geometry: 2 }]]);
  });

  it("stops as reach at a point over its cap", () => {
    const result = walk((k): SweepOutcome => (k === 6 ? { kind: "overCap" } : traced([])));
    expect([result.kind, result.tauStar]).toEqual(["reach", taus[5]]);
  });

  it("climbs on through τ 1 past a drift for a must-have cell, its τ* the first drift's", () => {
    const result = walk((k) => traced(k >= 4 ? [9] : []), { kind: "through", tau: 1 });
    expect([result.tauStar, result.visited.map((v) => v.candidate)]).toEqual([
      taus[4],
      [0, 2, 4, 6, 8, 3],
    ]);
  });

  it("sorts the floor's misses into fixed, ground and unsorted, A* the least ground miss's albedo", () => {
    const black = traced([1], [[3, false]]);
    const sort = sortSweepFloor(black, [
      [0.05, traced([1, 2])],
      [0.2, traced([2, 3, 4])],
      [0.3, traced([4])],
    ]);
    expect([sort.fixed, [...sort.ground.entries()], sort.unsorted, sort.albedoStar]).toEqual([
      [1],
      [
        [2, 0.05],
        [4, 0.2],
      ],
      [3],
      0.05,
    ]);
  });

  it("finds A* 1 where no ground miss occurs", () => {
    expect(sortSweepFloor(traced([1]), [[0.8, traced([1])]]).albedoStar).toBe(1);
  });
});

describe("the attribution of a miss", () => {
  // A twin of 1, its single scattering alone 0.7: a multiple-scattering share of 0.3.
  it("calls a miss within the twin's multiple-scattering share the analytic term's", () => {
    expect(attributionOf(1, 1.2, 0.7)).toMatchObject({
      againstShare: "smaller",
      attribution: "analytic term",
    });
  });

  it("calls a too-bright miss past the share the kernels': the single scattering alone outshines the reference", () => {
    expect(attributionOf(1, 0.6, 0.7)).toMatchObject({
      againstShare: "larger",
      attribution: "kernels",
    });
  });

  it("leaves a too-dim miss past the share undetermined", () => {
    expect(attributionOf(1, 1.5, 0.7)).toMatchObject({
      againstShare: "larger",
      attribution: "undetermined",
    });
  });

  it("gives the share and the miss it compares, in the twin's units", () => {
    const { share, miss } = attributionOf(2, 1.5, 1.4);
    expect([share.toFixed(12), miss.toFixed(12)]).toEqual([(0.3).toFixed(12), (0.25).toFixed(12)]);
  });
});

describe("the sweep's reader", () => {
  it("names the path of a field of the wrong shape", () => {
    expect(() => readSweepOutput({ format: 1, kind: "sweep", case: 7 }, "bad.json")).toThrow(
      "bad.json.case is not a string",
    );
  });

  it("refuses an output that is not a sweep's", () => {
    expect(() => readSweepOutput({ format: 1, kind: "pilot" }, "pilot.json")).toThrow(
      "pilot.json is not a sweep output of format 1",
    );
  });
});

describe("the boundary's record", () => {
  it.skipIf(WHOLE)("holds every twin value the record reads", () => {
    expect(missingSweepTwinValues(RECORDED_VALUES, sweepTwinSpecs(RECOMPUTE))).toEqual([]);
  });

  it.skipIf(WHOLE)("holds no twin value that is not finite or is negative", () => {
    expect(blockingSweepTwinValues(RECORDED_VALUES)).toEqual([]);
  });

  it("reads every kept body's τ, ω̄, ḡ, τ* and A* finite and not negative", () => {
    const bad = BODIES.flatMap(({ name, medium }) =>
      regimeChannels(medium).flatMap((c, i) =>
        [
          c.opticalDepth,
          c.singleScatteringAlbedo,
          c.asymmetry,
          c.boundary.tauStar,
          c.boundary.albedoStar,
        ].every((x) => Number.isFinite(x) && x >= 0)
          ? []
          : [`${name}/${i}`],
      ),
    );
    expect(bad).toEqual([]);
  });

  it(
    "writes sweepBoundary.json, the recompute, the attribution and the classes, whole under a bless or by hand",
    { timeout: WHOLE ? LIMIT_MS.whole : LIMIT_MS.recorded },
    async () => {
      const record = sweepBoundaryRecord(RECOMPUTE, {
        twinValues: runValues(),
        bodies: BODIES,
        labelMedia: labelMedia(),
        caseGate: caseGateSummary(caseGateCommitted),
        head: HEAD,
      });
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );

  it("reads a surface share at every drift cell's first failing geometries", () => {
    const keys = new Set(sweepTwinSpecs(RECOMPUTE).map(sweepTwinKey));
    const wanted = driftAttributions(RECOMPUTE).flatMap((d) =>
      [d.firstFailing, d.firstNotFloorMiss].map((g) =>
        sweepTwinKey({
          quantity: "surface",
          file: d.cell.file,
          column: d.cell.column,
          candidate: d.cell.tauStarCandidate ?? -1,
          geometry: geometryName(d.cell.file, g),
        }),
      ),
    );
    expect([wanted.length, wanted.filter((key) => !keys.has(key))]).toEqual([100, []]);
  });

  it("reads a surface share and a zeroed radiance at every failing geometry of the raises' points", () => {
    const keys = new Set(sweepTwinSpecs(RECOMPUTE).map(sweepTwinKey));
    const wanted = raisePoints(RECOMPUTE).flatMap((r) =>
      r.point.failing.flatMap((g) =>
        (["surface", "zeroed"] as const).map((quantity) =>
          sweepTwinKey({
            quantity,
            file: r.cell.file,
            column: r.cell.column,
            candidate: r.candidate,
            geometry: geometryName(r.cell.file, g),
          }),
        ),
      ),
    );
    expect(wanted.filter((key) => !keys.has(key))).toEqual([]);
  });

  it("samples twin values the record reads", () => {
    const keys = new Set(sweepTwinSpecs(RECOMPUTE).map(sweepTwinKey));
    expect(SAMPLE.map((spec) => keys.has(sweepTwinKey(spec)))).toEqual([true, true]);
  });

  it.skipIf(WHOLE)(
    "recomputes the sampled twin values as recorded",
    { timeout: LIMIT_MS.sampled },
    () => {
      const keys = SAMPLE.map(sweepTwinKey);
      const recomputed = computeTwinValues(SAMPLE);
      expect(keys.map((key) => [key, recomputed.get(key)])).toEqual(
        keys.map((key) => [key, RECORDED_VALUES.get(key)]),
      );
    },
  );

  it("finds the record as one file", () => {
    expect(Object.keys(RECORD_TEXTS)).toEqual([RECORD_FILE]);
  });

  it("keeps sweepBoundary.json under the 450 kB split line", () => {
    const text = RECORD_TEXTS[RECORD_FILE] ?? "";
    expect(new TextEncoder().encode(text).length).toBeLessThan(RECORD_SPLIT_BYTES);
  });
});
