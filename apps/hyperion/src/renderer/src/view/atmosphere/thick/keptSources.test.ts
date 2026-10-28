/**
 * The kept scenes' sources (plan R08, R08.T14.c; `decision-r08-t13c2-classes.md` §6): each kept
 * medium's terms by their sources give the drawn terms at the render channels to the bit, and
 * `TEST EARTH`'s Cornette–Shanks aerosol, which bakes from its channel optics, takes the ground
 * rule's blend in every bake bin.
 */
import { describe, expect, it } from "vitest";

import { sourceMisses } from "../../../test/sourceChecks";
import { MARS_AIR, MARS_DUST_OPTICAL_DEPTH_550, MARS_FIT_COLUMN } from "../channels";
import {
  EARTH_COLUMN,
  EARTH_DRY_AIR,
  EARTH_REFERENCE,
  EARTH_REFERENCE_SOURCES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_SOURCES,
} from "../earth";
import { BAKE_WAVELENGTHS_NM, columnLengthM, type MediumTerm } from "../medium";
import {
  EARTH_CASE,
  EARTH_OZONE_SLAB_CASE,
  MARS_CASE,
  MARS_REFERENCE,
  MARS_REFERENCE_SOURCES,
  REFERENCE_CASES,
  SATURN_OBLATE_AIR,
  SATURN_OBLATE_CASE,
  SATURN_OBLATE_COLUMN,
  SATURN_OBLATE_REFERENCE_SOURCES,
} from "../reference/cases";
import { binGroundAlbedo } from "./bake";
import {
  absorberSourceOf,
  CHANNEL_SOURCE_MOMENT_ORDER,
  channelSource,
  cornetteShanksMoments,
  type SpectralSource,
  spectralColumnOf,
  spectralOptics,
  type SpectralTermOptics,
} from "./spectralColumn";

/** A medium's term at an index, or a test failure. */
function termAt(terms: ReadonlyArray<MediumTerm>, i: number): MediumTerm {
  const term = terms[i];
  if (term === undefined) {
    throw new Error(`the medium has no term ${i}`);
  }
  return term;
}

/** A case's sources, or a test failure for a case with none. */
function sourcesOf(referenceCase: (typeof REFERENCE_CASES)[number]): ReadonlyArray<SpectralSource> {
  const { sources } = referenceCase;
  if (sources === undefined) {
    throw new Error(`case ${referenceCase.name} has no sources`);
  }
  return sources;
}

const CASES = REFERENCE_CASES.map((c) => [c.name, c] as const);

// Mars's dust's TAMUdust optics at the channels and 550 nm, the others' closed forms.
const SOURCES_TIMEOUT_MS = 60_000;

describe("the kept media's sources", () => {
  it("are each case's exported constants, one a term", () => {
    expect([
      EARTH_CASE.sources === EARTH_REFERENCE_SOURCES,
      EARTH_OZONE_SLAB_CASE.sources === EARTH_SLAB_REFERENCE_SOURCES,
      MARS_CASE.sources === MARS_REFERENCE_SOURCES,
      SATURN_OBLATE_CASE.sources === SATURN_OBLATE_REFERENCE_SOURCES,
    ]).toEqual([true, true, true, true]);
    expect(REFERENCE_CASES.map((c) => [c.name, c.sources?.length])).toEqual(
      REFERENCE_CASES.map((c) => [c.name, c.medium.terms.length]),
    );
  });

  it.each(CASES)(
    "give %s's every term's drawn coefficients and phase at the render channels to the bit",
    { timeout: SOURCES_TIMEOUT_MS },
    (_, referenceCase) => {
      expect(sourceMisses(referenceCase.medium, sourcesOf(referenceCase))).toEqual([]);
    },
  );

  it("take the molecular terms' gases and their columns' n_s as `molecularTerm` takes them", () => {
    const molecular = [
      EARTH_REFERENCE_SOURCES[0],
      MARS_REFERENCE_SOURCES[0],
      SATURN_OBLATE_REFERENCE_SOURCES[0],
    ];
    expect(molecular).toEqual([
      {
        kind: "molecular",
        fractions: EARTH_DRY_AIR,
        numberDensityPerM3: EARTH_COLUMN.surfaceNumberDensityPerM3,
      },
      {
        kind: "molecular",
        fractions: MARS_AIR,
        numberDensityPerM3: MARS_FIT_COLUMN.surfaceNumberDensityPerM3,
      },
      {
        kind: "molecular",
        fractions: SATURN_OBLATE_AIR,
        numberDensityPerM3: SATURN_OBLATE_COLUMN.surfaceNumberDensityPerM3,
      },
    ]);
  });

  it("bake TEST EARTH's aerosol from its channel optics, and its ozone from its amount", () => {
    for (const [medium, sources] of [
      [EARTH_REFERENCE, EARTH_REFERENCE_SOURCES],
      [EARTH_SLAB_REFERENCE, EARTH_SLAB_REFERENCE_SOURCES],
    ] as const) {
      expect(sources[1]).toEqual(channelSource(termAt(medium.terms, 1)));
      expect(sources[2]).toEqual(absorberSourceOf(termAt(medium.terms, 2)));
    }
  });

  it("bake TEST MARS's dust on its own particles, σ_ext(550) as `aerosolTerm` formed it", () => {
    const dust = termAt(MARS_REFERENCE.terms, 1);
    const source = MARS_REFERENCE_SOURCES[1];
    if (source?.kind !== "aerosol") {
      throw new Error("Mars's dust has an aerosol source");
    }
    expect([source.particles.kind, source.extinction550PerM]).toEqual([
      "distribution",
      MARS_DUST_OPTICAL_DEPTH_550 / columnLengthM(dust.density, MARS_FIT_COLUMN.topHeightM),
    ]);
  });
});

/** TEST EARTH's aerosol's optics in each bake bin. */
function aerosolInBins(): ReadonlyArray<SpectralTermOptics | undefined> {
  return spectralOptics(
    spectralColumnOf(EARTH_REFERENCE, EARTH_REFERENCE_SOURCES),
    BAKE_WAVELENGTHS_NM,
  ).map((terms) => terms[1]);
}

describe("TEST EARTH's aerosol across the bake bins", () => {
  const aerosol = termAt(EARTH_REFERENCE.terms, 1);

  it("takes in every bin the ground rule's coefficients, `binGroundAlbedo`'s own to the bit", () => {
    expect(aerosolInBins().map((o) => [o?.scatteringPerM, o?.absorptionPerM])).toEqual(
      BAKE_WAVELENGTHS_NM.map((nm) => [
        binGroundAlbedo(aerosol.scattering, nm),
        binGroundAlbedo(aerosol.absorption, nm),
      ]),
    );
  });

  it("takes in every bin Cornette and Shanks's moments, the same in every channel, with no forward delta", () => {
    if (aerosol.phase.kind !== "cornette-shanks") {
      throw new Error("TEST EARTH's aerosol is Cornette–Shanks's");
    }
    const closed = cornetteShanksMoments(aerosol.phase.asymmetry, CHANNEL_SOURCE_MOMENT_ORDER);
    let worst = 0;
    for (const o of aerosolInBins()) {
      if (o?.phase.kind !== "moments" || o.phase.forwardDelta !== 0) {
        throw new Error("the aerosol's phase in a bin is its moments, with no forward delta");
      }
      for (const [l, chi] of o.phase.moments.entries()) {
        worst = Math.max(worst, Math.abs(chi - (closed[l] ?? Number.NaN)));
      }
    }
    // A blend of one channel's moments with the same moments is exact.
    expect(worst).toBe(0);
  });
});
