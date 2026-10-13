import { beforeAll, describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { expectCommittedTable } from "../../../test/bless";
import {
  EARTH_H2O_COLUMN_PER_M2,
  EARTH_O2_FRACTION,
  earthLineColumn,
  earthLineLayers,
  earthLinePaths,
  PATH_HEIGHTS_M,
} from "../../../test/earthLinePaths";
import type { Rgb } from "../../photometry/toneCurve";
import {
  ABSORBER_CROSS_SECTIONS,
  ABSORBER_PAIR_CROSS_SECTIONS,
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberCurve,
  absorberCurve,
  type AbsorberLayer,
  absorberOptics,
  type AbsorberSpec,
  absorberSpecs,
  absorberTransmittance,
  pairCrossSectionAt,
  type PairCrossSectionTable,
} from "../absorbers";
import { type BakeSpectrum, bakeSpectrumOf, deltaUv } from "../spectralColour";
import recordedReference from "./earthLineReference.json" with { type: "json" };
import recordedPaths from "./earthLinePaths.json" with { type: "json" };
import { loadKDistributions } from "./kDistribution";
import { inlinePart } from "../../../test/kDistributionFixtures";

/** A number to 7 significant figures, as the committed paths hold them. */
function seven(value: number): number {
  return Number(value.toPrecision(7));
}

/** The Earth paths as the committed file holds them. */
function pathsTable(): unknown {
  const column = earthLineColumn();
  return {
    description:
      "Earth's slant paths for R08.T4.d's end-to-end test (decision-r08-t4d-kdist.md, its tests), " +
      "written by apps/hyperion/src/renderer/src/view/atmosphere/absorbers/earthLines.test.ts " +
      "under a bless: the US Standard Atmosphere 1976's troposphere to its tropopause on Design note " +
      "3's column, O2 well mixed and AFGL's 1.42 g cm-2 of water to 300 hPa, in 80 layers equal in " +
      "ln p from each start height to 1 Pa. Each layer: its O2-weighted pressure (Pa) and " +
      "temperature (K), O2's and H2O's mole fractions, and each path's O2 and H2O columns (m-2) and " +
      "H2O's continuum pair columns (m-5), in zenithDeg's order. src/tools/lineReferences.ts reads " +
      "it to take the paths line by line.",
    o2Fraction: EARTH_O2_FRACTION,
    h2oColumnPerM2: seven(EARTH_H2O_COLUMN_PER_M2),
    starts: PATH_HEIGHTS_M.map((h) => {
      const paths = earthLinePaths(column, h);
      return {
        startHeightM: paths.startHeightM,
        zenithDeg: paths.zenithDeg.map(seven),
        airMasses: paths.airMasses.map(seven),
        layers: paths.layers.map((l) => ({
          pressurePa: seven(l.pressurePa),
          temperatureK: seven(l.temperatureK),
          o2Fraction: seven(l.o2Fraction),
          h2oFraction: seven(l.h2oFraction),
          o2PerM2: l.o2PerM2.map(seven),
          h2oPerM2: l.h2oPerM2.map(seven),
          h2oSelfPairPerM5: l.h2oSelfPairPerM5.map(seven),
          h2oAirPairPerM5: l.h2oAirPairPerM5.map(seven),
        })),
      };
    }),
  };
}

describe("Earth's line paths", () => {
  it("are written to absorbers/earthLinePaths.json under a bless, and checked unchanged otherwise", async () => {
    await expect(
      expectCommittedTable(pathsTable(), recordedPaths, "./earthLinePaths.json"),
    ).resolves.toBeUndefined();
  });

  it("run from 1 air mass to the horizon's 35, refraction-free on a spherical ray", () => {
    const ground = earthLinePaths(earthLineColumn(), 0);
    expect(ground.airMasses[0]).toBeCloseTo(1, 6);
    expect(ground.airMasses[3]).toBeCloseTo(3, 1);
    // Kasten and Young 1989's 38.09 at 90° includes refraction, which lengthens the grazing path.
    expect(ground.airMasses.at(-1)).toBeGreaterThan(34);
    expect(ground.airMasses.at(-1)).toBeLessThan(38.09);
  });

  it("hold the water column through each layer, the vertical path's summing to AFGL's 1.42 g cm⁻²", () => {
    const ground = earthLinePaths(earthLineColumn(), 0);
    const vertical = ground.layers.reduce((sum, l) => sum + (l.h2oPerM2[0] ?? 0), 0);
    expect(vertical / EARTH_H2O_COLUMN_PER_M2).toBeCloseTo(1, 2);
  });
});

/** One start height's line-by-line cells, as `earthLineReference.json` holds them. */
interface ReferenceStart {
  readonly startHeightM: number;
  readonly cells: ReadonlyArray<{
    readonly nm: number;
    readonly t: ReadonlyArray<number>;
    readonly h: ReadonlyArray<number>;
  }>;
}

/** The paths' file as committed. */
interface PathsFile {
  readonly starts: ReadonlyArray<{
    readonly startHeightM: number;
    readonly layers: ReadonlyArray<{
      readonly pressurePa: number;
      readonly temperatureK: number;
      readonly o2PerM2: ReadonlyArray<number>;
      readonly h2oPerM2: ReadonlyArray<number>;
      readonly h2oSelfPairPerM5: ReadonlyArray<number>;
      readonly h2oAirPairPerM5: ReadonlyArray<number>;
    }>;
  }>;
}

const PATHS: PathsFile = recordedPaths;
const REFERENCE: { readonly starts: ReadonlyArray<ReferenceStart> } = recordedReference;

/** The Sun: R06's `solar_colour()`, the 5,772 K main-sequence row of `bake_spectra.json`. */
const THE_SUN: BakeSpectrum = (() => {
  const row = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
  if (row === undefined) {
    throw new Error("bake_spectra.json holds no 5,772 K main-sequence sun");
  }
  return bakeSpectrumOf(row.bake_spectrum);
})();

/** Dry air's gases the continuum's foreign part takes as partners (U.S. Standard Atmosphere, 1976, Table 3). */
const AIR: ReadonlyMap<string, number> = new Map([
  ["N2", 0.780_84],
  ["O2", EARTH_O2_FRACTION],
  ["Ar", 0.009_34],
]);

/** A spec of the given name among a body's, or a test failure. */
function specNamed(specs: ReadonlyArray<AbsorberSpec>, name: string): AbsorberSpec {
  const spec = specs.find((s) => s.name === name);
  if (spec === undefined) {
    throw new Error(`no ${name} spec was formed`);
  }
  return spec;
}

/** The channel and colour errors of the client's chain against line by line on Earth's paths. */
interface PathErrors {
  readonly channel: number;
  readonly colour: number;
  /** The same, the line-by-line path taken homogeneous at its Curtis–Godson pressure. */
  readonly curtisGodsonChannel: number;
}

/** Earth's line curves under the Sun, and its continuum's pair spec. */
interface EarthCurves {
  readonly o2: AbsorberCurve;
  readonly h2o: AbsorberCurve;
  readonly pair: AbsorberCurve;
  readonly pairSpec: AbsorberSpec;
}

/** The read's τ between rows along u, as {@link readFalls} finds it. */
interface ReadFalls {
  /** The largest fall of τ from one column to the next, at fixed p̄. */
  readonly largestFall: number;
  /** Reads whose τ lies outside [0, −ln 10⁻⁶]. */
  readonly outside: number;
}

/**
 * Each line curve read at its rows and midway between them in ln p̄, at each column node and
 * midway between nodes in ln u, as the kernels and the twin read it ({@link absorberTransmittance}).
 */
function readFalls(curves: ReadonlyArray<AbsorberCurve>): ReadFalls {
  const ceiling = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);
  let largestFall = 0;
  let outside = 0;
  for (const curve of curves) {
    const lnP = curve.logPressures;
    const lnU = curve.logColumns;
    for (let r = 0; r + 1 < lnP.length; r += 1) {
      for (const f of [0, 0.5]) {
        const p = Math.exp((lnP[r] ?? Number.NaN) * (1 - f) + (lnP[r + 1] ?? Number.NaN) * f);
        let previous: Rgb | undefined;
        for (let j = 0; j + 1 < 2 * lnU.length; j += 1) {
          const k = Math.floor(j / 2);
          const x =
            j % 2 === 0
              ? (lnU[k] ?? Number.NaN)
              : 0.5 * ((lnU[k] ?? Number.NaN) + (lnU[k + 1] ?? Number.NaN));
          const t = absorberTransmittance(curve, { kind: "column", columnPerM2: Math.exp(x) }, p);
          const tau: Rgb = [-Math.log(t[0]), -Math.log(t[1]), -Math.log(t[2])];
          for (const [c, v] of tau.entries()) {
            if (!(v >= 0 && v <= ceiling + 1e-12)) {
              outside += 1;
            }
            largestFall = Math.max(largestFall, (previous?.[c] ?? v) - v);
          }
          previous = tau;
        }
      }
    }
  }
  return { largestFall, outside };
}

/** Earth's line curves under the Sun, from the committed tables on Earth's column. */
async function earthCurves(): Promise<EarthCurves> {
  const column = earthLineColumn();
  const tables = await loadKDistributions(["O2", "H2O"], inlinePart);
  const airColumn = (earthLineLayers(column)[0]?.columnPerM2 ?? Number.NaN) / EARTH_O2_FRACTION;
  const layers: AbsorberLayer[] = [
    ...earthLineLayers(column),
    { species: "N2", columnPerM2: 0.780_84 * airColumn, basePa: 101_325, topPa: 0 },
    { species: "Ar", columnPerM2: 0.009_34 * airColumn, basePa: 101_325, topPa: 0 },
  ];
  const { specs } = absorberSpecs(
    layers,
    column,
    ABSORBER_CROSS_SECTIONS,
    ABSORBER_PAIR_CROSS_SECTIONS,
    {
      kDistributions: tables,
      bulkMoleFractions: AIR,
    },
  );
  // Ar, listed for the air partner's weights, has no optics and shares O₂'s profile.
  const o2 = absorberCurve(specNamed(specs, "absorber:O2+Ar"), THE_SUN);
  const h2o = absorberCurve(specNamed(specs, "absorber:H2O"), THE_SUN);
  const pairSpec = specNamed(specs, "pair:H2O+H2O,H2O+air");
  return { o2, h2o, pair: absorberCurve(pairSpec, THE_SUN), pairSpec };
}

/** The chain's errors on every path of both start heights. */
function pathErrors({ o2, h2o, pair, pairSpec }: EarthCurves): ReadonlyArray<PathErrors> {
  const optics = absorberOptics(pairSpec, THE_SUN);
  const sunRgb = optics.weights.map((w) => w.reduce((a, b) => a + b, 0));
  const out: PathErrors[] = [];
  for (const start of PATHS.starts) {
    const reference = REFERENCE.starts.find((r) => r.startHeightM === start.startHeightM);
    if (reference === undefined) {
      throw new Error(`no line-by-line reference from ${start.startHeightM} m`);
    }
    // Each layer's continuum σ at its own temperature, on the pairs' 1 nm grid from 380 nm.
    const continuum = start.layers.map((l) => [
      pairCrossSectionAt("H2O+H2O", l.temperatureK)?.crossSectionsM5 ?? new Float64Array(0),
      pairCrossSectionAt("H2O+air", l.temperatureK)?.crossSectionsM5 ?? new Float64Array(0),
    ]);
    const paths = start.layers[0]?.o2PerM2.length ?? 0;
    for (let p = 0; p < paths; p += 1) {
      const sum = (of: (l: PathsFile["starts"][number]["layers"][number]) => number): number =>
        start.layers.reduce((s, l) => s + of(l), 0);
      const uO2 = sum((l) => l.o2PerM2[p] ?? 0);
      const uH2O = sum((l) => l.h2oPerM2[p] ?? 0);
      const pO2 = sum((l) => (l.o2PerM2[p] ?? 0) * l.pressurePa) / uO2;
      const pH2O = sum((l) => (l.h2oPerM2[p] ?? 0) * l.pressurePa) / uH2O;
      const pairs = sum((l) => (l.h2oSelfPairPerM5[p] ?? 0) + (l.h2oAirPairPerM5[p] ?? 0));
      const tO2 = absorberTransmittance(o2, { kind: "column", columnPerM2: uO2 }, pO2);
      const tH2O = absorberTransmittance(h2o, { kind: "column", columnPerM2: uH2O }, pH2O);
      const tPair = absorberTransmittance(
        pair,
        { kind: "pair", pairColumnPerM5: pairs },
        undefined,
      );
      const client: Rgb = [
        tO2[0] * tH2O[0] * tPair[0],
        tO2[1] * tH2O[1] * tPair[1],
        tO2[2] * tH2O[2] * tPair[2],
      ];
      const channels = (which: "t" | "h"): Rgb => {
        const passed = [0, 0, 0];
        for (const [i, nm] of optics.wavelengthsNm.entries()) {
          const cell = Math.round(nm);
          const lines = reference.cells.find((c) => c.nm === cell)?.[which][p] ?? 1;
          const row = Math.round(nm - 380);
          let tau = 0;
          for (const [k, l] of start.layers.entries()) {
            const [self, air] = continuum[k] ?? [];
            tau +=
              (self?.[row] ?? 0) * (l.h2oSelfPairPerM5[p] ?? 0) +
              (air?.[row] ?? 0) * (l.h2oAirPairPerM5[p] ?? 0);
          }
          for (const [c, weights] of optics.weights.entries()) {
            passed[c] = (passed[c] ?? 0) + (weights[i] ?? 0) * lines * Math.exp(-tau);
          }
        }
        const t = (c: 0 | 1 | 2): number =>
          Math.min(Math.max((passed[c] ?? 0) / (sunRgb[c] ?? 1), 1e-6), 1);
        return [t(0), t(1), t(2)];
      };
      const exact = channels("t");
      const homogeneous = channels("h");
      const light = (t: Rgb): Rgb => [
        t[0] * (sunRgb[0] ?? 0),
        t[1] * (sunRgb[1] ?? 0),
        t[2] * (sunRgb[2] ?? 0),
      ];
      out.push({
        channel: Math.max(...client.map((t, c) => Math.abs(t - (exact[c] ?? Number.NaN)))),
        colour: deltaUv(light(client), light(exact)),
        curtisGodsonChannel: Math.max(
          ...homogeneous.map((t, c) => Math.abs(t - (exact[c] ?? Number.NaN))),
        ),
      });
    }
  }
  return out;
}

describe("Earth's O₂ and H₂O against line by line", () => {
  let errors: ReadonlyArray<PathErrors> = [];
  let falls: ReadFalls = { largestFall: Number.NaN, outside: Number.NaN };
  beforeAll(async () => {
    const curves = await earthCurves();
    errors = pathErrors(curves);
    falls = readFalls([curves.o2, curves.h2o]);
  });

  it("draw each channel within 2 × 10⁻³ through 1–35 air masses, from the ground and from 3 km", () => {
    expect(errors).toHaveLength(16);
    expect(Math.max(...errors.map((e) => e.channel))).toBeLessThanOrEqual(2e-3);
  });

  it("draw the transmitted sunlight's colour within Δu′v′ 0.0005", () => {
    expect(Math.max(...errors.map((e) => e.colour))).toBeLessThanOrEqual(5e-4);
  });

  it("record the Curtis–Godson error on the slant paths, under the chain's tier", () => {
    // Recorded in R08's Risks, "Deviations in T4.d, as built": the line-by-line path taken
    // homogeneous at its column-weighted pressure and temperature, against the layered path.
    expect(Math.max(...errors.map((e) => e.curtisGodsonChannel))).toBeLessThanOrEqual(2e-3);
  });

  it("read τ in [0, −ln 10⁻⁶] at their rows and between them", () => {
    expect(falls.outside).toBe(0);
  });

  it("record the read's largest fall of τ along u between rows, under the chain's tier", () => {
    // Recorded in R08's Risks, "Deviations in T4.d, as built". The cubic in ln p̄ is unclamped
    // between rows, so τ at a fixed p̄ may fall a little as u grows; a fall δ raises T by at most
    // T δ ≤ δ, within the chain's 2 × 10⁻³ while δ is.
    expect(falls.largestFall).toBeLessThanOrEqual(2e-3);
  });
});

describe("an Earth-like body's line absorbers", () => {
  /** Earth's specs in real air, its 0.93% Ar listed, from the given pair tables. */
  async function earthSpecs(
    pairTables: ReadonlyArray<PairCrossSectionTable>,
  ): Promise<ReturnType<typeof absorberSpecs>> {
    const column = earthLineColumn();
    const tables = await loadKDistributions(["O2", "H2O"], inlinePart);
    const airColumn = (earthLineLayers(column)[0]?.columnPerM2 ?? Number.NaN) / EARTH_O2_FRACTION;
    const layers: AbsorberLayer[] = [
      ...earthLineLayers(column),
      { species: "N2", columnPerM2: 0.780_84 * airColumn, basePa: 101_325, topPa: 0 },
      { species: "Ar", columnPerM2: 0.009_34 * airColumn, basePa: 101_325, topPa: 0 },
    ];
    return absorberSpecs(layers, column, ABSORBER_CROSS_SECTIONS, pairTables, {
      kDistributions: tables,
      bulkMoleFractions: AIR,
    });
  }

  it("draw O₂, H₂O's lines and H₂O's continuum, reporting only the continuum's estimate below 567 nm", async () => {
    const { specs, approximations } = await earthSpecs(ABSORBER_PAIR_CROSS_SECTIONS);
    // A pair profile comes after the later listed of its members, merged pairs at the earliest.
    expect(specs.map((s) => s.name)).toEqual([
      "absorber:O2+Ar",
      "pair:O2+O2,O2+N2",
      "absorber:H2O",
      "pair:H2O+H2O,H2O+air",
    ]);
    // Ar's own report, a gas with no cross-sections the client does not yet state transparent, is
    // the transparency list's (`absorbers/transparency.ts`), not the lines'.
    expect(approximations).toEqual([
      { species: "Ar", provenance: "none", reason: "noCrossSections" },
      { species: "H2O+H2O", provenance: "estimated", reason: "estimatedOptics" },
      { species: "H2O+air", provenance: "estimated", reason: "estimatedOptics" },
    ]);
  });

  it("clear the continuum's label from 567 nm in real air, its 0.93% Ar taken as air", async () => {
    // The pair tables as they stand from 567 nm, where CKD's own values are measured: without the
    // estimate below it, the continuum is not approximate (decision-r08-t4d-kdist-chain.md §15),
    // and Ar's own report alone remains.
    const fromCkd = ABSORBER_PAIR_CROSS_SECTIONS.map((table) => {
      const { estimatedBelowNm, ...rest } = table;
      void estimatedBelowNm;
      return rest;
    });
    expect((await earthSpecs(fromCkd)).approximations).toEqual([
      { species: "Ar", provenance: "none", reason: "noCrossSections" },
    ]);
  });
});
