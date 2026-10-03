import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import {
  annulusEdges,
  annulusVisibleFraction,
  circleOverlapArea,
  DISC_ANNULI_HIGH,
  DISC_ANNULI_LOW,
  eclipseVisible,
  type LimbDarkenedDisc,
  type Occluder,
} from "./annuli";
import { denseAnnulusVisibleFraction } from "./oracle";

/**
 * Power-2 coefficients of a Sun-like star in V, c and α (Maxted 2018, A&A 616, A39, Fig. 1's
 * solar-type region; a representative pair for the error grid, not a fit).
 */
const SUN_LIKE = { c: 0.71, alpha: 0.6 } as const;

/**
 * A cool star's deeper limb (c 0.8) with a shallower profile (α 0.45), for a second point of the
 * grid; its worst error is the smaller, since the error follows the profile's curvature near the
 * centre more than c.
 */
const COOL = { c: 0.8, alpha: 0.45 } as const;

/** The grid of radius ratios, 0.1 to 30 log-spaced, and 41 separations over each overlap. */
function worstError(c: number, alpha: number, k: number): number {
  const annuli = annulusEdges(c, alpha, k);
  let worst = 0;
  for (let i = 0; i <= 40; i += 1) {
    const ratio = 0.1 * 300 ** (i / 40);
    for (let j = 0; j <= 40; j += 1) {
      const separation = (j / 40) * (1 + ratio);
      const error = Math.abs(
        annulusVisibleFraction(annuli, ratio, separation) -
          denseAnnulusVisibleFraction(c, alpha, ratio, separation),
      );
      worst = Math.max(worst, error);
    }
  }
  return worst;
}

describe("annulusEdges", () => {
  it("spaces K edges uniformly in μ from the centre to the limb", () => {
    const { edges } = annulusEdges(0.7, 0.6, 4);
    expect(edges).toHaveLength(5);
    expect(edges[0]).toBe(0);
    expect(edges[4]).toBe(1);
    expect(edges[2]).toBeCloseTo(Math.sqrt(1 - 0.25), 15);
  });

  it("shares the disc's flux among the annuli, summing to one", () => {
    const { flux } = annulusEdges(0.7, 0.6, 4);
    expect(flux.reduce((sum, value) => sum + value, 0)).toBeCloseTo(1, 14);
    for (const share of flux) {
      expect(share).toBeGreaterThan(0);
    }
  });
});

describe("circleOverlapArea", () => {
  it("is zero when the circles are apart", () => {
    expect(circleOverlapArea(1, 0.5, 1.5)).toBe(0);
  });

  it("is the smaller circle's area when one lies inside the other", () => {
    expect(circleOverlapArea(1, 0.5, 0.2)).toBeCloseTo(Math.PI * 0.25, 15);
    expect(circleOverlapArea(0.5, 1, 0.2)).toBeCloseTo(Math.PI * 0.25, 15);
  });

  it("is symmetric in its two radii", () => {
    expect(circleOverlapArea(1, 0.7, 0.9)).toBeCloseTo(circleOverlapArea(0.7, 1, 0.9), 14);
  });

  it("gives two equal unit circles a radius apart the lens 2π/3 − √3/2", () => {
    expect(circleOverlapArea(1, 1, 1)).toBeCloseTo((2 * Math.PI) / 3 - Math.sqrt(3) / 2, 14);
  });

  it("is continuous where one circle starts to cross the other", () => {
    expect(circleOverlapArea(1, 0.5, 0.5 + 1e-9)).toBeCloseTo(Math.PI * 0.25, 7);
    expect(circleOverlapArea(1, 0.5, 1.5 - 1e-9)).toBeCloseTo(0, 7);
  });
});

describe("the eclipse term against 400 annuli", () => {
  // Design note 6 states 0.62% at K = 4 and 1.5% at K = 2. With flux-exact annuli uniform in μ
  // the worst case is a concentric occulter (about 0.48 stellar radii at K = 4, 0.64 at K = 2),
  // and it grows with the darkening: 0.40% and 1.46% at c 0.5, α 0.5; 0.73% and 2.6% for the
  // Sun-like pair here. The bounds are the measured ones, recorded in the plan's Risks (T6.b).
  it.each([
    [SUN_LIKE, DISC_ANNULI_HIGH, 0.0074],
    [COOL, DISC_ANNULI_HIGH, 0.0062],
    [SUN_LIKE, DISC_ANNULI_LOW, 0.027],
    [COOL, DISC_ANNULI_LOW, 0.023],
  ] as const)("errs by at most the measured bound for %o at K = %i", (law, k, bound) => {
    expect(worstError(law.c, law.alpha, k)).toBeLessThanOrEqual(bound);
  });

  it.each([0.2, 0.5, 0.9])(
    "matches the concentric closed form for an occulter of %f radii",
    (k) => {
      // Visible flux of a concentric occultation: [(1 − c) μ_k² + 2c μ_k^(α+2) ÷ (α + 2)] ÷
      // [(1 − c) + 2c ÷ (α + 2)], μ_k = √(1 − k²); the dense sum meets it to 10⁻⁴ and the K = 4 term
      // within its measured bound.
      const { c, alpha } = SUN_LIKE;
      const muK = Math.sqrt(1 - k * k);
      const closed =
        ((1 - c) * muK ** 2 + (2 * c * muK ** (alpha + 2)) / (alpha + 2)) /
        (1 - c + (2 * c) / (alpha + 2));
      expect(Math.abs(denseAnnulusVisibleFraction(c, alpha, k, 0) - closed)).toBeLessThan(1e-4);
      expect(
        Math.abs(annulusVisibleFraction(annulusEdges(c, alpha, 4), k, 0) - closed),
      ).toBeLessThanOrEqual(0.0074);
    },
  );

  it("leaves exactly nothing in a total eclipse", () => {
    const annuli = annulusEdges(SUN_LIKE.c, SUN_LIKE.alpha, 4);
    expect(annulusVisibleFraction(annuli, 1.5, 0.3)).toBe(0);
    expect(annulusVisibleFraction(annuli, 1, 0)).toBe(0);
  });

  it("leaves everything when the occulter misses", () => {
    const annuli = annulusEdges(SUN_LIKE.c, SUN_LIKE.alpha, 4);
    expect(annulusVisibleFraction(annuli, 0.3, 1.3)).toBe(1);
  });
});

/** A 100 km body 3.8 × 10⁸ m sunward of the lit point, offset across the line of sight. */
function transitAt(offsetM: number): Occluder {
  return { centreM: vec3(1.496e11 - 3.8e8, offsetM, 0), radiusM: 1e5 };
}

describe("eclipseVisible", () => {
  const sun: LimbDarkenedDisc = {
    centreM: vec3(0, 0, 0),
    radiusM: 6.957e8,
    limbC: SUN_LIKE.c,
    limbAlpha: SUN_LIKE.alpha,
  };
  const point = vec3(1.496e11, 0, 0);

  it("is one with no occluders", () => {
    expect(eclipseVisible(sun, point, [], 4)).toBe(1);
  });

  it("ignores an occluder beyond the star", () => {
    expect(eclipseVisible(sun, point, [{ centreM: vec3(-1e10, 0, 0), radiusM: 7e7 }], 4)).toBe(1);
  });

  it("is zero for a point inside an occluder", () => {
    expect(eclipseVisible(sun, point, [{ centreM: point, radiusM: 1e6 }], 4)).toBe(0);
  });

  it("adds the losses of separate transits", () => {
    const one = transitAt(4e5);
    const other = transitAt(-4e5);
    const lossOne = 1 - eclipseVisible(sun, point, [one], 4);
    const lossOther = 1 - eclipseVisible(sun, point, [other], 4);
    expect(eclipseVisible(sun, point, [one, other], 4)).toBeCloseTo(1 - lossOne - lossOther, 14);
  });

  it("is zero inside a body's umbra", () => {
    // A Moon-sized body 3.8 × 10⁸ m sunward: its disc (0.262°) covers the Sun's (0.267°) only
    // nearly; a larger one covers it.
    const covering = { centreM: vec3(1.496e11 - 3.8e8, 0, 0), radiusM: 2e6 };
    expect(eclipseVisible(sun, point, [covering], 4)).toBe(0);
  });

  it("dims by the transit of a small body", () => {
    const transit = { centreM: vec3(1.496e11 - 3.8e8, 0, 0), radiusM: 1e5 };
    const visible = eclipseVisible(sun, point, [transit], 4);
    expect(visible).toBeLessThan(1);
    expect(visible).toBeGreaterThan(0.99);
  });
});
