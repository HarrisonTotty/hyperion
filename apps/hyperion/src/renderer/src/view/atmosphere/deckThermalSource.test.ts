/**
 * A cloud deck's thermal light in its tables' thermal source (plan R08, R08.T15.b.2;
 * `thermalSource.ts`'s `DeckThermal`, `thick/deckRead.ts`'s `deckThermalOf`): laid for the deck's
 * marches' reads, which search nothing, on a body of one band, and none on a body of several.
 */
import { describe, expect, it } from "vitest";

import { saturnBody } from "../../test/atmosphereFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { patternedDeckTables, withPatternedDeckThermal } from "../../test/thickTables";
import { R05_EARTH_REFERENCE } from "./earth";
import { tableRadiusM, type SpheroidFigure } from "./hillaire";
import { AtmosphereTables, kernelMedium } from "./tables";
import {
  DECK_THERMAL_ROWS,
  deckBelowInto,
  deckGlowInto,
  deckScatteringInto,
  type MediumTemperature,
  packThermalSource,
  THERMAL_SOURCE_HEIGHTS,
  THERMAL_TRAVEL_COSINES,
  type ThermalRgb,
  thermalScatteringInto,
  thermalSourceOf,
} from "./thermalSource";
import { deckThermalDrawn, deckThermalOf, upperMediumOf } from "./thick/deckRead";

/** R05's Earth's air above a deck 3 km up, the tables' medium over it. */
const DECK_ALTITUDE_M = 3_000;
const UPPER = upperMediumOf(R05_EARTH_REFERENCE, DECK_ALTITUDE_M);

/** A sphere of the deck's datum: one band. */
const SPHERE: SpheroidFigure = {
  equatorialRadiusM: 6_371_000 + DECK_ALTITUDE_M,
  polarRadiusM: 6_371_000 + DECK_ALTITUDE_M,
};

const HOT: MediumTemperature = {
  heightsM: Float64Array.of(0, 1e6),
  temperaturesK: Float64Array.of(3_000, 3_000),
};

/** Glowing deck tables over a surface on `bands`. */
function glowingDeck(tables: AtmosphereTables): ReturnType<typeof withPatternedDeckThermal> {
  return withPatternedDeckThermal(
    patternedDeckTables(tables.plan.bands, kernelMedium(tables.medium), 1, DECK_ALTITUDE_M, true),
  );
}

/**
 * `thermalDeck.wgsl`'s read of a part `offset` rows of 16 into the band of the packed buffer, at
 * row `row` and `muTravel`, done on the buffer's own floats: what the kernels read.
 */
function packedRow(
  packed: Float32Array,
  offset: number,
  row: number,
  muTravel: number,
): ThermalRgb {
  const at = (i: number, c: number): number => packed[i * 4 + c] ?? Number.NaN;
  const bands = at(0, 2);
  const altitudes = at(0, 3);
  const base = 1 + at(0, 1) + bands * altitudes * (1 + THERMAL_TRAVEL_COSINES);
  const y = Math.min(Math.max((muTravel + 1) * 8 - 0.5, 0), 15);
  const v0 = Math.min(Math.floor(y), 14);
  const g = y - v0;
  const i = base + 1 + (offset + row) * THERMAL_TRAVEL_COSINES + v0;
  const read = (c: number): number => at(i, c) * (1 - g) + at(i + 1, c) * g;
  return [read(0), read(1), read(2)];
}

/** What a CPU read writes. */
function cpu(read: (out: ThermalRgb) => void): ThermalRgb {
  const out: ThermalRgb = [0, 0, 0];
  read(out);
  return out;
}

/** Whether `a` matches `b` to 10⁻⁶ of each channel, or of 1 where the channel is smaller. */
function near(a: ThermalRgb, b: ThermalRgb): boolean {
  return a.every((x, c) => {
    const y = b[c] ?? Number.NaN;
    return Math.abs(x - y) <= 1e-6 * Math.max(Math.abs(y), 1);
  });
}

describe("a deck's thermal light in the thermal source (R08.T15.b.2)", () => {
  it("is drawn on a body of one band whose deck holds it", async () => {
    const engine = await countingRenderEngine();
    const one = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    expect([one.plan.bands.length, deckThermalDrawn(glowingDeck(one))]).toEqual([1, true]);
  });

  it("is not drawn on a body of several bands, nor where the deck's bake holds none", async () => {
    const engine = await countingRenderEngine();
    const several = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE), [], saturnBody());
    const plain = patternedDeckTables(
      several.plan.bands,
      kernelMedium(UPPER),
      1,
      DECK_ALTITUDE_M,
      true,
    );
    expect([
      several.plan.bands.length > 1,
      deckThermalDrawn(glowingDeck(several)),
      deckThermalDrawn(plain),
    ]).toEqual([true, false, false]);
  });

  it("lays no band for a body of several bands", async () => {
    const engine = await countingRenderEngine();
    const several = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE), [], saturnBody());
    expect(deckThermalOf(glowingDeck(several), UPPER.topHeightM).band).toBeNull();
  });

  it("resamples J_th to its rows as the band's own read gives it", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    const [deck] = glowingDeck(tables);
    if (deck?.above.thermal === undefined) {
      throw new Error("a glowing deck");
    }
    const thermal = deckThermalOf([deck], UPPER.topHeightM);
    const fromSplit = {
      ...deck.above.thermal,
      altitudesM: deck.above.thermal.altitudesM.map((h) => h - DECK_ALTITUDE_M),
    };
    const source = thermalSourceOf(HOT, UPPER.topHeightM, [fromSplit]);
    const row = 17;
    const heightM = (UPPER.topHeightM * row) / (DECK_THERMAL_ROWS - 1);
    const theirs: ThermalRgb = [0, 0, 0];
    const mine: ThermalRgb = [0, 0, 0];
    // At a row's height its read is the band's, to the f32 it is held in.
    thermalScatteringInto(source, [0], 1, heightM, (7 + 0.5) / 8 - 1, theirs);
    deckScatteringInto(thermal, heightM, (7 + 0.5) / 8 - 1, mine);
    expect(
      mine.map(
        (x, c) => Math.abs(x - (theirs[c] ?? Number.NaN)) / Math.abs(theirs[c] ?? Number.NaN),
      ),
    ).toEqual([0, 1, 2].map(() => expect.toSatisfy((miss: number) => miss < 1e-6)));
  });

  it("lays the glow on the upper half of the travel cosines, mirrored below, so that a grazing ray reads the first upward cosine's", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    const [deck] = glowingDeck(tables);
    if (deck?.thermalUp === null || deck === undefined) {
      throw new Error("a glowing deck");
    }
    const thermal = deckThermalOf([deck], UPPER.topHeightM);
    const up = (v: number): number[] =>
      [0, 1, 2].map((c) => deck.thermalUp?.[v * 3 + c] ?? Number.NaN);
    expect([
      cpu((out) => {
        deckGlowInto(thermal, (2 + 0.5) / 8, out);
      }),
      cpu((out) => {
        deckGlowInto(thermal, 0.01, out);
      }),
      cpu((out) => {
        deckGlowInto(thermal, -0.5, out);
      }),
    ]).toEqual([
      up(2).map((x) => expect.closeTo(x, 2)),
      up(0).map((x) => expect.closeTo(x, 2)),
      cpu((out) => {
        deckGlowInto(thermal, 0.5, out);
      }).map((x) => expect.closeTo(x, 6)),
    ]);
  });

  it("packs its section after the bands where the kernels' reads find it, the CPU's reads reading the same", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    tables.setDeckTables(glowingDeck(tables), kernelMedium(R05_EARTH_REFERENCE));
    tables.setThermalSource(HOT);
    const source = tables.thermal;
    const deck = source?.deck;
    if (source === null || deck === undefined || deck.band === null) {
      throw new Error("the tables carry the deck's thermal light");
    }
    const packed = packThermalSource(source);
    const bands = source.scattering?.length ?? 0;
    const altitudes = source.scattering?.[0]?.altitudesM.length ?? 0;
    const header = 1 + THERMAL_SOURCE_HEIGHTS + bands * altitudes * (1 + THERMAL_TRAVEL_COSINES);
    const mu = 0.41;
    expect([
      bands,
      Array.from(packed.subarray(header * 4, header * 4 + 4)),
      near(
        packedRow(packed, 0, 20, mu),
        cpu((out) => {
          deckScatteringInto(deck, (UPPER.topHeightM * 20) / (DECK_THERMAL_ROWS - 1), mu, out);
        }),
      ),
      near(
        packedRow(packed, DECK_THERMAL_ROWS, 0, mu),
        cpu((out) => {
          deckGlowInto(deck, mu, out);
        }),
      ),
      near(
        packedRow(packed, DECK_THERMAL_ROWS + 1, 5, -mu),
        cpu((out) => {
          deckBelowInto(deck, 5, 0, -mu, out);
        }),
      ),
      packed.length / 4,
    ]).toEqual([
      1,
      [Math.fround(UPPER.topHeightM), DECK_THERMAL_ROWS, 1, deck.belowAltitudeCount],
      true,
      true,
      true,
      header + 1 + (DECK_THERMAL_ROWS + 1 + deck.belowAltitudeCount) * THERMAL_TRAVEL_COSINES,
    ]);
    tables.dispose();
  });

  it("refuses to pack a deck's band whose field below is not its table below's rows", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    tables.setDeckTables(glowingDeck(tables), kernelMedium(R05_EARTH_REFERENCE));
    tables.setThermalSource(HOT);
    const source = tables.thermal;
    const deck = source?.deck;
    if (source === null || deck?.band === null || deck === undefined) {
      throw new Error("the tables carry the deck's thermal light");
    }
    const short = { ...source, deck: { ...deck, belowAltitudeCount: deck.belowAltitudeCount + 1 } };
    expect(() => packThermalSource(short)).toThrow(
      /a deck's thermal light needs 64 rows, a glow and its table below's rows of 16 travel cosines/u,
    );
    tables.dispose();
  });

  it("carries none on a body of several bands, its J above it unscattered too", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE), [], saturnBody());
    tables.setDeckTables(glowingDeck(tables), kernelMedium(R05_EARTH_REFERENCE));
    tables.setThermalSource(HOT);
    expect([tables.thermal?.deck?.band, tables.thermal?.scattering]).toEqual([null, null]);
    tables.dispose();
  });

  it("lets its deck's thermal light go with the deck", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    tables.setDeckTables(glowingDeck(tables), kernelMedium(R05_EARTH_REFERENCE));
    tables.setThermalSource(HOT);
    const held = tables.thermal?.deck?.band !== null;
    tables.setDeckTables(null);
    expect([held, tables.thermal?.deck]).toEqual([true, undefined]);
    tables.dispose();
  });
});

describe("a deck's thermal light when the tables refuse a deck (R08.T15.b.2)", () => {
  it("keeps the deck's thermal light the tables held when they refuse the next deck's J", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, UPPER, tableRadiusM(SPHERE));
    tables.setDeckTables(glowingDeck(tables), kernelMedium(R05_EARTH_REFERENCE));
    tables.setThermalSource(HOT);
    const held = tables.thermal?.deck;
    const [first] = glowingDeck(tables);
    if (first?.thermalUp === null || first === undefined) {
      throw new Error("a glowing deck");
    }
    // Another glow, its J's altitudes out of order, which the thick table's packing refuses.
    const refused = {
      ...first,
      thermalUp: first.thermalUp.map((x) => 2 * x),
      above: {
        ...first.above,
        altitudesM: first.above.altitudesM.map((h, a) => (a === 1 ? h - 1e6 : h)),
      },
    };
    expect(() => {
      tables.setDeckTables([refused], kernelMedium(R05_EARTH_REFERENCE));
    }).toThrow(/a thick table's 33 altitudes ascend from the ground/u);
    // The thermal source packed again reads the deck the tables still hold.
    tables.setThermalSource(HOT);
    expect(tables.thermal?.deck).toEqual(held);
    tables.dispose();
  });
});
