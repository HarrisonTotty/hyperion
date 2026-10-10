import { describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { expectCommittedTable } from "../../test/bless";
import type { Rgb } from "../photometry/toneCurve";
import recorded from "./channels.json" with { type: "json" };
import {
  caseErrors,
  type CaseSun,
  type CaseView,
  CHANNEL_FIT_GASES,
  CHANNEL_FIT_SEARCH,
  CHANNEL_FIT_START_NM,
  channelFitAirs,
  channelFitCases,
  channelObjective,
  EARTH_DRY_AIR,
  EARTH_FIT_COLUMN,
  earthFitAir,
  FIT_SKY_DIRECTIONS_DEG,
  FIT_SUN_ZENITHS_DEG,
  fitChannels,
  prepareFamily,
  type SpectralAir,
  viewResponse,
} from "./channels";
import { FITTED_CHANNEL_WAVELENGTHS_NM, fittedTriple } from "./fittedChannels";
import { CHANNEL_WAVELENGTHS_NM, columnLengthM, densityAt } from "./medium";
import { distanceToTopM, localRadiusM } from "./opticalDepth";
import { molecularMixture } from "./rayleigh";
import { bakeSpectrumOf, spectralRgb, sunWeights } from "./spectralColour";

/** Bruneton's triple, R05's `CHANNEL_WAVELENGTHS_NM` until R08.T6.d. */
const BRUNETON_NM: Rgb = [680, 550, 440];

/** The suns of R06's colour table that `bake_spectra.json` holds, by grid. */
const SUNS: ReadonlyArray<CaseSun & { readonly teffK: number; readonly grid: string }> =
  bakeSpectra.suns.map((sun) => ({
    name:
      sun.grid === "MainSequence"
        ? `${sun.teff_k} K, log g ${sun.log_g}`
        : `${sun.teff_k} K, log g ${sun.log_g}, ${sun.grid}`,
    teffK: sun.teff_k,
    grid: sun.grid,
    spectrum: bakeSpectrumOf(sun.bake_spectrum),
  }));

/** One of {@link SUNS} on the main-sequence grid, by its temperature. */
function sunAt(teffK: number): CaseSun {
  const sun = SUNS.find((s) => s.teffK === teffK && s.grid === "MainSequence");
  if (sun === undefined) {
    throw new Error(`bake_spectra.json holds no ${teffK} K main-sequence sun`);
  }
  return sun;
}

/** The Sun: R06's `solar_colour()`. */
const THE_SUN = sunAt(5_772);

/** The plan's three suns (R08.T4.a): 3,200 K, the Sun and 9,000 K. */
const PLAN_SUNS = [sunAt(3_200), THE_SUN, sunAt(9_000)];

/** The Sun's own colour, blue over red. */
const sunBlueToRed = (() => {
  const [red, , blue] = sunWeights(THE_SUN.spectrum).rgb;
  return blue / red;
})();

/** A value rounded up to 10⁻⁶, so that the record is an upper bound robust to the last bits. */
function ceilMicro(value: number): number {
  return Math.ceil(value * 1e6) / 1e6;
}

/** A value rounded to 10⁻⁶. */
function roundMicro(value: number): number {
  return Math.round(value * 1e6) / 1e6;
}

/**
 * `channels.json`'s table from the fit over a family's atmospheres and {@link SUNS}: the family,
 * the fit and its objective, and the comparisons.
 */
function tableOf(airs: ReadonlyArray<SpectralAir>) {
  const family = prepareFamily(channelFitCases(SUNS, airs));
  const earth = prepareFamily(channelFitCases(PLAN_SUNS, [earthFitAir()]));
  const fit = fitChannels(family, CHANNEL_FIT_SEARCH);
  return {
    description:
      "The render channels' fitted wavelengths, nm (rendering plan R08, Design note 5; R08.T4.a), " +
      "written by channels.test.ts under HYPERION_BLESS=1 and checked unchanged otherwise. The " +
      "objective is the largest CIE 1976 delta u'v' between a three-channel colour and the " +
      "spectral one over the family, rounded up to 1e-6: each sun's light through each air, as " +
      "its direct beam and as the singly scattered sky in each direction, at each sun zenith " +
      "angle, seen from the ground.",
    suns: SUNS.map((s) => s.name),
    airs: airs.map((a) => a.name),
    sunZenithsDeg: FIT_SUN_ZENITHS_DEG,
    skyDirectionsDeg: FIT_SKY_DIRECTIONS_DEG,
    startNm: CHANNEL_FIT_SEARCH.startNm,
    stepsNm: [CHANNEL_FIT_SEARCH.firstStepNm, CHANNEL_FIT_SEARCH.lastStepNm],
    wavelengthsNm: fit.wavelengthsNm,
    objective: ceilMicro(fit.objective),
    objectiveAtBrunetonsTriple: roundMicro(channelObjective(family, BRUNETON_NM)),
    objectiveAtTheStart: roundMicro(channelObjective(family, CHANNEL_FIT_START_NM)),
    earthAtThreeSuns: {
      suns: PLAN_SUNS.map((s) => s.name),
      objective: roundMicro(channelObjective(earth, fit.wavelengthsNm)),
      objectiveAtBrunetonsTriple: roundMicro(channelObjective(earth, BRUNETON_NM)),
    },
  };
}

/** The fit's atmospheres, built once: Mars's dust optics alone cost about a second. */
const AIRS = channelFitAirs();

/** The fit's atmosphere of a name. */
function airNamed(name: string): SpectralAir {
  const air = AIRS.find((a) => a.name === name);
  if (air === undefined) {
    throw new Error(`the fit has no atmosphere ${name}`);
  }
  return air;
}

/** `channels.json`'s table as this run computes it, once for the file's tests (about 3 s). */
const TABLE = tableOf(AIRS);

/** Whether this run's family is the recorded one. */
const SAME_FAMILY =
  JSON.stringify([TABLE.suns, TABLE.airs, TABLE.sunZenithsDeg, TABLE.skyDirectionsDeg]) ===
  JSON.stringify([recorded.suns, recorded.airs, recorded.sunZenithsDeg, recorded.skyDirectionsDeg]);

describe("a view's response", () => {
  const earth = earthFitAir();
  const [molecular] = earth.terms;
  if (molecular === undefined) {
    throw new Error("Earth's fit air has its molecular term first");
  }

  /** Dry air's Rayleigh optical depth at 550 nm, vertical, through the closed-form column. */
  const rayleigh550 =
    EARTH_FIT_COLUMN.surfaceNumberDensityPerM3 *
    molecularMixture(EARTH_DRY_AIR).crossSectionM2(550) *
    columnLengthM(EARTH_FIT_COLUMN.density, EARTH_FIT_COLUMN.topHeightM);

  it("is e^(−τ) at noon, τ dry air's Rayleigh column and the aerosol's, to 10⁻⁴", () => {
    const aerosol = 0.1 * -Math.expm1(-EARTH_FIT_COLUMN.topHeightM / 1_200);
    const depth = -Math.log(viewResponse(earth, { kind: "sun", sunZenithDeg: 0 }, 550));
    expect(Math.abs(depth / (rayleigh550 + aerosol) - 1)).toBeLessThan(1e-4);
  });

  it("puts Earth's Rayleigh optical depth at 550 nm at Bodhaine et al.'s 0.0971, to 1%", () => {
    // Bodhaine, Wood, Dutton and Slusser 1999, J. Atmos. Oceanic Technol. 16, 1854, Table 3:
    // 9.7069 × 10⁻² at 0.550 µm, sea level (1013.25 mb), 45° latitude and 360 ppm of CO₂.
    expect(Math.abs(rayleigh550 / 0.097_069 - 1)).toBeLessThan(0.01);
  });

  it("is the slant column of a 4,096-step sum, to 10⁻⁴, at 85° and at the horizon", () => {
    const columnOnly: SpectralAir = {
      ...earth,
      terms: [{ ...molecular, scatteringPerM: () => 1e-5, absorptionPerM: () => 0 }],
    };
    const shell = {
      bottomRadiusM: earth.groundRadiusM,
      topRadiusM: earth.groundRadiusM + earth.topHeightM,
    };
    for (const sunZenithDeg of [85, 90]) {
      const mu = Math.cos((sunZenithDeg * Math.PI) / 180);
      const length = distanceToTopM(shell, earth.groundRadiusM, mu);
      let column = 0;
      for (let i = 0; i < 4_096; i += 1) {
        const s = ((i + 0.5) * length) / 4_096;
        const h = localRadiusM(earth.groundRadiusM, mu, s) - earth.groundRadiusM;
        column += (densityAt(molecular.density, h) * length) / 4_096;
      }
      const response = viewResponse(columnOnly, { kind: "sun", sunZenithDeg }, 550);
      expect(Math.abs(-Math.log(response) / 1e-5 / column - 1)).toBeLessThan(1e-4);
    }
  });

  it("scatters the sky bluer than the Sun's light", () => {
    const [red, , blue] = spectralRgb(THE_SUN.spectrum, (nm) =>
      viewResponse(
        earth,
        { kind: "sky", sunZenithDeg: 45, elevationDeg: 90, azimuthFromSunDeg: 0 },
        nm,
      ),
    );
    expect(blue / red).toBeGreaterThan(sunBlueToRed);
  });

  it("reddens the Sun 5° above the horizon", () => {
    const [red, , blue] = spectralRgb(THE_SUN.spectrum, (nm) =>
      viewResponse(earth, { kind: "sun", sunZenithDeg: 85 }, nm),
    );
    expect(blue / red).toBeLessThan(sunBlueToRed);
  });
});

describe("Mars's dusty air", () => {
  const mars = airNamed("Mars");

  /** The colour of Mars's sky in one direction under the Sun, blue over red. */
  const skyBlueToRed = (view: CaseView): number => {
    const [red, , blue] = spectralRgb(THE_SUN.spectrum, (nm) => viewResponse(mars, view, nm));
    return blue / red;
  };

  it("colours the day sky away from the Sun redder than the Sun's light, butterscotch", () => {
    const view: CaseView = {
      kind: "sky",
      sunZenithDeg: 30,
      elevationDeg: 45,
      azimuthFromSunDeg: 180,
    };
    expect(skyBlueToRed(view)).toBeLessThan(sunBlueToRed);
  });

  it("colours the sky around a setting Sun bluer than the Sun's light", () => {
    const view: CaseView = {
      kind: "sky",
      sunZenithDeg: 85,
      elevationDeg: 10,
      azimuthFromSunDeg: 0,
    };
    expect(skyBlueToRed(view)).toBeGreaterThan(sunBlueToRed);
  });
});

describe("the fitted triple", () => {
  it("is written to channels.json under a bless, and checked unchanged otherwise", async () => {
    // Vitest writes a blessed file when the file's tests finish, whatever failed, so a refit that
    // worsens the record over the same family is stopped here, before it is handed over.
    expect(SAME_FAMILY ? TABLE.objective : recorded.objective).toBeLessThanOrEqual(
      recorded.objective,
    );
    await expect(expectCommittedTable(TABLE, recorded, "./channels.json")).resolves.toBeUndefined();
  });

  it("is refitted from (620, 540, 445)", () => {
    expect(CHANNEL_FIT_SEARCH.startNm).toEqual([620, 540, 445]);
  });

  // The plan's own test, which the bless test above also asserts before it writes. A bless that
  // changes the family records a new objective, which the old one does not bound.
  it.runIf(SAME_FAMILY)("does not worsen the recorded objective in the refit", () => {
    expect(TABLE.objective).toBeLessThanOrEqual(recorded.objective);
  });

  it.each([
    ["680/550/440", BRUNETON_NM],
    ["620/540/445", CHANNEL_FIT_START_NM],
  ])("scores the triple %s as the largest of its cases' errors", (_, triple) => {
    const family = prepareFamily(channelFitCases(PLAN_SUNS, [earthFitAir(), airNamed("Mars")]));
    const largest = Math.max(...caseErrors(family, triple).map((e) => e.deltaUv));
    expect(Math.abs(channelObjective(family, triple) - largest)).toBeLessThan(1e-12);
  });

  it("draws the Sun 5° above Earth's horizon over 10% too bright at 680/550/440", () => {
    const [sun] = caseErrors(
      prepareFamily([
        { name: "sun", sun: THE_SUN, air: earthFitAir(), view: { kind: "sun", sunZenithDeg: 85 } },
      ]),
      BRUNETON_NM,
    );
    expect(sun?.luminanceRatio).toBeGreaterThan(1.1);
  });

  it("draws the Sun 5° above Earth's horizon within 3% of its brightness at the fitted triple", () => {
    const [sun] = caseErrors(
      prepareFamily([
        { name: "sun", sun: THE_SUN, air: earthFitAir(), view: { kind: "sun", sunZenithDeg: 85 } },
      ]),
      TABLE.wavelengthsNm,
    );
    expect(Math.abs((sun?.luminanceRatio ?? Number.NaN) - 1)).toBeLessThan(0.03);
  });

  it.each([
    ["a last step of 0", { ...CHANNEL_FIT_SEARCH, lastStepNm: 0 }],
    ["a NaN last step", { ...CHANNEL_FIT_SEARCH, lastStepNm: Number.NaN }],
    ["a last step above the first", { ...CHANNEL_FIT_SEARCH, lastStepNm: 16 }],
    ["an infinite first step", { ...CHANNEL_FIT_SEARCH, firstStepNm: Number.POSITIVE_INFINITY }],
    ["an unordered start", { ...CHANNEL_FIT_SEARCH, startNm: [540, 620, 445] as const }],
    ["a start beyond 760 nm", { ...CHANNEL_FIT_SEARCH, startNm: [770, 540, 445] as const }],
  ])("refuses a search with %s", (_, search) => {
    expect(() => fitChannels({ views: [], cases: [] }, search)).toThrow(RangeError);
  });

  it("beats 680/550/440 at Earth under the Sun at a sun zenith of 85°", () => {
    const at85 = prepareFamily(
      channelFitCases([THE_SUN], [earthFitAir()]).filter((c) => c.view.sunZenithDeg === 85),
    );
    const worst = (triple: Rgb): number =>
      caseErrors(at85, triple).reduce((m, e) => Math.max(m, e.deltaUv), 0);
    expect(worst(TABLE.wavelengthsNm)).toBeLessThan(worst(BRUNETON_NM));
  });

  it("beats 680/550/440 over the family", () => {
    expect(TABLE.objective).toBeLessThan(TABLE.objectiveAtBrunetonsTriple);
  });

  it("beats 680/550/440 over Earth at the plan's three suns", () => {
    expect(TABLE.earthAtThreeSuns.objective).toBeLessThan(
      TABLE.earthAtThreeSuns.objectiveAtBrunetonsTriple,
    );
  });

  it("names each of its stars once, so that every case's name is its own", () => {
    expect(new Set(TABLE.suns).size).toBe(TABLE.suns.length);
  });

  it("spans R06's main sequence from 2,300 to 45,000 K", () => {
    const dwarfs = SUNS.filter((s) => s.grid === "MainSequence").map((s) => s.teffK);
    expect([Math.min(...dwarfs), Math.max(...dwarfs)]).toEqual([2_300, 45_000]);
  });

  it("holds a giant and a white dwarf", () => {
    expect(new Set(SUNS.map((s) => s.grid))).toEqual(
      new Set(["MainSequence", "Giant", "WhiteDwarf"]),
    );
  });

  it("is fitted through Earth, Mars and every gas with Rayleigh optics here", () => {
    expect(TABLE.airs).toEqual(["Earth", "Mars", ...CHANNEL_FIT_GASES]);
  });

  it("is what FITTED_CHANNEL_WAVELENGTHS_NM reads", () => {
    expect(FITTED_CHANNEL_WAVELENGTHS_NM).toEqual(recorded.wavelengthsNm);
  });

  it.each([[[544, 645, 433]], [[645, 544]], [[645, Number.NaN, 433]]])(
    "is refused as a record of %j",
    (wavelengthsNm) => {
      expect(() => fittedTriple({ wavelengthsNm })).toThrow(/channels\.json records three/u);
    },
  );

  it("leaves R05's CHANNEL_WAVELENGTHS_NM at 680/550/440 until R08.T6.d", () => {
    expect(CHANNEL_WAVELENGTHS_NM).toEqual(BRUNETON_NM);
  });
});
