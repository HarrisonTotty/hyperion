import { describe, expect, it } from "vitest";

import {
  dustyPhase,
  hapkeShadowHiding,
  oppositionSurge,
  POWER_LAW_PHASE_EXPONENT,
  powerLawNormalisation,
  powerLawPhase,
  ringPhase,
  ringReflectance,
  ringTransmitted,
  type RingScatter,
  SPARSE_SURGE_AMPLITUDE,
  SURGE_HALF_WIDTH_RAD,
  surgeAmplitude,
} from "./ringPhotometry";

const DEG = Math.PI / 180;

/** ϖ₀ P = 0.5: the hand-computed values below take ϖ₀ 0.5 and P 1. */
function scatterOf(tau: number, mu: number, mu0: number): RingScatter {
  return { tau, mu, mu0, albedo: 0.5, phase: 1 };
}

/** ½ ∫₀^π f(α) sin α dα, which is (1 ÷ 4π) ∫ f dΩ, by Simpson's rule over `n` intervals. */
function sphereMean(f: (alphaRad: number) => number, n = 20_000): number {
  const h = Math.PI / n;
  let sum = 0;
  for (let i = 0; i <= n; i += 1) {
    const weight = i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2;
    const alpha = i * h;
    sum += weight * f(alpha) * Math.sin(alpha);
  }
  return ((sum * h) / 3) * 0.5;
}

/** A thin dusty ring's lit face at phase angle α, rad, seen and lit at μ = μ₀ = 0.3. */
function dustyReflectanceAt(alphaRad: number): number {
  return ringReflectance({
    tau: 1e-6,
    mu: 0.3,
    mu0: 0.3,
    albedo: 0.5,
    phase: ringPhase(alphaRad, "dusty"),
  });
}

describe("the ring's lit and unlit faces", () => {
  // Hand-computed from Design note 14's forms, the factor ¼ included, at ϖ₀ P = 0.5: (μ, μ₀, τ,
  // lit I/F, unlit I/F).
  const HAND: ReadonlyArray<readonly [number, number, number, number, number]> = [
    [0.5, 0.3, 0.1, 0.01937595846046726, 0.0191623954695361],
    [0.5, 0.3, 2.0, 0.04687390738588392, 0.003195563453886444],
    [0.9, 0.2, 0.1, 0.010392102700813139, 0.010296737753633442],
    [0.9, 0.2, 2.0, 0.022727160911349027, 0.003868665117576192],
    [0.1, 0.7, 0.1, 0.07449459529269073, 0.07277060854273282],
    [0.1, 0.7, 2.0, 0.10937499998705247, 0.00837559000927596],
  ];

  it.each(HAND)(
    "match the hand-computed values at μ %f, μ₀ %f and τ %f",
    (mu, mu0, tau, lit, unlit) => {
      expect(ringReflectance(scatterOf(tau, mu, mu0))).toBeCloseTo(lit, 14);
      expect(ringTransmitted(scatterOf(tau, mu, mu0))).toBeCloseTo(unlit, 14);
    },
  );

  it("takes the shadowing factor on the lit face alone", () => {
    const scatter = scatterOf(1.5, 0.3, 0.4);
    expect(ringReflectance(scatter, 1.3)).toBeCloseTo(1.3 * ringReflectance(scatter), 15);
  });

  it("gives the unlit face its limit ϖ₀ P τ exp(−τ/μ) ÷ 4μ at μ = μ₀, continuously", () => {
    const limit = (0.5 * 0.7 * Math.exp(-0.7 / 0.4)) / (4 * 0.4);
    expect(limit).toBeCloseTo(0.03801305012978488, 15);
    expect(ringTransmitted(scatterOf(0.7, 0.4, 0.4))).toBeCloseTo(limit, 15);
    // Either side of the expansion's band, the closed form agrees with it.
    for (const offset of [1e-7, -1e-7, 2e-6, -2e-6, 1e-4, -1e-4]) {
      const near = ringTransmitted(scatterOf(0.7, 0.4 * (1 + offset), 0.4));
      expect(Math.abs(near - limit) / limit).toBeLessThan(4 * Math.abs(offset) * (0.7 / 0.4 + 1));
    }
  });

  it("is finite edge-on and dark with no star", () => {
    expect(ringReflectance(scatterOf(1, 0, 0.5))).toBeCloseTo(0.5 / 4, 15);
    expect(ringTransmitted(scatterOf(1, 0, 0.5))).toBeCloseTo((0.5 * Math.exp(-2)) / 4, 15);
    expect(ringReflectance(scatterOf(1, 0.5, 0))).toBe(0);
    expect(ringTransmitted(scatterOf(1, 0.5, 0))).toBe(0);
  });

  it("draws a dense ring darker from its unlit side than a sparse one", () => {
    const phase = powerLawPhase(30 * DEG);
    const dense = ringTransmitted({ tau: 2.5, mu: 0.4, mu0: 0.2, albedo: 0.5, phase });
    const sparse = ringTransmitted({ tau: 0.1, mu: 0.4, mu0: 0.2, albedo: 0.5, phase });
    expect(dense).toBeLessThan(sparse / 10);
  });

  it("draws a dusty ring brighter back-lit than front-lit", () => {
    expect(dustyReflectanceAt(170 * DEG)).toBeGreaterThan(5 * dustyReflectanceAt(10 * DEG));
  });
});

describe("the particle phase", () => {
  it("normalises the power law over 4π with n = 3.09", () => {
    expect(POWER_LAW_PHASE_EXPONENT).toBe(3.09);
    expect(sphereMean(powerLawPhase)).toBeCloseTo(1, 7);
  });

  it("takes c_n from its moment, exact at n = 3: 2 ÷ (π³ − 6π)", () => {
    expect(powerLawNormalisation(3)).toBeCloseTo(0.1645180505243281, 14);
  });

  it("scatters the main rings backwards, the power law falling to 0 at α = π", () => {
    expect(powerLawPhase(0)).toBeGreaterThan(5);
    expect(powerLawPhase(Math.PI)).toBe(0);
  });

  it("normalises the dusty lobe over 4π and scatters it forwards", () => {
    expect(sphereMean(dustyPhase)).toBeCloseTo(1, 6);
    expect(dustyPhase(Math.PI)).toBeGreaterThan(dustyPhase(0));
  });
});

describe("the opposition surge", () => {
  it.each([
    ["dense", 0.2],
    ["sparse", 0.26],
    ["gap", 0.28],
  ] as const)("has a half-width at half maximum of %s's %f°", (region, widthDeg) => {
    expect(SURGE_HALF_WIDTH_RAD[region]).toBeCloseTo(widthDeg * DEG, 15);
    const tau = 0.3;
    const peak = oppositionSurge(0, region, tau);
    expect(peak).toBeCloseTo(surgeAmplitude(tau), 15);
    expect(oppositionSurge(widthDeg * DEG, region, tau) - 1).toBeCloseTo((peak - 1) / 2, 14);
    // It is gone by the phase angles the shadowing table is tested at.
    expect(oppositionSurge(6 * DEG, region, tau)).toBeCloseTo(1, 6);
  });

  it("has an amplitude of about 1.45 in sparse rings, falling with τ", () => {
    expect(surgeAmplitude(0)).toBe(SPARSE_SURGE_AMPLITUDE);
    expect(SPARSE_SURGE_AMPLITUDE).toBe(1.45);
    const amplitudes = [0.05, 0.3, 1, 2, 5].map(surgeAmplitude);
    for (let i = 1; i < amplitudes.length; i += 1) {
      expect(amplitudes[i]).toBeLessThan(amplitudes[i - 1] ?? 0);
    }
    expect(oppositionSurge(0, "dusty", 1e-6)).toBe(1);
  });
});

describe("Hapke's shadow-hiding fallback", () => {
  it("doubles at opposition and fades with phase", () => {
    expect(hapkeShadowHiding(0, 0.35, 26 * DEG, 10)).toBeCloseTo(2, 15);
    expect(hapkeShadowHiding(60 * DEG, 0.35, 26 * DEG, 10)).toBeLessThan(1.1);
  });

  it("brightens with elevation at 6° only through D_eff", () => {
    const low = hapkeShadowHiding(6 * DEG, 0.35, 4.5 * DEG, 10);
    const high = hapkeShadowHiding(6 * DEG, 0.35, 26 * DEG, 10);
    expect(high).toBeGreaterThan(low);
    expect(hapkeShadowHiding(6 * DEG, 0, 26 * DEG, 10)).toBe(1);
  });
});
