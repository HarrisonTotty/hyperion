/**
 * A gas envelope's medium from its record (plan R08, R08.T16.a.1; Design notes 9, 13 and 17;
 * `decision-deep-decks-split.md` §3.3): no ground, its floor where τ(550) from the top reaches 10,
 * its modes at their input's pressures and cut at the floor, its datum the 1-bar figure's, on the
 * hand Jupiter and ice giant (`test/giantFixtures.ts`) and on the protocol fixture's two envelopes.
 */
import { describe, expect, it } from "vitest";

import type { AerosolMode, BodyEnvelope } from "../../lib/system/model";
import { detailOf, recordInput, type DetailAnswer } from "../../test/atmosphereRecords";
import {
  handIceGiantDetail,
  handJupiterDetail,
  ICE_GIANT_METHANE_TAU,
  JUPITER_EQUATORIAL_RADIUS_M,
  JUPITER_POLAR_RADIUS_M,
} from "../../test/giantFixtures";
import { memoised } from "../../test/memo";
import { hotJupiterDetail, jupiterDetail } from "../../test/planetaryFixture";
import { type AssembledMedium, assembleMedium, type DrawnMedium } from "./assemble";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "./thick/regime";
import { ENVELOPE_DATUM_PA, MODE_EDGE_LN_P, shareAbove, skyOpticalDepth550 } from "./envelope";
import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  type MediumTerm,
} from "./medium";
import { phaseTableAsymmetry } from "./aerosol";
import { materialApproximations } from "./materials/materials";
import { gravityRatio } from "./oblate";
import { molecularMixture } from "./rayleigh";

/** The fixtures by name. */
const ANSWERS = {
  handJupiter: () => handJupiterDetail(),
  handJupiterLayer: () => handJupiterDetail(6.5),
  iceGiant: () => handIceGiantDetail(),
  jupiter: () => jupiterDetail(),
  hotJupiter: () => hotJupiterDetail(),
} as const;

type Fixture = keyof typeof ANSWERS;

/** A fixture's envelope, as the client decodes it. */
function envelopeOf(answer: DetailAnswer): BodyEnvelope {
  const { envelope } = detailOf(answer).record;
  if (envelope.state !== "ok") {
    throw new Error("the fixture's giant has its envelope");
  }
  return envelope.value;
}

/** A fixture assembled with its envelope drawn, once a file. */
const assembled = memoised(
  (fixture: Fixture): AssembledMedium =>
    assembleMedium(recordInput(detailOf(ANSWERS[fixture]())), { envelopes: true }),
  (fixture) => fixture,
);

/** A fixture's drawn medium, which it has, its floor and column with it. */
function drawnOf(fixture: Fixture): {
  readonly medium: AtmosphereMedium;
  readonly drawn: DrawnMedium & { readonly envelope: NonNullable<DrawnMedium["envelope"]> };
  readonly result: AssembledMedium;
} {
  const result = assembled(fixture);
  if (result.medium === undefined) {
    throw new Error(`the ${fixture} draws a medium`);
  }
  const { envelope } = result.drawn;
  if (envelope === undefined) {
    throw new Error(`the ${fixture}'s medium is an envelope's`);
  }
  return { medium: result.medium, drawn: { ...result.drawn, envelope }, result };
}

/** A medium's term by name. */
function termOf(medium: AtmosphereMedium, name: string): MediumTerm {
  const term = medium.terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`the medium has no term ${name}`);
  }
  return term;
}

/** The height of the column's level at pressure `pressurePa`, which it must have. */
function levelHeightM(drawn: DrawnMedium, pressurePa: number): number {
  const { envelope, temperature } = drawn;
  const i = envelope?.pressuresPa.indexOf(pressurePa) ?? -1;
  const h = temperature?.heightsM[i];
  if (i < 0 || h === undefined) {
    throw new Error(`the column has no level at ${pressurePa} Pa`);
  }
  return h;
}

/** A term's share of its τ above height `heightM`: its column above over its whole. */
function shareAboveHeight(medium: AtmosphereMedium, term: MediumTerm, heightM: number): number {
  const whole = columnLengthM(term.density, medium.topHeightM);
  return (whole - columnLengthM(term.density, heightM)) / whole;
}

/** The τ(550) of the molecular term from the top down to height `heightM`. */
function molecularDepth550(fixture: Fixture, heightM: number): number {
  const { medium } = drawnOf(fixture);
  const rayleigh = termOf(medium, "rayleigh");
  const gases = envelopeOf(ANSWERS[fixture]()).composition;
  const mixture = molecularMixture(gases);
  // The term's green coefficient is n_s σ(green); the τ rule's is n_s σ(550).
  const perM550 =
    (rayleigh.scattering[1] * mixture.crossSectionM2(550)) /
    mixture.crossSectionM2(CHANNEL_WAVELENGTHS_NM[1]);
  return (
    perM550 *
    (columnLengthM(rayleigh.density, medium.topHeightM) - columnLengthM(rayleigh.density, heightM))
  );
}

/**
 * A drawn mode's whole τ(550): its source's σ_ext(550) at relative density 1 times its density's
 * column (the term and its source share an index). `material` is its term's name after `aerosol:`,
 * `NH3:2` for a material's second mode.
 */
function drawnDepth550(fixture: Fixture, material: string): number {
  const { medium, drawn } = drawnOf(fixture);
  const index = medium.terms.findIndex((t) => t.name === `aerosol:${material}`);
  const term = medium.terms[index];
  const source = drawn.sources[index];
  if (term === undefined || source?.kind !== "aerosol") {
    throw new Error(`the ${fixture} draws no ${material} mode`);
  }
  return source.extinction550PerM * columnLengthM(term.density, medium.topHeightM);
}

/** The τ(550) every mode of an envelope's inventory puts above pressure `pressurePa`, at coverage. */
function modesAbove(modes: ReadonlyArray<AerosolMode>, pressurePa: number): number {
  return modes.reduce(
    (sum, mode) => sum + skyOpticalDepth550(mode) * shareAbove(mode, pressurePa),
    0,
  );
}

/** A mode of the fixture's inventory by material. */
function modeOf(fixture: Fixture, material: string): AerosolMode {
  const mode = envelopeOf(ANSWERS[fixture]()).aerosols.modes.find((m) => m.material === material);
  if (mode === undefined) {
    throw new Error(`the ${fixture}'s inventory has no ${material} mode`);
  }
  return mode;
}

/**
 * The fixture's NH₃ deck's haze: its second NH₃ mode, after the deck's own (P14.T24.h.b part 6,
 * from generator version 27).
 */
function hazeOf(fixture: Fixture): AerosolMode {
  const modes = envelopeOf(ANSWERS[fixture]()).aerosols.modes.filter((m) => m.material === "NH3");
  const haze = modes[1];
  if (modes.length !== 2 || haze === undefined) {
    throw new Error(`the ${fixture}'s NH₃ deck has its haze`);
  }
  return haze;
}

/**
 * An envelope's photosphere, Pa: where its grey profile's thermal optical depth is ⅔, κ₁ p₁
 * (p ÷ p₁)ⁿ ÷ (n g) = ⅔ at its bulk gravity (P14.T24.d's `EnvelopeProfile::photosphere_pressure`).
 */
function photospherePa(answer: DetailAnswer): number {
  const { profile } = envelopeOf(answer);
  const { bulk } = detailOf(answer).record;
  if (bulk.state !== "ok") {
    throw new Error("the fixture's giant has its bulk section");
  }
  const n = profile.opticalDepthExponent;
  const g = bulk.value.surfaceGravityMS2;
  return 1e5 * (((2 / 3) * n * g) / (profile.thermalOpacityM2PerKg * 1e5)) ** (1 / n);
}

/** The hot Jupiter fixture's photosphere, Pa: 0.2049 bar at 27's κ₁ (0.2067 at 26's). */
const HOT_JUPITER_PHOTOSPHERE_PA = photospherePa(hotJupiterDetail());

describe("an envelope's medium, asked for", () => {
  it("is not drawn, the envelope NOT YET MODELLED, unless the assembly is asked for envelopes", () => {
    const result = assembleMedium(recordInput(detailOf(jupiterDetail())));
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  // The fixture Jupiter's assembly, which the file's later tests share. Measured 2026-11-01 at load
  // 34: 9.1–11.2 s alone (8.3–9.4 s on 24's record, its molten decks not drawn), 19.4 s in the
  // whole suite; 2026-11-04 at load 8, on 27's record, its NH₃ deck's two modes and its chromophore
  // its only modes drawn: 0.88–0.89 s alone.
  it("is drawn when asked, with no NOT YET MODELLED", { timeout: 60_000 }, () => {
    const { result } = drawnOf("jupiter");
    expect(result.labels).not.toContain("atmosphereNotYetModelled");
  });
});

describe("the hand Jupiter's medium", () => {
  it("has no ground: a black one", () => {
    expect(drawnOf("handJupiter").medium.groundAlbedo).toEqual([0, 0, 0]);
  });

  it("runs past τ 1,000 of green from the top, which its deck reads to inside it", () => {
    const { medium } = drawnOf("handJupiter");
    const rayleigh = termOf(medium, "rayleigh");
    expect(
      rayleigh.scattering[1] * columnLengthM(rayleigh.density, medium.topHeightM),
    ).toBeGreaterThan(1_000);
  });

  it("is split at its floor, above its ground", () => {
    const { drawn, result } = drawnOf("handJupiter");
    expect(result.medium === undefined ? null : result.regime).toEqual({
      kind: "cloudDeck",
      deckAltitudeM: drawn.envelope.floorAltitudeM,
    });
    expect(drawn.envelope.floorAltitudeM).toBeGreaterThan(0);
  });

  it("puts its floor where τ(550) from the top reaches 10", () => {
    const { floorPa, floorAltitudeM } = drawnOf("handJupiter").drawn.envelope;
    const modes = envelopeOf(ANSWERS.handJupiter()).aerosols.modes;
    const depth = molecularDepth550("handJupiter", floorAltitudeM) + modesAbove(modes, floorPa);
    expect(Math.abs(depth / CLOUD_DECK_SPLIT_OPTICAL_DEPTH - 1)).toBeLessThan(1e-4);
  });

  // Under 30's NH₃ haze (`giantFixtures.ts`'s `coldGiantHazes`, τ(550) 7 above the NH₃ deck) the
  // floor rises above the water deck's base, to 2.12 bar.
  it("puts its floor between its NH₃ deck's base and its water deck's", () => {
    const { floorPa } = drawnOf("handJupiter").drawn.envelope;
    expect(floorPa).toBeGreaterThan(modeOf("handJupiter", "NH3").basePa);
    expect(floorPa).toBeLessThan(modeOf("handJupiter", "H2O").basePa);
  });

  it("tops its column above every mode it reads", () => {
    for (const fixture of ["handJupiter", "hotJupiter", "jupiter"] as const) {
      const { pressuresPa } = drawnOf(fixture).drawn.envelope;
      const inventory = envelopeOf(ANSWERS[fixture]()).aerosols;
      const tops = [...inventory.modes, ...inventory.absorbers].map((m) => m.topPa);
      expect(pressuresPa.at(-1) ?? Number.NaN).toBeLessThan(Math.min(...tops));
    }
  });

  it("lays its NH₃ deck at 0.5–1 bar, a level at each end of each deck above its floor", () => {
    const { medium, drawn } = drawnOf("handJupiter");
    const nh3 = modeOf("handJupiter", "NH3");
    expect(nh3.basePa).toBeGreaterThan(0.5e5);
    expect(nh3.basePa).toBeLessThan(1e5);
    // Inside its ends lies all of each deck's τ but the ramps aerosolTerm lays past them, a deck
    // that straddles the floor cut there; the water deck, below the floor, takes no term.
    const { floorPa } = drawn.envelope;
    const outside = ["NH4SH", "NH3"].map((material) => {
      const mode = modeOf("handJupiter", material);
      const term = termOf(medium, `aerosol:${material}`);
      const base = levelHeightM(drawn, Math.min(mode.basePa, floorPa));
      const top = levelHeightM(drawn, mode.topPa);
      const inside = shareAboveHeight(medium, term, base) - shareAboveHeight(medium, term, top);
      return (1 - inside) / (MODE_EDGE_LN_P * (mode.fSed + 1));
    });
    expect(outside.every((share) => share < 1)).toBe(true);
  });

  it.each<[string, Fixture]>([
    ["the sheet the record sends at 24–25, f 368", "handJupiter"],
    ["a layer as P14.T52.g sends from 26, f 6.5", "handJupiterLayer"],
  ])("resolves its NH₃ deck drawn as %s, to its profile's share", (_, fixture) => {
    const { medium, drawn } = drawnOf(fixture);
    const mode = modeOf(fixture, "NH3");
    const term = termOf(medium, "aerosol:NH3");
    const { pressuresPa } = drawn.envelope;
    const heights = drawn.temperature?.heightsM ?? new Float64Array();
    let worst = 0;
    let levels = 0;
    for (const [i, p] of pressuresPa.entries()) {
      if (p > mode.topPa && p < mode.basePa) {
        levels += 1;
        const share = shareAbove(mode, p);
        const drawnShare = shareAboveHeight(medium, term, heights[i] ?? Number.NaN);
        worst = Math.max(worst, Math.abs(drawnShare - share));
      }
    }
    expect(levels).toBeGreaterThan(20);
    expect(worst).toBeLessThan(2e-3);
  });

  it("stands on its 1-bar figure lowered by the 1-bar level's h*, so that the level lies on it", () => {
    const { drawn } = drawnOf("handJupiter");
    const depthM = levelHeightM(drawn, ENVELOPE_DATUM_PA);
    const { figure, gravity } = drawn.body;
    expect(figure.equatorialRadiusM + depthM / gravityRatio(gravity, 0)).toBeCloseTo(
      JUPITER_EQUATORIAL_RADIUS_M,
      -2,
    );
    expect(figure.polarRadiusM + depthM / gravityRatio(gravity, Math.PI / 2)).toBeCloseTo(
      JUPITER_POLAR_RADIUS_M,
      -2,
    );
    // Jupiter's figure gives four slices and three bands (Design note 17).
    expect([gravity.slicing.kappa.length, gravity.slicing.bandGravityRatio.length]).toEqual([4, 3]);
  });

  it("holds its drawn temperature at the floor's below the floor", () => {
    const { envelope, temperature } = drawnOf("handJupiter").drawn;
    const floorK = temperature?.temperaturesK[envelope.pressuresPa.indexOf(envelope.floorPa)];
    const below = [...(temperature?.temperaturesK ?? [])].filter(
      (_, i) => (envelope.pressuresPa[i] ?? 0) > envelope.floorPa,
    );
    expect(below.length).toBeGreaterThan(100);
    expect(below.every((t) => t === floorK)).toBe(true);
  });

  it("is not self-luminous", () => {
    const { result } = drawnOf("handJupiter");
    expect(result.medium === undefined ? null : result.regime.selfLuminous).toBeUndefined();
  });
});

describe("the protocol fixture's Jupiter (generator version 31's record)", () => {
  // From 27 its NH₃ deck carries its cloud's haze as its second mode (P14.T24.h.b part 6): NH₃ ice
  // of 1 µm, well mixed from the deck's base to 0.1 bar, at τ(550) 7 from 31 (10 to 30, which
  // alone reached the split's τ and put the floor inside the haze at 0.5661 bar). From 31 the NH₃
  // deck, its haze and the chromophore together hold 9.46 above the NH₄SH deck's top, so the floor
  // lies inside the NH₄SH and H₂O decks. 26's record had no haze and its decks' τ 5.7 lay under the
  // split's 10, its floor at 14.45 bar in its molten silicate decks' tails, below its water deck.
  it("bases its NH₃, NH₄SH and H₂O decks at 0.6348, 2.107 and 4.822 bar, τ 2.39, 1.59 and 1.73, and its NH₃ deck's haze at its base, τ 7 to 0.1 bar", () => {
    const decks = ["NH3", "NH4SH", "H2O"].map((m) => modeOf("jupiter", m));
    expect(decks.map((m) => Math.round(m.basePa / 10) / 1e4)).toEqual([0.6348, 2.1066, 4.8221]);
    expect(decks.map((m) => Math.round(skyOpticalDepth550(m) * 100) / 100)).toEqual([
      2.39, 1.59, 1.73,
    ]);
    // 5.7 together, under the split's 10; with the haze's 7, 12.7.
    expect(decks.reduce((sum, m) => sum + skyOpticalDepth550(m), 0)).toBeLessThan(
      CLOUD_DECK_SPLIT_OPTICAL_DEPTH,
    );
    const haze = hazeOf("jupiter");
    expect([haze.basePa, haze.topPa, skyOpticalDepth550(haze), haze.fSed]).toEqual([
      modeOf("jupiter", "NH3").basePa,
      1e4,
      7,
      0,
    ]);
  });

  it("draws its NH₃ deck, past Mie's reach, and its haze, each conservative and forward by its sizes and cut at the floor (R08.T16.a)", () => {
    const { medium, drawn, result } = drawnOf("jupiter");
    const { floorPa } = drawn.envelope;
    for (const [name, mode] of [
      ["NH3", modeOf("jupiter", "NH3")],
      ["NH3:2", hazeOf("jupiter")],
    ] as const) {
      const term = termOf(medium, `aerosol:${name}`);
      // Each straddles the floor, so draws the τ(550) its profile puts above it.
      const tau = skyOpticalDepth550(mode) * shareAbove(mode, floorPa);
      expect(Math.abs(drawnDepth550("jupiter", name) / tau - 1)).toBeLessThan(1e-12);
      const scattering = term.scattering[1];
      expect(term.absorption[1] / (scattering + term.absorption[1])).toBeLessThan(1e-5);
      if (term.phase.kind !== "tabulated") {
        throw new Error(`the ${name} mode's phase is tabulated`);
      }
      const asymmetry = phaseTableAsymmetry(term.phase.table, 1);
      expect(asymmetry).toBeGreaterThan(0.7);
      expect(asymmetry).toBeLessThan(0.9);
    }
    expect(result.reasons.filter((r) => r.includes("sizes lie outside the optics"))).toEqual([]);
  });

  it("puts its floor below its NH₃ deck and its haze, inside its NH₄SH and H₂O decks, at 1.878 bar", () => {
    const { floorPa, floorAltitudeM } = drawnOf("jupiter").drawn.envelope;
    // Measured 2026-11-08 at 31: 1.878 bar, the gas's τ 0.171 and the modes' 9.829 above it: the
    // NH₃ deck's 2.389, its haze's 7, the chromophore's 0.074, the NH₄SH deck's 0.364 and the H₂O
    // deck's 0.002 (to 30 0.5661 bar, inside the NH₃ deck and its haze; 26's: 14.45 bar).
    for (const mode of [modeOf("jupiter", "NH3"), hazeOf("jupiter")]) {
      expect(floorPa).toBeGreaterThan(mode.basePa);
    }
    for (const mode of [modeOf("jupiter", "NH4SH"), modeOf("jupiter", "H2O")]) {
      expect(floorPa).toBeGreaterThan(mode.topPa);
      expect(floorPa).toBeLessThan(mode.basePa);
    }
    const modes = envelopeOf(ANSWERS.jupiter()).aerosols.modes;
    const depth = molecularDepth550("jupiter", floorAltitudeM) + modesAbove(modes, floorPa);
    expect(Math.abs(depth / CLOUD_DECK_SPLIT_OPTICAL_DEPTH - 1)).toBeLessThan(1e-4);
    expect(Math.abs(floorPa / 1.878e5 - 1)).toBeLessThan(1e-3);
  });

  it("drops every deck whose top lies below the floor, all but its NH₃, NH₄SH and H₂O decks' modes, with no term and no label", () => {
    const { medium, drawn, result } = drawnOf("jupiter");
    // Measured 2026-11-08 at 31: every deck's top below the H₂O deck's, 1.771 bar, lies below the
    // floor (to 30 the NH₄SH and H₂O decks were dropped too; 26's floor dropped Fe, Cr, MnS and
    // Na₂S alone).
    const dropped = [
      "Fe",
      "CaTiO3",
      "Al2O3",
      "Mg2SiO4",
      "Cr",
      "MgSiO3",
      "MnS",
      "Na2S",
      "KCl",
      "ZnS",
    ];
    expect(drawn.envelope.dropped).toEqual(dropped);
    expect(medium.terms.map((t) => t.name).filter((n) => n.startsWith("aerosol:"))).toEqual([
      "aerosol:H2O",
      "aerosol:NH4SH",
      "aerosol:NH3",
      "aerosol:NH3:2",
      "aerosol:chromophore",
    ]);
    // No reason names a dropped deck (to 30 two named H₂O, then dropped, but not its deck).
    const named = new RegExp(`\\b(?:${dropped.join("|")})\\b`, "u");
    expect(result.reasons.filter((r) => named.test(r))).toEqual([]);
  });

  it("lets its decks and its haze enter with the τ their profiles put above the floor, cut there", () => {
    const { floorPa } = drawnOf("jupiter").drawn.envelope;
    const cut = [
      modeOf("jupiter", "NH3"),
      hazeOf("jupiter"),
      modeOf("jupiter", "NH4SH"),
      modeOf("jupiter", "H2O"),
    ].map((mode) => Math.round(skyOpticalDepth550(mode) * shareAbove(mode, floorPa) * 1000) / 1000);
    // Measured 2026-11-08 at 31: the NH₃ deck's 2.389 and the haze's 7 whole, 0.364 of the NH₄SH
    // deck's 1.59 and 0.002 of the H₂O deck's 1.73 (to 30 1.116 of the NH₃ deck's and 8.715 of
    // the haze's 10). 26's record let its molten Mg₂SiO₄ and MgSiO₃ decks' tails in so, 0.958 and
    // 1.663, which 27's floor drops.
    expect(cut).toEqual([2.389, 7, 0.364, 0.002]);
  });

  it("lays its cut NH₃ deck and haze above the floor", () => {
    const { medium, drawn } = drawnOf("jupiter");
    for (const name of ["aerosol:NH3", "aerosol:NH3:2"]) {
      const term = termOf(medium, name);
      expect(shareAboveHeight(medium, term, drawn.envelope.floorAltitudeM)).toBeCloseTo(1, 6);
    }
  });

  it("drops its KCl deck, its top far below the floor, with no reason of its own", () => {
    const kcl = modeOf("jupiter", "KCl");
    const { medium, drawn, result } = drawnOf("jupiter");
    // Measured 2026-11-04 at 27: its top at 14.42 bar, 25 times the floor's pressure (7.7 at 31).
    // 26's floor lay 0.16% below its top, let its tail in and left it out at the term limit,
    // labelled (`assembleEarth.test.ts` holds a mode left out at the term limit).
    expect(kcl.topPa).toBeGreaterThan(drawn.envelope.floorPa);
    expect(shareAbove(kcl, drawn.envelope.floorPa)).toBe(0);
    expect(medium.terms.map((t) => t.name)).not.toContain("aerosol:KCl");
    expect(result.reasons.filter((r) => r.includes("KCl"))).toEqual([]);
  });

  it("labels none of its dropped decks and none below the floor by the stand-in rule", () => {
    // materials.ts's depth rule: a stand-in shown while less than τ 10 lies above its top, which
    // every mode the cut lets in has, and none it drops. Seven of its ten dropped decks would
    // carry one drawn at the top (its liquid metals and silicates and its Na₂S; to 30 its NH₄SH
    // too, of twelve).
    const { result, drawn } = drawnOf("jupiter");
    const standIns = materialApproximations(
      drawn.envelope.dropped.map((material) => ({
        material,
        form: { phase: modeOf("jupiter", material).phase },
        opticalDepthAbove550: 0,
      })),
    ).map((a) => a.reason);
    expect(standIns).toHaveLength(7);
    expect(result.reasons.filter((r) => standIns.includes(r))).toEqual([]);
  });
});

describe("the protocol fixture's hot Jupiter (generator version 27's record)", () => {
  // From 26 a hot envelope's dayside decks are cut at 1 bar (P14.T52.h): every silicate and Fe
  // deck's top lies there, below the photosphere at 0.2049 bar (27's κ₁, refitted at the giants'
  // own Bond albedos, P14.T24.h.b; 0.2067 bar at 26). 27's composition adds P14.T52.e's further
  // carriers (40 species, 33 before).
  it("puts its floor below its photosphere, at its silicate decks' 1-bar tops", () => {
    const { floorPa } = drawnOf("hotJupiter").drawn.envelope;
    const forsterite = modeOf("hotJupiter", "Mg2SiO4");
    expect(forsterite.topPa).toBe(1e5);
    expect(floorPa).toBeGreaterThan(HOT_JUPITER_PHOTOSPHERE_PA);
    expect(floorPa).toBeGreaterThan(forsterite.topPa);
    expect(floorPa).toBeLessThan(forsterite.basePa);
  });

  it("draws of Mg₂SiO₄'s τ of 7.2 × 10⁴ only the share its profile puts above the floor", () => {
    const forsterite = modeOf("hotJupiter", "Mg2SiO4");
    const { floorPa } = drawnOf("hotJupiter").drawn.envelope;
    const tau = skyOpticalDepth550(forsterite) * shareAbove(forsterite, floorPa);
    expect(skyOpticalDepth550(forsterite)).toBeGreaterThan(7e4);
    expect(Math.abs(drawnDepth550("hotJupiter", "Mg2SiO4") / tau - 1)).toBeLessThan(1e-12);
    expect(tau).toBeLessThan(CLOUD_DECK_SPLIT_OPTICAL_DEPTH);
  });

  it("drops none of its decks, every top above the floor", () => {
    expect(drawnOf("hotJupiter").drawn.envelope.dropped).toEqual([]);
  });

  it("shows the stand-in's label for its liquid Fe, under the gas alone at its 1-bar top", () => {
    // materials.ts's depth rule: its top lies above the floor, so under less than τ 10. The decks
    // above it, Cr's and MnS's, cover none of the sky, and a mode's term takes its τ over the whole
    // sky (`modesOf`), so no mode's τ lies above it. 25's record labelled it too, its top then at
    // 0.0188 bar.
    const fe = modeOf("hotJupiter", "Fe");
    const { drawn, result } = drawnOf("hotJupiter");
    expect(fe.topPa).toBeLessThan(drawn.envelope.floorPa);
    expect(modesAbove(envelopeOf(ANSWERS.hotJupiter()).aerosols.modes, fe.topPa)).toBe(0);
    expect(result.reasons).toContain(
      "no liquid Fe on file: metallic Fe films at room temperature stands in, a named analogue",
    );
  });

  it("draws its liquid Fe with its τ cut at the floor, and no reason left for its τ", () => {
    const fe = modeOf("hotJupiter", "Fe");
    const { drawn, result } = drawnOf("hotJupiter");
    expect([fe.opticalDepth.kind, Math.round(fe.topPa) / 1e5]).toEqual(["value", 1]);
    // Measured 2026-11-04 at 27: 2.46 of its 1.12 × 10⁵ above the floor, at 1.0002 bar (26's: 2.44
    // of 1.14 × 10⁵).
    const tau = skyOpticalDepth550(fe) * shareAbove(fe, drawn.envelope.floorPa);
    expect(Math.abs(drawnDepth550("hotJupiter", "Fe") / tau - 1)).toBeLessThan(1e-12);
    expect(result.reasons.filter((r) => r.includes("Fe particles"))).toEqual([]);
  });

  it("puts no deck's τ above its photosphere", () => {
    expect(Math.round(HOT_JUPITER_PHOTOSPHERE_PA / 10) / 1e4).toBe(0.2049);
    const modes = envelopeOf(ANSWERS.hotJupiter()).aerosols.modes;
    // Measured 2026-11-03, and 2026-11-04 at 27: none, where 25's record put 172.9 there, its Fe's
    // 38.2 of it.
    expect(modesAbove(modes, HOT_JUPITER_PHOTOSPHERE_PA)).toBe(0);
  });

  it("knows every gas of its composition, P14.T52.e's further carriers among them", () => {
    // From 27 its composition carries SH, OH, P, P₂, PO, PH₂ and Cl, which the client's table of
    // the registry's flags holds from the same generator version, neither flag set: each notes its
    // scattering not drawn, and none is unknown or left out as an absorber.
    const { result } = drawnOf("hotJupiter");
    const carriers = ["SH", "OH", "P", "P2", "PO", "PH2", "Cl"];
    expect([
      result.reasons.filter((r) => r.includes("not known to this console")),
      carriers.filter((key) =>
        result.reasons.includes(`${key}: no scattering data on file, its scattering not drawn`),
      ),
      result.reasons.filter((r) => carriers.some((key) => r.startsWith(`${key}: left out`))),
    ]).toEqual([[], carriers, []]);
  });

  it("is self-luminous", () => {
    const { result } = drawnOf("hotJupiter");
    expect(result.medium === undefined ? null : result.regime.selfLuminous).toBe(true);
  });
});

describe("the hand ice giant", () => {
  it("puts its floor inside its CH₄ deck of τ 13.5", () => {
    const methane = modeOf("iceGiant", "CH4");
    const { floorPa } = drawnOf("iceGiant").drawn.envelope;
    expect(skyOpticalDepth550(methane)).toBe(ICE_GIANT_METHANE_TAU);
    expect(floorPa).toBeGreaterThan(methane.topPa);
    expect(floorPa).toBeLessThan(methane.basePa);
  });

  it("draws its CH₄ deck with the τ its profile puts above the floor, cut and not rescaled", () => {
    const methane = modeOf("iceGiant", "CH4");
    const { floorPa } = drawnOf("iceGiant").drawn.envelope;
    const tau = ICE_GIANT_METHANE_TAU * shareAbove(methane, floorPa);
    expect(Math.abs(drawnDepth550("iceGiant", "CH4") / tau - 1)).toBeLessThan(1e-12);
  });

  it("lays its CH₄ deck above the floor alone", () => {
    const { medium, drawn } = drawnOf("iceGiant");
    const term = termOf(medium, "aerosol:CH4");
    expect(shareAboveHeight(medium, term, drawn.envelope.floorAltitudeM)).toBeCloseTo(1, 6);
  });

  it("gives its cut CH₄ deck no reason of its own", () => {
    const { result } = drawnOf("iceGiant");
    expect(
      result.reasons.filter((r) => /CH4 ice particles: .*(not drawn|left out)/u.test(r)),
    ).toEqual([]);
  });

  it("drops its H₂S deck below the floor, with no term and no label", () => {
    const { medium, drawn, result } = drawnOf("iceGiant");
    expect(drawn.envelope.dropped).toEqual(["H2S"]);
    expect(medium.terms.map((t) => t.name)).not.toContain("aerosol:H2S");
    expect(result.reasons.filter((r) => r.includes("H2S"))).toEqual([]);
  });
});
