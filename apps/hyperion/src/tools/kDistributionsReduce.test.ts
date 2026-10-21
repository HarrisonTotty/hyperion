import { describe, expect, it, vi } from "vitest";

import {
  belowGridError,
  CHI_CO2_CO2,
  CHI_CO2_N2,
  chiHeldFlags,
  CURVE_DEPTH_MAX,
  encodeCell,
  lineByLine,
  MIX_SHARES,
  quantisationError,
  type RayleighCrossSection,
  READ_TOP_PA,
  reduceGridCell,
  type SaturationCurve,
  type ShapePolicy,
  STORED_PRESSURES_PA,
} from "./kDistributions";
import {
  LINE_SOURCES,
  lineTableSource,
  mergePartials,
  type PartialCell,
  type PartialFile,
  reducePartialCell,
  withLineBroadening,
} from "./kDistributionsCli";
import { backgroundError, referenceStates } from "./lineReferences";
import {
  cellWavenumbersPerCm,
  ladderLines,
  type LineList,
  type LineSpecies,
} from "./molecularLines";

/**
 * A toy species: lines of ¹⁶O₂ (isotopologue 36) at the given centres and intensity, γ_air 0.05 and
 * γ_self 0.06 cm⁻¹ atm⁻¹, n_air 0.7, E″ 100 cm⁻¹, and γ_CO2 0.08 where `withCo2`; its partition
 * sum 1 at every kelvin.
 */
function toySpecies(
  centres: ReadonlyArray<number>,
  intensity: number,
  withCo2 = false,
): LineSpecies {
  const n = centres.length;
  const fill = (v: number): Float64Array => new Float64Array(n).fill(v);
  const lines: LineList = {
    isotopologues: new Uint16Array(n).fill(36),
    wavenumbersPerCm: Float64Array.from(centres).toSorted(),
    intensitiesCm: fill(intensity),
    lowerEnergiesPerCm: fill(100),
    airHalfWidthsPerCmAtm: fill(0.05),
    airExponents: fill(0.7),
    selfHalfWidthsPerCmAtm: fill(0.06),
    foreignHalfWidthsPerCmAtm: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(withCo2 ? 0.08 : Number.NaN),
      H2O: fill(Number.NaN),
    },
    selfExponents: fill(Number.NaN),
    foreignExponents: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(Number.NaN),
      H2O: fill(Number.NaN),
    },
  };
  return {
    lines,
    partitionSums: new Map([[36, { firstK: 1, values: new Float64Array(1_000).fill(1) }]]),
  };
}

/** The plain shape: Voigt within 25 cm⁻¹, estimated past γ_L = 0.25 cm⁻¹. */
const PLAIN: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: 25,
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/** A Rayleigh cross-section whose column, −ln 10⁻⁶ ÷ σ_R, is 10²¹ cm⁻² at every wavelength. */
const SCATTERING: RayleighCrossSection = () => CURVE_DEPTH_MAX / 1e21;

/** A small grid: four temperatures and pressures that pass the read's top. */
const GRID = {
  temperaturesK: [200, 250, 300, 350],
  pressuresPa: [1e5, 1e6, 3.162e6, 1e7, 1.333e7],
};

const [LOW, HIGH] = cellWavenumbersPerCm(690);
const CENTRES = [LOW + 4, LOW + 9.5, HIGH - 3];

describe("a cell's reduction on its grid", () => {
  it("stores a cell whose peak is under 0.02 at its gated column as its mean alone (G = 1)", () => {
    const weak = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-30), PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    expect([weak?.weights.length, (weak?.worstNodeError ?? 1) <= 2e-4]).toEqual([1, true]);
  });

  it("keeps a saturating cell's g-points, its chain test stopping at the read's top", () => {
    const strong = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-22), PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    expect(strong?.weights.length).toBeGreaterThan(2);
    const tested = [strong?.worstChain, strong?.worstBeyondCap, ...(strong?.failures ?? [])];
    expect(Math.max(...tested.map((e) => e?.pressurePa ?? 0))).toBeLessThanOrEqual(READ_TOP_PA);
  });

  it("records each temperature's last pressure node whose cell's γ_L is under the policy's", () => {
    // γ_L = 0.05 (296 ÷ T)^0.7 p ÷ 1 atm passes 0.25 cm⁻¹ between 0.1 and 1 MPa at every T here
    // (0.44 cm⁻¹ at 1 MPa and 350 K).
    const cell = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-24), PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    expect(Array.from(cell?.measuredShapeTo ?? [])).toEqual([0, 0, 0, 0]);
  });

  it("tests a two-variant cell's mix of its χ sets, and none for a plain one", () => {
    const chi: ShapePolicy = {
      variants: [
        { broadener: "self", chi: CHI_CO2_CO2 },
        { broadener: "air", chi: CHI_CO2_N2 },
      ],
      cutoffPerCm: 60,
      pedestal: false,
      estimatedAboveHalfWidthPerCm: 0.25,
    };
    const mixed = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-23), chi),
      GRID,
      chi,
      () => 1e25,
      () => 1e25,
    );
    const plain = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-23), PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    expect([mixed?.mixing === undefined, plain?.mixing]).toEqual([false, undefined]);
  });

  it("records a two-variant cell's mix at each of §16's five self shares, not gating it", () => {
    const chi: ShapePolicy = {
      variants: [
        { broadener: "self", chi: CHI_CO2_CO2 },
        { broadener: "air", chi: CHI_CO2_N2 },
      ],
      cutoffPerCm: 60,
      pedestal: false,
      estimatedAboveHalfWidthPerCm: 0.25,
    };
    const mixed = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-23), chi),
      GRID,
      chi,
      () => 1e25,
      () => 1e25,
    );
    const limits = mixed?.mixing?.limitColumnsCm2 ?? [];
    expect([MIX_SHARES, mixed?.mixing?.worst.length, limits.every((u) => u > 0)]).toEqual([
      [0.05, 0.25, 0.5, 0.75, 0.95],
      5,
      true,
    ]);
  });

  it("flags each temperature node whose variants hold their χ, beyond each set's fit", () => {
    const chi: ShapePolicy = {
      variants: [
        { broadener: "self", chi: CHI_CO2_CO2 },
        { broadener: "air", chi: CHI_CO2_N2 },
      ],
      cutoffPerCm: 60,
      pedestal: false,
      estimatedAboveHalfWidthPerCm: 0.25,
    };
    // CO₂–CO₂ is fitted at 218–751 K and CO₂–N₂ at 295–773 K.
    expect(chiHeldFlags(chi, [150, 250, 300])).toEqual([
      [true, true],
      [false, true],
      [false, false],
    ]);
    expect(chiHeldFlags(PLAIN, [150, 300])).toEqual([[false], [false]]);
  });

  it("marks no node estimated for a held χ, R08.T10.a bounding it on the body's own column", () => {
    const chi: ShapePolicy = {
      variants: [
        { broadener: "self", chi: CHI_CO2_CO2 },
        { broadener: "air", chi: CHI_CO2_N2 },
      ],
      cutoffPerCm: 60,
      pedestal: false,
      estimatedAboveHalfWidthPerCm: 0.25,
    };
    const grid = { temperaturesK: [150, 200, 250, 300], pressuresPa: [10, 31.6, 100] };
    const cell = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-22), chi),
      grid,
      chi,
      () => 1e25,
      () => 1e25,
    );
    expect(Array.from(cell?.measuredShapeTo ?? [])).toEqual([2, 2, 2, 2]);
  });

  it("holds the 10 Pa node at 1 Pa and its 16-bit values to their f64 ones", () => {
    const species = toySpecies(CENTRES, 1e-23);
    const grid = { temperaturesK: [200, 250, 300, 350], pressuresPa: [10, 100, 1e3, 1e4] };
    const cell = reduceGridCell(
      690,
      lineByLine(species, PLAIN),
      grid,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    if (cell === undefined) {
      throw new Error("the toy cell has no lines");
    }
    expect(
      belowGridError(690, lineByLine(species, PLAIN), grid, PLAIN, cell, 31.98983),
    ).toBeLessThan(2e-3);
    expect(quantisationError(cell, encodeCell(cell), 16, () => 1e25).within).toBeLessThan(2e-4);
  });

  describe("a held first node's check (R08.T4.e's economy E4)", () => {
    const species = toySpecies(CENTRES, 1e-23);
    const grid = { temperaturesK: [200, 250, 300, 350], pressuresPa: [10, 100, 1e3, 1e4] };
    const cell = reduceGridCell(
      690,
      lineByLine(species, PLAIN),
      grid,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    if (cell === undefined) {
      throw new Error("the toy cell has no lines");
    }
    const errorOf = (of: typeof cell, checksPa: ReadonlyArray<number>, heldNode: number): number =>
      belowGridError(
        690,
        lineByLine(species, PLAIN),
        grid,
        PLAIN,
        of,
        31.98983,
        checksPa,
        heldNode,
      );

    it("is the worst over every pressure it is checked at", () => {
      expect(errorOf(cell, [1, 5], 0)).toBe(Math.max(errorOf(cell, [1], 0), errorOf(cell, [5], 0)));
    });

    it("holds the node it is given, as a table whose first node that is", () => {
      // The 100 Pa node's values in the 10 Pa node's place, at every temperature.
      const g = cell.weights.length;
      const nP = grid.pressuresPa.length;
      const raised = Float64Array.from(cell.values);
      for (let t = 0; t < grid.temperaturesK.length; t += 1) {
        raised.set(cell.values.subarray((t * nP + 1) * g, (t * nP + 2) * g), t * nP * g);
      }
      expect(errorOf(cell, [1], 1)).toBe(errorOf({ ...cell, values: raised }, [1], 0));
    });
  });

  it("reads a row that is not a stored node between the nodes, even at a node's temperature", () => {
    // Two stored nodes and a row between them, read linearly in σ: the row at a node's temperature
    // is a state of the chain test, and fails it.
    const grid = {
      temperaturesK: [200, 250, 300, 350],
      pressuresPa: [1e5, 1e7],
      rowsPa: [1e5, 1e6, 1e7],
    };
    const cell = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-22), PLAIN),
      grid,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    const atRow = (cell?.failures ?? []).filter(
      (f) => f.pressurePa === 1e6 && grid.temperaturesK.includes(f.temperatureK),
    );
    expect(atRow.length).toBeGreaterThan(0);
  });

  it("gates the 16-bit test within each node's cap, and records it beyond", () => {
    const weak = reduceGridCell(
      690,
      lineByLine(toySpecies(CENTRES, 1e-30), PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    if (weak === undefined) {
      throw new Error("the toy cell has no lines");
    }
    // ln σ over 600 across the nodes: a step of about 10⁻², felt only where τ nears 1.
    const wide = Object.assign({}, weak, {
      values: Float64Array.from(weak.values, (v, i) => v * Math.exp(-30 * i)),
    });
    const largest = Math.max(...wide.values);
    const error = quantisationError(wide, encodeCell(wide), wide.values.length, () => {
      // A cap where the largest σ takes τ = 0.02, in cm⁻²: σ m² × u m⁻², u = cap × 10⁴.
      return 0.02 / largest / 1e4;
    });
    expect([error.within <= 2e-4, error.beyond > error.within]).toEqual([true, true]);
  });

  it("stops the between-node test at its own cap, the nodes and their g-points as at the node cap", () => {
    const species = toySpecies(CENTRES, 1e-22);
    const wide = reduceGridCell(
      690,
      lineByLine(species, PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e25,
    );
    const narrow = reduceGridCell(
      690,
      lineByLine(species, PLAIN),
      GRID,
      PLAIN,
      () => 1e25,
      () => 1e21,
    );
    expect([
      Array.from(narrow?.values ?? []).join() === Array.from(wide?.values ?? []).join(),
      (wide?.worstChain.columnCm2 ?? 0) > 1e21,
      (narrow?.worstChain.columnCm2 ?? Infinity) <= 1e21,
      (narrow?.worstBeyondCap.error ?? 0) >= (wide?.worstBeyondCap.error ?? 0),
    ]).toEqual([true, true, true, true]);
  });

  it("stops a partial cell's between-node test at its species' own Rayleigh column", () => {
    vi.spyOn(process.stdout, "write").mockImplementation(() => true);
    const species = toySpecies(CENTRES, 1e-22);
    const source = { species: "O2", policy: PLAIN, molecularMassDa: 31.98983 };
    const helium = reducePartialCell(690, lineByLine(species, PLAIN), GRID, source);
    const own = reducePartialCell(690, lineByLine(species, PLAIN), GRID, {
      ...source,
      rayleighCm2: SCATTERING,
    });
    expect([
      (helium?.partial.worstChain.columnCm2 ?? 0) > 1e21,
      (own?.partial.worstChain.columnCm2 ?? Infinity) <= 1e21,
      own?.partial.k === helium?.partial.k,
    ]).toEqual([true, true, true]);
  });

  it("stops a partial cell's between-node test at its condensing cap alone, its g-points as without it", () => {
    vi.spyOn(process.stdout, "write").mockImplementation(() => true);
    const species = toySpecies(CENTRES, 1e-22);
    const source = { species: "O2", policy: PLAIN, molecularMassDa: 31.98983 };
    // p_sat 6.64 × 10⁻⁴ Pa at every temperature: 400 p_sat ÷ (m g) at 0.5 m s⁻² is 10²¹ cm⁻².
    const condensing: SaturationCurve = {
      kind: "antoine",
      a: Math.log10(((1e25 * 31.98983 * 1.660_539_066_6e-27 * 0.5) / 400) * 1e-5),
      bK: 0,
      cK: 0,
      fromK: 100,
      criticalK: 1_000,
      source: "a test's",
    };
    const open = reducePartialCell(690, lineByLine(species, PLAIN), GRID, source);
    const capped = reducePartialCell(690, lineByLine(species, PLAIN), GRID, {
      ...source,
      chainSaturation: condensing,
    });
    expect([
      (open?.partial.worstChain.columnCm2 ?? 0) > 1e21,
      (capped?.partial.worstChain.columnCm2 ?? Infinity) <= 1e21 * (1 + 1e-9),
      capped?.partial.k === open?.partial.k,
    ]).toEqual([true, true, true]);
  });

  it("finds no lines in a cell beyond every line's cut-off", () => {
    expect(
      reduceGridCell(
        500,
        lineByLine(toySpecies(CENTRES, 1e-23), PLAIN),
        GRID,
        PLAIN,
        () => 1e25,
        () => 1e25,
      ),
    ).toBeUndefined();
  });
});

/** A HITRAN `.par` record of PH₃ (molecule 28, local 1) in its fixed columns. */
function parRecord(nu: number, s: number, upper: string, lower: string): string {
  return (
    "28" +
    "1" +
    nu.toFixed(6).padStart(12) +
    s.toExponential(3).toUpperCase().padStart(10) +
    "1.000E-01".padStart(10) +
    "0.0500".slice(1).padStart(5) +
    "0.060".padStart(5) +
    "100.0000".padStart(10) +
    "0.70".padStart(4) +
    "0.000000".padStart(8) +
    upper.padStart(15) +
    lower.padStart(15) +
    " ".repeat(63)
  );
}

describe("an overtone ladder's lines", () => {
  it("move the anchor band to the target centroid and divide its intensity by 10 a quantum", () => {
    const text = [
      parRecord(4_570, 3e-20, "1 0 1 0", "0 0 0 0"),
      parRecord(4_580, 1e-20, "1 0 1 0", "0 0 0 0"),
      parRecord(4_600, 9e-20, "0 1 1 0", "0 0 0 0"),
    ].join("\n");
    const lines = ladderLines(text, {
      isotopologue: 79,
      upper: "1 0 1 0",
      lower: "0 0 0 0",
      targetPerCm: 13_700,
      quanta: 5,
    });
    // The band's centroid is 4,572.5 cm⁻¹: its lines land 2.5 below and 7.5 above the target.
    expect(Array.from(lines.wavenumbersPerCm)).toEqual([13_697.5, 13_707.5]);
    const [first, second] = lines.intensitiesCm;
    expect([(first ?? 0) / 3e-25, (second ?? 0) / 1e-25]).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
    ]);
  });

  it("refuses an anchor file with no line of the band", () => {
    expect(() =>
      ladderLines(parRecord(4_570, 3e-20, "0 1 1 0", "0 0 0 0"), {
        isotopologue: 79,
        upper: "1 0 1 0",
        lower: "0 0 0 0",
        targetPerCm: 13_700,
        quanta: 5,
      }),
    ).toThrow("no line of the band");
  });
});

describe("the cell references' states", () => {
  it("take three node pressures at the node nearest 250 K and three midpoints beside them", () => {
    const states = referenceStates({
      temperaturesK: [200, 225, 250, 300],
      pressuresPa: [1e3, 1.778e3, 1e5, 1.778e5, 1e6, 1.333e6],
    });
    expect(
      states.map((s) => [s.kind, s.temperatureK, Number(s.pressurePa.toPrecision(4))]),
    ).toEqual([
      ["node", 250, 1e3],
      ["midpoint", 275, 1_333],
      ["node", 250, 1e5],
      ["midpoint", 275, 1.333e5],
      ["node", 250, 1e6],
      ["midpoint", 275, 1.155e6],
    ]);
  });
});

describe("a curve on a foreign background", () => {
  it("is 0 at p_eff where the gas's widths are air's times the species' one ratio", () => {
    // Every line's γ_CO2 is 1.6 γ_air, so p r_CO2 with γ_air gives the same widths.
    expect(backgroundError(toySpecies(CENTRES, 1e-22, true), 690, 1e5, "CO2", PLAIN)).toBeLessThan(
      1e-12,
    );
  });

  it("is 0 where HITRAN gives the gas no width, taken as air", () => {
    expect(backgroundError(toySpecies(CENTRES, 1e-22), 690, 1e5, "CO2", PLAIN)).toBe(0);
  });
});

/** A chain test's failure at a temperature, over the chain tier. */
function fail(t: number): PartialCell["failures"][number] {
  return { error: 3e-3, temperatureK: t, pressurePa: 1e5, columnCm2: 1e24 };
}

/** A partial cell with its codes and records, its failures given, of one variant or two. */
function partialCell(
  nm: number,
  failures: PartialCell["failures"] = [],
  variants = 1,
): PartialCell {
  const none = { error: 0, temperatureK: 0, pressurePa: 0, columnCm2: 0 };
  const codes = Buffer.from(new Uint16Array(variants * 2 * 2 * 1).fill(7).buffer).toString(
    "base64",
  );
  return {
    nm,
    w: [1],
    o: -70,
    s: 1e-3,
    k: codes,
    worstNodeError: 1e-4,
    worstChain: none,
    worstBeyondCap: none,
    failures,
    measuredShapeTo: [1, 1],
    beyondCutoffShare: [0, 0, 0, 0],
    nearWingShare: [0, 0, 0, 0],
    quantisation: 1e-5,
    belowGrid: 1e-4,
  };
}

/** A partial of cells on a 2 × 2 grid. */
function partial(cells: ReadonlyArray<PartialCell>, temperaturesK = [100, 150]): PartialFile {
  return {
    species: "CO",
    temperaturesK,
    pressuresPa: [1e4, 1e5],
    broadening: { self: 1.1 },
    broadeningExponents: { self: -0.05 },
    cells,
  };
}

describe("a species' merge", () => {
  const line = LINE_SOURCES["CO"];
  if (line === undefined) {
    throw new Error("the tool lists no CO");
  }
  const source = lineTableSource(line);

  it("asks to refine an interval wider than 12.5 K whose midpoint fails", () => {
    const outcome = mergePartials(
      source,
      [partial([partialCell(698, [fail(125)])])],
      () => undefined,
    );
    expect(outcome).toEqual({ kind: "refine", temperaturesK: [100, 125, 150] });
  });

  it("asks for a lattice midpoint where the read fails between two held 4-a-decade rows", () => {
    // 316 and 562 kPa held, the read through exact rows the larger part of the failure at 421.7 kPa
    // (decision-r08-t4e-unsold's follow-up).
    const rows = [STORED_PRESSURES_PA[18] ?? Number.NaN, STORED_PRESSURES_PA[19] ?? Number.NaN];
    const failure = {
      error: 2.2e-3,
      temperatureK: 100,
      pressurePa: 10 * 10 ** (37 / 8),
      columnCm2: 1e25,
      exactRowsError: 2.1e-3,
    };
    const outcome = mergePartials(
      source,
      [{ ...partial([partialCell(698, [failure])]), pressuresPa: rows }],
      () => undefined,
    );
    expect(outcome).toEqual({
      kind: "refine",
      temperaturesK: [100, 150],
      pressuresPa: [rows[0], expect.closeTo(421_696.5, 1), rows[1]],
    });
  });

  it("refuses a failure at a node's temperature", () => {
    expect(
      mergePartials(source, [partial([partialCell(698, [fail(100)])])], () => undefined).kind,
    ).toBe("refused");
  });

  it("writes a sidecar and parts, a failing 12.5 K interval recorded and estimated", () => {
    const outcome = mergePartials(
      source,
      [partial([partialCell(698, [fail(106.25)]), partialCell(699)], [100, 112.5])],
      () => undefined,
    );
    if (outcome.kind !== "written") {
      throw new Error(`the merge ${outcome.kind}`);
    }
    const sidecar: unknown = JSON.parse(outcome.sidecar);
    expect(sidecar).toMatchObject({
      species: "CO",
      estimatedTemperaturesK: [[100, 112.5]],
      broadeningExponents: { self: -0.05 },
      linearCells: 2,
    });
    expect(outcome.parts.map((p) => p.file)).toEqual(["CO-1.bin"]);
  });

  it("writes a HITRAN source's lines and TIPS files as the sidecar's inputs, under T4.d's description", () => {
    const outcome = mergePartials(source, [partial([partialCell(698)])], () => undefined);
    if (outcome.kind !== "written") {
      throw new Error(`the merge ${outcome.kind}`);
    }
    expect(JSON.parse(outcome.sidecar)).toMatchObject({
      sha256: `${line.lines.name} ${line.lines.sha256}; q26.txt ${line.tips[0]?.sha256 ?? ""}`,
      description: expect.stringMatching(
        /^Line absorbers' k-distributions per 1 nm cell \(rendering plan R08, R08\.T4\.d;/u,
      ),
    });
  });

  it("writes another table source's inputs and description into its sidecar", () => {
    const outcome = mergePartials(
      { ...source, inputs: "a.trans 0123", description: "An ExoMol table's own description." },
      [partial([partialCell(698)])],
      () => undefined,
    );
    if (outcome.kind !== "written") {
      throw new Error(`the merge ${outcome.kind}`);
    }
    expect(JSON.parse(outcome.sidecar)).toMatchObject({
      sha256: "a.trans 0123",
      description: "An ExoMol table's own description.",
    });
  });

  /** A CO₂ cell whose mix errs past the tier at w = 0.25, at the tested temperatures given. */
  function mixedCell(byTemperature: ReadonlyArray<number | null>, temperaturesK: number[]) {
    const worst = MIX_SHARES.map((_, i) => ({
      error: i === 1 ? 1e-2 : 1e-3,
      temperatureK: 250,
      pressurePa: 1e6,
      columnCm2: 1e29,
    }));
    const none = temperaturesK.map(() => null);
    return {
      ...partialCell(715, [], 2),
      mixing: {
        worst,
        limitColumnsCm2: [null, 1e29, null, null, null],
        temperaturesK,
        byTemperature: MIX_SHARES.map((_, i) => (i === 1 ? byTemperature : none)),
      },
    } satisfies PartialCell;
  }

  /** CO₂'s merge of one partial, its sidecar parsed, or a test failure. */
  function co2Sidecar(cells: ReadonlyArray<PartialCell>, temperaturesK: number[]): unknown {
    const co2Line = LINE_SOURCES["CO2"];
    if (co2Line === undefined) {
      throw new Error("the tool lists no CO2");
    }
    const co2 = lineTableSource(co2Line);
    const outcome = mergePartials(
      co2,
      [{ ...partial(cells, temperaturesK), species: "CO2" }],
      () => undefined,
    );
    if (outcome.kind !== "written") {
      throw new Error(`the merge ${outcome.kind}`);
    }
    return JSON.parse(outcome.sidecar);
  }

  it("records a two-variant cell's mix past the tier by tested temperature, not refusing it (§§16–17)", () => {
    const sidecar = co2Sidecar([mixedCell([null, 1e29, 3e29], [250, 275, 300])], [250, 300]);
    expect(sidecar).toMatchObject({
      mixing: {
        shares: [0.05, 0.25, 0.5, 0.75, 0.95],
        temperaturesK: [250, 275, 300],
        worstErrors: [1e-3, 1e-2, 1e-3, 1e-3, 1e-3],
        limitColumnsCm2: [
          [
            [null, null, null],
            [null, 1e29, 3e29],
            [null, null, null],
            [null, null, null],
            [null, null, null],
          ],
        ],
      },
    });
  });

  it("sets no u_mix at a sample inside an estimated temperature interval (§17)", () => {
    // 106.25 K fails the chain at 12.5 K steps, so 100–112.5 K is estimated and its midpoint's
    // u_mix goes; the nodes about it keep theirs.
    const failing: PartialCell = {
      ...mixedCell([1e29, 2e29, 3e29], [100, 106.25, 112.5]),
      failures: [fail(106.25)],
    };
    const sidecar = co2Sidecar([failing], [100, 112.5]);
    expect(sidecar).toMatchObject({
      estimatedTemperaturesK: [[100, 112.5]],
      mixing: {
        limitColumnsCm2: [
          [
            [null, null, null],
            [1e29, null, 3e29],
            [null, null, null],
            [null, null, null],
            [null, null, null],
          ],
        ],
      },
    });
  });

  it("refuses mix records written before §17, with no temperatures", () => {
    const co2Line = LINE_SOURCES["CO2"];
    if (co2Line === undefined) {
      throw new Error("the tool lists no CO2");
    }
    const co2 = lineTableSource(co2Line);
    const old: PartialCell = {
      ...partialCell(715, [], 2),
      mixing: { worst: [], limitColumnsCm2: [null, null, null, null, null] },
    };
    expect(
      mergePartials(co2, [{ ...partial([old], [250, 300]), species: "CO2" }], () => undefined).kind,
    ).toBe("refused");
  });

  it("refuses partials that hold no broadening exponents, written before §13", () => {
    const old: PartialFile = Object.assign({}, partial([partialCell(698)]), {
      broadeningExponents: undefined,
    });
    expect(mergePartials(source, [old], () => undefined).kind).toBe("refused");
  });

  it("refuses partials whose broadening records differ", () => {
    const other: PartialFile = Object.assign({}, partial([partialCell(699)]), {
      broadening: { self: 1.2 },
    });
    expect(mergePartials(source, [partial([partialCell(698)]), other], () => undefined).kind).toBe(
      "refused",
    );
  });

  it("take their broadening records from the species' lines with --lines, their own set aside", () => {
    const lines = toySpecies(CENTRES, 1e-22, true).lines;
    const [p] = withLineBroadening([partial([partialCell(698)])], lines);
    // γ_self 0.06 and γ_CO2 0.08 over γ_air 0.05; every exponent n_air's, so each offset 0.
    expect([p?.broadening, p?.broadeningExponents]).toEqual([
      { self: expect.closeTo(1.2, 12), CO2: expect.closeTo(1.6, 12) },
      { self: expect.closeTo(0, 12), CO2: expect.closeTo(0, 12) },
    ]);
  });

  it("refuses partials on two temperature grids", () => {
    expect(() =>
      mergePartials(
        source,
        [partial([partialCell(698)]), partial([partialCell(699)], [100, 200])],
        () => undefined,
      ),
    ).toThrow("different temperature grids");
  });
});
