import { describe, expect, it } from "vitest";

import { TITAN_FRACTIONS, titanClassMethaneMedium } from "../../../test/atmosphereFixtures";
import type { Rgb } from "../../photometry/toneCurve";
import { absorberTerm, thomsonTerm } from "../absorbers";
import { type AerosolModeSpec, aerosolTerm, deltaMTruncation } from "../aerosol";
import { gaussLegendre } from "../irradianceQuadrature";
import {
  type AtmosphereMedium,
  BAKE_WAVELENGTHS_NM,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  type MediumTerm,
  phaseAt,
} from "../medium";
import { molecularTerm } from "../rayleigh";
import { binGroundAlbedo } from "./bake";
import { TITAN_HAZE, titanClassColumn, VENUS_CLOUD_PARTICLES } from "../../../test/bakeFixtures";
import { cornetteShanksMeanCosine } from "./regime";
import {
  absorberSourceOf,
  CHANNEL_SOURCE_MOMENT_ORDER,
  channelBlendAt,
  channelSource,
  cornetteShanksMoments,
  forwardDeltaShare,
  phaseMoments,
  type SpectralPhase,
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
    case "moments":
      values = [...phase.moments, phase.forwardDelta];
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

/** P_0 … P_order at μ by the three-term recurrence. */
function legendreSeries(order: number, mu: number): Float64Array {
  const p = new Float64Array(order + 1);
  p[0] = 1;
  if (order >= 1) {
    p[1] = mu;
  }
  for (let l = 1; l < order; l += 1) {
    p[l + 1] = ((2 * l + 1) * mu * (p[l] ?? Number.NaN) - l * (p[l - 1] ?? Number.NaN)) / (l + 1);
  }
  return p;
}

describe("Cornette and Shanks's moments", () => {
  it("are a 400-node Gauss–Legendre quadrature of phaseAt's phase to 10⁻¹⁰ for l ≤ 32 at g 0, 0.3, 0.584 and 0.85", () => {
    const order = 32;
    const rule = gaussLegendre(400);
    let worst = 0;
    for (const g of [0, 0.3, 0.584, 0.85]) {
      const moments = cornetteShanksMoments(g, order);
      const integrals = new Float64Array(order + 1);
      for (const [i, t] of rule.nodes.entries()) {
        const mu = 2 * t - 1;
        const weight =
          2 *
          (rule.weights[i] ?? Number.NaN) *
          2 *
          Math.PI *
          phaseAt({ kind: "cornette-shanks", asymmetry: g }, mu)[0];
        for (const [l, p] of legendreSeries(order, mu).entries()) {
          integrals[l] = (integrals[l] ?? 0) + weight * p;
        }
      }
      for (const [l, integral] of integrals.entries()) {
        worst = Math.max(worst, Math.abs((moments[l] ?? Number.NaN) - integral));
      }
    }
    // Measured 6.9 × 10⁻¹⁵ (2026-10-27).
    expect(worst).toBeLessThan(1e-10);
  });

  it("take χ₀ as 1 and χ₁ as the phase's mean cosine, and at g = 0 Rayleigh's χ₂ of 1 ÷ 10 alone", () => {
    const misses = [0.3, 0.584, 0.85, -0.4].map((g) => {
      const moments = cornetteShanksMoments(g, 4);
      return [moments[0], ulps(moments[1] ?? Number.NaN, cornetteShanksMeanCosine(g)) <= 2];
    });
    expect(misses).toEqual([
      [1, true],
      [1, true],
      [1, true],
      [1, true],
    ]);
    const isotropicShape = cornetteShanksMoments(0, 8);
    expect(Math.abs((isotropicShape[2] ?? Number.NaN) - 0.1)).toBeLessThanOrEqual(2e-17);
    expect([...isotropicShape].filter((_, l) => l !== 2)).toEqual([1, 0, 0, 0, 0, 0, 0, 0]);
  });

  it("refuses a g outside (−1, 1) and an order that is not whole", () => {
    expect(() => cornetteShanksMoments(1, 4)).toThrow(RangeError);
    expect(() => cornetteShanksMoments(Number.NaN, 4)).toThrow(RangeError);
    expect(() => cornetteShanksMoments(0.5, 2.5)).toThrow(RangeError);
  });
});

describe("a phase known by its moments", () => {
  const moments = cornetteShanksMoments(0.584, CHANNEL_SOURCE_MOMENT_ORDER);
  const phase: SpectralPhase = { kind: "moments", moments, forwardDelta: 0.25 };

  it("gives its first moments to any even order up to its own, and its forward delta", () => {
    expect([...phaseMoments(phase, 16)]).toEqual([...moments.subarray(0, 17)]);
    expect(phaseMoments(phase, CHANNEL_SOURCE_MOMENT_ORDER)).toEqual(moments);
    expect(forwardDeltaShare(phase)).toBe(0.25);
  });

  it("refuses an order past its own", () => {
    expect(() => phaseMoments(phase, CHANNEL_SOURCE_MOMENT_ORDER + 2)).toThrow(RangeError);
  });
});

/** A term of a medium by name, or a test failure. */
function termNamed(medium: AtmosphereMedium, name: string): MediumTerm {
  const term = medium.terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`the medium has no term ${name}`);
  }
  return term;
}

/** A phase's moments, or a test failure for a phase of another kind. */
function momentsOf(phase: SpectralPhase): Extract<SpectralPhase, { kind: "moments" }> {
  if (phase.kind !== "moments") {
    throw new Error(`a ${phase.kind} phase, not one of moments`);
  }
  return phase;
}

/** A phase's table, or a test failure for a phase of another kind. */
function tableOf(phase: SpectralPhase): Extract<SpectralPhase, { kind: "tabulated" }>["table"] {
  if (phase.kind !== "tabulated") {
    throw new Error(`a ${phase.kind} phase, not a table`);
  }
  return phase.table;
}

/**
 * The channel source's test wavelengths, nm: the three channels' (645, 544, 433.625), one beyond
 * each end of their range, and one between each neighbouring pair.
 */
const BLEND_WAVELENGTHS_NM = [...CHANNEL_WAVELENGTHS_NM, 400, 700, 500, 600] as const;

/** Rayleigh's χ₂ at the depolarisation ratio ρ: (1 − ρ) ÷ (5(2 + ρ)). */
function chi2(rho: number): number {
  return (1 - rho) / (5 * (2 + rho));
}

/** One drawn term's channel source's optics at {@link BLEND_WAVELENGTHS_NM}. */
function channelOpticsOf(term: MediumTerm): ReadonlyArray<SpectralTermOptics> {
  const column = spectralColumnOf(
    { name: "one term", topHeightM: 100_000, groundAlbedo: [0, 0, 0], terms: [term] },
    [channelSource(term)],
  );
  return spectralOptics(column, BLEND_WAVELENGTHS_NM).map(([optics]) => {
    if (optics === undefined) {
      throw new Error("the column has its one term");
    }
    return optics;
  });
}

/** An array's entry, or NaN. */
function at(values: ArrayLike<number>, i: number): number {
  return values[i] ?? Number.NaN;
}

describe("a channel source", () => {
  /** The Venus-class droplets' drawn term, its table with a matrix, and its source's optics. */
  const droplets = once(() => {
    const term = termNamed(fixture().medium, DROPLETS.name);
    if (term.phase.kind !== "tabulated" || term.phase.table.matrix === undefined) {
      throw new Error("the droplets' phase is a table with a matrix");
    }
    return { term, table: term.phase.table, optics: channelOpticsOf(term) };
  });

  it(
    "gives at a channel's wavelength the term's coefficients to the bit and its table's channel as a table of its own",
    { timeout: 60_000 },
    () => {
      const { term, table, optics } = droplets();
      for (const c of [0, 1, 2] as const) {
        const own = optics[c];
        const copy = tableOf(own?.phase ?? { kind: "none" });
        expect([own?.scatteringPerM, own?.absorptionPerM]).toEqual([
          term.scattering[c],
          term.absorption[c],
        ]);
        expect([...copy.values, copy.matrix?.b1[0], copy.matrix?.a2[2]]).toEqual([
          table.values[c],
          table.values[c],
          table.values[c],
          table.matrix?.b1[c],
          table.matrix?.a2[c],
        ]);
        expect(forwardDeltaShare(own?.phase ?? { kind: "none" })).toBe(table.forwardPeak[c]);
        expect(phaseMoments(own?.phase ?? { kind: "none" }, 16)).toEqual(
          deltaMTruncation(table, c, 8).fullMoments,
        );
      }
    },
  );

  it("holds the nearer channel's own beyond the channels' range: blue below 433.625 nm, red above 645 nm", () => {
    const { term, optics } = droplets();
    const [, , , below, above] = optics;
    expect([below?.scatteringPerM, tableOf(below?.phase ?? { kind: "none" }).values[0]]).toEqual([
      term.scattering[2],
      tableOf(optics[2]?.phase ?? { kind: "none" }).values[0],
    ]);
    expect([above?.scatteringPerM, tableOf(above?.phase ?? { kind: "none" }).values[0]]).toEqual([
      term.scattering[0],
      tableOf(optics[0]?.phase ?? { kind: "none" }).values[0],
    ]);
  });

  it("blends two channels between their wavelengths by the ground rule: its coefficients and forward delta the ground's own rule to the bit, its moments Σ w_c χ_l,c to 10⁻¹⁵", () => {
    const { term, table, optics } = droplets();
    let worst = 0;
    for (const [k, nm] of [
      [5, 500],
      [6, 600],
    ] as const) {
      const blend = channelBlendAt(nm);
      if (blend.kind !== "two") {
        throw new Error(`${nm} nm lies between two channels`);
      }
      const own = optics[k];
      const phase = momentsOf(own?.phase ?? { kind: "none" });
      expect([own?.scatteringPerM, own?.absorptionPerM, phase.forwardDelta]).toEqual([
        binGroundAlbedo(term.scattering, nm),
        binGroundAlbedo(term.absorption, nm),
        binGroundAlbedo(table.forwardPeak, nm),
      ]);
      const w = blend.offsetNm / blend.spanNm;
      const low = deltaMTruncation(table, blend.low, CHANNEL_SOURCE_MOMENT_ORDER / 2).fullMoments;
      const high = deltaMTruncation(table, blend.high, CHANNEL_SOURCE_MOMENT_ORDER / 2).fullMoments;
      expect(phase.moments.length).toBe(CHANNEL_SOURCE_MOMENT_ORDER + 1);
      expect(phase.moments[0]).toBe(1);
      for (let l = 1; l <= CHANNEL_SOURCE_MOMENT_ORDER; l += 1) {
        const expected = (1 - w) * at(low, l) + w * at(high, l);
        worst = Math.max(worst, Math.abs(at(phase.moments, l) - expected));
      }
    }
    // Measured 5.6 × 10⁻¹⁷: the ground rule's order of operations against the weights' (2026-10-27).
    expect(worst).toBeLessThanOrEqual(1e-15);
  });

  it("takes a Rayleigh term's own ρ at a channel and, between two, the blend of their χ₂ alone", () => {
    const molecular = termNamed(fixture().medium, "rayleigh");
    if (molecular.phase.kind !== "rayleigh") {
      throw new Error("the molecular term's phase is Rayleigh's");
    }
    const rho = molecular.phase.depolarisation;
    const optics = channelOpticsOf(molecular);
    expect(optics.slice(0, 3).map((o) => o.phase)).toEqual([
      { kind: "rayleigh", depolarisation: rho[0] },
      { kind: "rayleigh", depolarisation: rho[1] },
      { kind: "rayleigh", depolarisation: rho[2] },
    ]);
    const w =
      (500 - CHANNEL_WAVELENGTHS_NM[2]) / (CHANNEL_WAVELENGTHS_NM[1] - CHANNEL_WAVELENGTHS_NM[2]);
    const between = momentsOf(optics[5]?.phase ?? { kind: "none" });
    expect(
      Math.abs(at(between.moments, 2) - ((1 - w) * chi2(rho[2]) + w * chi2(rho[1]))),
    ).toBeLessThan(1e-16);
    expect([...between.moments].filter((_, l) => l !== 0 && l !== 2).every((m) => m === 0)).toBe(
      true,
    );
  });

  it("takes a Cornette–Shanks term by its closed-form moments at every wavelength, with no forward delta", () => {
    const optics = channelOpticsOf({
      name: "aerosol",
      density: { kind: "exponential", scaleHeightM: 1_200 },
      scattering: [1e-5, 2e-5, 3e-5],
      absorption: [1e-6, 2e-6, 3e-6],
      phase: { kind: "cornette-shanks", asymmetry: 0.584 },
    });
    const closed = cornetteShanksMoments(0.584, CHANNEL_SOURCE_MOMENT_ORDER);
    let worst = 0;
    for (const o of optics) {
      const phase = momentsOf(o.phase);
      expect(phase.forwardDelta).toBe(0);
      for (const [l, m] of phase.moments.entries()) {
        worst = Math.max(worst, Math.abs(m - at(closed, l)));
      }
    }
    expect(momentsOf(optics[1]?.phase ?? { kind: "none" }).moments).toEqual(closed);
    // A blend of one set of moments with itself is exact.
    expect(worst).toBe(0);
  });

  it("is refused on an absorber term, which takes its amount", () => {
    const absorber = fixture().medium.terms.find((t) => t.absorber !== undefined);
    if (absorber === undefined) {
      throw new Error("the medium has an absorber");
    }
    const one: AtmosphereMedium = {
      name: "one absorber",
      topHeightM: 100_000,
      groundAlbedo: [0, 0, 0],
      terms: [absorber],
    };
    expect(() => spectralColumnOf(one, [channelSource(absorber)])).toThrow(RangeError);
    expect(spectralColumnOf(one, [absorberSourceOf(absorber)]).terms[0]?.source.kind).toBe(
      "absorber",
    );
  });
});

describe("the ground rule", () => {
  it("takes each channel's own value at its wavelength, and blends its neighbours between", () => {
    const albedo = [0.1, 0.5, 0.9] as const;
    expect(CHANNEL_WAVELENGTHS_NM.map((nm) => binGroundAlbedo(albedo, nm))).toEqual([...albedo]);
    expect(CHANNEL_WAVELENGTHS_NM.map((nm) => channelBlendAt(nm))).toEqual([
      { kind: "one", channel: 0 },
      { kind: "one", channel: 1 },
      { kind: "one", channel: 2 },
    ]);
    // 392.67 and 418 nm under blue's 433.625, 646 nm and up over red's 645.
    expect(BAKE_WAVELENGTHS_NM.map((nm) => channelBlendAt(nm).kind)).toEqual([
      ...Array.from({ length: 2 }, () => "one"),
      ...Array.from({ length: 8 }, () => "two"),
      ...Array.from({ length: 5 }, () => "one"),
    ]);
  });
});
