import { describe, expect, it } from "vitest";

import { gaussLegendre } from "../lighting/quadrature";
import { refractiveIndex } from "./materials/materials";
import { mieSphere, mieTermCount } from "./mie";
import {
  modeOptics,
  PHASE_TABLE_MU,
  PHASE_TABLE_SIZE,
  PHASE_TABLE_U,
  type ScatteringMatrix,
  type SizeDistribution,
  sizeQuadrature,
  sizeRange,
  sphereModeOptics,
  sphereScatteringMatrix,
} from "./sizeDistribution";

/** r_eff and v_eff of a quadrature, from its area weights (Hansen and Travis 1974, (2.53)–(2.54)). */
function moments(sizes: SizeDistribution): { rEff: number; vEff: number } {
  const { radiiUm, areaWeights } = sizeQuadrature(sizes, 550);
  let rEff = 0;
  for (let i = 0; i < radiiUm.length; i += 1) {
    rEff += (areaWeights[i] ?? 0) * (radiiUm[i] ?? 0);
  }
  let vEff = 0;
  for (let i = 0; i < radiiUm.length; i += 1) {
    vEff += (areaWeights[i] ?? 0) * ((radiiUm[i] ?? 0) - rEff) ** 2;
  }
  return { rEff, vEff: vEff / (rEff * rEff) };
}

function at(values: Float64Array, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no element ${i}`);
  }
  return value;
}

/** ½ ∫ f dμ over a Gauss–Legendre grid: ∫ f dΩ ÷ 4π for an azimuth-free f. */
function halfIntegral(
  rule: { x: Float64Array; w: Float64Array },
  f: (i: number) => number,
): number {
  let sum = 0;
  for (let i = 0; i < rule.x.length; i += 1) {
    sum += at(rule.w, i) * f(i);
  }
  return 0.5 * sum;
}

/** Hansen and Travis 1974's Rayleigh matrix, eq. (2.15) with δ = 0, at a cosine. */
function rayleigh(mu: number): {
  a1: number;
  b1: number;
  a3: number;
  b2: number;
} {
  return { a1: 0.75 * (1 + mu * mu), b1: -0.75 * (1 - mu * mu), a3: 1.5 * mu, b2: 0 };
}

/** The largest difference of any element from the Rayleigh matrix, over the matrix's cosines. */
function rayleighDeviation(matrix: ScatteringMatrix): number {
  let worst = 0;
  for (let j = 0; j < matrix.mu.length; j += 1) {
    const expected = rayleigh(at(matrix.mu, j));
    worst = Math.max(
      worst,
      Math.abs(at(matrix.a1, j) - expected.a1),
      Math.abs(at(matrix.a2, j) - expected.a1),
      Math.abs(at(matrix.b1, j) - expected.b1),
      Math.abs(at(matrix.a3, j) - expected.a3),
      Math.abs(at(matrix.a4, j) - expected.a3),
      Math.abs(at(matrix.b2, j) - expected.b2),
    );
  }
  return worst;
}

/** Venus's mode-2 droplets: Hansen and Hovenier 1974's a = 1.05 µm, b = 0.07, in Hansen and Travis's (2.56). */
const VENUS_MODE_2: SizeDistribution = {
  kind: "gamma",
  effectiveRadiusUm: 1.05,
  effectiveVariance: 0.07,
};

describe("size distributions", () => {
  const cases: ReadonlyArray<SizeDistribution> = [
    VENUS_MODE_2,
    { kind: "gamma", effectiveRadiusUm: 0.15, effectiveVariance: 0.28 },
    { kind: "gamma", effectiveRadiusUm: 2, effectiveVariance: 0.4 },
    { kind: "gamma", effectiveRadiusUm: 10, effectiveVariance: 0.01 },
    { kind: "logNormal", effectiveRadiusUm: 1.05, effectiveVariance: 0.07 },
    { kind: "logNormal", effectiveRadiusUm: 0.15, effectiveVariance: 0.28 },
    { kind: "logNormal", effectiveRadiusUm: 1.5, effectiveVariance: 0.3 },
  ];

  it.each(cases)(
    "reproduces the $kind distribution's r_eff $effectiveRadiusUm µm and v_eff $effectiveVariance to 10⁻⁴",
    (sizes) => {
      const { rEff, vEff } = moments(sizes);
      expect(Math.abs(rEff / sizes.effectiveRadiusUm - 1)).toBeLessThan(1e-4);
      expect(Math.abs(vEff / sizes.effectiveVariance - 1)).toBeLessThan(1e-4);
    },
  );

  it("covers the geometric weight below and the Rayleigh weight above", () => {
    // For a log-normal the ends are ±5.26σ about the r² and r⁶ weights' centres, ln r_g + 2σ² and
    // ln r_g + 6σ².
    const sigma2 = Math.log1p(0.07);
    const lnRg = Math.log(1.05) - 2.5 * sigma2;
    const half = Math.sqrt(sigma2) * Math.sqrt(2 * Math.log(1e6));
    const range = sizeRange({
      kind: "logNormal",
      effectiveRadiusUm: 1.05,
      effectiveVariance: 0.07,
    });
    expect(Math.log(range.fromUm)).toBeCloseTo(lnRg + 2 * sigma2 - half, 12);
    expect(Math.log(range.toUm)).toBeCloseTo(lnRg + 6 * sigma2 + half, 12);
  });

  it("refuses a quadrature at a wavelength that is not positive, or at a bad level", () => {
    expect(() => sizeQuadrature(VENUS_MODE_2, 0)).toThrow(RangeError);
    expect(() => sizeQuadrature(VENUS_MODE_2, Number.NaN)).toThrow(RangeError);
    expect(() => sizeQuadrature(VENUS_MODE_2, 550, 1.5)).toThrow(RangeError);
    expect(() => sizeQuadrature(VENUS_MODE_2, 550, 17)).toThrow(RangeError);
  });

  it("refuses a gamma variance outside (0, 0.5) and a radius that is not positive", () => {
    expect(() =>
      sizeRange({ kind: "gamma", effectiveRadiusUm: 1, effectiveVariance: 0.5 }),
    ).toThrow(RangeError);
    expect(() =>
      sizeRange({ kind: "logNormal", effectiveRadiusUm: 0, effectiveVariance: 0.1 }),
    ).toThrow(RangeError);
  });
});

describe("the phase tables' angles", () => {
  it("run from forward to backward on u = √(θ ÷ π)", () => {
    expect(PHASE_TABLE_U).toHaveLength(PHASE_TABLE_SIZE);
    expect(at(PHASE_TABLE_MU, 0)).toBe(1);
    expect(at(PHASE_TABLE_MU, PHASE_TABLE_SIZE - 1)).toBeCloseTo(-1, 15);
    expect(Math.acos(at(PHASE_TABLE_MU, 128)) / Math.PI).toBeCloseTo(
      at(PHASE_TABLE_U, 128) ** 2,
      12,
    );
  });
});

describe("the scattering matrix", () => {
  it("of a sphere of x = 10⁻³ is Hansen and Travis 1974's Rayleigh matrix, eq. (2.15) with δ = 0, to 10⁻⁶", () => {
    const x = 1e-3;
    const index = { n: 1.5, k: 0.01 };
    const result = mieSphere(x, index, PHASE_TABLE_MU);
    expect(rayleighDeviation(sphereScatteringMatrix(result, x, PHASE_TABLE_MU))).toBeLessThan(1e-6);
  });

  it("of a narrow mode of x = 10⁻³ spheres is the Rayleigh matrix too", () => {
    const radiusUm = (1e-3 * 0.55) / (2 * Math.PI);
    const optics = sphereModeOptics(
      { kind: "logNormal", effectiveRadiusUm: radiusUm, effectiveVariance: 1e-6 },
      { n: 1.44, k: 0 },
      550,
    );
    expect(rayleighDeviation(optics.matrix)).toBeLessThan(1e-6);
  });

  it("gives b₂ the sign of Hansen and Travis 1974's Fig. 16 at 140°", () => {
    // Fig. 16 (Space Sci. Rev. 16, p. 560; n_r 1.33, the standard gamma distribution with
    // b = 0.07, positive regions crosshatched) has 100 P⁴³ ÷ P¹¹ inside its −80 contour near
    // θ = 140° at λ = 0.45 µm for a = 1 µm, x_eff ≈ 14; P⁴³ = −b₂ (de Rooij and van der Stap 1984,
    // eq. 16).
    const x = 14;
    const sizes: SizeDistribution = {
      kind: "gamma",
      effectiveRadiusUm: (x * 0.55) / (2 * Math.PI),
      effectiveVariance: 0.07,
    };
    const mu = Float64Array.of(Math.cos((140 * Math.PI) / 180));
    const matrix = sphereModeOptics(sizes, { n: 1.33, k: 0 }, 550, mu).matrix;
    const ratio = at(matrix.b2, 0) / at(matrix.a1, 0);
    expect(ratio).toBeGreaterThan(0.75);
    expect(ratio).toBeLessThan(1);
  });

  it("gives Rayleigh's degree of polarisation, −b₁ ÷ a₁ = sin²θ ÷ (1 + cos²θ)", () => {
    const x = 1e-3;
    const mu = Float64Array.of(0, 0.5);
    const matrix = sphereScatteringMatrix(mieSphere(x, { n: 1.33, k: 0 }, mu), x, mu);
    expect(-at(matrix.b1, 0) / at(matrix.a1, 0)).toBeCloseTo(1, 6);
    expect(-at(matrix.b1, 1) / at(matrix.a1, 1)).toBeCloseTo(0.75 / 1.25, 6);
  });

  it("of one sphere keeps a₁² = b₁² + a₃² + b₂², and a mode's lies inside it", () => {
    const x = 7.3;
    const single = sphereScatteringMatrix(
      mieSphere(x, { n: 1.5, k: 0.02 }, PHASE_TABLE_MU),
      x,
      PHASE_TABLE_MU,
    );
    const mode = sphereModeOptics(VENUS_MODE_2, { n: 1.44, k: 0 }, 550).matrix;
    for (let j = 0; j < PHASE_TABLE_SIZE; j += 1) {
      const a1 = at(single.a1, j);
      const sum = at(single.b1, j) ** 2 + at(single.a3, j) ** 2 + at(single.b2, j) ** 2;
      expect(Math.abs(a1 * a1 - sum)).toBeLessThan(1e-10 * a1 * a1);
      const m1 = at(mode.a1, j);
      expect(at(mode.b1, j) ** 2 + at(mode.a3, j) ** 2 + at(mode.b2, j) ** 2).toBeLessThanOrEqual(
        m1 * m1 * (1 + 1e-12),
      );
      expect(at(mode.a2, j)).toBe(m1);
      expect(at(mode.a4, j)).toBe(at(mode.a3, j));
    }
  });

  it("of a mode integrates to 1, with its asymmetry, to 10⁻⁹ on a Gauss–Legendre grid", () => {
    const largest = (2 * Math.PI * sizeRange(VENUS_MODE_2).toUm) / 0.55;
    const rule = gaussLegendre(mieTermCount(largest) + 8);
    const optics = sphereModeOptics(VENUS_MODE_2, { n: 1.44, k: 1e-3 }, 550, rule.x);
    expect(Math.abs(halfIntegral(rule, (i) => at(optics.matrix.a1, i)) - 1)).toBeLessThan(1e-9);
    const cosine = halfIntegral(rule, (i) => at(optics.matrix.a1, i) * at(rule.x, i));
    expect(Math.abs(cosine - optics.asymmetry)).toBeLessThan(1e-9);
  });
});

describe("a mode's optics", () => {
  it("of a narrow distribution equal the single sphere's", () => {
    const radiusUm = 0.6;
    const index = { n: 1.33, k: 1e-3 };
    const x = (2 * Math.PI * radiusUm) / 0.55;
    const sphere = mieSphere(x, index, PHASE_TABLE_MU);
    const single = sphereScatteringMatrix(sphere, x, PHASE_TABLE_MU);
    const mode = sphereModeOptics(
      { kind: "logNormal", effectiveRadiusUm: radiusUm, effectiveVariance: 1e-8 },
      index,
      550,
    );
    expect(Math.abs(mode.qExt / sphere.qExt - 1)).toBeLessThan(1e-6);
    expect(Math.abs(mode.qSca / sphere.qSca - 1)).toBeLessThan(1e-6);
    expect(Math.abs(mode.asymmetry - sphere.asymmetry)).toBeLessThan(1e-6);
    expect(mode.geometricCrossSectionM2 / (Math.PI * (radiusUm * 1e-6) ** 2)).toBeCloseTo(1, 6);
    for (let j = 0; j < PHASE_TABLE_SIZE; j += 1) {
      const scale = at(single.a1, j);
      expect(Math.abs(at(mode.matrix.a1, j) - scale)).toBeLessThan(1e-5 * scale);
      expect(Math.abs(at(mode.matrix.b1, j) - at(single.b1, j))).toBeLessThan(1e-5 * scale);
      expect(Math.abs(at(mode.matrix.a3, j) - at(single.a3, j))).toBeLessThan(1e-5 * scale);
      expect(Math.abs(at(mode.matrix.b2, j) - at(single.b2, j))).toBeLessThan(1e-5 * scale);
    }
  });

  // Hansen and Hovenier 1974 (J. Atmos. Sci. 31, 1137) give Venus's cloud particles r_eff 1.05 µm,
  // v_eff 0.07 and n = 1.44 ± 0.015 at 550 nm. Their figure could not be read here (GISS and AMS
  // refused the fetch, 2026-10-09). The same physics is Hansen and Travis 1974's Fig. 12 (Space
  // Sci. Rev. 16, p. 555; the standard gamma distribution with b = 0.07, n_i = 0), digitised from
  // the ADS scan against its axes: at 2πa ÷ λ = 12.0 the n_r = 1.44 curve reads ⟨cos α⟩ 0.7145
  // (the n_r = 1.55 curve, beside it, 0.700), each to about ±0.002, the curves' line width. The
  // bracket, 0.705 to 0.725, holds that reading with the figure's own drafting error (a 2% shift in
  // x moves g by about 0.003 here).
  it("of Venus's mode-2 droplets give Hansen and Travis 1974's asymmetry at n 1.44 and x_eff 12", () => {
    const optics = sphereModeOptics(VENUS_MODE_2, { n: 1.44, k: 0 }, 550);
    expect(optics.asymmetry).toBeGreaterThan(0.705);
    expect(optics.asymmetry).toBeLessThan(0.725);
    expect(optics.singleScatteringAlbedo).toBeCloseTo(1, 12);
    expect(optics.nodes).toBeLessThanOrEqual(2656);
  });

  it("of Venus's mode 2 on the sulphuric-acid file stay inside the same figure", () => {
    const optics = modeOptics({ material: "H2SO4", sizes: VENUS_MODE_2 }, 550);
    expect(optics.provenance).toBe("measured");
    expect(optics.asymmetry).toBeGreaterThan(0.705);
    expect(optics.asymmetry).toBeLessThan(0.725);
    expect(optics.singleScatteringAlbedo).toBeGreaterThan(0.99999);
  });

  it("of an unknown material take the generic stand-in's index and say so", () => {
    const sizes: SizeDistribution = {
      kind: "gamma",
      effectiveRadiusUm: 0.5,
      effectiveVariance: 0.1,
    };
    const unknown = modeOptics({ material: "Xx9", sizes }, 550);
    const generic = sphereModeOptics(sizes, { n: 1.5, k: 0 }, 550);
    expect(unknown.provenance).toBe("standIn");
    expect(unknown.qExt).toBe(generic.qExt);
  });

  it("mark a non-sphere's Mie values by its shape and carry no matrix for it", () => {
    const sizes: SizeDistribution = {
      kind: "gamma",
      effectiveRadiusUm: 1.5,
      effectiveVariance: 0.3,
    };
    const dust = modeOptics({ material: "mars_dust", sizes }, 550);
    expect(dust.shape).toBe("nonSphericalMineral");
    expect(dust.matrix).toBeUndefined();
    expect(dust.qExt).toBe(
      sphereModeOptics(sizes, refractiveIndex("mars_dust", 550).index, 550).qExt,
    );
    const ice = modeOptics({ material: "H2O", form: { phase: "solid" }, sizes }, 550);
    expect(ice.shape).toBe("crystal");
    expect(ice.matrix).toBeUndefined();
    const droplets = modeOptics({ material: "H2O", sizes }, 550);
    expect(droplets.shape).toBe("sphere");
    expect(droplets.matrix).toBeDefined();
  });

  it("refuse an aggregate, whose optics are T5.c's", () => {
    const sizes: SizeDistribution = {
      kind: "gamma",
      effectiveRadiusUm: 0.05,
      effectiveVariance: 0.1,
    };
    expect(() => modeOptics({ material: "tholin", shape: "aggregate", sizes }, 550)).toThrow(
      RangeError,
    );
  });

  it("gives per-particle cross-sections from the distribution's ⟨r²⟩", () => {
    // ⟨r²⟩ of the standard gamma distribution is a²(1 − b)(1 − 2b) (Hansen and Travis 1974).
    const optics = sphereModeOptics(VENUS_MODE_2, { n: 1.44, k: 0 }, 550);
    const meanSquareM2 = 1.05e-6 ** 2 * (1 - 0.07) * (1 - 0.14);
    expect(optics.geometricCrossSectionM2).toBeCloseTo(Math.PI * meanSquareM2, 20);
    expect(optics.extinctionCrossSectionM2 / optics.geometricCrossSectionM2).toBeCloseTo(
      optics.qExt,
      12,
    );
    expect(optics.volumeM3).toBeCloseTo((4 / 3) * Math.PI * 1.05e-6 * meanSquareM2, 28);
  });

  it("refuses a distribution past Mie's largest size parameter", () => {
    expect(() =>
      sphereModeOptics(
        { kind: "gamma", effectiveRadiusUm: 2000, effectiveVariance: 0.1 },
        { n: 1.33, k: 0 },
        550,
      ),
    ).toThrow(RangeError);
  });
});
