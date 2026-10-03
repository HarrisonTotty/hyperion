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
import { denseAnnulusVisibleFraction, eclipseIntegralVisibleFraction } from "./oracle";

/** A power-2 law's c and α. */
interface Power2 {
  readonly c: number;
  readonly alpha: number;
}

/**
 * The Sun's power-2 laws per band, Maxted 2018, A&A 616, A39, Table 2 (CDS J/A+A/616/A39) at
 * 5,750 K, log g 4.5, [Fe/H] 0 (decision-r07-dn6).
 */
const SUN_B: Power2 = { c: 0.846, alpha: 0.83 };
// Maxted's V-band α, 0.707, only happens to resemble 1 ÷ √2.
// oxlint-disable-next-line approx-constant
const SUN_V: Power2 = { c: 0.771, alpha: 0.707 };
const SUN_R: Power2 = { c: 0.712, alpha: 0.625 };

/** Two generic laws of the ruling's table: a solar-type R-band pair and a mild one. */
const GENERIC: Power2 = { c: 0.71, alpha: 0.6 };
const MILD: Power2 = { c: 0.5, alpha: 0.5 };

/** The grid's radius ratios, 0.1 to 30 log-spaced, and 41 separations over each overlap. */
function grid(): Array<readonly [number, number]> {
  const points: Array<readonly [number, number]> = [];
  for (let i = 0; i <= 40; i += 1) {
    const ratio = 0.1 * 300 ** (i / 40);
    for (let j = 0; j <= 40; j += 1) {
      points.push([ratio, (j / 40) * (1 + ratio)]);
    }
  }
  return points;
}

/** The term's worst absolute error over the grid against the exact integral. */
function worstError(law: Power2, k: number): number {
  const annuli = annulusEdges(law.c, law.alpha, k);
  let worst = 0;
  for (const [ratio, separation] of grid()) {
    const error = Math.abs(
      annulusVisibleFraction(annuli, ratio, separation) -
        eclipseIntegralVisibleFraction(law.c, law.alpha, ratio, separation),
    );
    worst = Math.max(worst, error);
  }
  return worst;
}

describe("annulusEdges", () => {
  it("runs K edges from the centre to the limb, rising", () => {
    const { edges } = annulusEdges(SUN_V.c, SUN_V.alpha, 4);
    expect(edges).toHaveLength(5);
    expect(edges[0]).toBe(0);
    expect(edges[4]).toBe(1);
    for (let j = 1; j <= 4; j += 1) {
      expect(edges[j] ?? 0).toBeGreaterThan(edges[j - 1] ?? 1);
    }
  });

  it("places the Sun's V-band edges at μ 0.813, 0.609 and 0.375", () => {
    const { edges } = annulusEdges(SUN_V.c, SUN_V.alpha, 4);
    const mu = [1, 2, 3].map((j) => Math.sqrt(1 - (edges[j] ?? 0) ** 2));
    expect(mu[0]).toBeCloseTo(0.813, 2);
    expect(mu[1]).toBeCloseTo(0.609, 2);
    expect(mu[2]).toBeCloseTo(0.375, 2);
  });

  it("shares the disc's flux among the annuli, summing to one", () => {
    const { flux } = annulusEdges(SUN_V.c, SUN_V.alpha, 4);
    expect(flux.reduce((sum, value) => sum + value, 0)).toBeCloseTo(1, 14);
    for (const share of flux) {
      expect(share).toBeGreaterThan(0);
    }
  });

  it.each([
    [SUN_B, 4],
    [SUN_V, 2],
    [MILD, 4],
  ] as const)(
    "predicts the grid's worst error by its equal dip, to 3 × 10⁻⁴ (%o, K %i)",
    (law, k) => {
      const { dip } = annulusEdges(law.c, law.alpha, k);
      expect(Math.abs(worstError(law, k) - dip)).toBeLessThanOrEqual(3e-4);
    },
  );
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

describe("the eclipse term against the exact integral", () => {
  // decision-r07-dn6's bounds, about 5% over its measured worst: 0.699, 0.556 and 0.462% for the
  // Sun's B, V and R at K = 4; 2.696, 2.130 and 1.767% at K = 2.
  it.each([
    [SUN_B, DISC_ANNULI_HIGH, 0.0073],
    [SUN_V, DISC_ANNULI_HIGH, 0.0058],
    [SUN_R, DISC_ANNULI_HIGH, 0.0048],
    [GENERIC, DISC_ANNULI_HIGH, 0.0047],
    [MILD, DISC_ANNULI_HIGH, 0.0028],
    [SUN_B, DISC_ANNULI_LOW, 0.028],
    [SUN_V, DISC_ANNULI_LOW, 0.022],
    [SUN_R, DISC_ANNULI_LOW, 0.0185],
    [GENERIC, DISC_ANNULI_LOW, 0.0178],
    [MILD, DISC_ANNULI_LOW, 0.0105],
  ] as const)("errs by at most its bound for %o at K = %i", (law, k, bound) => {
    expect(worstError(law, k)).toBeLessThanOrEqual(bound);
  });

  it("is met by the 400-annulus sum to 10⁻⁵", () => {
    let worst = 0;
    for (const [ratio, separation] of grid().filter((_, index) => index % 7 === 0)) {
      worst = Math.max(
        worst,
        Math.abs(
          denseAnnulusVisibleFraction(SUN_V.c, SUN_V.alpha, ratio, separation) -
            eclipseIntegralVisibleFraction(SUN_V.c, SUN_V.alpha, ratio, separation),
        ),
      );
    }
    expect(worst).toBeLessThan(1e-5);
  });

  it.each([0.2, 0.5, 0.9])(
    "matches the concentric closed form for an occulter of %f radii",
    (k) => {
      // Visible flux of a concentric occultation: [(1 − c) μ_k² + 2c μ_k^(α+2) ÷ (α + 2)] ÷
      // [(1 − c) + 2c ÷ (α + 2)], μ_k = √(1 − k²).
      const { c, alpha } = SUN_V;
      const muK = Math.sqrt(1 - k * k);
      const closed =
        ((1 - c) * muK ** 2 + (2 * c * muK ** (alpha + 2)) / (alpha + 2)) /
        (1 - c + (2 * c) / (alpha + 2));
      expect(Math.abs(eclipseIntegralVisibleFraction(c, alpha, k, 0) - closed)).toBeLessThan(1e-5);
      expect(
        Math.abs(annulusVisibleFraction(annulusEdges(c, alpha, 4), k, 0) - closed),
      ).toBeLessThanOrEqual(0.0058);
    },
  );

  it("leaves exactly nothing in a total eclipse", () => {
    const annuli = annulusEdges(SUN_V.c, SUN_V.alpha, 4);
    expect(annulusVisibleFraction(annuli, 1.5, 0.3)).toBe(0);
    expect(annulusVisibleFraction(annuli, 1, 0)).toBe(0);
  });

  it("leaves everything when the occulter misses", () => {
    const annuli = annulusEdges(SUN_V.c, SUN_V.alpha, 4);
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
    limbC: SUN_V.c,
    limbAlpha: SUN_V.alpha,
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
