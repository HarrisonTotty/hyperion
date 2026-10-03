import { describe, expect, it } from "vitest";

import { ev100FromAverageLuminance, exposureScale } from "../photometry/exposure";
import { HALF_FLOAT_MAX } from "../photometry/toneCurve";
import {
  BLOOM_LEVELS,
  BLOOM_THRESHOLD_EXPOSED,
  bloomChain,
  bloomDown,
  bloomEnergy,
  bloomExcess,
  bloomImage,
  bloomKernel,
  bloomThreshold,
  bloomUpTent,
  encircledEnergy,
  FIT_RADII_PX,
  levelWeight,
  roundToHalf,
  spreadPixelEE,
} from "./bloom";
import { glareSourceVeil, sphereIntegral, type GlareSource } from "./glare";

const EYE = { ageYears: 25, pigmentation: 0.5 };
const RAD_PER_PX_1080P = (60 * Math.PI) / 180 / 1920;

describe("the threshold", () => {
  it("sits at AgX's top of range, about 157 times the metered average", () => {
    const average = 3.7;
    const scale = exposureScale(ev100FromAverageLuminance(average));
    expect(BLOOM_THRESHOLD_EXPOSED).toBeCloseTo(16.29, 2);
    expect(bloomThreshold(scale, scale)).toBeCloseTo(BLOOM_THRESHOLD_EXPOSED, 12);
    expect(bloomThreshold(scale, 1) / average).toBeCloseTo(156.4, 0);
  });

  it("blooms only the light above it", () => {
    expect(bloomExcess(10, 16)).toBe(0);
    expect(bloomExcess(100, 16)).toBe(84);
  });
});

describe("roundToHalf", () => {
  it("keeps representable values", () => {
    expect(roundToHalf(1, "toward-zero")).toBe(1);
    expect(roundToHalf(0, "nearest")).toBe(0);
    expect(roundToHalf(2 ** -20, "nearest")).toBe(2 ** -20);
  });

  it("truncates toward zero or rounds to the nearest even half float", () => {
    const ulp = 2 ** -10;
    expect(roundToHalf(1 + 0.75 * ulp, "toward-zero")).toBe(1);
    expect(roundToHalf(1 + 0.75 * ulp, "nearest")).toBe(1 + ulp);
    expect(roundToHalf(1 + 0.5 * ulp, "nearest")).toBe(1);
    expect(roundToHalf(1 + 1.5 * ulp, "nearest")).toBe(1 + 2 * ulp);
    expect(roundToHalf(1 + 0.75 * ulp, "unknown")).toBe(1);
  });

  it("clamps to the format's largest value", () => {
    expect(roundToHalf(1e9, "nearest")).toBe(HALF_FLOAT_MAX);
    expect(roundToHalf(1e9, "toward-zero")).toBe(HALF_FLOAT_MAX);
  });
});

describe("the chain's passes", () => {
  it("conserve energy, the downsample over four texels a level", () => {
    const image = bloomImage(32, 32);
    image.values[13 * 32 + 17] = 1;
    image.values[18 * 32 + 14] = 2;
    const down = bloomDown(image);
    expect(down.widthPx).toBe(16);
    expect(4 * bloomEnergy(down)).toBeCloseTo(3, 12);
    const up = bloomUpTent(down, 32, 32);
    expect(bloomEnergy(up)).toBeCloseTo(3, 12);
  });
});

describe("bloomKernel", () => {
  it("has non-negative weights that sum to one at both settings", () => {
    for (const setting of ["high", "low"] as const) {
      for (const role of ["eye", "camera"] as const) {
        const kernel = bloomKernel(setting, role, RAD_PER_PX_1080P, EYE);
        expect(kernel.weights.length).toBe(kernel.levels);
        let sum = 0;
        for (const w of kernel.weights) {
          expect(w).toBeGreaterThanOrEqual(0);
          sum += w;
        }
        expect(sum).toBeCloseTo(1, 6);
      }
    }
  });

  it("uses fewer levels at quarter resolution on the low setting", () => {
    const high = bloomKernel("high", "eye", RAD_PER_PX_1080P, EYE);
    const low = bloomKernel("low", "eye", 2 * RAD_PER_PX_1080P, EYE);
    expect(high.firstLevel).toBe(0);
    expect(low.firstLevel).toBe(1);
    expect(low.levels).toBeLessThan(high.levels);
    expect(levelWeight(low, 0)).toBe(0);
    expect(BLOOM_LEVELS.low.levels).toBe(low.levels);
  });

  it("matches the spread function's encircled energy to 10% at 1, 4, 16 and 64 px", () => {
    const radii = [1, 4, 16, 64];
    for (const role of ["eye", "camera"] as const) {
      const kernel = bloomKernel("high", role, RAD_PER_PX_1080P, EYE);
      const side = 768;
      // Off the coarse levels' alignment, so the fit is checked away from the phase it was made at.
      const ci = 381;
      const cj = 386;
      const impulse = bloomImage(side, side);
      impulse.values[cj * side + ci] = 1;
      const response = bloomChain(impulse, kernel, null);
      const chain = encircledEnergy(response, ci, cj, radii);
      const target = spreadPixelEE(role, RAD_PER_PX_1080P, EYE);
      radii.forEach((r, k) => {
        const expected = target[FIT_RADII_PX.indexOf(r)] ?? Number.NaN;
        expect(Math.abs((chain[k] ?? Number.NaN) / expected - 1)).toBeLessThan(0.1);
      });
    }
  });
});

describe("stored and injected glare", () => {
  // 2 × 10⁸: the injected veil carries nearly all of it; 1.5 × 10⁵: the stored part, bloomed
  // through the chain, carries more than a third.
  it.each([2e8, 1.5e5])("together hold the unclamped energy to 1% (L = %d)", (trueLuminance) => {
    const side = 768;
    const centre = side / 2;
    const radiusPx = 10;
    // Pre-exposed by this frame's own exposure, so the threshold is AgX's top of range.
    const threshold = bloomThreshold(1, 1);
    const stored = bloomImage(side, side);
    const shown = bloomImage(side, side);
    let discPixels = 0;
    for (let j = 0; j < side; j += 1) {
      for (let i = 0; i < side; i += 1) {
        if (Math.hypot(i - centre, j - centre) <= radiusPx) {
          const value = Math.min(trueLuminance, HALF_FLOAT_MAX);
          stored.values[j * side + i] = bloomExcess(value, threshold);
          shown.values[j * side + i] = Math.min(value, threshold);
          discPixels += 1;
        }
      }
    }
    const unclamped = discPixels * trueLuminance;
    const source: GlareSource = {
      direction: { x: 0, y: 0, z: -1 },
      angularRadiusRad: Math.sqrt(discPixels / Math.PI) * RAD_PER_PX_1080P,
      excessLuminance: [trueLuminance - HALF_FLOAT_MAX, 0, 0],
    };
    for (const role of ["eye", "camera"] as const) {
      const kernel = bloomKernel("high", role, RAD_PER_PX_1080P, EYE);
      const bloomed = bloomChain(stored, kernel, "toward-zero");
      const injectedSrSum = sphereIntegral((t) => glareSourceVeil(source, t, role, EYE)[0]);
      const injected = injectedSrSum / (RAD_PER_PX_1080P * RAD_PER_PX_1080P);
      const total = bloomEnergy(shown) + bloomEnergy(bloomed) + injected;
      expect(Math.abs(total / unclamped - 1)).toBeLessThan(0.01);
    }
  });

  it("keep the stored part's energy through the chain, less the truncation", () => {
    const side = 256;
    const excess = bloomImage(side, side);
    for (let k = 0; k < 20; k += 1) {
      excess.values[(100 + k) * side + 120 + k] = 5000 + 37 * k;
    }
    const kernel = bloomKernel("high", "eye", RAD_PER_PX_1080P, EYE);
    const exact = bloomEnergy(bloomChain(excess, kernel, null));
    const truncated = bloomEnergy(bloomChain(excess, kernel, "toward-zero"));
    expect(exact / bloomEnergy(excess)).toBeCloseTo(1, 2);
    expect(truncated).toBeLessThan(exact);
    expect(truncated / exact).toBeGreaterThan(0.99);
  });
});
