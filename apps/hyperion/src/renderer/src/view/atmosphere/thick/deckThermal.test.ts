/**
 * The cloud-deck split's thermal light (plan R08, R08.T18.b over R08.T15.b.2's route; `deck.ts`'s
 * remarks, "Thermal light"): the deck emitting as its layers and what lies beneath them do, the
 * upper medium's field the whole column's above the split, the deck's glow at its top and the field
 * below a deck over a surface.
 */
import { describe, expect, it } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { nh3DeckFixture, saturnBakeBody } from "../../../test/deckFixtures";
import { BAKE_BIN_COUNT } from "../medium";
import { emittedLuminance } from "../thermal";
import type { MediumTemperature } from "../thermalSource";
import {
  bakeBinSolve,
  type BinTerm,
  prepareBake,
  THICK_BAKE_GRID,
  type ThickBakeBand,
  thickBakeBands,
  type ThickBakeGrid,
  type ThickBakeLight,
} from "./bake";
import {
  bakeDeck,
  bakeDeckBins,
  type DeckFloor,
  deckColumnSolve,
  type PreparedDeck,
  prepareDeck,
} from "./deck";
import { EXP_SUM_OPTIONS } from "./expSum";
import type { SpectralColumn } from "./spectralColumn";

/** A small grid, cheap to bake, with the eight view cosines the thermal light's read takes. */
const GRID: ThickBakeGrid = {
  layers: 16,
  altitudeStride: 2,
  sunCosines: Float64Array.of(0.3, 0.7, 1),
  upwardCosines: THICK_BAKE_GRID.upwardCosines,
};

/** The deck's layers on {@link GRID}. */
const DECK_LAYERS = 8;

/** The Sun with no absorber's curves: the fixture has no absorber term. */
const SUN: ThickBakeLight = { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] };

/** The reference bin, 570 nm. */
const BIN = 7;

const body = saturnBakeBody();

/** The body's first band. */
function band0(): ThickBakeBand {
  const band = thickBakeBands(body)[0];
  if (band === undefined) {
    throw new Error("a band");
  }
  return band;
}

/** A column hot at its ground, 1,500 K, cooling linearly to 400 K at the fixture's deck top and above. */
function heated(column: SpectralColumn, deckTopM: number): SpectralColumn {
  const temperature: MediumTemperature = {
    heightsM: Float64Array.of(0, deckTopM, column.topHeightM),
    temperaturesK: Float64Array.of(1_500, 400, 400),
  };
  return { ...column, temperature };
}

/** The NH₃ deck fixture's column, at `temperature` where given, prepared over `floor`. */
function preparedOf(
  floor: DeckFloor,
  temperature: "none" | "heated" | MediumTemperature,
  floorAlbedo?: number,
): PreparedDeck {
  const fixture = nh3DeckFixture();
  let column = fixture.column;
  if (temperature === "heated") {
    column = heated(column, fixture.deckAltitudeM);
  } else if (temperature !== "none") {
    column = { ...column, temperature };
  }
  return prepareDeck(
    column,
    [SUN],
    { altitudeM: fixture.deckAltitudeM, floor },
    { grid: GRID, deckLayers: DECK_LAYERS, ...(floorAlbedo === undefined ? {} : { floorAlbedo }) },
  );
}

/** The unabsorbed term every bin of a column without gases solves. */
function onlyTerm(prepared: PreparedDeck, bin: number): BinTerm {
  const term = prepared.upper.lights[0]?.bins[bin]?.terms[0];
  if (term === undefined) {
    throw new Error(`bin ${bin}'s term`);
  }
  return term;
}

/** The largest |a − b| over the largest |b|. */
function relativeMiss(actual: ArrayLike<number>, expected: ArrayLike<number>): number {
  let largest = 0;
  let worst = 0;
  for (let i = 0; i < expected.length; i += 1) {
    largest = Math.max(largest, Math.abs(expected[i] ?? Number.NaN));
  }
  for (let i = 0; i < expected.length; i += 1) {
    worst = Math.max(worst, Math.abs((actual[i] ?? Number.NaN) - (expected[i] ?? Number.NaN)));
  }
  return worst / largest;
}

describe("a deck's thermal light (R08.T15.b.2)", () => {
  it("bakes none for a column without a temperature", () => {
    const tables = bakeDeck(preparedOf("ground", "none"), body, 0);
    expect([tables.thermalUp, tables.below?.thermal, tables.above.thermal]).toEqual([
      null,
      null,
      undefined,
    ]);
  });

  it("glows at its top as the whole column does at the split, its own glow and its reflection of the air above, to 10⁻⁹", () => {
    const prepared = preparedOf("ground", "heated");
    const share = bakeDeckBins(prepared, body, 0);
    const glow = share.bins[BIN]?.thermalUp ?? new Float64Array();
    const field = deckColumnSolve(prepared, band0(), BIN, onlyTerm(prepared, BIN)).solution.thermal
      ?.mode.userUp;
    const views = GRID.upwardCosines.length;
    const atSplit = field?.slice(GRID.layers * views, (GRID.layers + 1) * views) ?? [];
    expect([glow.length, relativeMiss(glow, atSplit)]).toEqual([
      views,
      expect.toSatisfy((miss: number) => miss < 1e-9),
    ]);
  });

  it("gives the air above the split the J_th of T14.b's own bake of the whole column, to 10⁻⁹", () => {
    const prepared = preparedOf("ground", "heated");
    const fixture = nh3DeckFixture();
    const whole = prepareBake(
      heated(fixture.column, fixture.deckAltitudeM),
      [SUN],
      { ...GRID, layers: GRID.layers + DECK_LAYERS },
      EXP_SUM_OPTIONS,
      { layering: prepared.whole, fitLayering: prepared.whole },
    );
    const term = whole.lights[0]?.bins[BIN]?.terms[0];
    if (term === undefined) {
      throw new Error("the whole bake's term");
    }
    const theirs = bakeBinSolve(whole, band0(), BIN, term).thermalSource ?? new Float64Array();
    const mine = bakeDeckBins(prepared, body, 0).above.bins[BIN]?.thermal ?? new Float64Array();
    // The upper medium's table altitudes are the whole column's from the split up.
    const v2 = 2 * GRID.upwardCosines.length;
    const offset = (DECK_LAYERS / GRID.altitudeStride) * v2;
    expect([mine.length, relativeMiss(mine, theirs.subarray(offset))]).toEqual([
      theirs.length - offset,
      expect.toSatisfy((miss: number) => miss < 1e-9),
    ]);
  });

  it("reads B upward at the ground of an isothermal column over a black floor, through V_cb, to f32", () => {
    const temperatureK = 1_500;
    const prepared = preparedOf(
      "ground",
      {
        heightsM: Float64Array.of(0, 1e6),
        temperaturesK: Float64Array.of(temperatureK, temperatureK),
      },
      0,
    );
    const below = bakeDeck(prepared, body, 0).below;
    const thermal = below?.thermal ?? new Float32Array();
    const views = GRID.upwardCosines.length;
    const black = emittedLuminance(new Float64Array(BAKE_BIN_COUNT).fill(1), temperatureK);
    // At the ground, altitude 0, every upward travel cosine: what a black body at T emits.
    const misses = Array.from({ length: views }, (_, u) =>
      ([0, 1, 2] as const).map(
        (c) => Math.abs((thermal[(views + u) * 3 + c] ?? Number.NaN) - black[c]) / black[c],
      ),
    ).flat();
    expect(Math.max(...misses)).toBeLessThan(1e-6);
  });

  it("glows over an envelope's black floor too, red at every view cosine, with no field below it", () => {
    const tables = bakeDeck(preparedOf("envelope", "heated"), body, 0);
    const glow = tables.thermalUp ?? new Float32Array();
    // A cool glow's blue, and under about 960 K its green, lie outside Rec. 709's gamut, below 0
    // (`thermal.ts`' remarks).
    expect([
      glow.length,
      Array.from(glow).every(Number.isFinite),
      Array.from({ length: GRID.upwardCosines.length }, (_, v) => (glow[v * 3] ?? 0) > 0),
      tables.below,
    ]).toEqual([
      3 * GRID.upwardCosines.length,
      true,
      Array.from({ length: GRID.upwardCosines.length }, () => true),
      null,
    ]);
  });
});
