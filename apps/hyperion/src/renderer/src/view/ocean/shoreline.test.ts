import { describe, expect, it } from "vitest";

import {
  BREAKER_INDEX,
  setRmsHeightM,
  SHORE_FADE_MIN_M,
  shoreAmplitudeScale,
  shoreFade,
  shoreFoamShare,
  shorePixelReachM,
  SURF_HEIGHT_RATIO,
  SURF_SIMILARITY_BOUNDS,
  surfSimilarity,
} from "./shoreline";
import shorelineWgsl from "./shoreline.wgsl?raw";

/** A sea of H_rms 2 m and L₀ 100 m: its steepness H ÷ L₀ 0.02, √ of it ≈ 0.1414. */
const H = 2;
const L0 = 100;

/** The slope whose surf similarity over that sea is `xi`. */
function slopeFor(xi: number): number {
  return xi * Math.sqrt(H / L0);
}

describe("a set's root-mean-square height (R11.T8.d)", () => {
  it("is 2 √(Σ A²), the Rayleigh sea's √8 σ_η", () => {
    // σ_η² = Σ A² ÷ 2 = 0.125 m², H_rms = √8 σ_η = 1 m.
    expect(setRmsHeightM({ components: [{ amplitudeM: 0.3 }, { amplitudeM: 0.4 }] })).toBeCloseTo(
      1,
      12,
    );
    expect(setRmsHeightM({ components: [] })).toBe(0);
  });
});

describe("the waves' amplitude by depth (R11.T8.d)", () => {
  it("keeps H_rms within 0.42 d (Thornton and Guza 1983), whole in deep water and flat at none", () => {
    expect(SURF_HEIGHT_RATIO).toBe(0.42);
    expect(shoreAmplitudeScale(0, H)).toBe(0);
    expect(shoreAmplitudeScale(-1, H)).toBe(0);
    expect(shoreAmplitudeScale(H / 0.42 / 2, H)).toBeCloseTo(0.5, 12);
    expect(shoreAmplitudeScale(H / 0.42, H)).toBeCloseTo(1, 12);
    expect(shoreAmplitudeScale(1e4, H)).toBe(1);
  });

  it("leaves a calm sea alone", () => {
    expect(shoreAmplitudeScale(0, 0)).toBe(1);
  });
});

describe("the surf zone's foam (R11.T8.d)", () => {
  it("forms the surf similarity tan β ÷ √(H ÷ L₀) (Battjes 1974)", () => {
    expect(surfSimilarity(0.1, H, L0)).toBeCloseTo(0.1 / Math.sqrt(0.02), 12);
    expect(surfSimilarity(-0.1, H, L0)).toBeCloseTo(0.1 / Math.sqrt(0.02), 12);
    expect(surfSimilarity(0.1, 0, L0)).toBe(Number.POSITIVE_INFINITY);
  });

  it("breaks the offshore Rayleigh share above γ d, γ = 0.73 (Baldock et al. 1998; Battjes and Stive 1985)", () => {
    expect(BREAKER_INDEX).toBe(0.73);
    const flat = slopeFor(0.2);
    expect(shoreFoamShare(0, flat, H, L0)).toBe(1);
    expect(shoreFoamShare(H / 0.73, flat, H, L0)).toBeCloseTo(Math.exp(-1), 12);
    expect(shoreFoamShare(10 * H, flat, H, L0)).toBeLessThan(1e-20);
  });

  it("is whole on a spilling beach, none on a surging one, and half between at ξ = 1.9", () => {
    const [spilling, surging] = SURF_SIMILARITY_BOUNDS;
    expect([spilling, surging]).toEqual([0.5, 3.3]);
    expect(shoreFoamShare(0, slopeFor(spilling), H, L0)).toBeCloseTo(1, 12);
    expect(shoreFoamShare(0, slopeFor(0.5 * (spilling + surging)), H, L0)).toBeCloseTo(0.5, 12);
    expect(shoreFoamShare(0, slopeFor(surging), H, L0)).toBeCloseTo(0, 12);
    expect(shoreFoamShare(0, slopeFor(10), H, L0)).toBe(0);
  });

  it("is none for a calm sea", () => {
    expect(shoreFoamShare(0, 0, 0, L0)).toBe(0);
  });
});

describe("a pixel's reach along the bed's slope (R11.T8.d)", () => {
  it("is the footprint t δ across the view and t δ ÷ μ_v along it", () => {
    // 100 m away, 1 mrad, the view 30° below the horizon: 0.1 m across, 0.2 m along.
    expect(shorePixelReachM(100, 1e-3, 0.5, 0, 0.01)).toBeCloseTo(0.1, 12);
    expect(shorePixelReachM(100, 1e-3, 0.5, 1, 0.01)).toBeCloseTo(0.2, 12);
    expect(shorePixelReachM(100, 1e-3, -0.5, 1, 0.01)).toBeCloseTo(0.2, 12);
    // At 45° to the view, the ellipse's extent there.
    const h = Math.SQRT1_2;
    expect(shorePixelReachM(100, 1e-3, 0.5, h, 0.01)).toBeCloseTo(0.1 * Math.hypot(2 * h, h), 12);
  });

  it("holds the view's cosine at its least", () => {
    expect(shorePixelReachM(100, 1e-3, 0, 1, 0.01)).toBeCloseTo(10, 12);
  });
});

describe("the depth fade at the waterline (R11.T8.d)", () => {
  it("is a smoothstep to the depth a pixel spans on the beach, ℓ tan β", () => {
    // A pixel 0.5 m across on a 1 in 5 beach spans 0.1 m of depth.
    expect(shoreFade(0, 0.5, 0.2)).toBe(0);
    expect(shoreFade(0.05, 0.5, 0.2)).toBeCloseTo(0.5, 12);
    expect(shoreFade(0.1, 0.5, 0.2)).toBe(1);
    expect(shoreFade(1, 0.5, 0.2)).toBe(1);
  });

  it("spans at least 5 cm, a film the bed shows through, on a flat bed or under a fine pixel", () => {
    expect(SHORE_FADE_MIN_M).toBe(0.05);
    expect(shoreFade(0.025, 0.01, 0)).toBeCloseTo(0.5, 12);
    expect(shoreFade(0.025, 0.01, 0.2)).toBeCloseTo(0.5, 12);
  });

  it("rises without a step across the waterline's pixels", () => {
    let last = 0;
    for (let d = 0; d <= 0.1; d += 0.005) {
      const alpha = shoreFade(d, 0.5, 0.2);
      expect(alpha).toBeGreaterThanOrEqual(last);
      expect(alpha - last).toBeLessThan(0.12);
      last = alpha;
    }
  });
});

describe("the shoreline's shader, against its twin's figures (R11.T8.d)", () => {
  it("holds the twin's height ratio, breaker index, similarity bounds and least fade", () => {
    const [spilling, surging] = SURF_SIMILARITY_BOUNDS;
    expect(shorelineWgsl).toContain(
      `const SHORE_SURF_HEIGHT_RATIO : f32 = ${String(SURF_HEIGHT_RATIO)};`,
    );
    expect(shorelineWgsl).toContain(`const SHORE_BREAKER_INDEX : f32 = ${String(BREAKER_INDEX)};`);
    expect(shorelineWgsl).toContain(`const SHORE_SPILLING : f32 = ${String(spilling)};`);
    expect(shorelineWgsl).toContain(`const SHORE_SURGING : f32 = ${String(surging)};`);
    expect(shorelineWgsl).toContain(`const SHORE_FADE_MIN : f32 = ${String(SHORE_FADE_MIN_M)};`);
  });
});
