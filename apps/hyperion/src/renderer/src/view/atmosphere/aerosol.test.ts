import { describe, expect, it } from "vitest";

import {
  aerosolModeOptics,
  aerosolPhaseTable,
  aerosolTerm,
  deltaMTruncation,
  henyeyGreenstein,
  leastValidatedBasis,
  MODEL_RESIDUALS_LABEL,
  phaseApproximations,
  phaseLabels,
  phaseTableAsymmetry,
  phaseTableIntegral,
  SMALL_BIAS_ANALOGUES,
} from "./aerosol";
import { gaussLegendre } from "../lighting/quadrature";
import { hydrostaticColumn } from "./column";
import { CHANNEL_WAVELENGTHS_NM, columnLengthM, densityAt, phaseAt } from "./medium";
import { opticsOfParticles } from "./nonSpherical";
import { type ModeOptics, PHASE_TABLE_U } from "./sizeDistribution";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "./thick/regime";

/** A Mars-like column: 610 Pa, 210 K, CO₂, Mars's surface gravity and mean radius. */
const COLUMN = hydrostaticColumn({
  surfacePa: 610,
  temperature: { kind: "isothermal", temperatureK: 210 },
  meanMolarMassGPerMol: 44.01,
  referenceGravityMS2: 3.71,
  referenceRadiusM: 3.3895e6,
});

/** Mars dust in the modes' volume-to-area radius: Wolff et al. 2009's surface-equivalent 1.5 µm × (2 ÷ 3)^½. */
const MARS_DUST = {
  kind: "distribution",
  mode: {
    material: "mars_dust",
    sizes: { kind: "gamma", effectiveRadiusUm: 1.2247, effectiveVariance: 0.3 },
  },
} as const;

const SULPHATE = {
  kind: "distribution",
  mode: {
    material: "H2SO4",
    sizes: { kind: "logNormal", effectiveRadiusUm: 0.2, effectiveVariance: 0.2 },
  },
} as const;

const TITAN_HAZE = {
  kind: "aggregate",
  mode: { material: "tholin", monomerRadiusUm: 0.05, monomerCount: 3000, fractalDimension: 2 },
} as const;

/** A value computed on its first use, so that a describe block's setup runs inside its tests. */
function once<T>(make: () => T): () => T {
  let value: { readonly made: T } | undefined;
  return () => {
    value ??= { made: make() };
    return value.made;
  };
}

function channels(
  particles: Parameters<typeof aerosolModeOptics>[0],
): [ModeOptics, ModeOptics, ModeOptics] {
  const [r, g, b] = CHANNEL_WAVELENGTHS_NM;
  return [
    aerosolModeOptics(particles, r),
    aerosolModeOptics(particles, g),
    aerosolModeOptics(particles, b),
  ];
}

describe("optics by shape", () => {
  it("draws a liquid as spheres by Mie, a mineral on the hexahedra, an aggregate by the MMF", () => {
    expect(aerosolModeOptics(SULPHATE, 550).phaseModel).toBe("mie");
    expect(aerosolModeOptics(MARS_DUST, 550).phaseModel).toBe("tamudust2020");
    expect(aerosolModeOptics(TITAN_HAZE, 550).phaseModel).toBe("mmf");
  });

  it("draws liquid iron as spheres and solid iron on the longwave kernel", () => {
    const sizes = { kind: "gamma", effectiveRadiusUm: 0.5, effectiveVariance: 0.1 } as const;
    const liquid = aerosolModeOptics(
      { kind: "distribution", mode: { material: "Fe", form: { phase: "liquid" }, sizes } },
      550,
    );
    const solid = aerosolModeOptics({ kind: "distribution", mode: { material: "Fe", sizes } }, 550);
    expect(liquid.shape).toBe("sphere");
    expect(liquid.phaseModel).toBe("mie");
    expect(solid.shape).toBe("nonSphericalMineral");
    expect(solid.phaseModel).toBe("tamudust2020");
  });

  it("refuses a distribution declared an aggregate", () => {
    expect(() =>
      aerosolModeOptics(
        {
          kind: "distribution",
          mode: {
            material: "soot",
            shape: "aggregate",
            sizes: { kind: "gamma", effectiveRadiusUm: 0.1, effectiveVariance: 0.1 },
          },
        },
        550,
      ),
    ).toThrow(RangeError);
  });
});

describe("the phase tables", () => {
  const cases = [
    ["spheres", SULPHATE],
    ["hexahedra", MARS_DUST],
    ["an aggregate", TITAN_HAZE],
  ] as const;

  for (const [name, particles] of cases) {
    const optics = once(() => channels(particles));
    const table = once(() => aerosolPhaseTable(optics()));

    it(`integrate to 1 to 10⁻⁶ in every channel, for ${name}`, () => {
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(phaseTableIntegral(table(), c) - 1)).toBeLessThan(1e-6);
      }
    });

    it(`stand on the 256 √θ entries and carry the model's matrix, for ${name}`, () => {
      expect(table().u).toBe(PHASE_TABLE_U);
      expect(table().u.length).toBe(256);
      expect(table().matrix).toBeDefined();
    });

    it(`keep the model's asymmetry within 10⁻⁴ as read, for ${name}`, () => {
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(phaseTableAsymmetry(table(), c) - optics()[c].asymmetry)).toBeLessThan(
          1e-4,
        );
      }
    });

    it(`are read by phaseAt in sr⁻¹, for ${name}`, () => {
      const [red] = phaseAt({ kind: "tabulated", table: table() }, -1);
      expect(red).toBeCloseTo(table().values[0][255] ?? Number.NaN, 12);
    });
  }

  it("tabulate the Henyey–Greenstein fallback from g, with no matrix", () => {
    const rutile = {
      material: "TiO2",
      shape: "nonSphericalMineral",
      index: () => ({ n: 2.65, k: 0 }),
      provenance: "measured",
      table: undefined,
    } as const;
    const sizes = { kind: "gamma", effectiveRadiusUm: 0.5, effectiveVariance: 0.1 } as const;
    const [red, green, blue] = CHANNEL_WAVELENGTHS_NM;
    const optics: [ModeOptics, ModeOptics, ModeOptics] = [
      opticsOfParticles(rutile, sizes, red),
      opticsOfParticles(rutile, sizes, green),
      opticsOfParticles(rutile, sizes, blue),
    ];
    const table = aerosolPhaseTable(optics);
    expect(table.matrix).toBeUndefined();
    expect(Math.abs(phaseTableIntegral(table, 1) - 1)).toBeLessThan(1e-6);
    // The table is Henyey–Greenstein's of the channel's g times one normalising factor near 1.
    const g = optics[1].asymmetry;
    const factor = (j: number): number =>
      (table.values[1][j] ?? 0) / henyeyGreenstein(g, Math.cos(Math.PI * (j / 255) ** 2));
    expect(factor(20)).toBeCloseTo(factor(200), 12);
    expect(Math.abs(factor(128) - 1)).toBeLessThan(1e-3);
  });

  it("truncate by delta-M to a series that, with its fraction forward, keeps the integral and g", () => {
    const table = aerosolPhaseTable(channels(MARS_DUST));
    const streams = 8;
    const series = deltaMTruncation(table, 1, streams);
    const f = series.fraction;
    expect(f).toBeGreaterThan(0);
    expect(f).toBeLessThan(1);
    expect(series.moments.length).toBe(2 * streams);
    // The truncated phase p′(μ) = Σ (2l + 1) χ′ₗ Pₗ(μ) ÷ 4π, rebuilt and integrated by
    // Gauss–Legendre in μ, with f put back as a forward spike: ∫ dΩ gives 1 and ∫ μ dΩ gives g.
    const rule = gaussLegendre(64);
    let integral = 0;
    let first = 0;
    for (const [i, mu] of rule.x.entries()) {
      let previous = 1;
      let current = mu;
      let phase = series.moments[0] ?? 0;
      for (let l = 1; l < 2 * streams; l += 1) {
        phase += (2 * l + 1) * (series.moments[l] ?? 0) * current;
        const next = ((2 * l + 1) * mu * current - l * previous) / (l + 1);
        previous = current;
        current = next;
      }
      const weight = (rule.w[i] ?? 0) * 2 * Math.PI * (phase / (4 * Math.PI));
      integral += weight;
      first += weight * mu;
    }
    expect((1 - f) * integral + f).toBeCloseTo(phaseTableIntegral(table, 1), 9);
    expect((1 - f) * first + f).toBeCloseTo(phaseTableAsymmetry(table, 1), 9);
  });
});

/**
 * A table channel's ∫ p P(μ) dΩ as R08.T12.c's tracer and `tables.ts` read it, independently of
 * `aerosol.ts`: p linear in u between entries, 4π² ∫₀¹ p(u) P(cos πu²) u sin(πu²) du, by
 * composite Simpson with 8 panels an interval.
 */
function readerIntegral(
  u: Float64Array,
  values: Float64Array,
  weight: (mu: number) => number,
): number {
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const p0 = values[i] ?? Number.NaN;
    const p1 = values[i + 1] ?? Number.NaN;
    const h = (u1 - u0) / 8;
    for (let k = 0; k <= 8; k += 1) {
      const f = k / 8;
      const x = u0 + f * (u1 - u0);
      const angle = Math.PI * x * x;
      const simpson = k === 0 || k === 8 ? 1 : k % 2 === 1 ? 4 : 2;
      sum +=
        (simpson * h * (p0 + f * (p1 - p0)) * x * Math.sin(angle) * weight(Math.cos(angle))) / 3;
    }
  }
  return 4 * Math.PI * Math.PI * sum;
}

describe("every committed phase file's tables, as the tracer reads them", () => {
  // A mode near each file's smallest sizes and one near its largest, where the files' angles miss
  // the forward peak; each within the table's reach at 440 nm.
  const files = [
    ["mars-dust.json", "mars_dust", 1.2247, 30],
    ["enstatite-glass.json", "MgSiO3", 1, 20],
    ["forsterite-amorphous.json", "Mg2SiO4", 1, 20],
    ["iron.json", "Fe", 0.5, 3],
    ["water-ice.json", "H2O", 1, 60],
  ] as const;
  for (const [file, material, small, large] of files) {
    for (const radiusUm of [small, large]) {
      const particles = {
        kind: "distribution",
        mode: {
          material,
          form: { phase: "solid" },
          sizes: { kind: "gamma", effectiveRadiusUm: radiusUm, effectiveVariance: 0.1 },
        },
      } as const;
      const optics = once(() => channels(particles));
      const table = once(() => aerosolPhaseTable(optics()));

      it(`normalise to 10⁻⁴ under the linear-in-u rule, for ${file} at r_eff ${radiusUm} µm`, () => {
        expect(optics()[1].phaseModel).toMatch(/tamudust2020|yang2013/u);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(readerIntegral(table().u, table().values[c], () => 1) - 1)).toBeLessThan(
            1e-4,
          );
        }
      });

      it(`keep the model's g to 10⁻⁴ as read, for ${file} at r_eff ${radiusUm} µm`, () => {
        for (const c of [0, 1, 2] as const) {
          const g = readerIntegral(table().u, table().values[c], (mu) => mu);
          expect(Math.abs(g - optics()[c].asymmetry)).toBeLessThan(1e-4);
        }
      });
    }
  }

  it("carry a large particle's unresolved forward peak forward, not across every angle", () => {
    // Cirrus of r_eff 30 µm: Yang et al.'s angles hold 0.96 of its scattering, and the rest is a
    // forward peak; rescaling every angle instead would raise side and back light by 4%.
    const cirrus = {
      kind: "distribution",
      mode: {
        material: "H2O",
        form: { phase: "solid" },
        sizes: { kind: "gamma", effectiveRadiusUm: 30, effectiveVariance: 0.1 },
      },
    } as const;
    const optics = aerosolModeOptics(cirrus, 550);
    const peak = optics.matrix?.forwardPeak ?? 0;
    expect(peak).toBeGreaterThan(0.02);
    expect(peak).toBeLessThan(0.1);
    const table = aerosolPhaseTable(channels(cirrus));
    // The peak lies on the first entries alone: past them the table is the matrix's a₁ ÷ 4π.
    const a1 = optics.matrix?.a1 ?? new Float64Array(0);
    for (const j of [8, 64, 128, 200, 255]) {
      expect((table.values[1][j] ?? 0) / ((a1[j] ?? 0) / (4 * Math.PI))).toBeCloseTo(1, 2);
    }
  });
});

describe("the term", () => {
  const spec = {
    name: "dust",
    particles: MARS_DUST,
    opticalDepth550: 0.5,
    basePa: 610,
    topPa: 50,
    mixingExponent: 0,
  } as const;
  const built = once(() => aerosolTerm(spec, COLUMN));

  it("gives the column its τ(550)", () => {
    const { term, at550 } = built();
    const green = CHANNEL_WAVELENGTHS_NM.indexOf(550);
    const extinction = (term.scattering[green] ?? 0) + (term.absorption[green] ?? 0);
    expect(extinction * columnLengthM(term.density, COLUMN.topHeightM)).toBeCloseTo(0.5, 9);
    expect(at550.wavelengthNm).toBe(550);
  });

  it("scales the channels by the mode's extinction and splits them by its ω", () => {
    const { term, channels: optics } = built();
    for (const c of [0, 1, 2] as const) {
      const extinction = term.scattering[c] + term.absorption[c];
      expect(term.scattering[c] / extinction).toBeCloseTo(optics[c].singleScatteringAlbedo, 12);
    }
  });

  it("holds nothing above its top", () => {
    const { term } = built();
    const pressures = COLUMN.pressuresPa;
    const above = COLUMN.altitudesM.find((_, i) => (pressures[i] ?? 0) < 40) ?? Number.NaN;
    expect(densityAt(term.density, above)).toBe(0);
    expect(densityAt(term.density, 0)).toBeGreaterThan(0);
  });

  it("draws a tabulated phase", () => {
    expect(built().term.phase.kind).toBe("tabulated");
  });

  it("falls off above its base as (p ÷ p_base)^f with the column's density", () => {
    const f = 2;
    const { term } = aerosolTerm({ ...spec, basePa: 600, topPa: 100, mixingExponent: f }, COLUMN);
    // Two column levels inside the layer: the ratio of the mode's densities is the gas's times
    // the mixing ratios'.
    const inside = Array.from(COLUMN.pressuresPa, (p, i) => ({ p, i })).filter(
      ({ p }) => p < 500 && p > 200,
    );
    const lower = inside[0];
    const upper = inside.at(-1);
    if (lower === undefined || upper === undefined) {
      throw new Error("the column has no levels inside the layer");
    }
    const at = (i: number): number => densityAt(term.density, COLUMN.altitudesM[i] ?? Number.NaN);
    const gas = (COLUMN.density.relative[upper.i] ?? 0) / (COLUMN.density.relative[lower.i] ?? 1);
    expect(at(upper.i) / at(lower.i)).toBeCloseTo(gas * (upper.p / lower.p) ** f, 9);
  });

  it("refuses an inverted profile", () => {
    expect(() => aerosolTerm({ ...spec, basePa: 10, topPa: 50 }, COLUMN)).toThrow(RangeError);
  });
});

describe("the label", () => {
  const rutile = {
    material: "TiO2",
    shape: "nonSphericalMineral",
    index: () => ({ n: 2.65, k: 0 }),
    provenance: "measured",
    table: undefined,
  } as const;
  const tio2 = once(() =>
    opticsOfParticles(rutile, { kind: "gamma", effectiveRadiusUm: 3, effectiveVariance: 0.1 }, 550),
  );
  const dust = once(() => aerosolModeOptics(MARS_DUST, 550));

  it("labels a 3 µm TiO₂ fallback under the τ rule, and not beneath a deck", () => {
    expect(phaseLabels([{ material: "TiO2", optics: [tio2()], opticalDepthAbove550: 0 }])).toEqual([
      "atmosphereApproximate",
    ]);
    expect(
      phaseLabels([
        {
          material: "TiO2",
          optics: [tio2()],
          opticalDepthAbove550: CLOUD_DECK_SPLIT_OPTICAL_DEPTH + 20,
        },
      ]),
    ).toEqual([]);
    expect(
      phaseApproximations([{ material: "TiO2", optics: [tio2()], opticalDepthAbove550: 1 }])[0]
        ?.reason,
    ).toContain("fallback");
  });

  it("labels an NH₃-ice analogue under the τ rule, and not a water-ice model", () => {
    const sizes = { kind: "gamma", effectiveRadiusUm: 10, effectiveVariance: 0.1 } as const;
    const ammonia = aerosolModeOptics(
      { kind: "distribution", mode: { material: "NH3", sizes } },
      550,
    );
    const water = aerosolModeOptics(
      { kind: "distribution", mode: { material: "H2O", form: { phase: "solid" }, sizes } },
      550,
    );
    expect(phaseLabels([{ material: "NH3", optics: [ammonia], opticalDepthAbove550: 0 }])).toEqual([
      "atmosphereApproximate",
    ]);
    expect(phaseLabels([{ material: "H2O", optics: [water], opticalDepthAbove550: 0 }])).toEqual(
      [],
    );
  });

  it("labels the Titan-like haze, an MMF analogue in its blue channel, under the τ rule", () => {
    const { channels: titan } = aerosolTerm(
      {
        name: "haze",
        particles: TITAN_HAZE,
        opticalDepth550: 1,
        basePa: 500,
        topPa: 100,
        mixingExponent: 0,
      },
      COLUMN,
    );
    expect(titan.map((c) => c.phaseBasis)).toEqual(["model", "model", "analogue"]);
    expect(leastValidatedBasis(titan)).toBe("analogue");
    const reasons = phaseApproximations([
      { material: "tholin", optics: titan, opticalDepthAbove550: 0 },
    ]);
    expect(reasons[0]?.reason).toContain("440 nm");
  });

  it("does not label a Mars dust mode, a model", () => {
    expect(
      phaseLabels([{ material: "mars_dust", optics: [dust()], opticalDepthAbove550: 0 }]),
    ).toEqual([]);
  });

  it("builds the two pending labelling questions to the ruling's lean", () => {
    expect(MODEL_RESIDUALS_LABEL).toBe("unlabelled");
    expect(SMALL_BIAS_ANALOGUES).toBe("labelled");
  });

  it("refuses a bad optical depth", () => {
    expect(() =>
      phaseLabels([{ material: "TiO2", optics: [tio2()], opticalDepthAbove550: -1 }]),
    ).toThrow(RangeError);
  });
});
