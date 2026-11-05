/**
 * The polarisation correction's bake (plan R08, R08.T14.e; Design note 10): its table on the bake's
 * grid, the depolarisers (test 3), the 8-stream correction against a 16-stream vector solve on four
 * columns (test 4), the modes past m = 2 (test 5), the channels against the 15 bins (test 6), the
 * predicate and the refusals, and R08.T15.b.2's guard of the Venus-class cases' scalar gate. Every
 * check is deterministic; each column's solves are made once and shared by the tests that read
 * them. Tests 4 to 6 hold T14.e's figures, which are a sun's above the horizontal, on the grid's daylit rows (`bake.ts`'s `THICK_BAKE_DAY_GRID`); below it the
 * correction is held formed and finite (R08.T14.d.2).
 */
import { describe, expect, it } from "vitest";

import { METRIC_FLOOR } from "../gate";
import {
  BAKE_WAVELENGTHS_NM,
  CHANNEL_WAVELENGTHS_NM,
  densityAt,
  phaseAt,
  type PhaseFunction,
  type PhaseMatrixElement,
  type PhaseMatrixTable,
  phaseTable,
} from "../medium";
import {
  bakeMultipleScattering,
  prepareBake,
  THICK_BAKE_DAY_GRID,
  THICK_BAKE_GRID,
  thickBakeBands,
} from "./bake";
import {
  lightOf,
  titanClassFixture,
  type ThickFixture,
  venusClassFixture,
} from "../../../test/bakeFixtures";
import { binConversion } from "./binConversion";
import {
  bakePolarisationCorrection,
  needsPolarisationCorrection,
  POLARISATION_MODES,
  type PolarisationSources,
  polarisationSources,
  type PolarisedTermOptics,
  type PreparedPolarisation,
  preparePolarisation,
} from "./polarisation";
import { earthFixture, threeBarFixture } from "../../../test/polarisationFixtures";
import type { SpectralColumn, SpectralSource } from "./spectralColumn";

/** A value computed on its first use, so that a describe block's setup runs inside its tests. */
function once<T>(make: () => T): () => T {
  let value: { readonly made: T } | undefined;
  return () => {
    value ??= { made: make() };
    return value.made;
  };
}

/** An array's entry, or a test failure. */
function entry(values: ArrayLike<number>, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/** A light's sources, or a test failure. */
function first(sources: ReadonlyArray<PolarisationSources>): PolarisationSources {
  const source = sources[0];
  if (source === undefined) {
    throw new Error("the correction has one light");
  }
  return source;
}

/** DN10's floor, F of an image's brightest, below which the metric's tolerance is that floor. */
const FLOOR = METRIC_FLOOR;

/** The tests' azimuths, Δφ: the ruling's every 45°. */
const AZIMUTHS_DEG = [0, 45, 90, 135, 180];

/** The modes the 8-stream solves carry, past the table's, for test 5: all of NSTR 8's. */
const ALL_MODES = 7;

/** A column's preparation and its solves, at 8 streams (m ≤ 7) and 16 (m ≤ 2), a channel each. */
interface Solved {
  readonly fixture: ThickFixture;
  readonly prepared: PreparedPolarisation;
  readonly coarse: ReadonlyArray<PolarisationSources>;
  readonly fine: ReadonlyArray<PolarisationSources>;
}

function solvedOf(fixture: ThickFixture): Solved {
  const prepared = preparePolarisation(fixture.column, [lightOf(fixture)], {
    grid: THICK_BAKE_DAY_GRID,
  });
  const channels = [0, 1, 2];
  return {
    fixture,
    prepared,
    coarse: channels.map((c) =>
      first(polarisationSources(prepared, fixture.body, 0, c, { maxMode: ALL_MODES })),
    ),
    fine: channels.map((c) =>
      first(polarisationSources(prepared, fixture.body, 0, c, { streams: 16 })),
    ),
  };
}

const venusFixture = once(venusClassFixture);

/** A test column: its name, its solves, and the bounds its comparisons are held to. */
interface Column {
  readonly name: string;
  readonly solved: () => Promise<Solved>;
  /**
   * Test 4's boundary radiances, in units of the metric's tolerance (0.5% of I, or the floor): 1
   * where the plan's figure is met; the recorded miss, rounded up, where it is not.
   */
  readonly boundaryBound: number;
  /**
   * Test 4's whole source, in units of the metric's tolerance (0.5% of the value, or the floor):
   * 1 where the plan's figure is met; the recorded miss, rounded up, where it is not (plan R08's
   * record of R08.T14.e: the worst points are at the grid's lowest sun, μ₀ 1 ÷ 64).
   */
  readonly sourceBound: number;
  /** Test 5's share of the source the modes past m = 2 carry: the plan's 0.1%, or the recorded miss. */
  readonly modesBound: number;
  /**
   * Test 5 at the boundaries, in units of max(0.1% of I, the floor): 1 where the plan's figure is
   * met; the recorded miss, rounded up, where it is not.
   */
  readonly modesBoundaryBound: number;
}

const COLUMNS: ReadonlyArray<Column> = [
  {
    name: "the generated Earth",
    solved: once(async () => solvedOf(await earthFixture())),
    boundaryBound: 1,
    sourceBound: 1.3,
    modesBound: 0.001,
    modesBoundaryBound: 1,
  },
  {
    name: "the three-bar N₂ world",
    solved: once(async () => Promise.resolve(solvedOf(threeBarFixture()))),
    boundaryBound: 1,
    sourceBound: 1.4,
    modesBound: 0.001,
    modesBoundaryBound: 1,
  },
  {
    name: "the Venus-class column",
    solved: once(async () => solvedOf(await venusFixture())),
    boundaryBound: 1,
    sourceBound: 1,
    modesBound: 0.001,
    modesBoundaryBound: 1,
  },
  {
    name: "the Titan-class column",
    solved: once(async () => Promise.resolve(solvedOf(titanClassFixture()))),
    // Its haze rising aloft (R08.T17) misses at 2.62, 4.86, 0.64% and 1.82 (2026-11-03).
    boundaryBound: 2.7,
    sourceBound: 4.9,
    modesBound: 0.0065,
    modesBoundaryBound: 1.9,
  },
];

/** A column term's density profile, or a test failure. */
function termDensity(
  column: SpectralColumn,
  i: number,
): SpectralColumn["terms"][number]["density"] {
  const term = column.terms[i];
  if (term === undefined) {
    throw new Error(`the column has no term ${i}`);
  }
  return term.density;
}

/** A term's phase function at cos Θ, sr⁻¹: its exact single scattering's. */
function phaseOf(term: PolarisedTermOptics, cosTheta: number): number {
  let value: number;
  switch (term.phase.kind) {
    case "rayleigh": {
      const rho = term.phase.depolarisation;
      value = phaseAt({ kind: "rayleigh", depolarisation: [rho, rho, rho] }, cosTheta)[0];
      break;
    }
    case "tabulated":
      value = phaseAt({ kind: "tabulated", table: term.phase.table }, cosTheta)[0];
      break;
    case "none":
      value = 0;
      break;
    case "moments": {
      // The phase as its Legendre series, Σ (2l + 1) ÷ 4π χ_l P_l(cos Θ).
      let p0 = 1;
      let p1 = cosTheta;
      value = 0;
      for (const [l, chi] of term.phase.moments.entries()) {
        const p = l === 0 ? p0 : p1;
        value += ((2 * l + 1) / (4 * Math.PI)) * chi * p;
        if (l > 0) {
          const next = ((2 * l + 1) * cosTheta * p1 - l * p0) / (l + 1);
          p0 = p1;
          p1 = next;
        }
      }
      break;
    }
  }
  return value;
}

/** A table point's geometry: its travel cosine and the cosine of its scattering angle at Δφ. */
function scatteringCosine(mu0: number, travel: number, azimuthRad: number): number {
  return (
    Math.sqrt(1 - mu0 * mu0) * Math.sqrt(1 - travel * travel) * Math.cos(azimuthRad) - mu0 * travel
  );
}

/** The grid's travel cosine v: the downward half, then the upward. */
function travelCosine(v: number): number {
  const up = THICK_BAKE_GRID.upwardCosines;
  return v < up.length ? -entry(up, up.length - 1 - v) : entry(up, v - up.length);
}

/** Σ_m X_m cos mΔφ of a source at point `base`, its modes `modes` a point, m < `upTo`. */
function atAzimuth(
  values: Float64Array,
  base: number,
  upTo: number,
  azimuthRad: number,
  from = 0,
): number {
  let sum = 0;
  for (let m = from; m < upTo; m += 1) {
    sum += entry(values, base + m) * Math.cos(m * azimuthRad);
  }
  return sum;
}

/** A comparison's worst share of the metric's tolerance and of the value, over an image. */
interface Worst {
  /** The largest |Δ| ÷ max(0.5% of |value|, F × the image's brightest). */
  ofTolerance: number;
  /** The largest |Δ| ÷ |value|. */
  ofValue: number;
}

/**
 * Test 4's source comparison at every table point and the azimuths (the lean of the brief): the
 * 8-stream ΔJ added to the 16-stream scalar J against the 16-stream vector J, the exact single
 * scattering in both, each image one altitude and one sun.
 */
function sourceComparison(solved: Solved, c: number): Worst {
  const { prepared, coarse, fine, fixture } = solved;
  const coarseSource = coarse[c];
  const fineSource = fine[c];
  const point = prepared.points[c];
  if (coarseSource === undefined || fineSource === undefined || point === undefined) {
    throw new Error(`no channel ${c}`);
  }
  const grid = THICK_BAKE_DAY_GRID;
  const suns = grid.sunCosines.length;
  const v2 = 2 * grid.upwardCosines.length;
  const worst: Worst = { ofTolerance: 0, ofValue: 0 };
  prepared.layering.altitudesM.forEach((h, a) => {
    const weights = point.terms.map(
      (t, i) => t.scatteringPerM * densityAt(termDensity(fixture.column, i), h),
    );
    const sigma = weights.reduce((x, y) => x + y, 0);
    if (!(sigma > 0)) {
      return;
    }
    for (let s = 0; s < suns; s += 1) {
      const mu0 = entry(grid.sunCosines, s);
      const kernel = entry(fineSource.kernelTransmission, a * suns + s);
      const image: Array<readonly [number, number]> = [];
      for (let v = 0; v < v2; v += 1) {
        for (const degrees of AZIMUTHS_DEG) {
          const phi = (degrees * Math.PI) / 180;
          const cosTheta = scatteringCosine(mu0, travelCosine(v), phi);
          let single = 0;
          point.terms.forEach((t, i) => {
            single += (entry(weights, i) / sigma) * phaseOf(t, cosTheta);
          });
          const fineBase = ((a * suns + s) * v2 + v) * POLARISATION_MODES;
          const coarseBase = ((a * suns + s) * v2 + v) * (ALL_MODES + 1);
          const reference =
            atAzimuth(fineSource.vector, fineBase, POLARISATION_MODES, phi) + single * kernel;
          const corrected =
            atAzimuth(fineSource.scalar, fineBase, POLARISATION_MODES, phi) +
            single * kernel +
            atAzimuth(coarseSource.correction, coarseBase, POLARISATION_MODES, phi);
          image.push([reference, corrected - reference]);
        }
      }
      const brightest = Math.max(...image.map(([value]) => Math.abs(value)));
      for (const [value, miss] of image) {
        worst.ofTolerance = Math.max(
          worst.ofTolerance,
          Math.abs(miss) / Math.max(0.005 * Math.abs(value), FLOOR * brightest),
        );
        worst.ofValue = Math.max(worst.ofValue, Math.abs(miss) / Math.abs(value));
      }
    }
  });
  return worst;
}

/**
 * Test 4's boundary comparison: I upward at the top and downward at the ground at the grid's
 * cosines and the azimuths, the 16-stream scalar I plus the 8-stream (vector − scalar) against the
 * 16-stream vector I, each image one boundary and one sun.
 */
function boundaryComparison(solved: Solved, c: number): Worst {
  const coarseSource = solved.coarse[c];
  const fineSource = solved.fine[c];
  if (coarseSource === undefined || fineSource === undefined) {
    throw new Error(`no channel ${c}`);
  }
  const grid = THICK_BAKE_DAY_GRID;
  const views = grid.upwardCosines.length;
  const worst: Worst = { ofTolerance: 0, ofValue: 0 };
  const boundaries = [
    [fineSource.topScalar, fineSource.topVector, coarseSource.topScalar, coarseSource.topVector],
    [
      fineSource.groundScalar,
      fineSource.groundVector,
      coarseSource.groundScalar,
      coarseSource.groundVector,
    ],
  ] as const;
  for (const [fineScalar, fineVector, coarseScalar, coarseVector] of boundaries) {
    for (let s = 0; s < grid.sunCosines.length; s += 1) {
      const image: Array<readonly [number, number]> = [];
      for (let u = 0; u < views; u += 1) {
        for (const degrees of AZIMUTHS_DEG) {
          const phi = (degrees * Math.PI) / 180;
          const fineBase = (s * views + u) * POLARISATION_MODES;
          const coarseBase = (s * views + u) * (ALL_MODES + 1);
          const reference = atAzimuth(fineVector, fineBase, POLARISATION_MODES, phi);
          const corrected =
            atAzimuth(fineScalar, fineBase, POLARISATION_MODES, phi) +
            atAzimuth(coarseVector, coarseBase, POLARISATION_MODES, phi) -
            atAzimuth(coarseScalar, coarseBase, POLARISATION_MODES, phi);
          image.push([reference, corrected - reference]);
        }
      }
      const brightest = Math.max(...image.map(([value]) => Math.abs(value)));
      for (const [value, miss] of image) {
        worst.ofTolerance = Math.max(
          worst.ofTolerance,
          Math.abs(miss) / Math.max(0.005 * Math.abs(value), FLOOR * brightest),
        );
        worst.ofValue = Math.max(worst.ofValue, Math.abs(miss) / Math.abs(value));
      }
    }
  }
  return worst;
}

/**
 * Test 5: the 8-stream correction's modes 3 … 7 at every table point and the azimuths, against the
 * point's whole source at 8 streams (every mode, the exact single scattering included).
 */
function modesPastTwo(solved: Solved, c: number): number {
  const { prepared, coarse, fixture } = solved;
  const source = coarse[c];
  const point = prepared.points[c];
  if (source === undefined || point === undefined) {
    throw new Error(`no channel ${c}`);
  }
  const grid = THICK_BAKE_DAY_GRID;
  const suns = grid.sunCosines.length;
  const v2 = 2 * grid.upwardCosines.length;
  let worst = 0;
  prepared.layering.altitudesM.forEach((h, a) => {
    const weights = point.terms.map(
      (t, i) => t.scatteringPerM * densityAt(termDensity(fixture.column, i), h),
    );
    const sigma = weights.reduce((x, y) => x + y, 0);
    if (!(sigma > 0)) {
      return;
    }
    for (let s = 0; s < suns; s += 1) {
      const mu0 = entry(grid.sunCosines, s);
      const kernel = entry(source.kernelTransmission, a * suns + s);
      for (let v = 0; v < v2; v += 1) {
        for (const degrees of AZIMUTHS_DEG) {
          const phi = (degrees * Math.PI) / 180;
          const cosTheta = scatteringCosine(mu0, travelCosine(v), phi);
          let single = 0;
          point.terms.forEach((t, i) => {
            single += (entry(weights, i) / sigma) * phaseOf(t, cosTheta);
          });
          const base = ((a * suns + s) * v2 + v) * (ALL_MODES + 1);
          const whole = atAzimuth(source.vector, base, ALL_MODES + 1, phi) + single * kernel;
          const past = atAzimuth(source.correction, base, ALL_MODES + 1, phi, POLARISATION_MODES);
          worst = Math.max(worst, Math.abs(past) / Math.abs(whole));
        }
      }
    }
  });
  return worst;
}

/**
 * Test 5 at the boundaries: the 8-stream correction's modes 3 … 7 in the top's upward and the
 * ground's downward I, against the 8-stream vector I of every mode, in units of the metric's form at
 * the plan's 0.1%: max(0.1% of I, F of the image's brightest), an image one boundary and one sun.
 */
function modesPastTwoAtBoundaries(solved: Solved, c: number): number {
  const source = solved.coarse[c];
  if (source === undefined) {
    throw new Error(`no channel ${c}`);
  }
  const grid = THICK_BAKE_DAY_GRID;
  const views = grid.upwardCosines.length;
  const modes = ALL_MODES + 1;
  let worst = 0;
  const boundaries = [
    [source.topScalar, source.topVector],
    [source.groundScalar, source.groundVector],
  ] as const;
  for (const [scalar, vector] of boundaries) {
    for (let s = 0; s < grid.sunCosines.length; s += 1) {
      const image: Array<readonly [number, number]> = [];
      for (let u = 0; u < views; u += 1) {
        for (const degrees of AZIMUTHS_DEG) {
          const phi = (degrees * Math.PI) / 180;
          const base = (s * views + u) * modes;
          const whole = atAzimuth(vector, base, modes, phi);
          const past =
            atAzimuth(vector, base, modes, phi, POLARISATION_MODES) -
            atAzimuth(scalar, base, modes, phi, POLARISATION_MODES);
          image.push([whole, past]);
        }
      }
      const brightest = Math.max(...image.map(([value]) => Math.abs(value)));
      for (const [value, past] of image) {
        worst = Math.max(
          worst,
          Math.abs(past) / Math.max(0.001 * Math.abs(value), FLOOR * brightest),
        );
      }
    }
  }
  return worst;
}

// The slowest test is a column's first, which makes its solves: the Titan-class column's 8-stream
// (m ≤ 7) and 16-stream solves of its twelve terms, about 50 s on one worker at a load of 5 beside
// #120's nextest, and the Venus-class preparation's Mie at the channels, 550 and 570 nm, about 5 s
// (2026-10-27). Six times the loaded figure: the config's largest step, 300 s.
const COLUMN_TIMEOUT_MS = 300_000;

/** The Earth's band-0 correction and thick bake, made once for the table's tests. */
const earthTables = once(async () => {
  const fixture = await earthFixture();
  const light = lightOf(fixture);
  return {
    correction: bakePolarisationCorrection(
      preparePolarisation(fixture.column, [light]),
      fixture.body,
      0,
    ),
    thick: bakeMultipleScattering(prepareBake(fixture.column, [light]), fixture.body, 0),
  };
});

describe("the correction's table", () => {
  it(
    "lays a band out as 33 altitudes, 46 μ₀, 16 travel cosines, m = 0 … 2 and RGB, 874,368 B a light",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { correction } = await earthTables();
      const slot = correction.slots[0];
      expect([
        correction.altitudesM.length,
        correction.sunCosines.length,
        correction.travelCosines.length,
        correction.modes,
        slot?.values.length,
        slot?.values.byteLength,
      ]).toEqual([33, 46, 16, 3, 33 * 46 * 16 * 3 * 3, 874_368]);
    },
  );

  it(
    "stands on the thick bake's own altitudes and travel cosines, downward first",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { correction, thick } = await earthTables();
      expect([correction.altitudesM, correction.travelCosines]).toEqual([
        thick.altitudesM,
        thick.travelCosines,
      ]);
    },
  );

  it("holds finite values", { timeout: COLUMN_TIMEOUT_MS }, async () => {
    const { correction } = await earthTables();
    expect(correction.slots[0]?.values.every(Number.isFinite)).toBe(true);
  });

  it(
    "solves the Earth's air on the symmetric path alone, carrying no V",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { correction } = await earthTables();
      const slot = correction.slots[0];
      expect([slot?.complexPairs, slot?.nonsymmetricLayers]).toEqual([0, 0]);
    },
  );
});

describe("the depolarisers (test 3)", () => {
  for (const [name, make] of [
    ["the generated Earth", earthFixture],
    ["the Titan-class column", async () => Promise.resolve(titanClassFixture())],
  ] as const) {
    it(
      `make the correction 0 to 10⁻¹² on ${name} with every term a depolariser, Rayleigh's too`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const fixture = await make();
        const prepared = preparePolarisation(fixture.column, [lightOf(fixture)], {
          matrices: "depolarising",
        });
        const table = bakePolarisationCorrection(prepared, fixture.body, 0);
        let largest = 0;
        for (const value of table.slots[0]?.values ?? [Number.NaN]) {
          largest = Math.max(largest, Math.abs(value));
        }
        // Measured 1.5 × 10⁻¹⁴ (Earth) and 2.5 × 10⁻¹⁴ (Titan-class) sr⁻¹ (2026-10-27).
        expect(largest).toBeLessThan(1e-12);
      },
    );
  }
});

describe("the correction below the horizontal (R08.T14.d.2)", () => {
  /** The Titan-class column's grid: a μ₀ past its top's shadow, two below the horizontal, one above. */
  const twilightGrid = once(() => {
    const fixture = titanClassFixture();
    const band = thickBakeBands(fixture.body)[0];
    if (band === undefined) {
      throw new Error("Titan has one band");
    }
    const r = band.bottomRadiusM;
    const limit = -Math.sqrt(1 - (r / (r + fixture.column.topHeightM)) ** 2);
    return { ...THICK_BAKE_GRID, sunCosines: Float64Array.of(limit - 0.05, -0.1, -1 / 64, 0.5) };
  });
  /** Its correction's values, each with its sun cosine's index, under `matrices`. */
  const correctionOf = (
    matrices: "depolarising" | undefined,
  ): ReadonlyArray<{ readonly value: number; readonly sun: number }> => {
    const fixture = titanClassFixture();
    const grid = twilightGrid();
    const prepared = preparePolarisation(fixture.column, [lightOf(fixture)], {
      grid,
      ...(matrices === undefined ? {} : { matrices }),
    });
    const values = bakePolarisationCorrection(prepared, fixture.body, 0).slots[0]?.values ?? [];
    const perSun = 2 * grid.upwardCosines.length * POLARISATION_MODES * 3;
    return Array.from(values, (value, i) => ({
      value,
      sun: Math.floor(i / perSun) % grid.sunCosines.length,
    }));
  };
  const polarised = once(() => correctionOf(undefined));

  it("is finite at every sun cosine", { timeout: COLUMN_TIMEOUT_MS }, () => {
    expect(polarised().every(({ value }) => Number.isFinite(value))).toBe(true);
  });

  it("is formed below the horizontal", { timeout: COLUMN_TIMEOUT_MS }, () => {
    const below = polarised().filter(({ sun }) => sun === 1 || sun === 2);
    expect(below.some(({ value }) => value !== 0)).toBe(true);
  });

  it("is 0 past the shadow of the table's top", { timeout: COLUMN_TIMEOUT_MS }, () => {
    const past = polarised().filter(({ sun }) => sun === 0);
    expect(Math.max(...past.map(({ value }) => Math.abs(value)))).toBe(0);
  });

  it("is 0 to 10⁻¹² with every term a depolariser", { timeout: COLUMN_TIMEOUT_MS }, () => {
    const largest = Math.max(...correctionOf("depolarising").map(({ value }) => Math.abs(value)));
    expect(largest).toBeLessThan(1e-12);
  });
});

describe("the 8-stream correction against a 16-stream vector solve (test 4)", () => {
  for (const { name, solved, boundaryBound, sourceBound } of COLUMNS) {
    it(
      `holds ${name}'s boundary radiances to ${boundaryBound * 0.5}% of I, over the metric's floor`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const columns = await solved();
        for (let c = 0; c < 3; c += 1) {
          expect(boundaryComparison(columns, c).ofTolerance).toBeLessThan(boundaryBound);
        }
      },
    );
    it(
      `holds ${name}'s whole source, single scattering included, to ${sourceBound * 0.5}% of it, over the metric's floor`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const columns = await solved();
        for (let c = 0; c < 3; c += 1) {
          expect(sourceComparison(columns, c).ofTolerance).toBeLessThan(sourceBound);
        }
      },
    );
  }
});

describe("the modes past m = 2 (test 5)", () => {
  for (const { name, solved, modesBound, modesBoundaryBound } of COLUMNS) {
    it(
      `change ${name}'s whole source by under ${modesBound * 100}%`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const columns = await solved();
        for (let c = 0; c < 3; c += 1) {
          expect(modesPastTwo(columns, c)).toBeLessThan(modesBound);
        }
      },
    );
    it(
      `change ${name}'s boundary radiances by under ${modesBoundaryBound * 0.1}% of I, over the metric's floor`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const columns = await solved();
        for (let c = 0; c < 3; c += 1) {
          expect(modesPastTwoAtBoundaries(columns, c)).toBeLessThan(modesBoundaryBound);
        }
      },
    );
  }
});

describe("the channels against the 15 bins converted (test 6)", () => {
  // The Titan-class column's haze misses the plan's 0.5% (plan R08's record of R08.T14.e): held to
  // its recorded miss, rounded up, in units of the metric's tolerance.
  for (const [name, make, bound] of [
    ["the Venus-class column", venusFixture, 1],
    ["the Titan-class column", async () => Promise.resolve(titanClassFixture()), 6],
  ] as const) {
    it(
      `gives ${name}'s correction at the channels as the bins' converted, to ${bound * 0.5}% of its source`,
      { timeout: COLUMN_TIMEOUT_MS },
      async () => {
        const fixture = await make();
        const light = lightOf(fixture);
        const channels = preparePolarisation(fixture.column, [light], {
          grid: THICK_BAKE_DAY_GRID,
        });
        const bins = preparePolarisation(fixture.column, [light], {
          grid: THICK_BAKE_DAY_GRID,
          wavelengthsNm: BAKE_WAVELENGTHS_NM,
        });
        const perBin = BAKE_WAVELENGTHS_NM.map((_, b) =>
          first(polarisationSources(bins, fixture.body, 0, b)),
        );
        const conversion = binConversion(fixture.sun);
        const grid = THICK_BAKE_DAY_GRID;
        const suns = grid.sunCosines.length;
        const v2 = 2 * grid.upwardCosines.length;
        for (let c = 0; c < 3; c += 1) {
          const source = first(polarisationSources(channels, fixture.body, 0, c));
          const point = channels.points[c];
          const factors = conversion.factors[c === 0 ? 0 : c === 1 ? 1 : 2];
          if (point === undefined) {
            throw new Error(`no channel ${c}`);
          }
          let worst = 0;
          channels.layering.altitudesM.forEach((h, a) => {
            const sigma = entry(source.scatteringPerM, a);
            if (!(sigma > 0)) {
              return;
            }
            const weights = point.terms.map(
              (t, i) => t.scatteringPerM * densityAt(termDensity(fixture.column, i), h),
            );
            for (let s = 0; s < suns; s += 1) {
              const mu0 = entry(grid.sunCosines, s);
              const kernel = entry(source.kernelTransmission, a * suns + s);
              const image: Array<readonly [number, number]> = [];
              for (let v = 0; v < v2; v += 1) {
                for (const degrees of AZIMUTHS_DEG) {
                  const phi = (degrees * Math.PI) / 180;
                  const cosTheta = scatteringCosine(mu0, travelCosine(v), phi);
                  let single = 0;
                  point.terms.forEach((t, i) => {
                    single += entry(weights, i) * phaseOf(t, cosTheta);
                  });
                  const base = ((a * suns + s) * v2 + v) * POLARISATION_MODES;
                  // In E, per unit length: the channel's σ_s ΔJ against the bins' converted.
                  const whole =
                    sigma * atAzimuth(source.vector, base, POLARISATION_MODES, phi) +
                    single * kernel;
                  const atChannel =
                    sigma * atAzimuth(source.correction, base, POLARISATION_MODES, phi);
                  let fromBins = 0;
                  perBin.forEach((binSource, b) => {
                    fromBins +=
                      entry(factors, b) *
                      entry(binSource.scatteringPerM, a) *
                      atAzimuth(binSource.correction, base, POLARISATION_MODES, phi);
                  });
                  image.push([whole, atChannel - fromBins]);
                }
              }
              const brightest = Math.max(...image.map(([value]) => Math.abs(value)));
              for (const [value, miss] of image) {
                worst = Math.max(
                  worst,
                  Math.abs(miss) / Math.max(0.005 * Math.abs(value), FLOOR * brightest),
                );
              }
            }
          });
          expect(worst).toBeLessThan(bound);
        }
      },
    );
  }
});

/** The largest |Σ_m (vector − scalar) cos mΔφ| ÷ Σ_m scalar cos mΔφ over `points` and the azimuths. */
function worstShare(
  vector: Float64Array,
  scalar: Float64Array,
  points: ReadonlyArray<number>,
  upTo: number,
): number {
  let worst = 0;
  for (const base of points) {
    for (const degrees of AZIMUTHS_DEG) {
      const phi = (degrees * Math.PI) / 180;
      const s = atAzimuth(scalar, base, upTo, phi);
      const v = atAzimuth(vector, base, upTo, phi);
      if (!(s > 0)) {
        throw new Error(`the scalar I is ${s} at point ${base}, Δφ ${degrees}°`);
      }
      worst = Math.max(worst, Math.abs(v - s) / s);
    }
  }
  return worst;
}

/** The boundaries' points, `[s][u]` at the grid's daylit μ₀ and upward view cosines. */
function boundaryPoints(): number[] {
  const grid = THICK_BAKE_DAY_GRID;
  return Array.from(
    { length: grid.sunCosines.length * grid.upwardCosines.length },
    (_, i) => i * POLARISATION_MODES,
  );
}

/**
 * The Venus-class cases' guard (plan R08, R08.T15.b.2; main's ruling of 2026-11-01): the cases are
 * gated by their references' scalar I, the Stokes mode's I weight not converging over their
 * thousands of Rayleigh scatterings, so the polarisation the scalar reading neglects is held here,
 * deterministically. The 16-stream vector solve against its own scalar solve (m ≤ 2), on the
 * Venus-class column at the cases' ground albedo 0.15, on the daylit grid: vector − scalar within
 * 1% of the scalar, a fifth of the gate's 5% term. If a changed column or matrix fails it, the
 * scalar gate is refused for Venus. Measured (research, 2026-11-01): the ground's sky −0.62% to
 * +0.20%, J below the cloud base −0.56% to +0.55%, the top's azimuthal mean −0.17% to +0.13%.
 */
describe("the Venus-class cases' scalar gate's guard (R08.T15.b.2)", () => {
  /** The cases' ground albedo (`thickCases.ts`'s Venus cases). */
  const CASE_ALBEDO = 0.15;
  /** vector ÷ scalar − 1, at most: the ruling's 1%. */
  const BOUND = 0.01;
  const guard = once(async () => {
    const fixture = await venusFixture();
    const column: SpectralColumn = {
      ...fixture.column,
      groundAlbedo: [CASE_ALBEDO, CASE_ALBEDO, CASE_ALBEDO],
    };
    const prepared = preparePolarisation(column, [lightOf(fixture)], {
      grid: THICK_BAKE_DAY_GRID,
    });
    const sources = [0, 1, 2].map((c) =>
      first(polarisationSources(prepared, fixture.body, 0, c, { streams: 16 })),
    );
    return { column, prepared, sources };
  });

  it(
    "holds the sky at the ground within 1% of the scalar, every μ₀, μ_v and Δφ 0–180° every 45°",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { sources } = await guard();
      for (const source of sources) {
        expect(
          worstShare(
            source.groundVector,
            source.groundScalar,
            boundaryPoints(),
            POLARISATION_MODES,
          ),
        ).toBeLessThan(BOUND);
      }
    },
  );

  it(
    "holds J at every altitude below the cloud base within 1% of the scalar",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { column, prepared, sources } = await guard();
      const cloud = column.terms.find((term) => term.source.kind === "aerosol");
      if (cloud === undefined) {
        throw new Error("the Venus-class column has its cloud");
      }
      const altitudes = prepared.layering.altitudesM;
      const baseM = Math.min(
        ...Array.from(altitudes).filter((h) => densityAt(cloud.density, h) > 0),
      );
      const below = Array.from(altitudes.entries()).filter(([, h]) => h < baseM);
      // The ground and the clear air beneath the cloud: 10 of the grid's 33 altitudes.
      expect(below.map(([, h]) => h)).toContain(0);
      expect(below.length).toBeGreaterThan(1);
      const grid = THICK_BAKE_DAY_GRID;
      const suns = grid.sunCosines.length;
      const v2 = 2 * grid.upwardCosines.length;
      const points = below.flatMap(([a]) =>
        Array.from({ length: suns * v2 }, (_, i) => (a * suns * v2 + i) * POLARISATION_MODES),
      );
      for (const source of sources) {
        expect(worstShare(source.vector, source.scalar, points, POLARISATION_MODES)).toBeLessThan(
          BOUND,
        );
      }
    },
  );

  it(
    "holds the top's upward I within 1% of the scalar, its azimuthal mean (m = 0) alone",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const { sources } = await guard();
      // m ≤ 2 cannot represent the droplets' lobes at the top: its sum at Δφ ≠ 0 is no usable I.
      for (const source of sources) {
        expect(worstShare(source.topVector, source.topScalar, boundaryPoints(), 1)).toBeLessThan(
          BOUND,
        );
      }
    },
  );
});

/** A column of one term, on the three-bar world's air. */
function columnOf(source: SpectralColumn["terms"][number]["source"]): SpectralColumn {
  const air = threeBarFixture().column;
  const [molecular] = air.terms;
  if (molecular === undefined) {
    throw new Error("the three-bar world has its molecular term");
  }
  return { ...air, terms: [{ name: "only", density: molecular.density, source }] };
}

describe("which worlds need it", () => {
  it("is every world with air: a molecular term scatters by Rayleigh's matrix", async () => {
    expect([
      needsPolarisationCorrection((await earthFixture()).column),
      needsPolarisationCorrection(threeBarFixture().column),
    ]).toEqual([true, true]);
  });

  it("is a column of free electrons alone, which scatter by Rayleigh's matrix too", () => {
    expect(
      needsPolarisationCorrection(columnOf({ kind: "thomson", electronDensityPerM3: 1e9 })),
    ).toBe(true);
  });

  it(
    "is an aerosol alone whose matrix has a b₁: the Venus-class cloud's droplets",
    { timeout: COLUMN_TIMEOUT_MS },
    async () => {
      const venus = await venusFixture();
      const cloud = venus.column.terms.filter((term) => term.source.kind === "aerosol");
      expect(needsPolarisationCorrection({ ...venus.column, terms: cloud })).toBe(true);
    },
  );

  it("is not an aerosol alone with no matrix: a 3 µm TiO₂ mineral, a Henyey–Greenstein fallback", () => {
    const rutile = columnOf({
      kind: "aerosol",
      particles: {
        kind: "distribution",
        mode: {
          material: "TiO2",
          form: { phase: "solid" },
          shape: "nonSphericalMineral",
          sizes: { kind: "gamma", effectiveRadiusUm: 3, effectiveVariance: 0.1 },
        },
      },
      extinction550PerM: 1e-5,
    });
    expect(needsPolarisationCorrection(rutile)).toBe(false);
  });

  it("is not a column that scatters nothing: absorbers alone", async () => {
    const earth = await earthFixture();
    const absorbers = earth.column.terms.filter((term) => term.source.kind === "absorber");
    expect(needsPolarisationCorrection({ ...earth.column, terms: absorbers })).toBe(false);
  });

  it("reads a channel source by its phase: Rayleigh's yes, a table's by its green b₁, Cornette–Shanks's and none's no", () => {
    const scattering = [1e-5, 1e-5, 1e-5] as const;
    const channels = (phase: PhaseFunction): SpectralSource => ({
      kind: "channels",
      scattering,
      absorption: [0, 0, 0],
      phase,
    });
    const u = Float64Array.of(0, 1);
    const flat = Float64Array.of(1 / (4 * Math.PI), 1 / (4 * Math.PI));
    const zeros = Float64Array.of(0, 0);
    const element = (red: Float64Array, green: Float64Array): PhaseMatrixElement => [
      red,
      green,
      zeros,
    ];
    const matrixWith = (b1: PhaseMatrixElement): PhaseMatrixTable => ({
      a2: element(flat, flat),
      a3: element(flat, flat),
      a4: element(flat, flat),
      b1,
      b2: element(zeros, zeros),
    });
    const polarising = Float64Array.of(0, -0.01);
    const table = (matrix?: PhaseMatrixTable): PhaseFunction => ({
      kind: "tabulated",
      table: phaseTable(u, [flat, flat, flat], matrix),
    });
    expect(
      [
        channels({ kind: "rayleigh", depolarisation: [0.03, 0.03, 0.03] }),
        channels(table(matrixWith(element(zeros, polarising)))),
        channels(table(matrixWith(element(polarising, zeros)))),
        channels(table()),
        channels({ kind: "cornette-shanks", asymmetry: 0.584 }),
        channels({ kind: "none" }),
      ].map((source) => needsPolarisationCorrection(columnOf(source))),
    ).toEqual([true, true, false, false, false, false]);
  });

  it("takes a channel source's Cornette–Shanks phase, known by its moments, as a total depolariser", () => {
    const prepared = preparePolarisation(
      columnOf({
        kind: "channels",
        scattering: [1e-5, 2e-5, 3e-5],
        absorption: [0, 0, 0],
        phase: { kind: "cornette-shanks", asymmetry: 0.584 },
      }),
      [],
    );
    expect(
      prepared.points.map((point) => [point.terms[0]?.phase.kind, point.terms[0]?.polarising]),
    ).toEqual([
      ["moments", false],
      ["moments", false],
      ["moments", false],
    ]);
  });
});

/** The Earth's correction prepared at the channels and at one wavelength, for the refusals. */
const earthPreparations = once(async () => {
  const fixture = await earthFixture();
  const light = lightOf(fixture);
  return {
    fixture,
    light,
    channels: preparePolarisation(fixture.column, [light]),
    green: preparePolarisation(fixture.column, [light], {
      wavelengthsNm: [CHANNEL_WAVELENGTHS_NM[1]],
    }),
  };
});

describe("the refusals", () => {
  it.for([
    [
      "a table of a preparation off the render channels",
      async () => {
        const { fixture, green } = await earthPreparations();
        bakePolarisationCorrection(green, fixture.body, 0);
      },
    ],
    [
      "a band the body does not have",
      async () => {
        const { fixture, channels } = await earthPreparations();
        bakePolarisationCorrection(channels, fixture.body, 1);
      },
    ],
    [
      "a wavelength the preparation does not have",
      async () => {
        const { fixture, channels } = await earthPreparations();
        polarisationSources(channels, fixture.body, 0, 3);
      },
    ],
    [
      "more streams than the preparation's moments reach",
      async () => {
        const { fixture, channels } = await earthPreparations();
        polarisationSources(channels, fixture.body, 0, 0, { streams: 18 });
      },
    ],
    [
      "a moment order that is not even",
      async () => {
        const { fixture, light } = await earthPreparations();
        preparePolarisation(fixture.column, [light], { momentOrder: 15 });
      },
    ],
  ] as const)("refuses %s", async ([, attempt]) => {
    await expect(attempt()).rejects.toThrow(RangeError);
  });

  it("prepares a column under no light, which then has no slot", async () => {
    const { fixture } = await earthPreparations();
    expect(preparePolarisation(fixture.column, []).lights).toEqual([]);
  });
});
