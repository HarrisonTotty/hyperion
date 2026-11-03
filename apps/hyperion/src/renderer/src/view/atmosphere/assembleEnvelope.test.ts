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
 * column (the term and its source share an index).
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

/** The hot Jupiter fixture's photosphere, Pa: 0.2067 bar. */
const HOT_JUPITER_PHOTOSPHERE_PA = photospherePa(hotJupiterDetail());

describe("an envelope's medium, asked for", () => {
  it("is not drawn, the envelope NOT YET MODELLED, unless the assembly is asked for envelopes", () => {
    const result = assembleMedium(recordInput(detailOf(jupiterDetail())));
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  // The fixture Jupiter's assembly, which the file's later tests share. Measured 2026-11-01 at load
  // 34: 9.1–11.2 s alone (8.3–9.4 s on 24's record, its molten decks not drawn), 19.4 s in the
  // whole suite.
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

  it("puts its floor below its water deck's base", () => {
    const { floorPa } = drawnOf("handJupiter").drawn.envelope;
    expect(floorPa).toBeGreaterThan(modeOf("handJupiter", "H2O").basePa);
  });

  it("tops its column above every mode it reads", () => {
    for (const fixture of ["handJupiter", "hotJupiter", "jupiter"] as const) {
      const { pressuresPa } = drawnOf(fixture).drawn.envelope;
      const inventory = envelopeOf(ANSWERS[fixture]()).aerosols;
      const tops = [...inventory.modes, ...inventory.absorbers].map((m) => m.topPa);
      expect(pressuresPa.at(-1) ?? Number.NaN).toBeLessThan(Math.min(...tops));
    }
  });

  it("lays its NH₃ deck at 0.5–1 bar, a level at each end of each deck", () => {
    const { medium, drawn } = drawnOf("handJupiter");
    const nh3 = modeOf("handJupiter", "NH3");
    expect(nh3.basePa).toBeGreaterThan(0.5e5);
    expect(nh3.basePa).toBeLessThan(1e5);
    // Inside its ends lies all of each deck's τ but the ramps aerosolTerm lays past them.
    const outside = ["H2O", "NH4SH", "NH3"].map((material) => {
      const mode = modeOf("handJupiter", material);
      const term = termOf(medium, `aerosol:${material}`);
      const base = levelHeightM(drawn, mode.basePa);
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

describe("the protocol fixture's Jupiter (generator version 26's record)", () => {
  it("bases its NH₃, NH₄SH and H₂O decks at 0.6415, 2.128 and 4.873 bar, τ 2.39, 1.60 and 1.73", () => {
    const decks = ["NH3", "NH4SH", "H2O"].map((m) => modeOf("jupiter", m));
    expect(decks.map((m) => Math.round(m.basePa / 10) / 1e4)).toEqual([0.6415, 2.1278, 4.8726]);
    expect(decks.map((m) => Math.round(skyOpticalDepth550(m) * 100) / 100)).toEqual([
      2.39, 1.6, 1.73,
    ]);
    // 5.7 together, under the split's 10: the gas and the molten decks' tails set the floor below
    // the water deck.
    expect(decks.reduce((sum, m) => sum + skyOpticalDepth550(m), 0)).toBeLessThan(
      CLOUD_DECK_SPLIT_OPTICAL_DEPTH,
    );
  });

  it("draws its NH₃, NH₄SH and H₂O decks, past Mie's reach, conservative and forward by their sizes (R08.T16.a)", () => {
    const { medium, result } = drawnOf("jupiter");
    for (const material of ["NH3", "NH4SH", "H2O"]) {
      const term = termOf(medium, `aerosol:${material}`);
      // Above the floor whole, each draws its record's τ(550).
      const tau = skyOpticalDepth550(modeOf("jupiter", material));
      expect(Math.abs(drawnDepth550("jupiter", material) / tau - 1)).toBeLessThan(1e-12);
      const scattering = term.scattering[1];
      expect(term.absorption[1] / (scattering + term.absorption[1])).toBeLessThan(1e-5);
      if (term.phase.kind !== "tabulated") {
        throw new Error(`the ${material} deck's phase is tabulated`);
      }
      const asymmetry = phaseTableAsymmetry(term.phase.table, 1);
      expect(asymmetry).toBeGreaterThan(0.7);
      expect(asymmetry).toBeLessThan(0.9);
    }
    expect(result.reasons.filter((r) => r.includes("sizes lie outside the optics"))).toEqual([]);
  });

  it("puts its floor below its water deck, in its molten silicate decks' tails, at 14.45 bar", () => {
    const { floorPa } = drawnOf("jupiter").drawn.envelope;
    // Measured 2026-11-01: 14.45 bar, the gas's τ 1.31 and the modes' 8.69 above it (24's: 42.10).
    // Not decision-deep-decks-split's 41 bar: from 25 its molten decks' tails put τ 2.66 above it.
    expect(floorPa).toBeGreaterThan(modeOf("jupiter", "H2O").basePa);
    for (const material of ["Mg2SiO4", "MgSiO3"]) {
      const mode = modeOf("jupiter", material);
      expect(floorPa).toBeGreaterThan(mode.topPa);
      expect(floorPa).toBeLessThan(mode.basePa);
    }
    expect(Math.abs(floorPa / 14.45e5 - 1)).toBeLessThan(1e-3);
  });

  it("drops the decks whose tops lie below the floor, Fe, Cr, MnS and Na₂S, with no term and no label", () => {
    const { medium, drawn, result } = drawnOf("jupiter");
    // Measured 2026-11-01: Fe's top, 23.76 bar, and Na₂S's, 37.25, above 24's floor, lie below 25's.
    const named = /\b(?:Fe|Cr|MnS|Na2S)\b/u;
    expect(drawn.envelope.dropped).toEqual(["Fe", "Cr", "MnS", "Na2S"]);
    expect(medium.terms.map((t) => t.name).filter((n) => named.test(n))).toEqual([]);
    expect(result.reasons.filter((r) => named.test(r))).toEqual([]);
  });

  it("lets its molten Mg₂SiO₄ and MgSiO₃ decks' tails enter with the τ their profiles put above the floor, cut there", () => {
    const { floorPa } = drawnOf("jupiter").drawn.envelope;
    const tails = ["Mg2SiO4", "MgSiO3"].map((material) => {
      const mode = modeOf("jupiter", material);
      const tau = skyOpticalDepth550(mode) * shareAbove(mode, floorPa);
      expect(Math.abs(drawnDepth550("jupiter", material) / tau - 1)).toBeLessThan(1e-12);
      return Math.round(tau * 1000) / 1000;
    });
    // Measured 2026-11-01: 0.958 of Mg₂SiO₄'s 1.35 × 10⁸ and 1.663 of MgSiO₃'s 6.34 × 10⁷.
    expect(tails).toEqual([0.958, 1.663]);
  });

  it("lays its molten decks' tails above the floor", () => {
    const { medium, drawn } = drawnOf("jupiter");
    for (const material of ["Mg2SiO4", "MgSiO3"]) {
      const term = termOf(medium, `aerosol:${material}`);
      expect(shareAboveHeight(medium, term, drawn.envelope.floorAltitudeM)).toBeCloseTo(1, 6);
    }
  });

  it("lets its KCl deck's tail enter, its top just above the floor, and leaves it out at the term limit, labelled", () => {
    const kcl = modeOf("jupiter", "KCl");
    const { medium, drawn, result } = drawnOf("jupiter");
    // Measured 2026-11-01: its top at 14.42 bar, 0.16% above the floor; its τ there 1.7 × 10⁻⁶ of its
    // 13,074 (24's: 0.071), left out with ZnS's, CaTiO₃'s and Al₂O₃'s tails, the least.
    expect(kcl.topPa).toBeLessThan(drawn.envelope.floorPa);
    expect(skyOpticalDepth550(kcl) * shareAbove(kcl, drawn.envelope.floorPa)).toBeLessThan(1e-5);
    expect(medium.terms.map((t) => t.name)).not.toContain("aerosol:KCl");
    expect(result.reasons).toContain(
      "solid KCl particles: left out, the console can draw no more layers",
    );
  });

  it("labels none of its dropped decks and none below the floor by the stand-in rule", () => {
    // materials.ts's depth rule: a stand-in shown while less than τ 10 lies above its top, which
    // every mode the cut lets in has, and none it drops.
    const { result } = drawnOf("jupiter");
    const dropped = drawnOf("jupiter").drawn.envelope.dropped;
    expect(result.reasons.filter((r) => dropped.some((m) => r.includes(m)))).toEqual([]);
  });
});

describe("the protocol fixture's hot Jupiter (generator version 26's record)", () => {
  // From 26 a hot envelope's dayside decks are cut at 1 bar (P14.T52.h): every silicate and Fe
  // deck's top lies there, below the photosphere at 0.2067 bar.
  it("puts its floor below its photosphere, at its silicate decks' 1-bar tops", () => {
    const { floorPa } = drawnOf("hotJupiter").drawn.envelope;
    const forsterite = modeOf("hotJupiter", "Mg2SiO4");
    expect(forsterite.topPa).toBe(1e5);
    expect(floorPa).toBeGreaterThan(HOT_JUPITER_PHOTOSPHERE_PA);
    expect(floorPa).toBeGreaterThan(forsterite.topPa);
    expect(floorPa).toBeLessThan(forsterite.basePa);
  });

  it("draws of Mg₂SiO₄'s τ of 7.3 × 10⁴ only the share its profile puts above the floor", () => {
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

  it("shows no stand-in's label for its liquid Fe, whose top the cut puts at 1 bar", () => {
    const { result } = drawnOf("hotJupiter");
    // Measured 2026-11-03: 25's record labelled it, its top then at 0.0188 bar.
    expect(result.reasons).not.toContain(
      "no liquid Fe on file: metallic Fe films at room temperature stands in, a named analogue",
    );
  });

  it("draws its liquid Fe with its τ cut at the floor, and no reason left for its τ", () => {
    const fe = modeOf("hotJupiter", "Fe");
    const { drawn, result } = drawnOf("hotJupiter");
    expect([fe.opticalDepth.kind, Math.round(fe.topPa) / 1e5]).toEqual(["value", 1]);
    // Measured 2026-11-03: 2.44 of its 1.14 × 10⁵ above the floor, at 1.0002 bar.
    const tau = skyOpticalDepth550(fe) * shareAbove(fe, drawn.envelope.floorPa);
    expect(Math.abs(drawnDepth550("hotJupiter", "Fe") / tau - 1)).toBeLessThan(1e-12);
    expect(result.reasons.filter((r) => r.includes("Fe particles"))).toEqual([]);
  });

  it("puts no deck's τ above its photosphere", () => {
    expect(Math.round(HOT_JUPITER_PHOTOSPHERE_PA / 10) / 1e4).toBe(0.2067);
    const modes = envelopeOf(ANSWERS.hotJupiter()).aerosols.modes;
    // Measured 2026-11-03: none, where 25's record put 172.9 there, its Fe's 38.2 of it.
    expect(modesAbove(modes, HOT_JUPITER_PHOTOSPHERE_PA)).toBe(0);
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
