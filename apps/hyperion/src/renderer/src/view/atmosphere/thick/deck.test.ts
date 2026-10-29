/**
 * The cloud-deck split's bakes (plan R08, R08.T15.a.1): the deck's reflectance, the table above it
 * by latitude band, and the polarisation correction over it with the deck a depolarising ground.
 */
import { describe, expect, it } from "vitest";

import {
  planckBakeSpectrum,
  saturnScaleHeightM,
  SUN_TEFF_K,
} from "../../../test/atmosphereFixtures";
import {
  conservativeEnvelopeFixture,
  type DeckFixture,
  NH3_DECK_TOP_BAR,
  nh3DeckFixture,
  saturnBakeBody,
} from "../../../test/deckFixtures";
import { BAKE_WAVELENGTHS_NM } from "../medium";
import {
  bakeBinSolve,
  THICK_BAKE_GRID,
  THICK_BAKE_MODES,
  THICK_BAKE_STREAMS,
  type ThickBakeBand,
  thickBakeBands,
  type ThickBakeBody,
  type ThickBakeGrid,
  type ThickBakeLight,
} from "./bake";
import {
  bakeDeck,
  bakeDeckPolarisation,
  type BelowDeckTable,
  DECK_FLOOR_OPTICAL_DEPTH,
  deckColumnSolve,
  deckGround,
  deckPolarisationGround,
  type DeckTables,
  prepareDeck,
  prepareDeckPolarisation,
  type PreparedDeck,
} from "./deck";
import { doubleGaussQuadrature, groundCosinesOf } from "./discreteOrdinates";
import { polarisationSources } from "./polarisation";

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

/** A small grid, cheap to bake: 16 layers, three suns, two view cosines a hemisphere. */
const GRID: ThickBakeGrid = {
  layers: 16,
  altitudeStride: 2,
  sunCosines: Float64Array.of(0.3, 0.7, 1),
  upwardCosines: Float64Array.of(0.25, 0.75),
};

/** The deck's layers on {@link GRID}. */
const DECK_LAYERS = 8;

/** The Sun with no absorber's curves: the fixtures have no absorber term. */
const SUN: ThickBakeLight = { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] };

/** The reference bin, 570 nm: every bin's deck is the same grey cloud. */
const BIN = 7;

function preparedOf(fixture: DeckFixture, floor: "ground" | "envelope"): PreparedDeck {
  return prepareDeck(
    fixture.column,
    [SUN],
    { altitudeM: fixture.deckAltitudeM, floor },
    {
      grid: GRID,
      deckLayers: DECK_LAYERS,
    },
  );
}

/** A body's band, or a test failure. */
function bandOf(body: ThickBakeBody, band: number): ThickBakeBand {
  const geometry = thickBakeBands(body)[band];
  if (geometry === undefined) {
    throw new Error(`no band ${band}`);
  }
  return geometry;
}

/** The unabsorbed term every bin of a column without gases solves. */
function onlyTerm(
  prepared: PreparedDeck,
  bin: number,
): PreparedDeck["upper"]["lights"][number]["bins"][number]["terms"][number] {
  const term = prepared.upper.lights[0]?.bins[bin]?.terms[0];
  if (term === undefined) {
    throw new Error(`bin ${bin} has a term`);
  }
  return term;
}

describe("a thick non-absorbing deck", () => {
  const fixture = once(conservativeEnvelopeFixture);
  const prepared = once(() => preparedOf(fixture(), "envelope"));
  const body = saturnBakeBody();

  it("reads an envelope's deck to τ 1,000 from the top, beneath its split at τ 10", () => {
    const { floorAltitudeM } = prepared();
    const scaleHeightM = saturnScaleHeightM();
    // The cloud's τ from the top is 3,000 e^(−h/H) (its top 16 H up adds 3,000 e^(−16) ≈ 3 × 10⁻⁴),
    // with the air's 0.28 at most beside it: the floor near H ln 3 and the split near H ln 300.
    expect([
      Math.abs(floorAltitudeM / scaleHeightM - Math.log(3)) < 1e-3,
      Math.abs(fixture().deckAltitudeM / scaleHeightM - Math.log(300)) < 0.03,
      DECK_FLOOR_OPTICAL_DEPTH,
    ]).toEqual([true, true, 1_000]);
  });

  it("reflects its incident flux to 1% at every node and sun", () => {
    const geometry = bandOf(body, 0);
    const cosines = groundCosinesOf(THICK_BAKE_STREAMS, GRID.upwardCosines, GRID.sunCosines);
    const rho = deckGround(prepared(), geometry)(BIN, onlyTerm(prepared(), BIN), cosines, 0);
    const { nodes, weights } = doubleGaussQuadrature(THICK_BAKE_STREAMS);
    const columns = cosines.incidentCosines.length;
    // The plane albedo, F↑ ÷ μ′ = 2 Σ_i w_i μ_i ρ⁰(μ_i, μ′), on the nodes (the outgoing cosines' first).
    const albedos = Array.from(cosines.incidentCosines, (_, j) => {
      let sum = 0;
      for (const [i, mu] of nodes.entries()) {
        sum += 2 * entry(weights, i) * mu * entry(rho.modes[0] ?? [], i * columns + j);
      }
      return sum;
    });
    const least = Math.min(...albedos);
    const most = Math.max(...albedos);
    // Measured 2026-10-29: 0.9906 at the least and 0.9975 at the most, the deck's 1 − T over its
    // black floor.
    expect([least > 0.99, most <= 1 + 1e-12]).toEqual([true, true]);
  });

  it("returns its incident flux to the top of the medium above it to 1%", () => {
    const geometry = bandOf(body, 0);
    const solve = bakeBinSolve(
      prepared().upper,
      geometry,
      BIN,
      onlyTerm(prepared(), BIN),
      deckGround(prepared(), geometry),
    );
    const returned = solve.solution.beams.map(
      (beam) => entry(beam.fluxes.diffuseUp, 0) / beam.beamCosine,
    );
    // Measured 2026-10-29: 1.0061, 0.9935 and 0.9906 at μ₀ 0.3, 0.7 and 1. The excess at the low sun
    // is the pseudo-spherical beam's, whose slant through the medium above is not 1 ÷ μ₀.
    expect(returned.every((share) => Math.abs(share - 1) < 0.01)).toBe(true);
  });

  it("is reciprocal, ρ^m(μ, μ′) = ρ^m(μ′, μ), to 10⁻¹² of each mode's largest", () => {
    const geometry = bandOf(body, 0);
    const shared = Float64Array.of(0.1, 0.3, 0.55, 0.8, 1);
    const rho = deckGround(prepared(), geometry)(
      BIN,
      onlyTerm(prepared(), BIN),
      { outgoingCosines: shared, incidentCosines: shared },
      THICK_BAKE_MODES - 1,
    );
    let worst = 0;
    for (const mode of rho.modes) {
      const largest = Math.max(...Array.from(mode, Math.abs));
      for (let i = 0; i < shared.length; i += 1) {
        for (let j = 0; j < i; j += 1) {
          const ij = entry(mode, i * shared.length + j);
          const ji = entry(mode, j * shared.length + i);
          worst = Math.max(worst, Math.abs(ij - ji) / largest);
        }
      }
    }
    // Measured 2026-10-29: 1.5 × 10⁻¹⁵, the formal solution's own reciprocity (R08.T14.a's (g)).
    expect(worst).toBeLessThan(1e-12);
  });
});

describe("an NH₃-like deck on a Saturn-class figure", () => {
  const fixture = once(nh3DeckFixture);
  const prepared = once(() => preparedOf(fixture(), "envelope"));
  const body = saturnBakeBody();
  const tables = once((): ReadonlyArray<DeckTables> =>
    thickBakeBands(body).map((_, band) => bakeDeck(prepared(), body, band)),
  );

  it("lies at the same pressure in every one of its four bands", () => {
    const pressuresBar = tables().map((table) =>
      Math.exp(-table.deckAltitudeM / saturnScaleHeightM()),
    );
    expect([
      tables().length,
      pressuresBar.every((p) => Math.abs(p - NH3_DECK_TOP_BAR) < 1e-12),
    ]).toEqual([4, true]);
  });

  it("puts the column above it, and its own, in the inverse ratio of the bands' gravities, to 10⁻¹²", () => {
    const [first, ...rest] = tables();
    if (first === undefined) {
      throw new Error("four bands");
    }
    let worst = 0;
    for (const other of rest) {
      const ratio = other.band.gravityRatio / first.band.gravityRatio;
      for (let b = 0; b < BAKE_WAVELENGTHS_NM.length; b += 1) {
        for (const [one, two] of [
          [first.above.verticalOpticalDepth, other.above.verticalOpticalDepth],
          [first.deckOpticalDepth, other.deckOpticalDepth],
        ] as const) {
          worst = Math.max(worst, Math.abs(entry(one, b) / entry(two, b) / ratio - 1));
        }
      }
    }
    // Measured 2026-10-29: 3.3 × 10⁻¹⁶. The four bands' s are 0.868, 0.954, 1.048 and 1.152, the
    // deck's τ at 570 nm 23.17, 21.09, 19.19 and 17.47, and the air's above it 0.135–0.102.
    expect([worst < 1e-12, entry(first.deckOpticalDepth, BIN) > 10]).toEqual([true, true]);
  });

  it("stores the deck's reflectance and radiance by the bins' conversion: a grey deck's as its own, to f32", () => {
    const { column, deckAltitudeM } = fixture();
    // The deck alone, grey: every bin solves the same deck, which a flat spectrum converts to itself.
    const grey = prepareDeck(
      { ...column, terms: column.terms.filter((term) => term.name === "aerosol:deck") },
      [SUN],
      { altitudeM: deckAltitudeM, floor: "envelope" },
      { grid: GRID, deckLayers: DECK_LAYERS },
    );
    const table = bakeDeck(grey, body, 0);
    const slot = table.slots[0];
    if (slot === undefined) {
      throw new Error("a slot");
    }
    const geometry = table.band;
    const term = onlyTerm(grey, BIN);
    const cosines = groundCosinesOf(THICK_BAKE_STREAMS, GRID.upwardCosines, GRID.sunCosines);
    const ground = deckGround(grey, geometry);
    const rho = ground(BIN, term, cosines, THICK_BAKE_MODES - 1);
    const solve = bakeBinSolve(grey.upper, geometry, BIN, term, ground);
    const n = THICK_BAKE_STREAMS / 2;
    const columns = cosines.incidentCosines.length;
    const views = GRID.upwardCosines.length;
    const layers = GRID.layers;
    let worst = 0;
    GRID.sunCosines.forEach((mu0, s) => {
      const kernel = entry(solve.kernelBeams[s]?.interfaceTransmission ?? [], layers);
      for (let v = 0; v < views; v += 1) {
        for (let m = 0; m < THICK_BAKE_MODES; m += 1) {
          const r = entry(rho.modes[m] ?? [], (n + v) * columns + n + s);
          const leaving = entry(
            solve.solution.beams[s]?.modes[m]?.userUp ?? [],
            layers * views + v,
          );
          const diffuse = leaving - ((mu0 * kernel) / Math.PI) * r;
          const i = (s * views + v) * THICK_BAKE_MODES + m;
          for (let c = 0; c < 3; c += 1) {
            worst = Math.max(
              worst,
              Math.abs(entry(slot.reflectance, i * 3 + c) - r) / Math.max(Math.abs(r), 1e-3),
              Math.abs(entry(slot.diffuse, i * 3 + c) - diffuse) /
                Math.max(Math.abs(diffuse), 1e-3),
            );
          }
        }
      }
    });
    expect([worst < 1e-6, slot.solves]).toEqual([true, 15]);
  });

  it("holds finite tables of the grid's size", () => {
    const sizes = tables().map((table) => [
      table.above.slots[0]?.values.length,
      table.slots[0]?.reflectance.length,
      table.slots[0]?.diffuse.length,
      table.slots.every((slot) => [...slot.reflectance, ...slot.diffuse].every(Number.isFinite)),
    ]);
    const above = 9 * 3 * 4 * THICK_BAKE_MODES * 3;
    const deck = 3 * 2 * THICK_BAKE_MODES * 3;
    expect(sizes).toEqual(Array.from({ length: 4 }, () => [above, deck, deck, true]));
  });
});

describe("the polarisation correction over the deck", () => {
  const fixture = once(nh3DeckFixture);
  const prepared = once(() => preparedOf(fixture(), "envelope"));
  const body = saturnBakeBody();

  it("gives the thick bake's source over the deck as its scalar half at 16 streams, bin by bin, to 10⁻¹²", () => {
    const geometry = bandOf(body, 0);
    const bins = prepareDeckPolarisation(prepared(), [SUN], { wavelengthsNm: BAKE_WAVELENGTHS_NM });
    const ground = deckPolarisationGround(prepared(), geometry);
    let worst = 0;
    for (const bin of [2, BIN, 12]) {
      const [sources] = polarisationSources(bins, body, 0, bin, {
        streams: THICK_BAKE_STREAMS,
        maxMode: THICK_BAKE_MODES - 1,
        ground,
      });
      const solve = bakeBinSolve(
        prepared().upper,
        geometry,
        bin,
        onlyTerm(prepared(), bin),
        deckGround(prepared(), geometry),
      );
      if (sources === undefined) {
        throw new Error("one light");
      }
      const largest = Math.max(...Array.from(solve.source, Math.abs));
      const points = GRID.sunCosines.length * 2 * GRID.upwardCosines.length * THICK_BAKE_MODES;
      solve.source.forEach((value, i) => {
        const a = Math.floor(i / points);
        const scalar = entry(sources.scalar, i) * entry(sources.scatteringPerM, a);
        worst = Math.max(worst, Math.abs(scalar - value) / largest);
      });
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("vanishes over the deck where every term depolarises, to 10⁻¹²", () => {
    const polarisation = prepareDeckPolarisation(prepared(), [SUN], { matrices: "depolarising" });
    const table = bakeDeckPolarisation(prepared(), polarisation, body, 0);
    const largest = Math.max(...Array.from(table.slots[0]?.values ?? [], Math.abs));
    expect(largest).toBeLessThan(1e-12);
  });

  it("holds finite values over the deck, its altitudes the table's above it", () => {
    const polarisation = prepareDeckPolarisation(prepared(), [SUN]);
    const table = bakeDeckPolarisation(prepared(), polarisation, body, 0);
    const above = bakeDeck(prepared(), body, 0).above;
    expect([Array.from(table.altitudesM), table.slots[0]?.values.every(Number.isFinite)]).toEqual([
      Array.from(above.altitudesM),
      true,
    ]);
  });
});

/** A band's table below the deck, or a test failure. */
function belowOf(table: DeckTables | undefined): BelowDeckTable {
  if (table?.below === undefined || table.below === null) {
    throw new Error("a deck over a surface has a table below it");
  }
  return table.below;
}

describe("below a deck over a surface (R08.T15.b.1)", () => {
  const fixture = once(nh3DeckFixture);
  const prepared = once(() => preparedOf(fixture(), "ground"));
  const body = saturnBakeBody();
  const tables = once((): ReadonlyArray<DeckTables> =>
    thickBakeBands(body).map((_, band) => bakeDeck(prepared(), body, band)),
  );

  it("lays a table by altitude, view angle and sun angle, one a band, from the ground to the split", () => {
    const shapes = tables().map((table) => {
      const below = belowOf(table);
      const altitudes = below.altitudesM.length;
      const suns = below.sunCosines.length;
      const slot = below.slots[0];
      return [
        below.altitudesM[0],
        below.altitudesM[altitudes - 1] === table.deckAltitudeM,
        Array.from(below.travelCosines),
        slot?.radiance.length === altitudes * suns * 4 * THICK_BAKE_MODES * 3,
        slot?.irradiance.length === altitudes * suns * 3,
        slot?.direct.length === altitudes * suns * 3,
        [...(slot?.radiance ?? []), ...(slot?.irradiance ?? []), ...(slot?.direct ?? [])].every(
          Number.isFinite,
        ),
      ];
    });
    expect(shapes).toEqual(
      Array.from({ length: 4 }, () => [
        0,
        true,
        [-0.75, -0.25, 0.25, 0.75],
        true,
        true,
        true,
        true,
      ]),
    );
  });

  it("has none under an envelope, which has no view from below", () => {
    expect(bakeDeck(preparedOf(fixture(), "envelope"), body, 0).below).toBeNull();
  });

  it("gives the table's surface flux as the solver's, bin by bin through the conversion, to f32", () => {
    const table = tables()[0];
    const below = belowOf(table);
    const slot = below.slots[0];
    const conversion = prepared().upper.lights[0]?.conversion;
    if (table === undefined || slot === undefined || conversion === undefined) {
      throw new Error("a band, a slot and its conversion");
    }
    const ground = GRID.layers + DECK_LAYERS;
    const fluxes = BAKE_WAVELENGTHS_NM.map((_, b) =>
      deckColumnSolve(prepared(), table.band, b, onlyTerm(prepared(), b)).solution.beams.map(
        (beam) => entry(beam.fluxes.diffuseDown, ground),
      ),
    );
    let worst = 0;
    GRID.sunCosines.forEach((_, s) => {
      conversion.factors.forEach((factors, c) => {
        let expected = 0;
        fluxes.forEach((perSun, b) => {
          expected += entry(factors, b) * entry(perSun, s);
        });
        const stored = entry(slot.irradiance, s * 3 + c);
        worst = Math.max(worst, Math.abs(stored - expected) / expected);
      });
    });
    // Measured 2026-10-29: 4.7 × 10⁻⁸, the f32's rounding.
    expect(worst).toBeLessThan(1e-6);
  });

  it("integrates its surface radiance over the sky to its surface flux on its cells, to 1%", () => {
    // The default grid's eight view cosines, the cells' centres of an even division of μ.
    const grid: ThickBakeGrid = { ...GRID, upwardCosines: THICK_BAKE_GRID.upwardCosines };
    const deck = prepareDeck(
      fixture().column,
      [SUN],
      { altitudeM: fixture().deckAltitudeM, floor: "ground" },
      { grid, deckLayers: DECK_LAYERS },
    );
    const below = belowOf(bakeDeck(deck, body, 0));
    const slot = below.slots[0];
    if (slot === undefined) {
      throw new Error("a slot");
    }
    const views = grid.upwardCosines.length;
    const v2 = 2 * views;
    let worst = 0;
    grid.sunCosines.forEach((_, s) => {
      for (let c = 0; c < 3; c += 1) {
        let integral = 0;
        for (let v = 0; v < views; v += 1) {
          const mu = -entry(below.travelCosines, v);
          const i = (s * v2 + v) * THICK_BAKE_MODES * 3 + c;
          integral += 2 * Math.PI * entry(slot.radiance, i) * mu * (1 / views);
        }
        const flux = entry(slot.irradiance, s * 3 + c);
        worst = Math.max(worst, Math.abs(integral - flux) / flux);
      }
    });
    // Measured 2026-10-29: 0.18%, the midpoint rule's error on eight cells.
    expect(worst).toBeLessThan(0.01);
  });

  it("meets the field the deck's reflectance gives above it at the split, to 0.1%", () => {
    const table = tables()[0];
    if (table === undefined) {
      throw new Error("a band");
    }
    const geometry = table.band;
    const term = onlyTerm(prepared(), BIN);
    const whole = deckColumnSolve(prepared(), geometry, BIN, term);
    const split = bakeBinSolve(
      prepared().upper,
      geometry,
      BIN,
      term,
      deckGround(prepared(), geometry),
    );
    const k = GRID.layers;
    let worst = 0;
    let largest = 0;
    whole.solution.beams.forEach((beam, s) => {
      beam.modes.forEach((mode, m) => {
        const other = split.solution.beams[s]?.modes[m];
        for (const [one, two] of [
          [mode.quadratureDown, other?.quadratureDown],
          [mode.quadratureUp, other?.quadratureUp],
        ] as const) {
          const n = THICK_BAKE_STREAMS / 2;
          for (let i = 0; i < n; i += 1) {
            largest = Math.max(largest, Math.abs(entry(one, k * n + i)));
            worst = Math.max(worst, Math.abs(entry(one, k * n + i) - entry(two ?? [], k * n + i)));
          }
        }
      });
    });
    // Measured 2026-10-29: 1.9 × 10⁻⁴ of the largest intensity there, the deck's reflectance
    // being plane-parallel where the whole column's beam inside it is pseudo-spherical.
    expect(worst / largest).toBeLessThan(1e-3);
  });
});

describe("the deck's refusals", () => {
  const fixture = once(nh3DeckFixture);

  it("refuses a split at or below its floor, or at or above the column's top", () => {
    const { column } = fixture();
    for (const altitudeM of [0, -1, column.topHeightM, 2 * column.topHeightM, Number.NaN]) {
      expect(() =>
        prepareDeck(column, [SUN], { altitudeM, floor: "ground" }, { grid: GRID, deckLayers: 8 }),
      ).toThrow(RangeError);
    }
  });

  it("refuses an envelope split above which τ 1,000 is already reached", () => {
    const envelope = conservativeEnvelopeFixture();
    expect(() =>
      prepareDeck(envelope.column, [SUN], { altitudeM: 1, floor: "envelope" }, { grid: GRID }),
    ).toThrow(RangeError);
  });

  it("refuses deck layers that are not whole strides", () => {
    const { column, deckAltitudeM } = fixture();
    for (const deckLayers of [0, 7, 2.5]) {
      expect(() =>
        prepareDeck(
          column,
          [SUN],
          { altitudeM: deckAltitudeM, floor: "ground" },
          { grid: GRID, deckLayers },
        ),
      ).toThrow(RangeError);
    }
  });
});
