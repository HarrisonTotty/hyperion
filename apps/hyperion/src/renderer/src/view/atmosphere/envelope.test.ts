/**
 * A gas envelope's pure parts (plan R08, R08.T16.a.1): a mode's share of its τ above a pressure,
 * the cut of an inventory at a floor, the levels a column takes for its modes, and the lowered
 * figure the medium stands on.
 */
import { describe, expect, it } from "vitest";

import type { AerosolInventory, AerosolMode } from "../../lib/system/model";
import {
  cutAtFloor,
  ENVELOPE_TOP_PA,
  envelopeTopPa,
  loweredFigure,
  MODE_EDGE_LN_P,
  modeLevelsPa,
  shareAbove,
  STEEP_PROFILE_DEPTH,
  STEEP_PROFILE_STEP,
} from "./envelope";
import { type BodyGravity, bodyGravity, type SpheroidFigure } from "./oblate";
import { deckDatumOf } from "./thick/deckRead";

/** A mode of τ(550) `tau` (or none) from `basePa` up to `topPa` by the exponent `fSed`. */
function mode(
  material: string,
  tau: number | null,
  [basePa, topPa]: readonly [number, number],
  fSed: number,
): AerosolMode {
  return {
    material,
    phase: "solid",
    deck: material,
    shape: "sphere",
    sizeDistribution: {
      kind: "value",
      value: { kind: "gamma", effectiveRadiusM: 1e-6, effectiveVariance: 0.1 },
    },
    columnMassKgM2: { kind: "value", value: 0.01 },
    opticalDepth:
      tau === null
        ? { kind: "not_modelled" }
        : { kind: "value", value: { tau550: tau, basis: "sphere" } },
    basePa,
    topPa,
    fSed,
    coverage: 1,
    temperatureK: { kind: "value", value: 150 },
    notModelled: null,
  };
}

/** An inventory of `modes` alone. */
function inventoryOf(modes: ReadonlyArray<AerosolMode>): AerosolInventory {
  return { modes, decks: [], absorbers: [], notFormed: [] };
}

describe("shareAbove", () => {
  const deck = mode("NH3", 2.4, [6.4e4, 1.08e4], 6.5);

  it("is 0 at and above the top, and 1 at and below the base", () => {
    expect([1e3, deck.topPa, deck.basePa, 1e6].map((p) => shareAbove(deck, p))).toEqual([
      0, 0, 1, 1,
    ]);
  });

  it("is the column mass's share of (p ÷ p_b)^f dp between top and base", () => {
    // ∫_{p_t}^{p} (p′ ÷ p_b)^f dp′ over ∫_{p_t}^{p_b}, by Simpson's rule.
    const p = 4e4;
    const steps = 20_000;
    const integral = (to: number): number => {
      const h = (to - deck.topPa) / steps;
      let sum = 0;
      for (let i = 0; i <= steps; i += 1) {
        const weight = i === 0 || i === steps ? 1 : i % 2 === 1 ? 4 : 2;
        sum += weight * ((deck.topPa + i * h) / deck.basePa) ** deck.fSed;
      }
      return (sum * h) / 3;
    };
    expect(Math.abs(shareAbove(deck, p) / (integral(p) / integral(deck.basePa)) - 1)).toBeLessThan(
      1e-10,
    );
  });
});

describe("cutAtFloor", () => {
  const floorPa = 4e6;
  const above = mode("NH3", 2.4, [6.4e4, 1.08e4], 368);
  const straddling = mode("KCl", 13_074, [3.8e7, 1.44e6], 4.5);
  const below = mode("MnS", 2.6e5, [2.5e8, 1.0e7], 4.5);
  const unmodelledAbove = mode("Fe", null, [1e9, 2.4e6], 2);
  const unmodelledBelow = mode("Cr", null, [5e8, 1.8e7], 4.5);
  const cut = cutAtFloor(
    inventoryOf([above, straddling, below, unmodelledAbove, unmodelledBelow]),
    floorPa,
  );

  it("keeps a mode wholly above the floor as it is", () => {
    expect(cut.inventory.modes[0]).toBe(above);
  });

  it("bases a mode that straddles the floor at it, with its profile's share of its τ", () => {
    const entered = cut.inventory.modes[1];
    expect([entered?.basePa, entered?.topPa, entered?.fSed]).toEqual([
      floorPa,
      straddling.topPa,
      straddling.fSed,
    ]);
    expect(entered?.opticalDepth).toEqual({
      kind: "value",
      value: { tau550: 13_074 * shareAbove(straddling, floorPa), basis: "sphere" },
    });
  });

  it("keeps a mode of no τ whose top lies above the floor, to be labelled", () => {
    expect(cut.inventory.modes[2]).toBe(unmodelledAbove);
  });

  it("drops the modes whose tops lie below the floor, of a τ or none", () => {
    expect([cut.inventory.modes.length, cut.dropped]).toEqual([3, [below, unmodelledBelow]]);
  });
});

describe("modeLevelsPa", () => {
  const sheet = mode("NH3", 2.4, [6.4e4, 1.08e4], 368);
  const layer = mode("NH3", 2.4, [6.4e4, 1.08e4], 2);
  const step = Math.log(1e7) / 1_024;

  it("lays a level at each end of a mode and one 10⁻⁸ in ln p beyond each", () => {
    const levels = modeLevelsPa([layer], 1e-2, 1e9, step);
    const edge = Math.exp(MODE_EDGE_LN_P);
    expect(levels).toEqual([6.4e4 * edge, 6.4e4, 1.08e4, 1.08e4 / edge]);
  });

  it("lays a steep profile's levels STEEP_PROFILE_STEP e-folds of its density apart", () => {
    const steep = modeLevelsPa([sheet], 1e-2, 1e9, step).slice(4);
    const count = Math.ceil(Math.log(1 / STEEP_PROFILE_DEPTH) / STEEP_PROFILE_STEP);
    expect(steep).toHaveLength(count);
    expect(Math.log(6.4e4 / (steep[0] ?? Number.NaN)) * (368 + 1)).toBeCloseTo(
      STEEP_PROFILE_STEP,
      12,
    );
  });

  it("lays none for a mode of no τ, nor outside the column", () => {
    expect(modeLevelsPa([mode("Fe", null, [1e9, 2.4e6], 2)], 1e-2, 1e10, step)).toEqual([]);
    expect(modeLevelsPa([layer], 1e-2, 5e4, step)).toEqual([
      1.08e4,
      1.08e4 / Math.exp(MODE_EDGE_LN_P),
    ]);
  });
});

describe("envelopeTopPa", () => {
  it("is 10⁻² Pa, or a tenth of a higher mode's or slab's top", () => {
    expect(envelopeTopPa(inventoryOf([mode("NH3", 2.4, [6.4e4, 1.08e4], 2)]))).toBe(
      ENVELOPE_TOP_PA,
    );
    expect(envelopeTopPa(inventoryOf([mode("soot", 0.01, [0.27, 0.05], 0)]))).toBeCloseTo(
      0.005,
      15,
    );
  });
});

/** Jupiter's gravity on a figure, at its mass and System III spin (test/giantFixtures.ts). */
function jupiterGravityOn(figure: SpheroidFigure): BodyGravity {
  return bodyGravity(
    {
      figure,
      massKg: 1.898e27,
      angularVelocityRadS: (870.536 * Math.PI) / 180 / 86_400,
      bulkGravityMS2: 24.79,
      figureLaw: null,
    },
    (2 * figure.equatorialRadiusM + figure.polarRadiusM) / 3,
  );
}

describe("loweredFigure", () => {
  it("is the figure deckDatumOf raises back to the datum, to a millimetre", () => {
    // Jupiter's 1-bar figure, mass and System III spin (test/giantFixtures.ts).
    const figure = { equatorialRadiusM: 71_492e3, polarRadiusM: 66_854e3 };
    const depthM = 1.26e6;
    // As assemble.ts lowers it: its own gravity's ratios, ENVELOPE_DATUM_PASSES times.
    let lowered = figure;
    for (let pass = 0; pass < 6; pass += 1) {
      lowered = loweredFigure(figure, jupiterGravityOn(lowered), depthM);
    }
    const raised = deckDatumOf(lowered, jupiterGravityOn(lowered), depthM);
    expect(Math.abs(raised.equatorialRadiusM - figure.equatorialRadiusM)).toBeLessThan(1e-3);
    expect(Math.abs(raised.polarRadiusM - figure.polarRadiusM)).toBeLessThan(1e-3);
  });
});
