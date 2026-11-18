/**
 * What of a record the assembly carries for R11's clouds (plan R11, R11.T7.b; `DrawnMedium.clouds`):
 * the surface's cloud fraction and pressure, and each deck on the column with its thickest mode's
 * optics at 550 nm.
 */
import type { AerosolInventoryDto, AerosolModeDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  detailOf,
  gammaMode,
  recordInput,
  withInventory,
  withSurface,
} from "../../test/atmosphereRecords";
import { earthDetail } from "../../test/planetaryFixture";
import { servedDeckedDetail } from "../../test/servedDeckedWorld";
import { aerosolModeOptics } from "./aerosol";
import { type AssembledMedium, assembleMedium } from "./assemble";
import type { AtmosphereMedium } from "./medium";
import type { AtmosphereRegime } from "./thick/regime";
import { deckedAirRouted } from "./thick/routing";

/** A water deck from 800 to 600 hPa of in-cloud τ 2 over `coverage` of the sky. */
function waterDeck(coverage: number): AerosolInventoryDto {
  return {
    modes: [
      {
        ...gammaMode({
          material: "H2O",
          phase: "liquid",
          deck: "H2O",
          effectiveRadiusM: 1e-6,
          tau550: 2,
          basePa: 80_000,
          topPa: 60_000,
        }),
        coverage,
      },
    ],
    decks: [
      {
        condensate: "H2O",
        phase: "liquid",
        base_pa: 80_000,
        base_temperature_k: 275,
        top_pa: 60_000,
        vapour_mole_fraction_below: 0.0102,
        vapour_mole_fraction_above: 1e-4,
        f_sed: 0,
        extrapolated: false,
      },
    ],
    absorbers: [],
    not_formed: [],
  };
}

/** The generated Earth under a cloud fraction, 0.6 if not given, with `inventory`. */
function cloudyEarth(
  inventory: AerosolInventoryDto,
  options: { readonly cloudLayers?: boolean; readonly cloudFraction?: number } = {},
): AssembledMedium {
  const detail = withSurface(earthDetail(), (surface) => ({
    ...surface,
    cloud_fraction: options.cloudFraction ?? 0.6,
  }));
  return assembleMedium(
    recordInput(detailOf(withInventory(detail, inventory))),
    options.cloudLayers === undefined ? {} : { cloudLayers: options.cloudLayers },
  );
}

/** A medium's aerosol terms' names. */
function aerosolsOf(medium: AtmosphereMedium | undefined): ReadonlyArray<string> {
  return (medium?.terms ?? [])
    .map((term) => term.name)
    .filter((name) => name.startsWith("aerosol"));
}

/** {@link waterDeck} with a haze of sulphuric acid above it, of no deck. */
function deckUnderHaze(): AerosolInventoryDto {
  const deck = waterDeck(0.6);
  return {
    ...deck,
    modes: [
      ...deck.modes,
      gammaMode({
        material: "H2SO4",
        phase: "liquid",
        effectiveRadiusM: 5e-7,
        tau550: 0.05,
        basePa: 30_000,
        topPa: 10_000,
      }),
    ],
  };
}

describe("the clouds a surface's assembly carries (R11.T7.b)", () => {
  it("carries the surface's cloud fraction and pressure", () => {
    const clouds = cloudyEarth(waterDeck(0.6)).drawn?.clouds;
    expect(clouds?.cloudFraction).toBe(0.6);
    expect(clouds?.surfacePressurePa).toBeGreaterThan(90_000);
  });

  it("lays each deck on the column, its in-cloud τ and coverage the record's", () => {
    const [deck] = cloudyEarth(waterDeck(0.6)).drawn?.clouds?.decks ?? [];
    expect(deck).toMatchObject({
      condensate: "H2O",
      phase: "liquid",
      inCloudOpticalDepth550: 2,
      coverage: 0.6,
    });
    // 800 and 600 hPa stand some 2 and 4 km over an Earth's ground.
    expect(deck?.baseAltitudeM).toBeGreaterThan(1_500);
    expect(deck?.topAltitudeM).toBeLessThan(5_000);
    expect(deck?.topAltitudeM).toBeGreaterThan(deck?.baseAltitudeM ?? Number.POSITIVE_INFINITY);
  });

  it("takes the deck's optics at 550 nm from its mode's particles, the registry's", () => {
    const [deck] = cloudyEarth(waterDeck(0.6)).drawn?.clouds?.decks ?? [];
    const optics = aerosolModeOptics(
      {
        kind: "distribution",
        mode: {
          material: "H2O",
          form: { phase: "liquid" },
          shape: "sphere",
          sizes: { kind: "gamma", effectiveRadiusUm: 1, effectiveVariance: 0.1 },
        },
      },
      550,
      new Float64Array(0),
    );
    expect(deck?.optics?.asymmetry).toBeCloseTo(optics.asymmetry, 12);
    expect(deck?.optics?.coAlbedo).toBeCloseTo(1 - optics.singleScatteringAlbedo, 15);
    expect(deck?.optics?.coAlbedo).toBeGreaterThan(0);
  });

  it("carries no deck for a surface whose inventory has none", () => {
    const clouds = cloudyEarth({ modes: [], decks: [], absorbers: [], not_formed: [] }).drawn
      ?.clouds;
    expect(clouds?.decks).toEqual([]);
  });
});

/** An assembly's regime, `undefined` where it draws no medium. */
function regimeOf(each: AssembledMedium): AtmosphereRegime | undefined {
  return each.medium === undefined ? undefined : each.regime;
}

/** Whether an assembly's medium routes by the decked air's range alone (`deckedAirRouted`). */
function routed(each: AssembledMedium): boolean {
  return each.medium !== undefined && deckedAirRouted(each.regime, each.medium);
}

describe("the near medium without the deck R11's clouds draw (R11.T7.b; the R08 double count)", () => {
  it("leaves the layer's deck's modes out under cloudLayers, every other term as it was", () => {
    const full = cloudyEarth(deckUnderHaze());
    const near = cloudyEarth(deckUnderHaze(), { cloudLayers: true });
    expect([aerosolsOf(full.medium), aerosolsOf(near.medium)]).toEqual([
      ["aerosol:H2O", "aerosol:H2SO4"],
      ["aerosol:H2SO4"],
    ]);
    expect(near.drawn?.sources.length).toBe((full.drawn?.sources.length ?? 0) - 1);
    // The deck's clouds the same either way: the layer reads the full assembly's optics.
    expect(near.drawn?.clouds).toEqual(full.drawn?.clouds);
  });

  it("keeps the full medium, its sources and its regime for the disc table", () => {
    const full = cloudyEarth(deckUnderHaze());
    const near = cloudyEarth(deckUnderHaze(), { cloudLayers: true });
    expect(near.drawn?.disc?.medium).toEqual(full.medium);
    expect(near.drawn?.disc?.sources).toEqual(full.drawn?.sources);
    expect(near.drawn?.disc?.regime).toEqual(regimeOf(full));
    expect(full.drawn?.disc).toBeUndefined();
  });

  it.each([
    ["a clear sky", 0],
    ["an overcast", 1],
  ])("draws every mode under %s, which the layer does not draw", (_, cloudFraction) => {
    const near = cloudyEarth(deckUnderHaze(), { cloudLayers: true, cloudFraction });
    expect(aerosolsOf(near.medium)).toEqual(["aerosol:H2O", "aerosol:H2SO4"]);
    expect(near.drawn?.disc).toBeUndefined();
  });

  it("classes the served decked world's near medium as its clear air, its full medium routed as before", () => {
    const input = recordInput(servedDeckedDetail());
    const full = assembleMedium(input);
    const near = assembleMedium(input, { cloudLayers: true });
    expect(aerosolsOf(near.medium)).toEqual(["aerosol:H2SO4"]);
    // Thick by its clear air's depth alone (τ 0.13 to 0.66 against τ* 0.125), as a clear
    // Earth-like air is; the deck no longer routes it.
    expect(regimeOf(near)?.kind).toBe("thickScattering");
    expect(near.drawn?.disc?.regime).toEqual(regimeOf(full));
    // Its full medium took the decked air's route; its near medium, clear air under a thin haze,
    // leaves it.
    expect([routed(full), routed(near)]).toEqual([true, false]);
  });
});

/** {@link waterDeck} with its optical depth not modelled, as the wire sends a mode's optics not. */
function unmodelledDeck(): AerosolInventoryDto {
  const deck = waterDeck(0.6);
  return { ...deck, modes: deck.modes.map(withoutOpticalDepth) };
}

/** A mode with its optical depth not modelled. */
function withoutOpticalDepth(mode: AerosolModeDto): AerosolModeDto {
  const { tau_550: _depth, tau_basis: _basis, ...rest } = mode;
  return Object.assign(rest, { not_modelled: { type: "optics_not_modelled" } as const });
}

/** Whether a reason says a mode's member is not modelled and the mode not drawn. */
function notModelled(reason: string): boolean {
  return reason.includes("not modelled, not drawn");
}

describe("the reasons of the deck R11's clouds draw (R11.T7.b)", () => {
  it("says the layer draws a deck the record leaves unmodelled with a stand-in's optics and depth", () => {
    const full = cloudyEarth(unmodelledDeck());
    const near = cloudyEarth(unmodelledDeck(), { cloudLayers: true });
    expect([full.reasons.some(notModelled), near.reasons.some(notModelled)]).toEqual([true, false]);
    expect(near.reasons).toContain("H2O clouds: drawn with a stand-in's optics and optical depth");
  });

  it("says nothing of a deck the record's optics and optical depth draw", () => {
    const near = cloudyEarth(waterDeck(0.6), { cloudLayers: true });
    expect(near.reasons.filter((reason) => reason.includes("stand-in"))).toEqual([]);
  });
});
