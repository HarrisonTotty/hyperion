import { describe, expect, it } from "vitest";

import {
  amountValue,
  fourAbsorberMedium,
  r05EarthWithOzoneAbsorber,
} from "../../test/atmosphereFixtures";
import { R05_EARTH_REFERENCE } from "./earth";
import type { AtmosphereMedium } from "./medium";
import {
  absorberAmounts,
  absorberAmountsToTop,
  curvesOpticalDepth,
  distanceToTopM,
  intersectsGround,
  maxDistanceM,
  opticalDepth,
  opticalDepthToTop,
  texelUvFromUnit,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
  unitFromTexelUv,
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
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
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
    const [, green] = opticalDepth(R05_EARTH_REFERENCE, R, R, 1);
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

describe("Bruneton's texel mapping (Design note 19)", () => {
  it("puts 0 and 1 at the first and last texels' centres and inverts itself", () => {
    for (const n of [2, 32, 54, 64, 96, 108, 128, 192, 256]) {
      expect(unitFromTexelUv(0.5 / n, n)).toBeCloseTo(0, 14);
      expect(unitFromTexelUv((n - 0.5) / n, n)).toBeCloseTo(1, 14);
      for (let i = 0; i < n; i += 1) {
        const unit = unitFromTexelUv((i + 0.5) / n, n);
        expect(unit).toBeCloseTo(i / (n - 1), 14);
        expect(texelUvFromUnit(unit, n)).toBeCloseTo((i + 0.5) / n, 14);
      }
    }
  });
});

describe("opticalDepthToTop", () => {
  it("is opticalDepth for a ray that misses the ground", () => {
    const medium = exponential(100_000);
    for (const [rM, mu] of [
      [R, 1],
      [R + 1_000, 0],
      [R + 20_000, -0.05],
    ] as const) {
      expect(opticalDepthToTop(medium, R, rM, mu)).toEqual(opticalDepth(medium, R, rM, mu));
    }
  });

  it("takes the ray tangent to the ground through to the top, the table's last column", () => {
    const medium = exponential(100_000);
    const rM = R + 5_000;
    const mu = -Math.sqrt(1 - (R / rM) ** 2);
    // The tangent ray's grazing path is longer than its part to the tangent point.
    const [whole] = opticalDepthToTop(medium, R, rM, mu);
    const shell = { bottomRadiusM: R, topRadiusM: R + 100_000 };
    const toTangent = Math.sqrt(rM * rM - R * R);
    expect(distanceToTopM(shell, rM, mu)).toBeGreaterThan(2 * toTangent);
    const [half] = opticalDepthToTop(medium, R, rM, -mu);
    expect(whole).toBeGreaterThan(half);
  });
});

describe("the absorbers' columns and curves (R08.T6.c)", () => {
  const { medium, curves } = r05EarthWithOzoneAbsorber();
  const ozone = medium.terms.find((term) => term.absorber !== undefined);
  const columnPerM2 =
    ozone?.absorber?.kind === "column" ? ozone.absorber.spec.columnPerM2 : Number.NaN;
  const withoutOzone = { ...medium, terms: medium.terms.filter((term) => term !== ozone) };

  it("leaves an absorber out of the optical depth, its coefficients 0", () => {
    expect(opticalDepth(medium, R, R + 2, 1)).toEqual(opticalDepth(withoutOzone, R, R + 2, 1));
  });

  it("gives the absorber's whole column from the ground straight up", () => {
    const [ground] = absorberAmounts(medium, R, R, 1);
    expect(ground?.kind).toBe("column");
    expect((ground === undefined ? Number.NaN : amountValue(ground)) / columnPerM2).toBeCloseTo(
      1,
      9,
    );
  });

  it("gives half the column to the top from the tent's peak straight up", () => {
    const [peak] = absorberAmountsToTop(medium, R, R + 25_000, 1);
    expect((peak === undefined ? Number.NaN : amountValue(peak)) / columnPerM2).toBeCloseTo(0.5, 9);
  });

  it("gives a pair absorber its pair column, from the ground straight up its whole (R08.T4.c)", () => {
    const four = fourAbsorberMedium().medium;
    const amounts = absorberAmounts(four, R, R, 1);
    expect(amounts.map((amount) => amount.kind)).toEqual(["column", "column", "pair", "column"]);
    const pair = four.terms.find((term) => term.absorber?.kind === "pair")?.absorber;
    const vertical = pair?.kind === "pair" ? pair.spec.pairColumnPerM5 : Number.NaN;
    const [, , ground] = amounts;
    expect((ground === undefined ? Number.NaN : amountValue(ground)) / vertical).toBeCloseTo(1, 4);
  });

  it("refuses an amount of the other kind than its curve's", () => {
    const [curve] = curves[0] ?? [];
    if (curve === undefined) {
      throw new Error("the medium has its ozone's curve");
    }
    expect(() => curvesOpticalDepth([curve], [{ kind: "pair", pairColumnPerM5: 1 }], [])).toThrow(
      Error,
    );
  });

  it("reads a linear curve as σ u, R05's ozone depth", () => {
    const depth = curvesOpticalDepth(curves[0] ?? [], absorberAmounts(medium, R, R, 1), []);
    const r05 = opticalDepth(R05_EARTH_REFERENCE, R, R, 1);
    const others = opticalDepth(withoutOzone, R, R, 1);
    for (const c of [0, 1, 2] as const) {
      expect(depth[c] / (r05[c] - others[c])).toBeCloseTo(1, 6);
    }
  });

  it("refuses curves that do not match the columns one to one", () => {
    expect(() => curvesOpticalDepth([], [{ kind: "column", columnPerM2: 1 }], [])).toThrow(
      RangeError,
    );
  });
});
