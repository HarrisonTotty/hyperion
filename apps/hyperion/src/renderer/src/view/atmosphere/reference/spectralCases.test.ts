/**
 * R08.T17's spectral cases (plan R08; `spectralCases.ts`): each `<name>.case.json` is the case the
 * client's optics write now in the 15 bins, through `bless.ts`, its counts `spectralSampleCounts`'s
 * from the three-channel reference, and at each bin its terms are the sources' own.
 *
 * @remarks
 * A changed case is rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/spectralCases.test.ts`, and its reference traced afresh
 * from the repository's root by {@link spectralTraceCommand}.
 */

import { describe, expect, it } from "vitest";

import {
  ADDED_FILE_LIMIT_BYTES,
  spectralCaseJson,
  traceCommand,
} from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import {
  aerosolExtinctionPerM,
  EARTH_COLUMN,
  EARTH_DRY_AIR,
  R06_SOLAR_BAKE_SPECTRUM,
} from "../earth";
import { BAKE_WAVELENGTHS_NM } from "../medium";
import { molecularMixture } from "../rayleigh";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthSpectralDayCase from "./earth-spectral-day.case.json" with { type: "json" };
import earthSpectralLowCase from "./earth-spectral-low.case.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import marsSpectralDayCase from "./mars-spectral-day.case.json" with { type: "json" };
import marsSpectralLowCase from "./mars-spectral-low.case.json" with { type: "json" };
import earthSpectralDayReference from "./earth-spectral-day.reference.json" with { type: "json" };
import earthSpectralLowReference from "./earth-spectral-low.reference.json" with { type: "json" };
import marsSpectralDayReference from "./mars-spectral-day.reference.json" with { type: "json" };
import marsSpectralLowReference from "./mars-spectral-low.reference.json" with { type: "json" };
import { EARTH_CASE, MARS_CASE } from "./cases";
import type { ReferenceFile } from "./references";
import {
  EARTH_SPECTRAL_DAY_CASE,
  earthAerosolAt,
  MARS_SPECTRAL_LOW_CASE,
  SPECTRAL_CASES,
  SPECTRAL_LOW_SUN_ZENITH_DEG,
  SPECTRAL_OMITTED_IMAGE,
  spectralSampleCounts,
  spectralSigmaTarget,
} from "./spectralCases";

/** The three-channel references the counts are set from, by their case's name. */
const BASE_REFERENCES: Readonly<Record<string, ReferenceFile>> = {
  earth: earthReference,
  mars: marsReference,
};

/** The committed case files, by case. */
const COMMITTED: Readonly<Record<string, unknown>> = {
  "earth-spectral-day": earthSpectralDayCase,
  "earth-spectral-low": earthSpectralLowCase,
  "mars-spectral-day": marsSpectralDayCase,
  "mars-spectral-low": marsSpectralLowCase,
};

/** The committed references, by case. */
const REFERENCES: Readonly<Record<string, ReferenceFile>> = {
  "earth-spectral-day": earthSpectralDayReference,
  "earth-spectral-low": earthSpectralLowReference,
  "mars-spectral-day": marsSpectralDayReference,
  "mars-spectral-low": marsSpectralLowReference,
};

/** The committed case files' texts, for their sizes. */
const TEXTS = import.meta.glob<string>("./*-spectral-*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/**
 * The command, from the repository's root, that traces a spectral case's reference: the
 * three-channel cases' command with a work directory, so that a stopped trace resumes.
 */
function spectralTraceCommand(name: string): string {
  return `${traceCommand(name)} --work target/atmosphere-work/${name}`;
}

const CASES = SPECTRAL_CASES.map((c) => [c.name, c] as const);

describe("the spectral cases", () => {
  it.each(CASES)("%s.case.json is the case the client's optics write now", async (name, c) => {
    const reference = BASE_REFERENCES[c.base.name];
    if (reference === undefined) {
      throw new Error(`no three-channel reference for ${c.base.name}`);
    }
    try {
      await expect(
        expectCommittedTable(
          spectralCaseJson(c, spectralSampleCounts(c, reference)),
          COMMITTED[name],
          `./${name}.case.json`,
        ),
      ).resolves.toBeUndefined();
    } catch (error: unknown) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`${reason}; then trace its reference: \`${spectralTraceCommand(name)}\``, {
        cause: error,
      });
    }
  });

  it.each(CASES)(
    "%s.reference.json is its case's, traced with --stokes at its seed and counts",
    (name, c) => {
      const reference = REFERENCES[name];
      const base = BASE_REFERENCES[c.base.name];
      if (reference === undefined || base === undefined) {
        throw new Error(`no reference for ${name}`);
      }
      const counts = spectralSampleCounts(c, base);
      expect(reference.case).toBe(name);
      expect(reference.seed).toBe(c.seed);
      expect(reference.polarisation).toBe("stokes");
      expect(reference.wavelengthsNm).toHaveLength(c.wavelengthsNm.length);
      for (const [i, nm] of reference.wavelengthsNm.entries()) {
        expect(nm).toBeCloseTo(c.wavelengthsNm[i] ?? Number.NaN, 9);
      }
      expect(reference.geometries.map((g) => [g.name, g.samples])).toEqual(
        c.geometries.map((g) => [g.name, counts.geometries[g.name]]),
      );
      expect(reference.aggregates.map((a) => [a.name, a.samples])).toEqual(
        c.aggregates.map((a) => [a.name, counts.aggregates[a.name]]),
      );
    },
  );

  it.each(Object.keys(TEXTS))("keeps %s under the added-file hook", (file) => {
    expect(new TextEncoder().encode(TEXTS[file]).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });

  it("traces the 15 bake bins under the reference Sun's spectrum", () => {
    for (const c of SPECTRAL_CASES) {
      expect(c.wavelengthsNm).toEqual(BAKE_WAVELENGTHS_NM);
      expect(c.sunIrradianceWPerM2Nm).toHaveLength(15);
      expect(c.spectrum).toEqual(R06_SOLAR_BAKE_SPECTRUM);
      // The Sun's spectral irradiance at 1 au near 500 nm is about 1.9 W m⁻² nm⁻¹ (ASTM E-490).
      expect(c.sunIrradianceWPerM2Nm[4]).toBeGreaterThan(1.7);
      expect(c.sunIrradianceWPerM2Nm[4]).toBeLessThan(2.1);
    }
  });

  it("traces every geometry but the twilight image once over its two parts", () => {
    for (const base of [EARTH_CASE, MARS_CASE]) {
      const parts = SPECTRAL_CASES.filter((c) => c.base === base);
      expect(parts.map((c) => c.part)).toEqual(["day", "low"]);
      const traced = parts.flatMap((c) => c.geometries.map((g) => g.name));
      expect(new Set(traced).size).toBe(traced.length);
      const kept = new Set(traced);
      const left = base.geometries.filter((g) => !kept.has(g.name));
      expect(left.length).toBeGreaterThan(0);
      expect(left.every((g) => g.image === SPECTRAL_OMITTED_IMAGE)).toBe(true);
      expect(parts.flatMap((c) => c.aggregates)).toEqual(base.aggregates);
      for (const c of parts) {
        const low = c.geometries.map((g) => g.sun.zenithDeg >= SPECTRAL_LOW_SUN_ZENITH_DEG);
        expect(low.every((l) => l === (c.part === "low"))).toBe(true);
      }
    }
  });

  it("takes each molecular term's own cross-section at each bin", () => {
    const rayleigh = EARTH_SPECTRAL_DAY_CASE.terms.find((t) => t.name === "rayleigh");
    if (rayleigh === undefined) {
      throw new Error("Earth has its molecular term");
    }
    const mixture = molecularMixture(EARTH_DRY_AIR);
    for (const [k, nm] of BAKE_WAVELENGTHS_NM.entries()) {
      expect(rayleigh.scatteringPerM[k]).toBe(
        EARTH_COLUMN.surfaceNumberDensityPerM3 * mixture.crossSectionM2(nm),
      );
    }
  });

  it("draws Earth's aerosol on R05's Ångström law, not the bake's held ends", () => {
    const aerosol = EARTH_SPECTRAL_DAY_CASE.terms.find((t) => t.name === "aerosol");
    if (aerosol === undefined) {
      throw new Error("Earth has its aerosol");
    }
    for (const [k, nm] of BAKE_WAVELENGTHS_NM.entries()) {
      const at = earthAerosolAt(nm);
      expect(aerosol.scatteringPerM[k]).toBe(at.scatteringPerM);
      expect((aerosol.scatteringPerM[k] ?? 0) + (aerosol.absorptionPerM[k] ?? 0)).toBeCloseTo(
        aerosolExtinctionPerM(nm),
        12,
      );
    }
  });

  it("gives Mars's dust its mode's table and matrix at every bin", () => {
    const dust = MARS_SPECTRAL_LOW_CASE.terms.find((t) => t.name === "dust");
    if (dust === undefined || dust.phase.kind !== "tabulated") {
      throw new Error("Mars's dust is tabulated");
    }
    expect(dust.phase.tables).toHaveLength(15);
    expect(dust.phase.tables.every((t) => t.matrix !== undefined)).toBe(true);
    for (const g of dust.phase.asymmetry) {
      expect(g).toBeGreaterThan(0.5);
      expect(g).toBeLessThan(0.9);
    }
  });

  it("sets each geometry's relative σ target by its sun's zenith angle", () => {
    expect(spectralSigmaTarget(30)).toBe(0.01);
    expect(spectralSigmaTarget(79.9)).toBe(0.01);
    expect(spectralSigmaTarget(80)).toBe(0.015);
    expect(spectralSigmaTarget(90)).toBe(0.02);
  });

  it("scales a count by 1.5 times the square of its σ over the target", () => {
    const g = EARTH_SPECTRAL_DAY_CASE.geometries.find((each) => each.name === "ground-s30-v0-a0");
    if (g === undefined) {
      throw new Error("Earth by day has its zenith view");
    }
    const reference: ReferenceFile = {
      ...earthReference,
      geometries: [
        {
          name: g.name,
          detectorHalfAngleDeg: 0.5,
          samples: 4096,
          quarterSamples: 1024,
          radiance: [100, 200, 400],
          standardError: [1, 4, 4],
          quarterStandardError: [2, 8, 8],
          sunOpticalDepth: [[0, 0, 0]],
        },
      ],
      aggregates: [],
    };
    // Green's 2% over the 1% target: 1.5 × 4,096 × 2² samples.
    const counts = spectralSampleCounts(
      { ...EARTH_SPECTRAL_DAY_CASE, geometries: [g], aggregates: [] },
      reference,
    );
    expect(counts.geometries[g.name]).toBe(24_576);
  });

  it("keeps every count of the committed cases in whole blocks of 1,024, at least 4,096", () => {
    for (const c of SPECTRAL_CASES) {
      const reference = BASE_REFERENCES[c.base.name];
      if (reference === undefined) {
        throw new Error(`no three-channel reference for ${c.base.name}`);
      }
      const counts = spectralSampleCounts(c, reference);
      const all = [...Object.values(counts.geometries), ...Object.values(counts.aggregates)];
      expect(all.every((n) => n >= 4096 && n % 1024 === 0)).toBe(true);
      expect(Object.keys(counts.geometries)).toHaveLength(c.geometries.length);
    }
  });
});
