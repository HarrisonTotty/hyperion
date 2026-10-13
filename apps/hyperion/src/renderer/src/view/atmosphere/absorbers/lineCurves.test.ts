import { describe, expect, it } from "vitest";

import { earthLikeColumn, planckBakeSpectrum } from "../../../test/atmosphereFixtures";
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  absorberBinCurves,
  absorberBinTransmittance,
  absorberCurve,
  absorberCurvePressures,
  absorberCurves,
  absorberCurvesBySun,
  absorberCurvesFromSums,
  absorberCurveSums,
  absorberOptics,
  absorberSpecs,
  absorberTransmittance,
  type ColumnAbsorberSpec,
  channelTransmittance,
  LINEAR_CELL_DEPTH,
} from "../absorbers";
import { toyPart, toySidecar } from "../../../test/kDistributionFixtures";
import { BAKE_BIN_WIDTH_NM, BAKE_RANGE_NM } from "../medium";
import { bakeSpectrumOf } from "../spectralColour";
import {
  cellSigmasAt,
  cellTransmittance,
  type KDistributionFile,
  type KDistributionTable,
  readKDistribution,
} from "./kDistribution";

/** The stored pressures' first span, 4 a decade from 10 Pa to 10 MPa: the toy's grid. */
const PRESSURES_PA = Array.from({ length: 25 }, (_, k) => 10 * 10 ** (k / 4));
const TEMPERATURES_K = [150, 200, 250, 300, 350];

/** The toy species' σ at a node and g-point, m²: a core falling ∝ 1 ÷ √p past 1 kPa, rising with T. */
function toySigma(t: number, p: number, g: number): number {
  return (g === 0 ? 2e-30 : 6e-27) * (t / 250) * (g === 0 ? 1 : 1 / Math.sqrt(1 + p / 1e3));
}

/**
 * A toy line species: cells 690–692 nm, each with a strong and a weak g-point ({@link toySigma}).
 */
function toyTable(
  species: string,
  overrides: Partial<KDistributionFile> = {},
  cellsNm: ReadonlyArray<number> = [690, 691, 692],
): KDistributionTable {
  const sigmasM2 = TEMPERATURES_K.flatMap((t) =>
    PRESSURES_PA.flatMap((p) => [toySigma(t, p, 0), toySigma(t, p, 1)]),
  );
  const cells = cellsNm.map((nm) => ({ nm, weights: [0.8, 0.2], sigmasM2 }));
  const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, cells.length, {
    species,
    measuredRangeK: [150, 296],
    measuredShapeTo: [20, 20, 20, 20, 20],
    broadening: { self: 1.5 },
    ...overrides,
  });
  const part = toyPart(cells, {
    variants: 1,
    temperatures: TEMPERATURES_K.length,
    pressures: PRESSURES_PA.length,
  });
  return readKDistribution(sidecar, [part], `toy ${species}`);
}

/** A layer of a species over the whole of the Earth-like column, with a column, m⁻². */
function wellMixed(species: string, columnPerM2: number) {
  return { species, columnPerM2, basePa: 101_325, topPa: 0 };
}

const COLUMN = earthLikeColumn();
const SUN = planckBakeSpectrum(5_772);

/** The single column spec absorberSpecs forms, or a test failure. */
function onlyColumnSpec(specs: ReturnType<typeof absorberSpecs>): ColumnAbsorberSpec {
  const spec = specs.specs.find((s): s is ColumnAbsorberSpec => s.kind === "column");
  if (spec === undefined) {
    throw new Error("no column spec was formed");
  }
  return spec;
}

describe("a line species in absorberSpecs", () => {
  const table = toyTable("H2O");

  it("draws its lines, measured at the layer's temperature in air", () => {
    const specs = absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
      kDistributions: [table],
    });
    const lines = onlyColumnSpec(specs).species[0]?.lines;
    expect([lines?.provenance, lines?.broadeningFactor, specs.approximations]).toEqual([
      "measured",
      expect.closeTo(1, 3),
      [],
    ]);
  });

  it("is estimated past its measured temperatures, held there", () => {
    const hot = earthLikeColumn();
    const specs = absorberSpecs(
      [wellMixed("H2O", 1e26)],
      { ...hot, temperaturesK: hot.temperaturesK.map(() => 320) },
      [],
      [],
      {
        kDistributions: [table],
      },
    );
    expect(specs.approximations).toEqual([
      { species: "H2O", provenance: "estimated", reason: "temperatureOutsideTable" },
    ]);
  });

  it("is estimated where the layer's broadening pressure passes the measured line shape", () => {
    const narrow = toyTable("H2O", { measuredShapeTo: [10, 10, 10, 10, 10] });
    const specs = absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
      kDistributions: [narrow],
    });
    expect(specs.approximations).toEqual([
      { species: "H2O", provenance: "estimated", reason: "estimatedOptics" },
    ]);
  });

  it("is estimated on a background gas the table gives no width for", () => {
    const specs = absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
      kDistributions: [table],
      bulkMoleFractions: new Map([["H2", 1]]),
    });
    expect(specs.approximations).toEqual([
      { species: "H2O", provenance: "estimated", reason: "estimatedOptics" },
    ]);
  });

  it("takes its own mole fraction as self-broadening: x r_self + (1 − x) for air", () => {
    const column = 0.1 * COLUMN.surfaceNumberDensityPerM3 * 8_000;
    const specs = absorberSpecs([wellMixed("H2O", column)], COLUMN, [], [], {
      kDistributions: [table],
    });
    const lines = onlyColumnSpec(specs).species[0]?.lines;
    // The layer's number column is n_s ∫ρ dz; the species' share of it, x, takes 1.5.
    const numberColumn = COLUMN.surfaceNumberDensityPerM3 * integralOfDensity();
    const x = column / numberColumn;
    expect(lines?.broadeningFactor).toBeCloseTo(x * 1.5 + (1 - x), 6);
  });
});

/** ∫ρ dz over the column, m, by the trapezoid on its levels. */
function integralOfDensity(): number {
  let sum = 0;
  const z = COLUMN.altitudesM;
  const rho = COLUMN.density.relative;
  for (let i = 1; i < z.length; i += 1) {
    sum += 0.5 * ((rho[i] ?? 0) + (rho[i - 1] ?? 0)) * ((z[i] ?? 0) - (z[i - 1] ?? 0));
  }
  return sum;
}

describe("a line species' curves", () => {
  const table = toyTable("H2O");
  const spec = onlyColumnSpec(
    absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], { kDistributions: [table] }),
  );
  const curve = absorberCurve(spec, SUN);

  it("take a row at each stored pressure up to the layer's base and one beyond", () => {
    const f = spec.species[0]?.lines?.broadeningFactor ?? Number.NaN;
    const rows = Array.from(absorberCurvePressures(spec), (x) => Math.exp(x) * f);
    // 10 Pa to 10^5.25 = 1.78 × 10⁵ Pa, the first node at or past 101 325 Pa, and one more for the
    // read's stencil: 19, decision-r08-t4d-kdist.md §2.4's count for Earth.
    expect(rows.length).toBe(19);
    for (const [r, p] of rows.entries()) {
      expect(p / (PRESSURES_PA[r] ?? Number.NaN)).toBeCloseTo(1, 12);
    }
  });

  it("lie in [10⁻⁶, 1] and are non-increasing along each row's columns", () => {
    const nodes = curve.logColumns.length;
    let least = Infinity;
    let most = -Infinity;
    let rise = 0;
    for (const channel of curve.transmittance) {
      for (let r = 0; r < curve.logPressures.length; r += 1) {
        for (let j = 0; j < nodes; j += 1) {
          const t = channel[r * nodes + j] ?? Number.NaN;
          least = Math.min(least, t);
          most = Math.max(most, t);
          rise = Math.max(rise, j > 0 ? t - (channel[r * nodes + j - 1] ?? Number.NaN) : 0);
        }
      }
    }
    expect([least >= ABSORBER_TRANSMITTANCE_FLOOR, most <= 1, rise]).toEqual([true, true, 0]);
  });

  /** A row's node j against the channel transmittance at the row's pressure: [curve, direct]. */
  const atNode = (r: number, j: number): readonly [number, number] => {
    const p = Math.exp(curve.logPressures[r] ?? Number.NaN);
    const u = Math.exp(curve.logColumns[j] ?? Number.NaN);
    const optics = absorberOptics(spec, SUN);
    const direct = channelTransmittance(optics, { kind: "column", columnPerM2: u }, p);
    return [curve.transmittance[0][r * curve.logColumns.length + j] ?? Number.NaN, direct[0]];
  };

  it("equal the channel transmittance at each row's first node, where no running minimum acts", () => {
    for (const r of [0, 9, 17]) {
      const [found, direct] = atNode(r, 0);
      expect(found).toBeCloseTo(direct, 12);
    }
  });

  it("stay at or under the channel transmittance farther along each row", () => {
    for (const r of [0, 9, 17]) {
      for (const j of [120, 240]) {
        const [found, direct] = atNode(r, j);
        expect(found).toBeLessThanOrEqual(direct + 1e-12);
      }
    }
  });

  it("are read at a row's pressure as the row, and cubic in ln p between rows", () => {
    const nodes = curve.logColumns.length;
    const j = 160;
    const u = Math.exp(curve.logColumns[j] ?? Number.NaN);
    const at = (p: number): number =>
      absorberTransmittance(curve, { kind: "column", columnPerM2: u }, p)[0];
    const r = 12;
    const row = (k: number): number => curve.transmittance[0][k * nodes + j] ?? Number.NaN;
    expect(at(Math.exp(curve.logPressures[r] ?? Number.NaN))).toBeCloseTo(row(r), 12);
    // Between rows r and r + 1 the read lies near both: the toy's curve is smooth in ln p.
    const between = at(
      Math.exp(0.5 * ((curve.logPressures[r] ?? 0) + (curve.logPressures[r + 1] ?? 0))),
    );
    expect(between).toBeGreaterThan(Math.min(row(r), row(r + 1)) - 1e-3);
    expect(between).toBeLessThan(Math.max(row(r), row(r + 1)) + 1e-3);
  });

  it("refuse a read with no pressure where there are lines", () => {
    expect(() =>
      absorberTransmittance(curve, { kind: "column", columnPerM2: 1e26 }, undefined),
    ).toThrow("pressure");
  });
});

describe("a line species' bin curves", () => {
  const spec = onlyColumnSpec(
    absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
      kDistributions: [toyTable("H2O")],
    }),
  );
  const curve = absorberCurve(spec, SUN);
  const bins = absorberBinCurves(spec, SUN);
  const nodes = bins.logColumns.length;
  /** The bake bin holding the toy's cells, 690–692 nm. */
  const lineBin = Math.floor((691 - BAKE_RANGE_NM[0]) / BAKE_BIN_WIDTH_NM);

  it("take the channel curve's columns, rows and read's top", () => {
    expect([bins.logColumns, bins.logPressures, bins.readTopLogPressure]).toEqual([
      curve.logColumns,
      curve.logPressures,
      curve.readTopLogPressure,
    ]);
  });

  /** Bin b's transmittance at column node 160 in each pressure row. */
  const rowsOf = (b: number): number[] =>
    Array.from(
      { length: bins.logPressures.length },
      (_, r) => bins.transmittance[b]?.[r * nodes + 160] ?? Number.NaN,
    );

  it("move with pressure in the lines' bin", () => {
    const lines = rowsOf(lineBin);
    expect(Math.max(...lines) - Math.min(...lines)).toBeGreaterThan(1e-3);
  });

  it("stay at 1 in every bin without lines, at every pressure", () => {
    const moved = bins.transmittance.flatMap((_, b) =>
      b !== lineBin && rowsOf(b).some((t) => t !== 1) ? [b] : [],
    );
    expect(moved).toEqual([]);
  });

  it("are read at a row's pressure as the row, unclamped", () => {
    const r = 12;
    const j = 200;
    const u = Math.exp(bins.logColumns[j] ?? Number.NaN);
    const read = absorberBinTransmittance(
      bins,
      { kind: "column", columnPerM2: u },
      Math.exp(bins.logPressures[r] ?? Number.NaN),
    );
    expect(read[lineBin]).toBeCloseTo(
      bins.transmittance[lineBin]?.[r * nodes + j] ?? Number.NaN,
      12,
    );
  });

  it("refuse a read with no pressure where there are lines", () => {
    expect(() =>
      absorberBinTransmittance(bins, { kind: "column", columnPerM2: 1e26 }, undefined),
    ).toThrow("pressure");
  });
});

describe("a line species' curves under several suns", () => {
  it("are each sun's own, the lines' exponentials taken once for all of them", () => {
    const spec = onlyColumnSpec(
      absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
        kDistributions: [toyTable("H2O")],
      }),
    );
    const suns = [3_000, 5_772, 30_000].map((t) => planckBakeSpectrum(t));
    expect(absorberCurvesBySun(spec, suns)).toEqual(suns.map((sun) => absorberCurves(spec, sun)));
  });

  it("refuse a sun with no light in a bake bin, as one sun's do", () => {
    const spec = onlyColumnSpec(
      absorberSpecs([wellMixed("H2O", 1e26)], COLUMN, [], [], {
        kDistributions: [toyTable("H2O")],
      }),
    );
    // S is linear between the bins' centres, so bin 7 is dark only with its neighbours.
    const dark = bakeSpectrumOf(Array.from({ length: 15 }, (_, b) => (b >= 6 && b <= 8 ? 0 : 1)));
    expect(() => absorberCurvesBySun(spec, [SUN, dark])).toThrow(/bake bin/u);
  });
});

/**
 * The largest difference between two sets of curves' optical depths, channels and bins, relative
 * where a depth passes 1.
 */
function worstDepth(
  a: ReturnType<typeof absorberCurvesBySun>,
  b: ReturnType<typeof absorberCurvesBySun>,
): number {
  let worst = 0;
  for (const [s, curves] of a.entries()) {
    const other = b[s];
    const depths = [...curves.channels.opticalDepth, ...curves.bins.opticalDepth];
    const otherDepths = [
      ...(other?.channels.opticalDepth ?? []),
      ...(other?.bins.opticalDepth ?? []),
    ];
    for (const [d, depth] of depths.entries()) {
      for (const [i, v] of depth.entries()) {
        const w = otherDepths[d]?.[i] ?? Number.NaN;
        worst = Math.max(worst, Math.abs(v - w) / Math.max(Math.abs(w), 1));
      }
    }
  }
  return worst;
}

describe("a line species' curves split by cell across the optics workers (§15, ruling 3)", () => {
  // Two species on one profile, so that each share holds cells of both.
  const spec = onlyColumnSpec(
    absorberSpecs([wellMixed("H2O", 1e26), wellMixed("CO2", 5e25)], COLUMN, [], [], {
      kDistributions: [toyTable("H2O"), toyTable("CO2", { broadening: { self: 1.3 } })],
    }),
  );
  const suns = [5_772, 3_000].map((t) => planckBakeSpectrum(t));
  const whole = absorberCurvesBySun(spec, suns);

  // Four shares to the toy's three cells: share 3 holds no line cell and no point.
  it.each([1, 2, 3, 4])(
    "join %i shares' sums into the whole's curves, but for their order",
    (count) => {
      const sums = Array.from({ length: count }, (_, index) =>
        absorberCurveSums(spec, suns, { index, count }),
      );
      // Rounding alone parts the two, the shares' sums added in another order.
      expect(worstDepth(absorberCurvesFromSums(spec, suns, sums.toReversed()), whole)).toBeLessThan(
        1e-12,
      );
    },
  );

  it("refuse a split missing a share", () => {
    const first = absorberCurveSums(spec, suns, { index: 0, count: 2 });
    expect(() => absorberCurvesFromSums(spec, suns, [first])).toThrow(/every share/u);
  });

  it("refuse no shares at all, which would draw the absorber black", () => {
    expect(() => absorberCurvesFromSums(spec, suns, [])).toThrow(/every share/u);
  });

  it("refuse a share not of its count", () => {
    expect(() => absorberCurveSums(spec, suns, { index: 2, count: 2 })).toThrow(RangeError);
  });
});

describe("a line species in every cell of the range, its cells' sums underflowing at depth", () => {
  // Every 1 nm cell from 380 to 760 nm; at 10³ U = 10³⁴ m⁻² even the weak g-point's σu is about
  // 2 × 10⁴, so that each cell's Σ_g w_g e^(−σ_g u) underflows to 0 and every bin with it.
  const every = Array.from({ length: 381 }, (_, k) => 380 + k);
  const spec = onlyColumnSpec(
    absorberSpecs([wellMixed("H2O", 1e31)], COLUMN, [], [], {
      kDistributions: [toyTable("H2O", {}, every)],
    }),
  );
  const bins = absorberBinCurves(spec, SUN);

  it("keep every bin's optical depth finite, its cells' sums taken as logarithms", () => {
    expect(bins.opticalDepth.every((depths) => depths.every((d) => Number.isFinite(d)))).toBe(true);
  });

  it("read finite at the top column, not NaN", () => {
    const top = Math.exp(bins.logColumns.at(-1) ?? Number.NaN);
    const read = absorberBinTransmittance(bins, { kind: "column", columnPerM2: top }, 1e5);
    expect(read.every((t) => Number.isFinite(t) && t >= 0)).toBe(true);
  });
});

describe("a line cell linear at the curve's top column (§15, ruling 3)", () => {
  // The toy's strongest σ, 8.4 × 10⁻²⁷ m² at 350 K, times 10³ U = 10²⁴ m⁻²: 8.4 × 10⁻³, under
  // LINEAR_CELL_DEPTH, so that every row takes each cell's mean σ̄.
  const spec = onlyColumnSpec(
    absorberSpecs([wellMixed("H2O", 1e21)], COLUMN, [], [], { kDistributions: [toyTable("H2O")] }),
  );
  const curve = absorberCurve(spec, SUN);
  const nodes = curve.logColumns.length;
  const u = Math.exp(curve.logColumns[nodes - 1] ?? Number.NaN);
  const optics = absorberOptics(spec, SUN);
  /** The largest |ΔT| at the top column between the curve and the cells' own sums, every row. */
  const worst = Math.max(
    ...Array.from(curve.logPressures, (logPressure, r) => {
      const direct = channelTransmittance(
        optics,
        { kind: "column", columnPerM2: u },
        Math.exp(logPressure),
      );
      return Math.max(
        ...curve.transmittance.map((channel, c) =>
          Math.abs((channel[r * nodes + nodes - 1] ?? 0) - (direct[c] ?? 0)),
        ),
      );
    }),
  );

  it("is the ruling's 0.02", () => {
    expect(LINEAR_CELL_DEPTH).toBe(0.02);
  });

  it("takes its mean σ̄, not the cell's own sum", () => {
    // ½ Var(σ) u² of the toy's cells, a few 10⁻⁷ in T, far above the sums' rounding.
    expect(worst).toBeGreaterThan(1e-12);
  });

  it("takes it within (σ_max u)² ÷ 8 of the cell's own sum at every row", () => {
    expect(worst).toBeLessThanOrEqual((8.4e-3 * 1.01) ** 2 / 8);
  });
});

describe("two line species on one profile", () => {
  it("overlap at random within a cell: each point takes the product of the species' cell sums", () => {
    const a = toyTable("H2O");
    const b = toyTable("CO2", { broadening: { self: 1.3 } });
    const spec = onlyColumnSpec(
      absorberSpecs([wellMixed("H2O", 1e26), wellMixed("CO2", 3e26)], COLUMN, [], [], {
        kDistributions: [a, b],
      }),
    );
    const p = 1e4;
    const u = 2e26;
    const optics = absorberOptics(spec, SUN);
    const cellSum = (species: string): Map<number, number> => {
      const s = spec.species.find((x) => x.species === species);
      const lines = s?.lines;
      if (s === undefined || lines === undefined) {
        throw new Error(species + " has no lines");
      }
      const share = (s.columnPerM2 / spec.columnPerM2) * u;
      const sigmas = cellSigmasAt(lines.kDistribution, p * lines.broadeningFactor);
      return new Map(
        lines.kDistribution.cells.map((cell, c) => [
          cell.cellNm,
          cellTransmittance(cell.weights, sigmas[c] ?? new Float64Array(0), share),
        ]),
      );
    };
    const h2o = cellSum("H2O");
    const co2 = cellSum("CO2");
    let passed = 0;
    let total = 0;
    for (const [i, nm] of optics.wavelengthsNm.entries()) {
      const w = optics.weights[0][i] ?? 0;
      const cell = Math.floor(nm + 0.5);
      passed += w * (h2o.get(cell) ?? 1) * (co2.get(cell) ?? 1);
      total += w;
    }
    // Red, whose weight is positive over 690–692 nm, so that no clamp acts on the signed sum.
    const red = channelTransmittance(optics, { kind: "column", columnPerM2: u }, p)[0];
    expect(passed / total).toBeLessThan(1);
    expect(red).toBeCloseTo(passed / total, 12);
  });
});

/** A depth cubic in ln p, which the read reproduces exactly. */
function cubicTau(y: number): number {
  return 0.5 + 0.2 * (y - 13) - 0.05 * (y - 13) ** 2 + 0.01 * (y - 13) ** 3;
}

describe("the read between rows", () => {
  // Rows 4 a decade to 1 MPa and 8 a decade above, as the stored nodes run (§8), and one node of
  // columns: τ a cubic in ln p is read exactly by weights from the rows' own ln p.
  const logPressures = Float64Array.from(
    [5, 5.25, 5.5, 5.75, 6, 6.125, 6.25, 6.375],
    (d) => d * Math.LN10,
  );
  const depths = Float64Array.from(logPressures, cubicTau);
  const curve = {
    kind: "column" as const,
    logColumns: Float64Array.of(Math.log(1e24)),
    logPressures,
    readTopLogPressure: 6.25 * Math.LN10,
    transmittance: [
      depths.map((d) => Math.exp(-d)),
      depths.map((d) => Math.exp(-d)),
      depths.map((d) => Math.exp(-d)),
    ] as const,
    opticalDepth: [depths, depths, depths] as const,
  };
  const at = (pressurePa: number): number =>
    -Math.log(absorberTransmittance(curve, { kind: "column", columnPerM2: 1e24 }, pressurePa)[0]);

  it("takes its Lagrange weights from the rows' own ln p where their spacing halves", () => {
    for (const p of [3e5, 9e5, 1.5e6]) {
      expect(at(p)).toBeCloseTo(cubicTau(Math.log(p)), 10);
    }
  });

  it("holds p̄ at the read's top above it, the last row only centring the stencil", () => {
    expect(at(2e6)).toBeCloseTo(cubicTau(6.25 * Math.LN10), 10);
  });
});
