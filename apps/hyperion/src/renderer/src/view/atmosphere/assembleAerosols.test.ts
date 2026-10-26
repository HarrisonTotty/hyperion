/**
 * A body's inventory in its medium (plan R08, R08.T10.a; Design notes 6 and 12): its modes as
 * aerosol terms, its absorbers' layers, a deck's vapour, the keys this client does not know, and
 * the reasons each gives under the τ rule.
 */
import type { AerosolInventoryDto, AerosolModeDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  type DetailAnswer,
  detailOf,
  EMPTY_INVENTORY,
  gammaMode,
  layerAbsorber,
  plutoHazeDetail,
  recordInput,
  unknownKeysDetail,
  withInventory,
  withSurface,
} from "../../test/atmosphereRecords";
import { earthDetail } from "../../test/planetaryFixture";
import { type AssembledMedium, assembleMedium, sizeConvergenceReasons } from "./assemble";
import { modeOptics } from "./sizeDistribution";

/** The generated Earth with an inventory, assembled with no line tables. */
function earthWith(inventory: AerosolInventoryDto): AssembledMedium {
  return assembleMedium(recordInput(detailOf(withInventory(earthDetail(), inventory))));
}

/** The generated Earth's surface on a cold N₂–O₂ air (80% and 20%) at 80 K, with an inventory. */
function coldAirWith(inventory: AerosolInventoryDto): AssembledMedium {
  const air: DetailAnswer = withSurface(earthDetail(), (surface) => ({
    ...surface,
    mean_surface_temperature_k: 80,
    gases: [
      { species: "N2", mole_fraction: 0.8 },
      { species: "O2", mole_fraction: 0.2 },
    ],
    mean_molar_mass_g_per_mol: 0.8 * 28.0134 + 0.2 * 31.9988,
  }));
  return assembleMedium(recordInput(detailOf(withInventory(air, inventory))));
}

/** An inventory of modes alone. */
function modes(...list: ReadonlyArray<AerosolModeDto>): AerosolInventoryDto {
  return { ...EMPTY_INVENTORY, modes: [...list] };
}

/** The aerosol terms' names of an assembly. */
function aerosolTerms(result: AssembledMedium): ReadonlyArray<string> {
  return (result.medium?.terms ?? [])
    .map((term) => term.name)
    .filter((name) => name.startsWith("aerosol"));
}

describe("a record with a key this client does not know in each keyed member", () => {
  const result = assembleMedium(recordInput(unknownKeysDetail()));

  it("draws its air and its haze without a fault", () => {
    expect([result.medium !== undefined, aerosolTerms(result)]).toEqual([
      true,
      ["aerosol:newhaze"],
    ]);
  });

  it("carries APPROXIMATE for the gas, the absorber and the material it does not know", () => {
    expect([
      result.labels.includes("atmosphereApproximate"),
      result.reasons.filter(
        (reason) =>
          reason.includes("not known to this console") || reason.includes("no index file"),
      ),
    ]).toEqual([
      true,
      [
        "Xq: not known to this console, not drawn",
        "Zq2: not known to this console, not drawn",
        "no index file for unlisted material: the generic stand-in, a non-absorbing sphere of real index 1.5, a stated convention",
      ],
    ]);
  });
});

describe("a tenuous Pluto-like record", () => {
  it("draws its tholin haze in its thin air", () => {
    expect(aerosolTerms(assembleMedium(recordInput(plutoHazeDetail())))).toEqual([
      "aerosol:tholin",
    ]);
  });
});

/** A deck of a liquid's droplets at τ 1, from 800 to 500 hPa. */
function liquidDeck(material: string): AerosolModeDto {
  return gammaMode({
    material,
    phase: "liquid",
    effectiveRadiusM: 2e-6,
    tau550: 1,
    basePa: 80_000,
    topPa: 50_000,
  });
}

/** A thin haze of a solid's grains, from 200 to 50 hPa. */
function solidHaze(material: string): AerosolModeDto {
  return gammaMode({
    material,
    phase: "solid",
    effectiveRadiusM: 1e-6,
    tau550: 0.1,
    basePa: 20_000,
    topPa: 5_000,
  });
}

/**
 * A water cloud from 800 to 600 hPa: droplets of 1 µm keep Mie's size sum short, and the τ rule
 * reads only the cloud's τ.
 */
function waterCloud(tau550: number): AerosolModeDto {
  return gammaMode({
    material: "H2O",
    phase: "liquid",
    effectiveRadiusM: 1e-6,
    tau550,
    basePa: 80_000,
    topPa: 60_000,
  });
}

/**
 * NH4SH's stand-in grains under 900 hPa, of 0.5 µm, which keep its size sum short (R08.T5.f's
 * slowest case is near 2 µm).
 */
const AMMONIUM_HYDROSULPHIDE = gammaMode({
  material: "NH4SH",
  phase: "solid",
  effectiveRadiusM: 5e-7,
  tau550: 1,
  basePa: 100_000,
  topPa: 90_000,
});

/** The reasons naming NH4SH. */
function standInReasons(result: AssembledMedium): ReadonlyArray<string> {
  return result.reasons.filter((reason) => reason.includes("NH4SH"));
}

/** A water deck and its vapour's mole fraction above it. */
function waterDeck(vapourAbove: number): AerosolInventoryDto {
  return {
    modes: [
      gammaMode({
        material: "H2O",
        phase: "liquid",
        deck: "H2O",
        effectiveRadiusM: 1e-6,
        tau550: 2,
        basePa: 80_000,
        topPa: 60_000,
      }),
    ],
    decks: [{ condensate: "H2O", vapour_mole_fraction_above: vapourAbove }],
    absorbers: [],
  };
}

/** Liquid O2's reason, as its file states it (signoff-12.md, item 38(c)). */
const O2_REASON =
  "liquid O2: its absorption is the gas's O2–O2 pair absorption at 203 K carried to the liquid's density; no measurement of the liquid's is read";

describe("liquid O₂'s colour (signoff-12.md, item 38(c))", () => {
  it("raises APPROXIMATE with its own reason in a deck of τ 1", () => {
    const result = coldAirWith(modes(liquidDeck("O2")));
    expect([
      result.labels.includes("atmosphereApproximate"),
      result.reasons.includes(O2_REASON),
    ]).toEqual([true, true]);
  });

  it("is liquid O2's alone: a liquid N2 deck gives no reason of its material", () => {
    const result = coldAirWith(modes(liquidDeck("N2")));
    expect(result.reasons.filter((reason) => reason.includes("N2"))).toEqual([]);
  });
});

describe("a named analogue's phase word in the reasons (signoff-12.md, item 39(c))", () => {
  it("names a solid SO2 mode's analogue SO2 ice's", () => {
    expect(coldAirWith(modes(solidHaze("SO2"))).reasons).toContain(
      "no SO2 ice on file: liquid SO2 at 198 K stands in, a named analogue",
    );
  });

  it("names a solid Kr mode's analogue an atomic solid's", () => {
    expect(coldAirWith(modes(solidHaze("Kr"))).reasons).toContain(
      "no solid Kr on file: liquid Kr at 116 K stands in, a named analogue",
    );
  });
});

describe("Design note 12's τ rule", () => {
  it("shows a stand-in beneath a cloud thinner than the split's depth", () => {
    expect(
      standInReasons(earthWith(modes(AMMONIUM_HYDROSULPHIDE, waterCloud(2)))).length,
    ).toBeGreaterThan(0);
  });

  it("hides a stand-in beneath a cloud past the split's depth", () => {
    expect(standInReasons(earthWith(modes(AMMONIUM_HYDROSULPHIDE, waterCloud(20))))).toEqual([]);
  });
});

describe("the inventory's layers against the column", () => {
  it("skips a mode of τ 0 and a mode whose base is its top, without a fault", () => {
    const result = earthWith(
      modes(
        gammaMode({
          material: "H2O",
          phase: "liquid",
          effectiveRadiusM: 1e-6,
          tau550: 0,
          basePa: 80_000,
          topPa: 60_000,
        }),
        gammaMode({
          material: "H2O",
          phase: "liquid",
          effectiveRadiusM: 1e-6,
          tau550: 1,
          basePa: 70_000,
          topPa: 70_000,
        }),
      ),
    );
    expect(aerosolTerms(result)).toEqual([]);
  });

  it("skips an absorber layer of no column, or whose base is its top, without a fault", () => {
    const result = earthWith({
      ...EMPTY_INVENTORY,
      absorbers: [
        layerAbsorber("O3", 0, 50_000, 10_000),
        layerAbsorber("SO2", 1e21, 50_000, 50_000),
      ],
    });
    expect(
      result.medium?.terms.filter((term) => term.name.includes("O3") || term.name.includes("SO2")),
    ).toEqual([]);
  });

  it("drops an absorber layer wholly above the column's top and says so", () => {
    // The column's top is 10⁻⁷ of p_s, about 0.01 Pa.
    const result = earthWith({
      ...EMPTY_INVENTORY,
      absorbers: [layerAbsorber("O3", 1e22, 1e-3, 1e-4)],
    });
    expect(result.reasons).toContain("O3: its layer lies above the air drawn, not drawn");
  });

  it("drops an absorber layer wholly below the datum, which the ground hides, with no reason", () => {
    const result = earthWith({
      ...EMPTY_INVENTORY,
      absorbers: [layerAbsorber("O3", 1e22, 300_000, 200_000)],
    });
    expect([
      result.medium?.terms.some((term) => term.name.includes("O3")),
      result.reasons.filter((reason) => reason.startsWith("O3")),
    ]).toEqual([false, []]);
  });

  it("lays a layer that straddles the column's top wholly inside it", () => {
    const result = earthWith({
      ...EMPTY_INVENTORY,
      absorbers: [layerAbsorber("O3", 1e22, 1_000, 1e-4)],
    });
    const ozone = result.drawn?.absorbers.find((spec) => spec.name === "absorber:O3");
    expect([
      ozone?.kind === "column" ? ozone.columnPerM2 : undefined,
      result.reasons.filter((reason) => reason.startsWith("O3")),
    ]).toEqual([1e22, []]);
  });

  it("drops a mode wholly above the column's top and says so", () => {
    const result = earthWith(
      modes(
        gammaMode({
          material: "H2O",
          phase: "solid",
          effectiveRadiusM: 1e-6,
          tau550: 0.1,
          basePa: 1e-3,
          topPa: 1e-4,
        }),
      ),
    );
    expect(result.reasons).toContain("solid H2O particles lie above the air drawn, not drawn");
  });
});

describe("a deck's vapour", () => {
  it("is cut at the deck's top", () => {
    const pair = earthWith(waterDeck(0)).drawn?.absorbers.find((spec) => spec.kind === "pair");
    expect(pair?.topPa).toBe(60_000);
  });

  it("says its vapour above the deck is not drawn, where there is some", () => {
    expect(earthWith(waterDeck(1e-5)).reasons).toContain(
      "H2O: its vapour above its cloud deck is not drawn",
    );
  });

  it("says nothing of a deck with no vapour above it", () => {
    expect(earthWith(waterDeck(0)).reasons.filter((reason) => reason.includes("vapour"))).toEqual(
      [],
    );
  });
});

describe("a mode's size quadrature (R08.T5.f)", () => {
  const converged = modeOptics(
    {
      material: "H2O",
      form: { phase: "liquid" },
      sizes: { kind: "gamma", effectiveRadiusUm: 1, effectiveVariance: 0.1 },
    },
    550,
  );

  it("gives a reason where it did not converge at a wavelength drawn", () => {
    const unconverged = {
      ...converged,
      sizeConvergence: { converged: false, nodes: 1_048_576, lastChange: 3e-4 },
    };
    expect(sizeConvergenceReasons("H2O", [converged, unconverged])).toEqual([
      "H2O particles: their sizes summed short of convergence at 550 nm",
    ]);
  });

  it("gives none where it converged", () => {
    expect(sizeConvergenceReasons("H2O", [converged])).toEqual([]);
  });
});
