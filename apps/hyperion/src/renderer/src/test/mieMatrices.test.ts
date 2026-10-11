import { describe, expect, it } from "vitest";

import iprtA3Table from "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/iprt-a3.json" with { type: "json" };
import iprtA5Table from "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/iprt-a5.json" with { type: "json" };
import kokhanovskyAerosolTable from "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/kokhanovsky-aerosol.json" with { type: "json" };
import kokhanovskyCloudTable from "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/kokhanovsky-cloud.json" with { type: "json" };
import iprtA3Matrix from "../../../../../../crates/hyperion-fit/data/iprt_phase_a/a3_sphere_matrix.txt?raw";
import iprtA5Matrix from "../../../../../../crates/hyperion-fit/data/iprt_phase_a/a5_cloud_matrix.txt?raw";
import { blessMode, expectCommittedTable } from "./bless";
import {
  SPHERE_CASES,
  type SphereCase,
  cosinesOf,
  evenU,
  linearInUMoments,
  radiusNodes,
  sampledEntries,
  sphereCaseOptics,
  sphereMatrixFile,
  tableIntegral,
  tableValue,
} from "./mieMatrices";

/** A committed file's fields that the tests read, which its JSON import's type satisfies. */
interface CommittedFile {
  readonly nodes: number;
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
  readonly linearInUIntegral: number;
  readonly phase: {
    readonly u: readonly number[];
    readonly a1: readonly (readonly number[])[];
    readonly matrix: Readonly<
      Record<"a2" | "a3" | "a4" | "b1" | "b2", readonly (readonly number[])[]>
    >;
  };
}

/** The writer's committed files, by case, each with its path from this file. */
const COMMITTED: Readonly<
  Record<string, { readonly table: CommittedFile; readonly file: string }>
> = {
  "kokhanovsky-aerosol": {
    table: kokhanovskyAerosolTable,
    file: "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/kokhanovsky-aerosol.json",
  },
  "kokhanovsky-cloud": {
    table: kokhanovskyCloudTable,
    file: "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/kokhanovsky-cloud.json",
  },
  "iprt-a3": {
    table: iprtA3Table,
    file: "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/iprt-a3.json",
  },
  "iprt-a5": {
    table: iprtA5Table,
    file: "../../../../../../crates/hyperion-fit/src/atmosphere/sphere_matrices/iprt-a5.json",
  },
};

/**
 * Whether the files are checked whole by hand: `HYPERION_SLOW_TESTS=1 pnpm --filter hyperion exec
 * vitest run src/renderer/src/test/mieMatrices.test.ts` (no recipe sets it for this file).
 */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

/**
 * Whether every file is rewritten or checked whole: under a bless, or by hand ({@link slowTests}).
 * Otherwise only {@link ALWAYS_WHOLE} is, and the others are checked at every
 * {@link QUICK_STRIDE}-th entry, since the whole of the two Kokhanovsky cases takes minutes (59 s
 * and 148 s, measured 2026-10-10 at load 12–24).
 */
const WHOLE = blessMode() === "bless" || slowTests();

/** The case checked whole in every run, byte for byte through the writer: 69 radii, milliseconds. */
const ALWAYS_WHOLE = "iprt-a3";

/** The quick check's stride through each table's entries. */
const QUICK_STRIDE = 64;

/** The elements of a table, in the case format's names. */
const ELEMENTS = ["a1", "a2", "a3", "a4", "b1", "b2"] as const;

/** Whether a case is checked whole in this run. */
function checkedWhole(sphereCase: SphereCase): boolean {
  return WHOLE || sphereCase.name === ALWAYS_WHOLE;
}

function caseNamed(name: string): SphereCase {
  const found = SPHERE_CASES.find((c) => c.name === name);
  if (found === undefined) {
    throw new Error(`no sphere case is named ${name}`);
  }
  return found;
}

/** A case's committed file and its path, by the case's name. */
function committedOf(name: string): { readonly table: CommittedFile; readonly file: string } {
  const committed = COMMITTED[name];
  if (committed === undefined) {
    throw new Error(`no committed file is listed for ${name}`);
  }
  return committed;
}

/** A committed element's row, by the case format's name. */
function committedRow(file: CommittedFile, element: (typeof ELEMENTS)[number]): readonly number[] {
  const rows = element === "a1" ? file.phase.a1 : file.phase.matrix[element];
  const row = rows[0];
  if (row === undefined) {
    throw new Error(`the committed table has no ${element} row`);
  }
  return row;
}

/** IPRT's matrix rows as committed: θ (degrees), P1, P2, P3 and P4. */
function iprtMatrix(text: string): number[][] {
  return text
    .split("\n")
    .filter((line) => line.trim().length > 0 && !line.startsWith("#"))
    .map((line) => line.trim().split(/\s+/u).map(Number));
}

const WHOLE_CASES = SPHERE_CASES.filter(checkedWhole).map((c) => [c.name, c] as const);
const QUICK_CASES = SPHERE_CASES.filter((c) => !checkedWhole(c)).map((c) => [c.name, c] as const);

describe("the Mie-matrix writer's files", () => {
  it.each(WHOLE_CASES)(
    "%s is, whole, the table the client's Mie writes now",
    { timeout: 1_200_000 },
    async (name, sphereCase) => {
      const committed = committedOf(name);
      await expect(
        expectCommittedTable(sphereMatrixFile(sphereCase), committed.table, committed.file),
      ).resolves.toBeUndefined();
    },
  );

  it.runIf(QUICK_CASES.length > 0).each(QUICK_CASES)(
    "%s holds what the client's Mie writes now at every sixty-fourth entry",
    { timeout: 120_000 },
    (name, sphereCase) => {
      // The sampled entries, the bulk optics and the node count, exactly.
      const file = committedOf(name).table;
      const u = evenU(sphereCase.entries);
      expect(file.phase.u).toEqual(Array.from(u));
      const entries = sampledEntries(sphereCase.entries, QUICK_STRIDE);
      const optics = sphereCaseOptics(
        sphereCase,
        cosinesOf(Float64Array.from(entries, (i) => u[i] ?? Number.NaN)),
      );
      expect(optics.nodes).toBe(file.nodes);
      expect(optics.singleScatteringAlbedo).toBe(file.singleScatteringAlbedo);
      expect(optics.asymmetry).toBe(file.asymmetry);
      const recomputed = ELEMENTS.map((element) => ({
        element,
        values: entries.map((_, k) =>
          tableValue(optics.matrix[element][k] ?? Number.NaN, file.linearInUIntegral),
        ),
      }));
      const written = ELEMENTS.map((element) => ({
        element,
        values: entries.map((i) => committedRow(file, element)[i]),
      }));
      expect(recomputed).toEqual(written);
    },
  );
});

describe("the written tables", () => {
  it.each(SPHERE_CASES.map((c) => [c.name, c] as const))(
    "%s keeps the tracer's load checks",
    (name) => {
      const file = committedOf(name).table;
      const a1 = committedRow(file, "a1");
      const moments = linearInUMoments(file.phase.u, a1);
      expect(Math.abs(moments.integral - 1)).toBeLessThan(1e-9);
      expect(Math.abs(moments.meanCosine - file.asymmetry)).toBeLessThan(1e-4);
      for (const element of ["a2", "a3", "a4", "b1", "b2"] as const) {
        const row = committedRow(file, element);
        row.forEach((value, i) => {
          expect(Math.abs(value)).toBeLessThanOrEqual((a1[i] ?? Number.NaN) * (1 + 1e-9));
        });
      }
      a1.forEach((value) => {
        expect(value).toBeGreaterThanOrEqual(0);
      });
    },
  );
});

describe("the writer against the benchmarks' own matrices", () => {
  it("reproduces IPRT A3's matrix, its b₂ IPRT's P4 with the same sign", () => {
    // waso.mie.dat at its 68 angles, P1 averaging 1 over the sphere as the writer's matrix does;
    // measured 3.6 × 10⁻⁶ of P1 at worst, against the file's six printed digits.
    const rows = iprtMatrix(iprtA3Matrix);
    expect(rows).toHaveLength(68);
    const mu = Float64Array.from(rows, (r) => Math.cos(((r[0] ?? Number.NaN) * Math.PI) / 180));
    const optics = sphereCaseOptics(caseNamed("iprt-a3"), mu);
    rows.forEach((row, j) => {
      const p1 = row[1] ?? Number.NaN;
      const mine = [optics.matrix.a1, optics.matrix.b1, optics.matrix.a3, optics.matrix.b2];
      mine.forEach((element, e) => {
        expect(Math.abs((element[j] ?? Number.NaN) - (row[e + 1] ?? Number.NaN))).toBeLessThan(
          1e-5 * p1,
        );
      });
    });
    // The file's header: single-scattering albedo 0.975683, printed to six digits.
    expect(Math.abs(optics.singleScatteringAlbedo - 0.975683)).toBeLessThan(5e-7);
  });

  it("reproduces IPRT A5's matrix", { timeout: 120_000 }, () => {
    // watercloud.mie.dat at every fifth of its angles; measured 1.6 × 10⁻⁴ of P1 at worst over all
    // 450, where the water's resonances leave the client's Mie and libRadtran's apart.
    const rows = iprtMatrix(iprtA5Matrix);
    expect(rows).toHaveLength(91);
    const mu = Float64Array.from(rows, (r) => Math.cos(((r[0] ?? Number.NaN) * Math.PI) / 180));
    const optics = sphereCaseOptics(caseNamed("iprt-a5"), mu);
    rows.forEach((row, j) => {
      const p1 = row[1] ?? Number.NaN;
      const mine = [optics.matrix.a1, optics.matrix.b1, optics.matrix.a3, optics.matrix.b2];
      mine.forEach((element, e) => {
        expect(Math.abs((element[j] ?? Number.NaN) - (row[e + 1] ?? Number.NaN))).toBeLessThan(
          3e-4 * p1,
        );
      });
    });
    // The file's header: 0.999979.
    expect(Math.abs(optics.singleScatteringAlbedo - 0.999979)).toBeLessThan(5e-7);
  });

  it.each([
    // Kokhanovsky et al. 2010, §3: "0.79275 and 0.86114 for aerosol particles and water
    // droplets". Their quadrature (Mishchenko's spher.f) is not stated; the non-absorbing spheres'
    // resonances move a converged sum's asymmetry by up to about 10⁻⁴ from one level of
    // refinement to the next.
    ["kokhanovsky-aerosol", 0.79275],
    ["kokhanovsky-cloud", 0.86114],
  ] as const)("gives %s Kokhanovsky et al.'s asymmetry parameter", (name, published) => {
    expect(Math.abs(committedOf(name).table.asymmetry - published)).toBeLessThan(2e-4);
  });
});

/** A small matrix of ones at three entries even in u, with `forwardPeak`. */
function onesMatrix(forwardPeak: number): Parameters<typeof tableIntegral>[0] {
  const ones = new Float64Array(3).fill(1);
  return {
    mu: cosinesOf(evenU(3)),
    forwardPeak,
    a1: ones,
    a2: ones,
    a3: ones,
    a4: ones,
    b1: new Float64Array(3),
    b2: new Float64Array(3),
  };
}

describe("the writer's sums over size", () => {
  it("repeats libRadtran's grid for IPRT A3, cut at its last radius", () => {
    // From 0.02 r to 5 r, r = 0.0262 µm, in steps of Δx = max(0.03, 0.03 √x) at 350 nm.
    const nodes = radiusNodes(caseNamed("iprt-a3"));
    expect(nodes.radiiUm).toHaveLength(69);
    expect(nodes.radiiUm[0]).toBeCloseTo(0.02 * 0.0262, 15);
    expect(nodes.radiiUm.at(-1)).toBeCloseTo(0.128963, 6);
  });

  it.each([
    [{ kind: "libRadtran", radiusUm: 0.0262, nRMax: 5, dxMax: 0 }],
    [{ kind: "libRadtran", radiusUm: 0, nRMax: 5, dxMax: 0.03 }],
    [{ kind: "gaussLegendre", fromUm: 1, toUm: 1, level: 0 }],
    [{ kind: "gaussLegendre", fromUm: 0.1, toUm: 1, level: 0.5 }],
  ] as const)("refuses a quadrature rule outside its range: %o", (rule) => {
    expect(() => radiusNodes({ ...caseNamed("iprt-a3"), rule })).toThrow(RangeError);
  });

  it("writes values in sr⁻¹, over the table's integral, to ten significant digits", () => {
    expect(tableValue(4 * Math.PI, 1)).toBe(1);
    expect(tableValue(Math.PI, 1)).toBe(0.25);
    expect(tableValue(1, 3)).toBe(0.02652582385);
  });

  it("integrates a table as the tracer does", () => {
    // An isotropic table, 1 ÷ 4π, integrates to 1 with mean cosine 0 however coarse; a table of
    // (1 + cos Θ) ÷ 4π integrates to 1 with mean cosine 1 ÷ 3, read linearly in u to the grid's
    // error.
    const u = evenU(257);
    const isotropic = linearInUMoments(u, new Float64Array(257).fill(1 / (4 * Math.PI)));
    expect(isotropic.integral).toBeCloseTo(1, 12);
    expect(isotropic.meanCosine).toBeCloseTo(0, 12);
    const linear = linearInUMoments(
      u,
      cosinesOf(u).map((c) => (1 + c) / (4 * Math.PI)),
    );
    expect(linear.integral).toBeCloseTo(1, 4);
    expect(linear.meanCosine).toBeCloseTo(1 / 3, 4);
  });

  it("refuses a matrix with a forward delta, which a table cannot carry", () => {
    expect(() => tableIntegral(onesMatrix(0.1), evenU(3))).toThrow(RangeError);
    expect(tableIntegral(onesMatrix(0), evenU(3))).toBeGreaterThan(0);
  });

  it("refuses a matrix not given at the table's entries", () => {
    expect(() => tableIntegral(onesMatrix(0), evenU(4))).toThrow(RangeError);
  });

  it("samples the first and last entries and every stride between", () => {
    expect(sampledEntries(9, 4)).toEqual([0, 4, 8]);
    expect(sampledEntries(10, 4)).toEqual([0, 4, 8, 9]);
  });
});
