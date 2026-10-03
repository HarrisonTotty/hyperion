import { describe, expect, it } from "vitest";

import { EARTH_REFERENCE } from "./earth";
import type { AtmosphereMedium } from "./medium";
import {
  distanceToTopM,
  intersectsGround,
  maxDistanceM,
  opticalDepth,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
} from "./opticalDepth";

const R = 6_360_000;
const H = 8_000;
const BETA = 1e-5;

/** One exponential term with extinction `BETA` and scale height `H`, its top `topM` up. */
function exponential(topM: number): AtmosphereMedium {
  return {
    name: "test",
    topHeightM: topM,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "gas",
        density: { kind: "exponential", scaleHeightM: H },
        scattering: [BETA, 2 * BETA, 3 * BETA],
        absorption: [0, 0, 0],
        phase: { kind: "rayleigh" },
      },
    ],
  };
}

describe("the oracle against closed forms", () => {
  it("gives β H (1 − e^(−top/H)) at the zenith from the ground", () => {
    const top = 100_000;
    const [tau] = opticalDepth(exponential(top), R, R, 1);
    expect(tau / (BETA * H * -Math.expm1(-top / H))).toBeCloseTo(1, 9);
  });

  it("gives β H Ch(R/H, 90°) along the horizon, the Chapman function's asymptotic series", () => {
    // Ch(x, 90°) = x eˣ K₁(x) ≈ √(πx/2) (1 + 3/(8x) − 15/(128x²)) for large x, with the top far
    // enough (30 scale heights) that the cut is below 10⁻¹².
    const x = R / H;
    const chapman = Math.sqrt((Math.PI * x) / 2) * (1 + 3 / (8 * x) - 15 / (128 * x * x));
    const [tau] = opticalDepth(exponential(30 * H), R, R, 0, 400_000);
    expect(tau / (BETA * H * chapman)).toBeCloseTo(1, 5);
  });

  it("scales each channel by its own coefficient", () => {
    const [a, b, c] = opticalDepth(exponential(100_000), R, R + 1_000, 0.3);
    expect(b / a).toBeCloseTo(2, 12);
    expect(c / a).toBeCloseTo(3, 12);
  });

  it("stops at the ground for a ray that meets it", () => {
    const shell = { bottomRadiusM: R, topRadiusM: R + 100_000 };
    expect(intersectsGround(shell, R + 1_000, -0.5)).toBe(true);
    expect(maxDistanceM(shell, R + 1_000, -0.5)).toBeCloseTo(
      // r μ + √(r²μ² − r² + R²), the near root, with μ < 0
      (R + 1_000) * 0.5 - Math.sqrt((R + 1_000) ** 2 * 0.25 - (R + 1_000) ** 2 + R * R),
      3,
    );
    expect(intersectsGround(shell, R + 1_000, 0.1)).toBe(false);
    expect(maxDistanceM(shell, R + 1_000, 1)).toBeCloseTo(99_000, 6);
    expect(distanceToTopM(shell, R, 1)).toBeCloseTo(100_000, 6);
  });
});

describe("Earth's zenith optical depth", () => {
  it("is the sum of its terms' columns: about 0.22 at 550 nm", () => {
    const [, green] = opticalDepth(EARTH_REFERENCE, R, R, 1);
    // Rayleigh 11.487e-6 × 8,434.5 m (1 − e^(−100 km ÷ H)), aerosol 0.1, ozone 1.881e-6 × 15 km.
    const expected =
      11.487e-6 * 8_434.5 * -Math.expm1(-100_000 / 8_434.5) + 0.1 + 1.881e-6 * 15_000;
    expect(green / expected).toBeCloseTo(1, 6);
  });
});

describe("the transmittance table's parameterisation", () => {
  const shell = { bottomRadiusM: R, topRadiusM: R + 100_000 };

  it("puts the ground at v = 0 and the top at v = 1, straight up at u = 0", () => {
    expect(transmittanceUvToRMu(shell, 0, 0)).toEqual({ rM: R, mu: 1 });
    expect(transmittanceUvToRMu(shell, 0, 1).rM).toBeCloseTo(R + 100_000, 3);
  });

  it("puts the horizon at u = 1 for a viewer on the ground", () => {
    expect(transmittanceUvToRMu(shell, 1, 0).mu).toBeCloseTo(0, 9);
  });

  it("round-trips through its inverse", () => {
    for (const [u, v] of [
      [0.1, 0.2],
      [0.5, 0.5],
      [0.9, 0.75],
    ] as const) {
      const { rM, mu } = transmittanceUvToRMu(shell, u, v);
      const [u2, v2] = transmittanceRMuToUv(shell, rM, mu);
      expect(u2).toBeCloseTo(u, 9);
      expect(v2).toBeCloseTo(v, 9);
    }
  });
});
