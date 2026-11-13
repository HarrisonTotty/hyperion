import { describe, expect, it } from "vitest";

import type { AerosolInventory, AerosolMode } from "../../lib/system/model";
import { aerosolModeOptics } from "../atmosphere/aerosol";
import {
  type CloudDeck,
  cloudCondensateOf,
  cloudDecksOf,
  STAND_IN_CONDENSATES,
  standInCondensate,
  THIN_AIR_ICE_PRESSURE_PA,
} from "./condensates";

const WATER_DECK: CloudDeck = {
  condensate: "H2O",
  phase: "liquid",
  inCloudOpticalDepth550: 2.6,
  coverage: 0.675,
  baseAltitudeM: 900,
  topAltitudeM: 11_000,
  optics: { asymmetry: 0.86, coAlbedo: 6e-7 },
};

describe("the stand-in condensates", () => {
  // Each row's optics are the registry's at its size (the module's remarks): a change of the
  // registry's index or phase tables shows here, not as a silent drift.
  it.each(Object.values(STAND_IN_CONDENSATES))("$name holds the registry's optics", (row) => {
    const optics = aerosolModeOptics(
      {
        kind: "distribution",
        mode: {
          material: row.condensate,
          form: { phase: row.phase },
          ...(row.shape === "crystal" ? { shape: "crystal" as const } : {}),
          sizes: {
            kind: "gamma",
            effectiveRadiusUm: row.effectiveRadiusUm,
            effectiveVariance: 0.1,
          },
        },
      },
      550,
    );
    expect(Math.abs(optics.asymmetry - row.asymmetry)).toBeLessThan(1e-3);
    // CH₄ ice's crystals are conservative to round-off; every stand-in's is within 2%.
    expect(Math.abs(1 - optics.singleScatteringAlbedo - row.coAlbedo)).toBeLessThan(
      0.02 * row.coAlbedo,
    );
  });

  it("corrects liquid water's co-albedo below the plan's 10⁻⁵", () => {
    const water = STAND_IN_CONDENSATES.liquidWater;
    expect(water.coAlbedo).toBeLessThan(1e-6);
    expect(water.coAlbedo).toBeGreaterThan(1e-7);
  });

  it("holds every asymmetry inside the plan's cited range", () => {
    const ranges: ReadonlyArray<readonly [number, number, number]> = [
      [STAND_IN_CONDENSATES.liquidWater.asymmetry, 0.85, 0.87],
      [STAND_IN_CONDENSATES.terrestrialIce.asymmetry, 0.73, 0.77],
      [STAND_IN_CONDENSATES.martianWaterIce.asymmetry, 0.7, 0.8],
      [STAND_IN_CONDENSATES.martianCarbonDioxideIce.asymmetry, 0.6, 0.8],
      [STAND_IN_CONDENSATES.liquidMethane.asymmetry, 0.85, 0.9],
    ];
    for (const [g, low, high] of ranges) {
      expect(g).toBeGreaterThanOrEqual(low);
      expect(g).toBeLessThanOrEqual(high);
    }
  });

  it("takes Martian ice crystals under a thin air and cirrus under a thick one", () => {
    expect(standInCondensate("H2O", "solid", THIN_AIR_ICE_PRESSURE_PA / 3)?.name).toBe(
      "martianWaterIce",
    );
    expect(standInCondensate("H2O", "solid", 101_325)?.name).toBe("terrestrialIce");
    expect(standInCondensate("H2O", "liquid", 600)?.name).toBe("liquidWater");
    expect(standInCondensate("CO2", "solid", 600)?.name).toBe("martianCarbonDioxideIce");
    expect(standInCondensate("CH4", "liquid", 147_000)?.name).toBe("liquidMethane");
    expect(standInCondensate("NH3", "solid", 1e5)).toBeNull();
  });
});

describe("the condensate a layer draws", () => {
  it("is the deck of greatest sky-mean optical depth, its record's members kept", () => {
    const thin: CloudDeck = {
      ...WATER_DECK,
      condensate: "CO2",
      phase: "solid",
      inCloudOpticalDepth550: 5,
      coverage: 0.1,
    };
    const drawn = cloudCondensateOf([thin, WATER_DECK], 101_325);
    expect(drawn).toEqual({
      condensate: "H2O",
      phase: "liquid",
      asymmetry: 0.86,
      coAlbedo: 6e-7,
      inCloudOpticalDepth550: 2.6,
      baseAltitudeM: 900,
      topAltitudeM: 11_000,
      opticsSource: "record",
      opticalDepthSource: "record",
    });
  });

  it("stands in for the optics and the depth a record leaves out", () => {
    const bare: CloudDeck = { ...WATER_DECK, inCloudOpticalDepth550: null, optics: null };
    const drawn = cloudCondensateOf([bare], 101_325);
    expect(drawn?.asymmetry).toBe(STAND_IN_CONDENSATES.liquidWater.asymmetry);
    expect(drawn?.inCloudOpticalDepth550).toBe(
      STAND_IN_CONDENSATES.liquidWater.inCloudOpticalDepth550,
    );
    expect(drawn?.opticsSource).toBe("stand-in");
    expect(drawn?.opticalDepthSource).toBe("stand-in");
  });

  it("draws nothing for a body with no deck", () => {
    expect(cloudCondensateOf([], 101_325)).toBeNull();
  });
});

/** A water deck's mode, as the record sends one, with `overrides`. */
const mode = (overrides: Partial<AerosolMode>): AerosolMode => ({
  material: "H2O",
  phase: "liquid",
  deck: "H2O",
  shape: "sphere",
  sizeDistribution: { kind: "not_modelled" },
  columnMassKgM2: { kind: "not_modelled" },
  opticalDepth: { kind: "value", value: { tau550: 2, basis: "sphere" } },
  basePa: 90_000,
  topPa: 20_000,
  fSed: 1,
  coverage: 0.6,
  temperatureK: { kind: "not_modelled" },
  notModelled: null,
  ...overrides,
});
const inventory = (modes: ReadonlyArray<AerosolMode>): AerosolInventory => ({
  modes,
  decks: [
    {
      condensate: "H2O",
      phase: "liquid",
      basePa: 90_000,
      baseTemperatureK: 285,
      topPa: 20_000,
      vapourMoleFractionBelow: 0.01,
      vapourMoleFractionAbove: 1e-5,
      fSed: 1,
      extrapolated: false,
    },
  ],
  absorbers: [],
  notFormed: [],
});
const heightAtPa = (pa: number): number => 8_000 * Math.log(100_000 / pa);

describe("a record's decks", () => {
  it("sums their modes' in-cloud depths and takes the thickest mode's optics", () => {
    const decks = cloudDecksOf(
      inventory([
        mode({}),
        mode({ material: "dust", deck: null }),
        mode({ opticalDepth: { kind: "value", value: { tau550: 3, basis: "sphere" } } }),
      ]),
      heightAtPa,
      (index) => ({ asymmetry: 0.8 + index / 100, coAlbedo: 1e-6 }),
    );
    expect(decks).toHaveLength(1);
    expect(decks[0]?.inCloudOpticalDepth550).toBe(5);
    expect(decks[0]?.optics?.asymmetry).toBeCloseTo(0.82, 12);
    expect(decks[0]?.coverage).toBe(0.6);
    expect(decks[0]?.baseAltitudeM).toBeCloseTo(heightAtPa(90_000), 9);
    expect(decks[0]?.topAltitudeM).toBeCloseTo(heightAtPa(20_000), 9);
  });

  it("leaves a deck's depth unmodelled where any of its modes is", () => {
    const decks = cloudDecksOf(
      inventory([mode({}), mode({ opticalDepth: { kind: "not_modelled" } })]),
      heightAtPa,
      () => null,
    );
    expect(decks[0]?.inCloudOpticalDepth550).toBeNull();
    expect(decks[0]?.optics).toBeNull();
  });
});
