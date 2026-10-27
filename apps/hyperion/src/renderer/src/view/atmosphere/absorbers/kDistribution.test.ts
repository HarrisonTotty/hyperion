import { describe, expect, it, vi } from "vitest";

import registry from "../../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { inlinePart, type ToyCell, toyPart, toySidecar } from "../../../test/kDistributionFixtures";
import {
  broadeningFactor,
  cellMeanSigma,
  cellSigmasAt,
  cellTransmittance,
  committedLineSpecies,
  curveRowsPa,
  DIPOLAR_BROADENERS,
  fetchPart,
  type KDistributionFile,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
  mixLimitColumnM2,
  readKDistribution,
  SELF_BROADENED_ATOMS,
  STANDARD_PRESSURES_PA,
} from "./kDistribution";

/**
 * σ of a toy cell at a node, m²: ln σ a cubic in T and in ln p, which the 4-point interpolations
 * reproduce exactly but for the 16-bit step.
 */
function toySigma(temperatureK: number, pressurePa: number, g: number): number {
  const t = temperatureK / 100;
  const x = Math.log(pressurePa / 1e4);
  return Math.exp(-57 + g + 0.4 * t - 0.03 * t ** 3 + 0.2 * x - 0.01 * x ** 3);
}

const TEMPERATURES_K = [100, 150, 200, 250, 300];
const PRESSURES_PA = [1e1, 1e2, 1e3, 1e4, 1e5, 1e6];

/** A toy cell of two g-points over the toy grid. */
function toyCell(nm: number): ToyCell {
  return {
    nm,
    weights: [0.75, 0.25],
    sigmasM2: TEMPERATURES_K.flatMap((t) =>
      PRESSURES_PA.flatMap((p) => [toySigma(t, p, 0), toySigma(t, p, 1)]),
    ),
  };
}

/** A toy table of the toy cells, its sidecar's fields overridable. */
function toyTable(
  overrides: Partial<KDistributionFile> = {},
  cells: ReadonlyArray<ToyCell> = [toyCell(690)],
): KDistributionTable {
  const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, cells.length, {
    measuredRangeK: [100, 296],
    measuredShapeTo: [5, 5, 4, 4, 3],
    broadening: { self: 1.005, CO2: 1.18 },
    ...overrides,
  });
  const part = toyPart(cells, {
    variants: sidecar.variants.length,
    temperatures: sidecar.temperaturesK.length,
    pressures: sidecar.pressuresPa.length,
  });
  return readKDistribution(sidecar, [part], "toy");
}

/** The 16-bit step's largest relative error in σ for the toy cell's range. */
const STEP_RELATIVE = 2e-4;

/**
 * The registry's molecules with a permanent dipole of at least H₂S's (R08.T10.d): R08.T10.c's five,
 * then the seven `decision-r08-approximate-plate.md` §6 adds.
 */
const DIPOLAR_GASES = [
  "H2O",
  "NH3",
  "SO2",
  "H2S",
  "HCN",
  "HCl",
  "CH3OH",
  "SiO",
  "H2SO4",
  "TiO",
  "VO",
  "FeH",
] as const;

/** The registry's other molecules, each under H₂S's dipole (the science check of R08.T10.d). */
const NON_DIPOLAR_MOLECULES = [
  "H2",
  "CH4",
  "N2",
  "O2",
  "CO2",
  "N2O",
  "CO",
  "O3",
  "C2H2",
  "C2H4",
  "C2H6",
  "PH3",
  "S8",
] as const;

/** A mixture of H₂ and He at a giant's 85 : 15 with a species at its own mole fraction. */
function giantMixture(species: string, ownFraction: number): ReadonlyMap<string, number> {
  return new Map([
    ["H2", 0.85 * (1 - ownFraction)],
    ["He", 0.15 * (1 - ownFraction)],
    [species, ownFraction],
  ]);
}

describe("a k-distribution table", () => {
  it("joins its parts into one table, its cells in order", () => {
    const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, 1, {
      broadening: { self: 1.005, CO2: 1.18 },
      parts: [
        { file: "toy-1.bin", cells: 1, bytes: 0, sha256: "" },
        { file: "toy-2.bin", cells: 1, bytes: 0, sha256: "" },
      ],
    });
    const grid = {
      variants: 1,
      temperatures: TEMPERATURES_K.length,
      pressures: PRESSURES_PA.length,
    };
    const table = readKDistribution(
      sidecar,
      [toyPart([toyCell(690)], grid), toyPart([toyCell(700)], grid)],
      "toy",
    );
    expect([table.cells.map((c) => c.cellNm), [...table.broadening]]).toEqual([
      [690, 700],
      [
        ["self", 1.005],
        ["CO2", 1.18],
      ],
    ]);
  });

  it.each<[string, Partial<KDistributionFile>, string]>([
    ["an unknown basis", { basis: "guessed" }, "basis"],
    ["temperatures that do not ascend", { temperaturesK: [200, 100, 300, 350, 400] }, "ascend"],
    ["a measured range beyond its temperatures", { measuredRangeK: [50, 296] }, "measured range"],
    ["a broadening ratio of 0", { broadening: { self: 0 } }, "broadening by self"],
    ["a shape record of the wrong length", { measuredShapeTo: [1] }, "shape record"],
  ])("refuses %s", (_, overrides, message) => {
    expect(() => toyTable(overrides)).toThrow(message);
  });

  it.each<[string, ToyCell, string]>([
    ["a cell outside the bake range", { ...toyCell(690), nm: 770 }, "770 nm"],
    ["weights that do not sum to 1", { ...toyCell(690), weights: [0.75, 0.2] }, "summing to 1"],
  ])("refuses %s", (_, cell, message) => {
    expect(() => toyTable({}, [cell])).toThrow(message);
  });

  it("refuses a part that is not its sidecar's: a missing part, another grid, bytes past its cells", () => {
    const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, 1);
    const grid = {
      variants: 1,
      temperatures: TEMPERATURES_K.length,
      pressures: PRESSURES_PA.length,
    };
    const part = toyPart([toyCell(690)], grid);
    expect(() => readKDistribution(sidecar, [], "toy")).toThrow("parts");
    expect(() =>
      readKDistribution(sidecar, [toyPart([toyCell(690)], { ...grid, pressures: 5 })], "toy"),
    ).toThrow("header");
    const longer = new Uint8Array(part.byteLength + 2);
    longer.set(new Uint8Array(part));
    expect(() => readKDistribution(sidecar, [longer.buffer], "toy")).toThrow("past its cells");
    expect(() => readKDistribution(sidecar, [new ArrayBuffer(16)], "toy")).toThrow(
      "not a k-distribution part",
    );
  });
});

describe("a table's near-wing shares (R08.T10.a's held-χ bound)", () => {
  const nodes = TEMPERATURES_K.length * PRESSURES_PA.length;

  it("are carried one a node where the sidecar records them", () => {
    const shares = Array.from({ length: nodes }, (_, k) => k / 100);
    expect(Array.from(toyTable({ nearWingShare: shares }).nearWingShare ?? [])).toEqual(shares);
  });

  it("are absent where the sidecar records none", () => {
    expect(toyTable().nearWingShare).toBeUndefined();
  });

  it("refuse a record that is not one a node", () => {
    const shares = Array.from({ length: nodes - 1 }, () => 0.01);
    expect(() => toyTable({ nearWingShare: shares })).toThrow("near-wing shares");
  });

  it("refuse a share past 1", () => {
    const shares = Array.from({ length: nodes }, (_, k) => (k === 7 ? 1.2 : 0.01));
    expect(() => toyTable({ nearWingShare: shares })).toThrow("near-wing shares");
  });
});

describe("a k-distribution between its nodes", () => {
  const table = toyTable();

  it("takes ln σ by 4-point Lagrange in T, exact for a cubic but for the 16-bit step, held beyond", () => {
    const at = (t: number): number =>
      kDistributionAt(table, t).cells[0]?.sigmas[3 * 2] ?? Number.NaN;
    for (const t of [125, 175, 275]) {
      expect(Math.abs(at(t) / toySigma(t, 1e4, 0) - 1)).toBeLessThan(STEP_RELATIVE);
    }
    expect([at(50), at(400)]).toEqual([at(100), at(300)]);
  });

  it("is measured within its range and basis, estimated beyond", () => {
    expect([150, 299, 350].map((t) => kDistributionAt(table, t).provenance)).toEqual([
      "measured",
      "estimated",
      "estimated",
    ]);
  });

  it("gives the line shape's measured pressure as the lower of the two nodes' about it", () => {
    // Nodes 150 and 200 K measure their shape to nodes 5 and 4: 10⁵ Pa between them.
    expect(kDistributionAt(table, 175).measuredShapeToPa).toBe(1e5);
  });

  it("takes ln σ by 4-point Lagrange in ln p off the nodes, exact at a node, held beyond", () => {
    const kd = kDistributionAt(table, 200);
    for (const p of [3e2, 5e3, 2e5]) {
      const [g0, g1] = cellSigmasAt(kd, p)[0] ?? new Float64Array(0);
      expect(Math.abs((g0 ?? 0) / toySigma(200, p, 0) - 1)).toBeLessThan(STEP_RELATIVE);
      expect(Math.abs((g1 ?? 0) / toySigma(200, p, 1) - 1)).toBeLessThan(STEP_RELATIVE);
    }
    expect(Array.from(cellSigmasAt(kd, 1e3)[0] ?? [])).toEqual(
      Array.from(kd.cells[0]?.sigmas.slice(4, 6) ?? []),
    );
    expect(cellSigmasAt(kd, 1)).toEqual(cellSigmasAt(kd, 1e1));
    expect(cellSigmasAt(kd, 1e8)).toEqual(cellSigmasAt(kd, 1e6));
  });

  it("gives a cell's transmittance Σ w e^(−σu) and its mean σ Σ w σ", () => {
    const w = Float64Array.from([0.75, 0.25]);
    const s = Float64Array.from([1e-26, 4e-26]);
    expect(cellTransmittance(w, s, 5e25)).toBeCloseTo(
      0.75 * Math.exp(-0.5) + 0.25 * Math.exp(-2),
      14,
    );
    expect(cellMeanSigma(w, s)).toBeCloseTo(1.75e-26, 38);
  });
});

describe("the broadening factor", () => {
  const table = toyTable();

  it("weights each gas's ratio by its mole fraction, the species' own as self, N₂ as air", () => {
    const air = new Map([
      ["O2", 0.2],
      ["N2", 0.7],
      ["CO2", 0.1],
    ]);
    const factor = 0.2 * 1.005 + 0.7 + 0.1 * 1.18;
    const found = broadeningFactor(table, air, 296);
    expect(found.factor).toBeCloseTo(factor, 14);
    expect(found.selfShare).toBeCloseTo((0.2 * 1.005) / factor, 14);
    expect([found.estimated, found.widthlessShare]).toEqual([false, 0]);
  });

  it("takes a gas with no width as air, its share of the broadening pressure for the effect bound, not estimated (R08.T10.c)", () => {
    expect(broadeningFactor(table, new Map([["H2", 2]]), 296)).toEqual({
      factor: 1,
      selfShare: 0,
      estimated: false,
      widthlessShare: 1,
    });
    // ε = Σ_j x_j ÷ Σ_b x_b r_b, CH₄ the gas with no width, the mole fractions normalised.
    const found = broadeningFactor(
      table,
      new Map([
        ["O2", 0.2],
        ["N2", 0.7],
        ["CO2", 0.08],
        ["CH4", 0.02],
      ]),
      296,
    );
    const factor = 0.2 * 1.005 + 0.7 + 0.08 * 1.18 + 0.02;
    expect([found.estimated, found.factor, found.widthlessShare]).toEqual([
      false,
      expect.closeTo(factor, 14),
      expect.closeTo(0.02 / factor, 14),
    ]);
  });

  it("is estimated on a gas with a permanent dipole of at least H₂S's it holds no width for, at a part per billion, ε left 0", () => {
    // The five of R08.T10.c, then the seven R08.T10.d adds.
    const found = [...DIPOLAR_GASES].map((gas) => {
      const { estimated, widthlessShare } = broadeningFactor(
        table,
        new Map([
          ["N2", 1],
          [gas, 1e-9],
        ]),
        296,
      );
      return [gas, estimated, widthlessShare];
    });
    expect(found).toEqual(DIPOLAR_GASES.map((gas) => [gas, true, 0]));
  });

  it("takes a gas under H₂S's dipole that it holds no width for into ε, not estimated", () => {
    // CO, whose dipole is about a ninth of H₂S's.
    const { estimated, widthlessShare } = broadeningFactor(
      table,
      new Map([
        ["N2", 1],
        ["CO", 1e-9],
      ]),
      296,
    );
    // ε = x ÷ Σ x r, the mole fractions normalised: CO at 10⁻⁹ in N₂.
    expect([estimated, widthlessShare]).toEqual([false, expect.closeTo(1e-9 / (1 + 1e-9), 20)]);
  });

  it("leaves a table with a self width as it was: its own share takes r_self, outside ε", () => {
    // A dipolar species, SO₂, with its own width on file, at an even share with N₂.
    const found = broadeningFactor(
      toyTable({ species: "SO2", broadening: { self: 1.6 } }),
      new Map([
        ["N2", 0.5],
        ["SO2", 0.5],
      ]),
      296,
    );
    expect([found.estimated, found.widthlessShare, found.factor]).toEqual([
      false,
      0,
      expect.closeTo(0.5 * 1.6 + 0.5, 14),
    ]);
  });

  it("broadens by a dipolar gas's own ratio where the table gives it a width, neither estimated nor weighed", () => {
    const listed = toyTable({ broadening: { self: 1.005, H2O: 1.085 } });
    const found = broadeningFactor(
      listed,
      new Map([
        ["N2", 0.99],
        ["H2O", 0.01],
      ]),
      296,
    );
    expect([found.estimated, found.widthlessShare, found.factor]).toEqual([
      false,
      0,
      expect.closeTo(0.99 + 0.01 * 1.085, 14),
    ]);
  });

  it("refuses a mixture of nothing", () => {
    expect(() => broadeningFactor(table, new Map([["N2", 0]]), 296)).toThrow("above 0");
  });

  it("takes each gas's ratio to the temperature by its exponent offset, air's as 0", () => {
    const hot = toyTable({
      broadening: { self: 1.2, CO2: 1.1 },
      broadeningExponents: { self: -0.1 },
    });
    const found = broadeningFactor(
      hot,
      new Map([
        ["O2", 0.5],
        ["N2", 0.5],
      ]),
      500,
    );
    // O₂, the toy's species, at its own offset; N₂ as air.
    expect(found.factor).toBeCloseTo(0.5 * 1.2 * (296 / 500) ** -0.1 + 0.5, 14);
  });

  it("refuses an exponent offset for a gas it gives no ratio", () => {
    expect(() => toyTable({ broadeningExponents: { He: 0.1 } })).toThrow("exponent offset");
  });
});

describe("a two-variant table (CO₂'s χ sets)", () => {
  /** A cell whose self variant's σ is twice its air variant's at every node. */
  const air = toyCell(716).sigmasM2;
  const table = toyTable(
    { species: "CO2", variants: ["self", "air"], broadening: { self: 1.3, He: 0.6 } },
    [{ nm: 716, weights: [0.75, 0.25], sigmasM2: [...air.map((v) => 2 * v), ...air] }],
  );

  it("mixes its variants at fixed g by the self share of the Lorentz width", () => {
    const at = (share: number): number =>
      kDistributionAt(table, 175, share).cells[0]?.sigmas[7] ?? Number.NaN;
    // σ_self = 2 σ_air, so the mix is (1 + w) σ_air.
    expect(at(0.25) / at(0)).toBeCloseTo(1.25, 3);
    expect(at(1) / at(0)).toBeCloseTo(2, 3);
  });

  it("is estimated on any partner but itself, N₂, O₂ and Ar, whose wing takes CO₂–N₂'s χ", () => {
    const venus = broadeningFactor(
      table,
      new Map([
        ["CO2", 0.965],
        ["N2", 0.035],
      ]),
      296,
    );
    expect([venus.estimated, venus.selfShare]).toEqual([
      false,
      expect.closeTo((0.965 * 1.3) / (0.965 * 1.3 + 0.035), 12),
    ]);
    expect(
      broadeningFactor(
        table,
        new Map([
          ["CO2", 0.1],
          ["He", 0.9],
        ]),
        296,
      ).estimated,
    ).toBe(true);
  });

  it("is estimated on a gas it holds no width for, which the effect bound does not weigh (R08.T10.c)", () => {
    const found = broadeningFactor(
      table,
      new Map([
        ["CO2", 0.95],
        ["N2", 0.04],
        ["CH4", 0.01],
      ]),
      296,
    );
    expect([found.estimated, found.widthlessShare]).toEqual([true, 0]);
  });

  it("reports a held χ where a variant the mix takes holds it, at the nodes about the temperature", () => {
    // CO₂–N₂'s χ held below 295 K, CO₂–CO₂'s below 218 K: [t][v], v = self, air.
    const flagged = toyTable(
      {
        species: "CO2",
        variants: ["self", "air"],
        broadening: { self: 1.3 },
        chiHeld: [
          [true, true],
          [true, true],
          [true, true],
          [false, true],
          [false, false],
        ],
      },
      [{ nm: 716, weights: [0.75, 0.25], sigmasM2: [...air, ...air] }],
    );
    expect([
      kDistributionAt(flagged, 225, 1).chiHeld,
      kDistributionAt(flagged, 275, 1).chiHeld,
      kDistributionAt(flagged, 275, 0.5).chiHeld,
      kDistributionAt(flagged, 300, 0.5).chiHeld,
    ]).toEqual([true, false, true, false]);
  });

  it("refuses held-χ flags that are not one a variant at each temperature", () => {
    expect(() => toyTable({ chiHeld: [[false]] })).toThrow("held-χ flags");
  });

  it("refuses variants other than air alone or self and air, and a part of one variant's codes", () => {
    expect(() => toyTable({ variants: ["air", "self"] })).toThrow("variants");
    const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, 1, { variants: ["self", "air"] });
    const grid = {
      variants: 2,
      temperatures: TEMPERATURES_K.length,
      pressures: PRESSURES_PA.length,
    };
    expect(() => readKDistribution(sidecar, [toyPart([toyCell(690)], grid)], "toy")).toThrow(
      "ends inside",
    );
  });
});

describe("a two-variant table's fixed-g mix record (decision-r08-t4d-kdist-chain.md §§16–17)", () => {
  const air = toyCell(716).sigmasM2;
  const cells = [{ nm: 716, weights: [0.75, 0.25], sigmasM2: [...air.map((v) => 2 * v), ...air] }];
  const shares = [0.05, 0.25, 0.5, 0.75, 0.95];
  const temperaturesK = [100, 200, 300];
  const none = [null, null, null];
  /** The toy CO₂ with a mix record of one cell, `[share][t]` columns cm⁻² (null for none). */
  const withMix = (cell: ReadonlyArray<ReadonlyArray<number | null>>): KDistributionTable =>
    toyTable(
      {
        species: "CO2",
        variants: ["self", "air"],
        broadening: { self: 1.3 },
        mixing: {
          shares,
          temperaturesK,
          worstErrors: [4e-3, 1e-2, 8e-3, 4e-3, 1e-3],
          limitColumnsCm2: [cell],
        },
      },
      cells,
    );
  const table = withMix([[7e28, null, null], [1e29, null, 3e29], [1.5e29, 2e29, null], none, none]);

  it("reads each cell's u_mix by share and temperature in m⁻², none as ∞", () => {
    expect(Array.from(table.mixLimits?.columnsM2[0]?.[1] ?? [])).toEqual([1e33, Infinity, 3e33]);
  });

  it("takes the least over the two tested temperatures bracketing the body's", () => {
    // w = 0.25 is a recorded share, so ln u is taken there to rounding.
    expect(mixLimitColumnM2(table, 0, 0.25, 150) / 1e33).toBeCloseTo(1, 12);
    expect(mixLimitColumnM2(table, 0, 0.25, 250) / 3e33).toBeCloseTo(1, 12);
  });

  it("takes the end temperature's beyond the tested ones, as a held temperature", () => {
    expect(mixLimitColumnM2(table, 0, 0.25, 50) / 1e33).toBeCloseTo(1, 12);
    expect(mixLimitColumnM2(table, 0, 0.25, 700) / 3e33).toBeCloseTo(1, 12);
  });

  it("holds the end shares' values beyond them", () => {
    expect([mixLimitColumnM2(table, 0, 0.01, 100), mixLimitColumnM2(table, 0, 0.99, 100)]).toEqual([
      7e32,
      Infinity,
    ]);
  });

  it("takes ln u linear in w between two recorded shares", () => {
    expect(mixLimitColumnM2(table, 0, 0.375, 100) / Math.sqrt(1e33 * 1.5e33)).toBeCloseTo(1, 12);
  });

  it("takes the recorded column between a share that records one and one that records none", () => {
    expect(mixLimitColumnM2(table, 0, 0.6, 100)).toBe(1.5e33);
  });

  it("gives ∞ for a table that records no mix", () => {
    const plain = toyTable({ species: "CO2", variants: ["self", "air"] }, cells);
    expect(mixLimitColumnM2(plain, 0, 0.5, 250)).toBe(Infinity);
  });

  it("refuses a record whose rows are not one a cell", () => {
    expect(() =>
      toyTable(
        {
          species: "CO2",
          variants: ["self", "air"],
          mixing: { shares, temperaturesK, worstErrors: [0, 0, 0, 0, 0], limitColumnsCm2: [] },
        },
        cells,
      ),
    ).toThrow("mix record");
  });

  it("refuses a record on a table of one variant", () => {
    expect(() =>
      toyTable({
        mixing: {
          shares,
          temperaturesK,
          worstErrors: [0, 0, 0, 0, 0],
          limitColumnsCm2: [[none, none, none, none, none]],
        },
      }),
    ).toThrow("mix record");
  });

  it("refuses a column that is not finite and above 0", () => {
    expect(() => withMix([[0, null, null], none, none, none, none])).toThrow("mix record");
  });

  it("refuses a row that is not a column at each tested temperature", () => {
    expect(() => withMix([[1e29, null], none, none, none, none])).toThrow("mix record");
  });
});

describe("a table's estimated temperature intervals", () => {
  const table = toyTable({ estimatedTemperaturesK: [[100, 150]] });

  it("make the table estimated within them and measured beyond", () => {
    expect([
      kDistributionAt(table, 125).provenance,
      kDistributionAt(table, 125).inEstimatedInterval,
    ]).toEqual(["estimated", true]);
    expect(kDistributionAt(table, 200).provenance).toBe("measured");
  });

  it("are two of the table's temperatures", () => {
    expect(() => toyTable({ estimatedTemperaturesK: [[110, 150]] })).toThrow("estimated interval");
  });
});

describe("a table's curve rows (R08.T4.e's economy E3)", () => {
  const thinned = STANDARD_PRESSURES_PA.filter((p, k) => p > 1e6 * (1 - 1e-9) || k % 2 === 0);

  it("are the standard pressures over a thinned table's span", () => {
    expect([thinned.length, curveRowsPa(thinned)]).toEqual([20, STANDARD_PRESSURES_PA]);
  });

  it("are the standard pressures for a sidecar's thinned pressures, written to six figures", () => {
    // A sidecar writes 177,827.94… Pa as 177,828: its nodes still take the standard rows.
    const written = thinned.map((p) => Number(p.toPrecision(6)));
    expect(curveRowsPa(written)).toEqual(STANDARD_PRESSURES_PA);
  });

  it("start at a thinned table's raised first node (E4)", () => {
    const raised = thinned.slice(1);
    expect(curveRowsPa(raised)).toEqual(STANDARD_PRESSURES_PA.slice(2));
  });

  it("are a table's own nodes where it holds every standard pressure, or any other", () => {
    const own = [10, 50, 300];
    expect([curveRowsPa(STANDARD_PRESSURES_PA), curveRowsPa(own)]).toEqual([
      STANDARD_PRESSURES_PA,
      own,
    ]);
  });

  it("are 30: 4 a decade from 10 Pa to 1 MPa, 8 a decade to 10 MPa, and 13.3 MPa", () => {
    expect([
      STANDARD_PRESSURES_PA.length,
      (STANDARD_PRESSURES_PA[20] ?? Number.NaN) / 1e6,
      (STANDARD_PRESSURES_PA[28] ?? Number.NaN) / 1e7,
      (STANDARD_PRESSURES_PA[29] ?? Number.NaN) / 1.333_521e7,
    ]).toEqual([30, expect.closeTo(1, 9), expect.closeTo(1, 9), expect.closeTo(1, 6)]);
  });
});

describe("the committed k-distributions", () => {
  it("load nothing for a species the client holds no table for", async () => {
    await expect(loadKDistributions(["Xx"], inlinePart)).resolves.toEqual([]);
  });

  it("load every species they list, each through its sidecar and binary parts", async () => {
    const species = committedLineSpecies();
    const tables = await loadKDistributions(species, inlinePart);
    expect(tables.map((t) => t.species)).toEqual(species);
  }, 120_000);
});

describe("the dipolar broadeners (R08.T10.d)", () => {
  /** The registry's molecules, by the fixture's `kind`. */
  const molecules = registry.substances.filter((s) => s.kind === "molecule").map((s) => s.key);

  it("are the twelve molecules of the registry with a dipole of at least H₂S's", () => {
    expect([...DIPOLAR_BROADENERS]).toEqual(DIPOLAR_GASES);
  });

  it("hold only the registry's molecules", () => {
    expect(DIPOLAR_GASES.filter((gas) => !molecules.includes(gas))).toEqual([]);
  });

  it("leave out only the registry's molecules checked to lie under H₂S's dipole", () => {
    // A molecule added to the registry fails here until its dipole is checked and it is listed.
    expect(molecules.filter((key) => !DIPOLAR_BROADENERS.has(key)).toSorted()).toEqual(
      [...NON_DIPOLAR_MOLECULES].toSorted(),
    );
  });
});

describe("a table with no self width (R08.T10.d)", () => {
  it("makes NH₃'s lines estimated at any share of its own above 0: its table lists H₂ and He alone", async () => {
    const [nh3] = await loadKDistributions(["NH3"], inlinePart);
    if (nh3 === undefined) {
      throw new Error("NH3's k-distribution is committed");
    }
    const at = (x: number): boolean => broadeningFactor(nh3, giantMixture("NH3", x), 150).estimated;
    expect([[...nh3.broadening.keys()], [1e-9, 1e-4, 0.5, 1].map(at), at(0)]).toEqual([
      ["H2", "He"],
      [true, true, true, true],
      false,
    ]);
  }, 120_000);

  it("makes a neutral atom's lines estimated at any share of its own above 0: Na's table holds no width of its own", async () => {
    const [na] = await loadKDistributions(["Na"], inlinePart);
    if (na === undefined) {
      throw new Error("Na's k-distribution is committed");
    }
    const at = (x: number): boolean => broadeningFactor(na, giantMixture("Na", x), 1_500).estimated;
    expect([na.broadening.has("self"), na.broadening.has("Na"), [1e-9, 1e-6, 0.5].map(at)]).toEqual(
      [false, false, [true, true, true]],
    );
  }, 120_000);

  it("adds an ion's own share to ε, taken as H₂ and not estimated: Mg+'s table holds no width of its own", async () => {
    const [ion] = await loadKDistributions(["Mg+"], inlinePart);
    if (ion === undefined) {
      throw new Error("Mg+'s k-distribution is committed");
    }
    const x = 1e-6;
    const temperatureK = 4_500;
    const found = broadeningFactor(ion, giantMixture("Mg+", x), temperatureK);
    // Σ x r at the temperature, Mg+'s own r 1 (H₂'s, the atoms' tables' reference) and offset 0.
    const ratioAt = (gas: string): number =>
      (ion.broadening.get(gas) ?? Number.NaN) *
      (296 / temperatureK) ** (ion.broadeningExponents.get(gas) ?? 0);
    const sum = 0.85 * (1 - x) * ratioAt("H2") + 0.15 * (1 - x) * ratioAt("He") + x;
    expect([found.estimated, found.widthlessShare / (x / sum)]).toEqual([
      false,
      expect.closeTo(1, 12),
    ]);
  }, 120_000);
});

describe("a species' own width under its own key (R08.T10.d)", () => {
  it("broadens H's lines by H's own width, 2.198 times H₂'s: 1.531 in H 0.5, H₂ 0.35 and He 0.15", async () => {
    const [h] = await loadKDistributions(["H"], inlinePart);
    if (h === undefined) {
      throw new Error("H's k-distribution is committed");
    }
    const mixture = new Map([
      ["H", 0.5],
      ["H2", 0.35],
      ["He", 0.15],
    ]);
    // H's exponent offsets are 0, so the factor is the same at every temperature.
    const found = broadeningFactor(h, mixture, 4_000);
    expect([found.factor, found.widthlessShare, found.estimated]).toEqual([
      expect.closeTo(0.5 * 2.198 + 0.35 + 0.15 * 0.5488, 12),
      0,
      false,
    ]);
  }, 120_000);

  it("takes self, its ratio and its exponent offset, over its own key's entry where a table lists both", () => {
    const both = toyTable({
      species: "SO2",
      broadening: { self: 1.6, SO2: 3 },
      broadeningExponents: { self: -0.25, SO2: 0.4 },
    });
    const found = broadeningFactor(
      both,
      new Map([
        ["N2", 0.5],
        ["SO2", 0.5],
      ]),
      500,
    );
    expect(found.factor).toBeCloseTo(0.5 * 1.6 * (296 / 500) ** -0.25 + 0.5, 14);
  });

  it("takes its own key's ratio and exponent offset where a table lists no self", () => {
    const own = toyTable({
      species: "SO2",
      broadening: { SO2: 3 },
      broadeningExponents: { SO2: 0.4 },
    });
    const found = broadeningFactor(
      own,
      new Map([
        ["N2", 0.5],
        ["SO2", 0.5],
      ]),
      500,
    );
    expect(found.factor).toBeCloseTo(0.5 * 3 * (296 / 500) ** 0.4 + 0.5, 14);
  });
});

describe("the self-broadened atoms (R08.T10.d)", () => {
  it("are the registry's neutral atoms whose lines a committed table holds", () => {
    const atoms = new Set(registry.substances.filter((s) => s.kind === "atom").map((s) => s.key));
    expect([...SELF_BROADENED_ATOMS]).toEqual(
      committedLineSpecies().filter((species) => atoms.has(species)),
    );
  });
});

describe("the worker's part reader", () => {
  it("fetches a committed part through its bundled URL", async () => {
    const bytes = Uint8Array.of(1, 2, 3, 4);
    const fetched = vi.fn<(url: string) => Promise<Response>>(async () => new Response(bytes));
    vi.stubGlobal("fetch", fetched);
    const buffer = await fetchPart("kDistributions/O2-1.bin");
    expect([Array.from(new Uint8Array(buffer)), fetched.mock.calls[0]?.[0]]).toEqual([
      [1, 2, 3, 4],
      expect.stringMatching(/O2-1\.bin/u),
    ]);
  });

  it("fetches an ion's part, a + in its file name, through its bundled URL (R08.T4.e)", async () => {
    // Mg⁺'s is the first ion's table: + is a plain character in a URL's path and in a file name
    // on Linux, macOS (APFS) and Windows (NTFS), which forbid only <>:"/\|?* of the printable ones.
    const fetched = vi.fn<(url: string) => Promise<Response>>(
      async () => new Response(Uint8Array.of(7)),
    );
    vi.stubGlobal("fetch", fetched);
    await fetchPart("kDistributions/Mg+-1.bin");
    expect(fetched.mock.calls[0]?.[0]).toMatch(/Mg\+-1\.bin/u);
  });

  it("refuses a part the client does not hold, without fetching", async () => {
    const fetched = vi.fn<(url: string) => Promise<Response>>(
      async () => new Response(new Uint8Array(0)),
    );
    vi.stubGlobal("fetch", fetched);
    await expect(fetchPart("kDistributions/Xx-1.bin")).rejects.toThrow(
      "no committed k-distribution part",
    );
    expect(fetched).not.toHaveBeenCalled();
  });

  it("refuses a fetch that fails, naming its status", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<(url: string) => Promise<Response>>(async () => new Response(null, { status: 404 })),
    );
    await expect(fetchPart("kDistributions/O2-1.bin")).rejects.toThrow("failed: 404");
  });
});
