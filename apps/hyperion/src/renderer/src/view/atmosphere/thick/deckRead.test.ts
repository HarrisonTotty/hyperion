import { describe, expect, it } from "vitest";

import {
  gravityOfSpheroid,
  SATURN_SPHEROID,
  saturnScaleHeightM,
} from "../../../test/atmosphereFixtures";
import { patternedDeckTables } from "../../../test/thickTables";
import type { Rgb } from "../../photometry/toneCurve";
import { type AtmosphereMedium, densityAt, tabulatedDensity } from "../medium";
import { bodyGravity, gravityRatio } from "../oblate";
import deckReadWgsl from "../shaders/deckRead.wgsl?raw";
import { kernelMedium, type TablesBand } from "../tables";
import { DECK_MODES, DECK_PHASE_U, type DeckTables } from "./deck";
import {
  DECK_FLOOR_WORD,
  DECK_LAYER_BYTES,
  DECK_PHASE_ROW,
  DECK_TABLE_MODES,
  DECK_TABLE_PHASE_ENTRIES,
  DECK_TABLE_ROWS,
  DECK_TABLE_VIEW_COSINES,
  DECK_TABLE_WIDTH,
  DECK_TEXELS_A_CELL,
  deckAboveTables,
  deckCorrectionAbove,
  deckDatumDepartureM,
  deckDatumOf,
  deckGroundInto,
  deckReadScratch,
  deckTableBytes,
  packDeckTable,
  type PackedDeckTable,
  upperMediumOf,
} from "./deckRead";
import type { PolarisationTable } from "./polarisation";
import { packThickTable, THICK_SUN_COSINES } from "./read";

/** A medium of every profile the kernels read: an exponential gas, a tent and a tabulated cloud. */
const MEDIUM: AtmosphereMedium = {
  name: "deck read test",
  topHeightM: 100_000,
  groundAlbedo: [0.2, 0.2, 0.2],
  terms: [
    {
      name: "gas",
      density: { kind: "exponential", scaleHeightM: 8_000 },
      scattering: [5.8e-6, 1.35e-5, 3.31e-5],
      absorption: [1e-7, 2e-7, 3e-7],
      phase: { kind: "rayleigh", depolarisation: [0.03, 0.03, 0.03] },
    },
    {
      name: "ozone",
      density: { kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 },
      scattering: [0, 0, 0],
      absorption: [6.5e-7, 1.9e-6, 8.5e-8],
      phase: { kind: "none" },
    },
    {
      name: "aerosol:deck",
      density: tabulatedDensity(
        Float64Array.of(0, 4_000, 6_000, 6_001, 100_000),
        Float64Array.of(1, 1, 0.5, 0, 0),
      ),
      scattering: [1e-3, 1e-3, 1e-3],
      absorption: [1e-5, 1e-5, 1e-5],
      phase: { kind: "cornette-shanks", asymmetry: 0.75 },
    },
  ],
  pressureBar: tabulatedDensity(
    Float64Array.from({ length: 21 }, (_, i) => i * 5_000),
    Float64Array.from({ length: 21 }, (_, i) => Math.exp((-i * 5_000) / 8_000)),
  ),
};

/** The split: 5 km, inside the cloud's top ramp. */
const SPLIT_M = 5_000;

/** One band at s = 1, and two at s 0.95 and 1.05. */
const ONE_BAND: ReadonlyArray<TablesBand> = [
  { gravityRatio: 1, latitudeRad: 0, bottomRadiusM: 6.371e6, coefficientFactor: 1 },
];
const TWO_BANDS: ReadonlyArray<TablesBand> = [0.95, 1.05].map((s, b) => ({
  gravityRatio: s,
  latitudeRad: b,
  bottomRadiusM: 6e7,
  coefficientFactor: 1 / s,
}));

/** A `const NAME : u32 = …u;` literal of `deckRead.wgsl`. */
function deckReadConstant(name: string): number {
  const found = new RegExp(`const ${name} : u32 = ([0-9]+)u;`, "u").exec(deckReadWgsl);
  return Number(found?.[1] ?? Number.NaN);
}

/** The patterned deck over the upper medium of {@link MEDIUM}, `slots` light slots. */
function decks(bands: ReadonlyArray<TablesBand>, slots: number): DeckTables[] {
  return patternedDeckTables(bands, kernelMedium(upperMediumOf(MEDIUM, SPLIT_M)), slots, SPLIT_M);
}

/** `band` over an envelope's floor (R08.T16.a.2). */
function asEnvelope(band: DeckTables): DeckTables {
  return { ...band, floor: "envelope" };
}

/** Each layer's floor word of a packed table (R08.T16.a.2). */
function floorWords(packed: PackedDeckTable): number[] {
  const layerWords = packed.widthTexels * packed.heightTexels * 4;
  return Array.from(
    { length: packed.layers },
    (_, layer) => packed.texels[layer * layerWords + DECK_FLOOR_WORD] ?? Number.NaN,
  );
}

/** Every word of a packed table but its layers' floor words. */
function otherWords(packed: PackedDeckTable): Uint32Array {
  const layerWords = packed.widthTexels * packed.heightTexels * 4;
  return packed.texels.filter((_, i) => i % layerWords !== DECK_FLOOR_WORD);
}

/** An array's entry, or a test failure. */
function entry(values: ArrayLike<number>, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/**
 * The deck's light at a cell's centre, summed from its own tables (not the packing): the beam's
 * Σ_m βρᵐ cos mΔφ + G_β p(Θ), p read linearly in u, and the sky-lit Σ_m Dᵐ cos mΔφ, per channel.
 */
function atCellCentre(
  table: DeckTables,
  slot: number,
  s: number,
  v: number,
  dphi: number,
  carried: boolean,
  toSun: Rgb,
): [number, number, number] {
  const deck = table.slots[slot];
  if (deck === undefined) {
    throw new Error(`no slot ${slot}`);
  }
  const mu0 = entry(table.sunCosines, s);
  const muV = entry(table.viewCosines, v);
  const cosTheta = -mu0 * muV + Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV)) * Math.cos(dphi);
  const u = Math.sqrt(Math.acos(cosTheta) / Math.PI) * (DECK_TABLE_PHASE_ENTRIES - 1);
  const i0 = Math.min(Math.floor(u), DECK_TABLE_PHASE_ENTRIES - 2);
  const fu = u - i0;
  const point = s * DECK_TABLE_VIEW_COSINES + v;
  const share = Math.min(Math.max(32 * mu0 + 7, 0), 1);
  const channel = (c: number): number => {
    const p = entry(deck.phase, i0 * 3 + c) * (1 - fu) + entry(deck.phase, (i0 + 1) * 3 + c) * fu;
    const single = carried ? deck.singleBeamGeometry : deck.singleGeometry;
    const series = carried ? deck.beamReflection : deck.reflectance;
    let beam = entry(single, point * 3 + c) * p;
    let sky = 0;
    for (let m = 0; m < DECK_MODES; m += 1) {
      beam += entry(series, (point * DECK_MODES + m) * 3 + c) * Math.cos(m * dphi);
      if (m < table.diffuseModes) {
        sky += entry(deck.diffuse, (point * table.diffuseModes + m) * 3 + c) * Math.cos(m * dphi);
      }
    }
    const transmitted = carried ? 1 : (toSun[c] ?? Number.NaN);
    return Math.max((Math.max(mu0, 0) / Math.PI) * transmitted * beam + share * sky, 0);
  };
  return [channel(0), channel(1), channel(2)];
}

/** The read at a cell's centre at azimuth `dphi`, through the packed table. */
function readAt(
  bands: ReadonlyArray<DeckTables>,
  slot: number,
  s: number,
  v: number,
  dphi: number,
  toSun: Rgb,
  lnBands: ArrayLike<number> = [0],
  bandRatio = 1,
): [number, number, number] {
  const table = bands[0];
  if (table === undefined) {
    throw new Error("a band");
  }
  const mu0 = entry(table.sunCosines, s);
  const muV = entry(table.viewCosines, v);
  const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
  const out: [number, number, number] = [0, 0, 0];
  deckGroundInto(
    packDeckTable(bands),
    lnBands,
    bandRatio,
    mu0,
    muV,
    sines * Math.cos(dphi),
    -mu0 * muV + sines * Math.cos(dphi),
    slot,
    toSun,
    deckReadScratch(),
    out,
  );
  return out;
}

describe("the deck table's shape (R08.T15.a.2)", () => {
  it("is the deck's tables' grid, and the WGSL's", () => {
    expect([
      DECK_TABLE_MODES,
      DECK_TABLE_PHASE_ENTRIES,
      DECK_TABLE_VIEW_COSINES,
      DECK_TEXELS_A_CELL,
      DECK_TABLE_WIDTH,
      DECK_PHASE_ROW,
      DECK_TABLE_ROWS,
    ]).toEqual([DECK_MODES, DECK_PHASE_U.length, 8, 26, 208, THICK_SUN_COSINES, 48]);
    expect(
      [
        "DECK_TABLE_MODES",
        "DECK_TABLE_VIEW_COSINES",
        "DECK_TABLE_PHASE_ENTRIES",
        "DECK_TEXELS_A_CELL",
        "DECK_TABLE_WIDTH",
        "DECK_PHASE_ROW",
      ].map(deckReadConstant),
    ).toEqual([8, 8, 256, 26, 208, 46]);
  });

  it("packs a band and light into 159,744 B, its values' f32 bits", () => {
    const packed = packDeckTable(decks(TWO_BANDS, 2));
    expect([packed.widthTexels, packed.heightTexels, packed.layers, packed.slots]).toEqual([
      208, 48, 4, 2,
    ]);
    expect(DECK_LAYER_BYTES).toBe(159_744);
    expect(deckTableBytes(packed)).toBe(4 * 159_744);
    expect(packed.texels.length).toBe((4 * DECK_LAYER_BYTES) / 4);
  });

  it("holds an envelope's floor in every layer's floor word, and a surface's as 0 (R08.T16.a.2)", () => {
    const ground = packDeckTable(decks(TWO_BANDS, 2));
    const envelope = packDeckTable(decks(TWO_BANDS, 2).map(asEnvelope));
    expect([ground.floor, envelope.floor]).toEqual(["ground", "envelope"]);
    expect(floorWords(ground)).toEqual([0, 0, 0, 0]);
    expect(floorWords(envelope)).toEqual([1, 1, 1, 1]);
    // Every other word is the surface's deck's.
    expect(otherWords(envelope)).toEqual(otherWords(ground));
    expect(deckReadConstant("DECK_FLOOR_WORD")).toBe(DECK_FLOOR_WORD);
  });

  it("refuses a table off the grid, of other slots, or not finite", () => {
    const [band] = decks(ONE_BAND, 1);
    if (band === undefined) {
      throw new Error("a band");
    }
    const [other] = decks(ONE_BAND, 2);
    expect(() => packDeckTable([])).toThrow(RangeError);
    expect(() => packDeckTable([{ ...band, modes: 3 }])).toThrow(RangeError);
    expect(() =>
      packDeckTable([{ ...band, sunCosines: band.sunCosines.filter((mu) => mu > 0) }]),
    ).toThrow(RangeError);
    expect(() => packDeckTable([band, other ?? band])).toThrow(RangeError);
    expect(() => packDeckTable([band, asEnvelope(band)])).toThrow(RangeError);
    const slot = band.slots[0];
    if (slot === undefined) {
      throw new Error("a slot");
    }
    const broken = slot.phase.slice();
    broken[7] = Number.NaN;
    expect(() => packDeckTable([{ ...band, slots: [{ ...slot, phase: broken }] }])).toThrow(
      /phase entry 2/u,
    );
  });
});

describe("the deck's read (R08.T15.a.2)", () => {
  const bands = decks(ONE_BAND, 2);
  const [table] = bands;
  if (table === undefined) {
    throw new Error("a band");
  }
  const toSun: Rgb = [0.7, 0.6, 0.4];

  it("is the deck's series at each cell's centre, every azimuth, for a beam carried or not, to f32", () => {
    let worst = 0;
    for (const s of [14, 20, 31, 45, 3]) {
      for (const v of [0, 3, 7]) {
        for (const degrees of [0, 40, 95, 180]) {
          const dphi = (degrees * Math.PI) / 180;
          for (const [slot, carried] of [
            [0, true],
            [1, true],
            [2, false],
          ] as const) {
            const read = readAt(bands, slot, s, v, dphi, toSun);
            const expected = atCellCentre(table, Math.min(slot, 1), s, v, dphi, carried, toSun);
            for (let c = 0; c < 3; c += 1) {
              worst = Math.max(
                worst,
                Math.abs(entry(read, c) - entry(expected, c)) / Math.max(entry(expected, c), 1e-4),
              );
            }
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-5);
  });

  it("does not spin about the zenith: near it the read is the overhead sun's at every azimuth, to 1%", () => {
    const pack = packDeckTable(bands);
    const at = (mu0: number, dphi: number): number => {
      const out: [number, number, number] = [0, 0, 0];
      const sines = Math.sqrt((1 - mu0 * mu0) * (1 - 0.36));
      const horizontal = sines * Math.cos(dphi);
      deckGroundInto(
        pack,
        [0],
        1,
        mu0,
        0.6,
        horizontal,
        horizontal - 0.6 * mu0,
        0,
        toSun,
        deckReadScratch(),
        out,
      );
      return out[1];
    };
    const overhead = at(1, 0);
    // Every mode past 0 falls as sinᵐ θ₀, so it fades into the zenith's one value.
    for (const dphi of [0, 1, 2.5, Math.PI]) {
      expect(Math.abs(at(0.9999, dphi) / overhead - 1)).toBeLessThan(0.01);
    }
  });

  it("reads linearly in ln s between two bands", () => {
    const two = decks(TWO_BANDS, 1);
    const lnBands = TWO_BANDS.map((band) => Math.log(band.gravityRatio));
    const s = Math.sqrt(0.95 * 1.05);
    const one = (b: number): [number, number, number] =>
      readAt([two[b] ?? table], 0, 25, 4, 0.7, toSun, [0], 1);
    const between = readAt(two, 0, 25, 4, 0.7, toSun, lnBands, s);
    for (let c = 0; c < 3; c += 1) {
      const expected = 0.5 * (entry(one(0), c) + entry(one(1), c));
      expect(Math.abs(entry(between, c) - expected) / expected).toBeLessThan(1e-6);
    }
  });

  it("lights the deck by the sky alone below the horizontal, fading it out under the table's rows", () => {
    const pack = packDeckTable(bands);
    const at = (mu0: number): number => {
      const out: [number, number, number] = [0, 0, 0];
      const sines = Math.sqrt((1 - mu0 * mu0) * 0.75);
      deckGroundInto(
        pack,
        [0],
        1,
        mu0,
        0.5,
        0.3 * sines,
        0.3 * sines - 0.5 * mu0,
        0,
        toSun,
        deckReadScratch(),
        out,
      );
      return out[1];
    };
    expect(at(-0.05)).toBeGreaterThan(0);
    expect(at(-0.25)).toBe(0);
  });
});

describe("the kernels' medium over a deck (R08.T15.a.2)", () => {
  const upper = upperMediumOf(MEDIUM, SPLIT_M);

  it("holds each term's coefficient times its density from the split up, the top lowered by it", () => {
    expect(upper.topHeightM).toBe(MEDIUM.topHeightM - SPLIT_M);
    let worst = 0;
    for (const h of [0, 500, 999, 1_000, 1_000.5, 3_000, 20_000, 50_000, 94_999]) {
      for (const [t, term] of MEDIUM.terms.entries()) {
        const above = upper.terms[t];
        if (above === undefined) {
          throw new Error(`no term ${t}`);
        }
        for (let c = 0; c < 3; c += 1) {
          const want = (term.scattering[c] ?? 0) * densityAt(term.density, h + SPLIT_M);
          const got = (above.scattering[c] ?? 0) * densityAt(above.density, h);
          const absorbed = (term.absorption[c] ?? 0) * densityAt(term.density, h + SPLIT_M);
          const held = (above.absorption[c] ?? 0) * densityAt(above.density, h);
          worst = Math.max(
            worst,
            Math.abs(got - want) / Math.max(want, 1e-30),
            Math.abs(held - absorbed) / Math.max(absorbed, 1e-30),
          );
        }
      }
      const pressure = MEDIUM.pressureBar;
      const lowered = upper.pressureBar;
      if (pressure === undefined || lowered === undefined) {
        throw new Error("a pressure profile");
      }
      worst = Math.max(
        worst,
        Math.abs(densityAt(lowered, h) / densityAt(pressure, h + SPLIT_M) - 1),
      );
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("refuses a split outside the medium", () => {
    for (const split of [0, -1, MEDIUM.topHeightM, Number.NaN]) {
      expect(() => upperMediumOf(MEDIUM, split)).toThrow(RangeError);
    }
  });

  it("lays J above the deck and the correction over it from the split, as the thick table packs them", () => {
    const bands = decks(ONE_BAND, 1);
    const [above] = deckAboveTables(bands);
    expect(above?.altitudesM[0]).toBe(0);
    const [band] = ONE_BAND;
    if (band === undefined) {
      throw new Error("a band");
    }
    const correction: PolarisationTable = {
      band,
      altitudesM: Float64Array.of(SPLIT_M, SPLIT_M + 10),
      sunCosines: Float64Array.of(0.5),
      travelCosines: Float64Array.of(0.5),
      modes: 3,
      slots: [],
    };
    expect(Array.from(deckCorrectionAbove([correction], SPLIT_M)[0]?.altitudesM ?? [])).toEqual([
      0, 10,
    ]);
    expect(() => packThickTable(deckAboveTables(bands), kernelMedium(upper))).not.toThrow();
  });
});

describe("the kernels' datum over a deck (R08.T15.a.2)", () => {
  const gravity = gravityOfSpheroid(SATURN_SPHEROID);
  // The NH₃-like deck's top, 0.5 bar on the Saturn-class air: h* = H ln 2.
  const splitM = saturnScaleHeightM() * Math.log(2);

  it("stands on the level surface at the equator and the poles", () => {
    const raised = deckDatumOf(SATURN_SPHEROID, gravity, splitM);
    expect(raised.equatorialRadiusM).toBeGreaterThan(SATURN_SPHEROID.equatorialRadiusM);
    for (const latitude of [0, Math.PI / 2]) {
      expect(
        Math.abs(deckDatumDepartureM(SATURN_SPHEROID, gravity, splitM, latitude)),
      ).toBeLessThan(1e-6);
    }
  });

  it("departs from the level surface by under 2% of h* ÷ s between them, on a Saturn-class figure", () => {
    let worst = 0;
    let worstScale = 0;
    for (let degrees = 0; degrees <= 90; degrees += 1) {
      const latitude = (degrees * Math.PI) / 180;
      const departure = deckDatumDepartureM(SATURN_SPHEROID, gravity, splitM, latitude);
      const level = splitM / gravityRatio(gravity, latitude);
      worst = Math.max(worst, Math.abs(departure) / level);
      worstScale = Math.max(
        worstScale,
        (Math.abs(departure) * gravityRatio(gravity, latitude)) / saturnScaleHeightM(),
      );
    }
    // Measured 2026-10-30: 1.73% of h* ÷ s at most, at 46.5°, whatever the split (the spheroid's
    // height and the level surface's both go as h*), 0.012 of the local scale height H ÷ s for this
    // split, so that the medium at a given height above the deck's top reads some 1.2% dense there.
    expect(worst).toBeLessThan(0.02);
    expect(worstScale).toBeLessThan(0.02);
  });

  it("refuses a split at or below the datum, or not finite", () => {
    for (const split of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => deckDatumOf(SATURN_SPHEROID, gravity, split)).toThrow(RangeError);
    }
  });

  it("is a sphere's radius raised by the split on a body with no spheroid", () => {
    const sphere = { equatorialRadiusM: 5.8e7, polarRadiusM: 5.8e7 };
    const still = bodyGravity(
      { figure: sphere, massKg: 1, angularVelocityRadS: null, bulkGravityMS2: 10, figureLaw: null },
      sphere.equatorialRadiusM,
    );
    expect(deckDatumOf(sphere, still, splitM)).toEqual({
      equatorialRadiusM: 5.8e7 + splitM,
      polarRadiusM: 5.8e7 + splitM,
    });
  });
});
