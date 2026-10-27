import { describe, expect, it } from "vitest";

import { TITAN_FRACTIONS, titanClassMethaneMedium } from "../../../test/atmosphereFixtures";
import type { Rgb } from "../../photometry/toneCurve";
import { absorberTerm, thomsonTerm } from "../absorbers";
import { type AerosolModeSpec, aerosolTerm } from "../aerosol";
import { gaussLegendre } from "../irradianceQuadrature";
import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  type MediumTerm,
  phaseAt,
} from "../medium";
import { molecularTerm } from "../rayleigh";
import { TITAN_HAZE, titanClassColumn, VENUS_CLOUD_PARTICLES } from "./bakeFixtures";
import {
  phaseMoments,
  type SpectralSource,
  spectralColumnOf,
  spectralOptics,
  type SpectralTermOptics,
} from "./spectralColumn";

/** A channel's value of a colour, or NaN past its three. */
function channelOf(rgb: Rgb, c: number): number {
  return rgb[c] ?? Number.NaN;
}

/** A value's difference from another in units of `f64`'s last place, relatively; 0 for both 0. */
function ulps(value: number, expected: number): number {
  const scale = Math.max(Math.abs(expected), Math.abs(value));
  return scale === 0 ? 0 : Math.abs(value - expected) / (Number.EPSILON * scale);
}

/**
 * A phase as numbers to compare: its kind, then its values. Rayleigh's ρ; a table's entries and its
 * forward delta's share, in channel `c` of the drawn term's table or channel 0 of the column's.
 */
function phaseNumbers(
  phase: MediumTerm["phase"] | SpectralTermOptics["phase"],
  c: number,
): { readonly kind: string; readonly values: ReadonlyArray<number> } {
  let values: ReadonlyArray<number>;
  switch (phase.kind) {
    case "rayleigh":
      values = [
        typeof phase.depolarisation === "number"
          ? phase.depolarisation
          : channelOf(phase.depolarisation, c),
      ];
      break;
    case "tabulated":
      values = [...(phase.table.values[c] ?? []), channelOf(phase.table.forwardPeak, c)];
      break;
    case "cornette-shanks":
      values = [phase.asymmetry];
      break;
    case "none":
      values = [];
      break;
  }
  return { kind: phase.kind, values };
}

/** A mode of the test's: the Venus-class droplets, of τ(550) 2 from 1 bar to 0.1 bar. */
const DROPLETS: AerosolModeSpec = {
  name: "aerosol:H2SO4",
  particles: VENUS_CLOUD_PARTICLES,
  opticalDepth550: 2,
  basePa: 1e5,
  topPa: 1e4,
  mixingExponent: 0,
};

/** Mars dust on its non-spherical tables (`aerosol.test.ts`'s mode), a layer of τ(550) 0.5. */
const DUST: AerosolModeSpec = {
  name: "aerosol:mars_dust",
  particles: {
    kind: "distribution",
    mode: {
      material: "mars_dust",
      sizes: { kind: "gamma", effectiveRadiusUm: 0.8974, effectiveVariance: 0.3 },
    },
  },
  opticalDepth550: 0.5,
  basePa: 1.4e5,
  topPa: 5e4,
  mixingExponent: 0,
};

/**
 * A drawn medium on the Titan-class column and the source of each term: the molecular term, an
 * electron layer's Thomson term, its methane absorber, and three modes of three shapes.
 */
function drawnAndSources(): {
  readonly medium: AtmosphereMedium;
  readonly sources: ReadonlyArray<SpectralSource>;
} {
  const column = titanClassColumn();
  const top = column.topHeightM;
  const molecular = molecularTerm(column, TITAN_FRACTIONS).term;
  const electrons = {
    species: "e-",
    columnPerM2: 1e17,
    basePa: column.pressuresPa[0] ?? Number.NaN,
    topPa: 1e3,
  };
  const thomson = thomsonTerm(electrons, column);
  const methane = titanClassMethaneMedium().medium.terms.find((t) => t.absorber !== undefined);
  if (methane?.absorber?.kind !== "column") {
    throw new Error("the Titan-class medium has a methane absorber");
  }
  const absorber = absorberTerm(methane.absorber.spec, column);
  if (absorber.absorber?.kind !== "column") {
    throw new Error("methane is a column absorber");
  }
  const modes = [DROPLETS, DUST, TITAN_HAZE].map((spec) => ({
    spec,
    term: aerosolTerm(spec, column).term,
  }));
  const terms: MediumTerm[] = [molecular, thomson, absorber, ...modes.map((m) => m.term)];
  const sources: SpectralSource[] = [
    {
      kind: "molecular",
      fractions: TITAN_FRACTIONS,
      numberDensityPerM3: column.surfaceNumberDensityPerM3,
    },
    {
      kind: "thomson",
      electronDensityPerM3: electrons.columnPerM2 / columnLengthM(thomson.density, top),
    },
    {
      kind: "absorber",
      amount: { kind: "column", numberDensityPerM3: absorber.absorber.numberDensityPerM3 },
    },
    ...modes.map(({ spec, term }): SpectralSource => ({
      kind: "aerosol",
      particles: spec.particles,
      extinction550PerM: spec.opticalDepth550 / columnLengthM(term.density, top),
    })),
  ];
  return {
    medium: {
      name: "titan-class with modes",
      topHeightM: top,
      groundAlbedo: [0.2, 0.25, 0.3],
      terms,
    },
    sources,
  };
}

/** A value computed on its first use, so that a describe block's setup runs inside its tests. */
function once<T>(make: () => T): () => T {
  let value: { readonly made: T } | undefined;
  return () => {
    value ??= { made: make() };
    return value.made;
  };
}

/** The drawn medium, its sources, its column, and its optics `[channel][term]` at the channels. */
const fixture = once(() => {
  const { medium, sources } = drawnAndSources();
  const column = spectralColumnOf(medium, sources);
  const atChannels: ReadonlyArray<ReadonlyArray<SpectralTermOptics>> = spectralOptics(
    column,
    CHANNEL_WAVELENGTHS_NM,
  );
  return { medium, sources, column, atChannels };
});

describe("the spectral column", () => {
  // The modes' Mie, MMF and phase-file optics at the channels and 550 nm: 2.4 s with the module's
  // loading beside #107's sweep (load 4.5, 2026-10-26).
  it("carries the drawn medium's names, densities, top and ground", { timeout: 60_000 }, () => {
    const { medium, column } = fixture();
    expect(column.terms.map((t) => t.name)).toEqual(medium.terms.map((t) => t.name));
    for (const [t, term] of column.terms.entries()) {
      expect(term.density).toBe(medium.terms[t]?.density);
    }
    expect([column.topHeightM, column.groundAlbedo]).toEqual([
      medium.topHeightM,
      medium.groundAlbedo,
    ]);
  });

  it("gives at the channels' wavelengths each term's coefficients as drawn, to rounding", () => {
    const { medium, atChannels } = fixture();
    let worst = 0;
    for (const [c, terms] of atChannels.entries()) {
      for (const [t, optics] of terms.entries()) {
        const drawn = medium.terms[t];
        worst = Math.max(
          worst,
          ulps(optics.scatteringPerM, channelOf(drawn?.scattering ?? [0, 0, 0], c)),
          ulps(optics.absorptionPerM, channelOf(drawn?.absorption ?? [0, 0, 0], c)),
        );
      }
    }
    expect(worst).toBeLessThanOrEqual(4);
  });

  it("gives at the channels' wavelengths each term's phase as drawn: Rayleigh's ρ, and each mode's table and forward delta", () => {
    const { medium, atChannels } = fixture();
    let worst = 0;
    const kinds: string[] = [];
    const expectedKinds: string[] = [];
    for (const [c, terms] of atChannels.entries()) {
      for (const [t, optics] of terms.entries()) {
        const drawn = medium.terms[t];
        if (drawn === undefined) {
          throw new Error(`no term ${t}`);
        }
        // The column's table is its own channel 0 at every wavelength.
        const own = phaseNumbers(optics.phase, 0);
        const expected = phaseNumbers(drawn.phase, c);
        kinds.push(own.kind);
        expectedKinds.push(expected.kind);
        expect(own.values.length).toBe(expected.values.length);
        for (const [i, value] of own.values.entries()) {
          worst = Math.max(worst, ulps(value, expected.values[i] ?? Number.NaN));
        }
      }
    }
    expect(kinds).toEqual(expectedKinds);
    expect(worst).toBeLessThanOrEqual(4);
  });

  it("takes Rayleigh's moments in closed form, χ₂ = (1 − ρ) ÷ (5(2 + ρ)), the dipole phase's own to 10⁻¹⁴", () => {
    const rule = gaussLegendre(8);
    for (const rho of [0, 0.0279, 0.2]) {
      const moments = phaseMoments({ kind: "rayleigh", depolarisation: rho }, 4);
      const legendre = [
        () => 1,
        (mu: number) => mu,
        (mu: number) => (3 * mu * mu - 1) / 2,
        (mu: number) => (5 * mu ** 3 - 3 * mu) / 2,
        (mu: number) => (35 * mu ** 4 - 30 * mu * mu + 3) / 8,
      ];
      for (const [l, p] of legendre.entries()) {
        let integral = 0;
        for (const [i, t] of rule.nodes.entries()) {
          const mu = 2 * t - 1;
          integral +=
            2 *
            (rule.weights[i] ?? Number.NaN) *
            2 *
            Math.PI *
            phaseAt({ kind: "rayleigh", depolarisation: [rho, rho, rho] }, mu)[0] *
            p(mu);
        }
        expect(Math.abs((moments[l] ?? Number.NaN) - integral)).toBeLessThan(1e-14);
      }
    }
  });

  it("refuses sources that do not match the terms", () => {
    const { medium, sources } = fixture();
    expect(() => spectralColumnOf(medium, sources.slice(1))).toThrow(RangeError);
    const [first, second, third, ...rest] = sources;
    if (first === undefined || second === undefined || third === undefined) {
      throw new Error("the medium has three terms or more");
    }
    // The absorber's source on the molecular term, and the molecular source on the absorber.
    expect(() => spectralColumnOf(medium, [third, second, first, ...rest])).toThrow(RangeError);
  });
});
