import { describe, expect, it } from "vitest";

import {
  advected,
  CLOUD_OCTAVES_MAX,
  cloudFieldAt,
  type CloudFieldInput,
  cloudFieldParams,
  cloudRotationOf,
  cloudSeedOf,
  coverageTexels,
  normalDensity,
  normalTail,
  sampleCoverage,
  thresholdOf,
} from "./cloudDensity";
import { cloudNoiseTexels } from "./cloudNoise";
import { type CoverageGrid, FLOW_MAP_PERIOD_S } from "./coverage";

const NOISE = cloudNoiseTexels();
const EARTH_M = 6_378_137;
const SHELL_M = EARTH_M + 1_500;

/** A coverage grid of one fraction everywhere. */
function uniformGrid(fraction: number, widthCells = 64, heightCells = 32): CoverageGrid {
  return {
    widthCells,
    heightCells,
    fractions: new Float64Array(widthCells * heightCells).fill(fraction),
    offset: 0,
  };
}

const CALM = { eastMps: 0, northMps: 0 } as const;

/** A body's field input at a time, every octave drawn. */
function input(seconds: number, nanos = 0, rate = 1): CloudFieldInput {
  return {
    seed: cloudSeedOf("0200080020000000.0103"),
    condensate: { inCloudOpticalDepth550: 8 },
    radiusM: EARTH_M,
    time: { seconds, nanos },
    rate,
    windMps: 20,
    octaves: CLOUD_OCTAVES_MAX,
  };
}

/** A point on the shell at a geocentric latitude and longitude, degrees. */
function onShell(latitudeDeg: number, longitudeDeg: number): { x: number; y: number; z: number } {
  const phi = (latitudeDeg * Math.PI) / 180;
  const lambda = (longitudeDeg * Math.PI) / 180;
  return {
    x: SHELL_M * Math.cos(phi) * Math.cos(lambda),
    y: SHELL_M * Math.cos(phi) * Math.sin(lambda),
    z: SHELL_M * Math.sin(phi),
  };
}

/** Point i of n spread evenly over the sphere (a Fibonacci lattice), on the shell. */
function fibonacciPoint(i: number, n: number): { x: number; y: number; z: number } {
  const z = 1 - (2 * (i + 0.5)) / n;
  const r = Math.sqrt(1 - z * z);
  const angle = i * Math.PI * (3 - Math.sqrt(5));
  return { x: SHELL_M * r * Math.cos(angle), y: SHELL_M * r * Math.sin(angle), z: SHELL_M * z };
}

describe("the normal's tail and threshold", () => {
  it("meets Q at known points to the erfc's stated precision", () => {
    expect(normalTail(0)).toBeCloseTo(0.5, 7);
    expect(normalTail(1)).toBeCloseTo(0.158_655_254, 7);
    expect(normalTail(2)).toBeCloseTo(0.022_750_132, 8);
    expect(normalTail(-1)).toBeCloseTo(0.841_344_746, 7);
    expect(normalDensity(0)).toBeCloseTo(0.398_942_280, 9);
  });

  it("covers the coverage's share above its threshold, with τ's mean over it scaled to 1", () => {
    for (const coverage of [0.05, 0.3, 0.67, 0.95]) {
      const { threshold, scale } = thresholdOf(coverage);
      expect(normalTail(threshold)).toBeCloseTo(coverage, 6);
      // ∫ N (z − z_c) φ(z) dz over z > z_c, ÷ C: the in-cloud τ's mean in units of τ̄.
      let integral = 0;
      const dz = 1e-3;
      for (let z = threshold + dz / 2; z < 9; z += dz) {
        integral += scale * (z - threshold) * normalDensity(z) * dz;
      }
      expect(integral / coverage).toBeCloseTo(1, 4);
    }
  });

  it("holds a clear sky and an overcast at its limits", () => {
    expect(thresholdOf(0).threshold).toBe(6);
    expect(thresholdOf(1).threshold).toBe(-6);
    expect(Number.isFinite(thresholdOf(0).scale)).toBe(true);
  });
});

describe("the coverage texture", () => {
  it("is bilinear between cell centres and repeats around a parallel", () => {
    const grid = uniformGrid(0.4, 8, 4);
    const fractions = Float64Array.from(grid.fractions, (_, k) => 0.1 + 0.8 * ((k % 8) / 7));
    const texels = coverageTexels({ ...grid, fractions }, () => CALM);
    const at = (lon: number): number => sampleCoverage(texels, onShell(0, lon)).threshold;
    // Cell 0's centre is at −157.5°, cell 7's at 157.5°: −180° is half-way between them.
    const half = 0.5 * (thresholdOf(0.1).threshold + thresholdOf(0.9).threshold);
    expect(at(-180)).toBeCloseTo(half, 6);
    expect(at(-157.5)).toBeCloseTo(
      0.5 * (thresholdOf(0.1).threshold + thresholdOf(0.1).threshold),
      6,
    );
  });
});

describe("advected", () => {
  it("turns a point about the pole under an eastward wind, keeping its height and latitude", () => {
    const point = onShell(30, 10);
    const moved = advected(point, 25, 0, 3_600);
    expect(Math.hypot(moved.x, moved.y, moved.z)).toBeCloseTo(SHELL_M, 3);
    expect(moved.z).toBe(point.z);
    const rho = Math.hypot(point.x, point.y);
    const turned = Math.atan2(moved.y, moved.x) - Math.atan2(point.y, point.x);
    expect(turned).toBeCloseTo((-25 * 3_600) / rho, 12);
  });

  it("takes the structure from the south under a northward wind", () => {
    const point = onShell(20, 0);
    const moved = advected(point, 0, 10, 3_600);
    expect(Math.hypot(moved.x, moved.y, moved.z)).toBeCloseTo(SHELL_M, 3);
    const latitude = Math.asin(moved.z / SHELL_M);
    expect(latitude).toBeCloseTo((20 * Math.PI) / 180 - (10 * 3_600) / SHELL_M, 10);
  });
});

describe("the body's rotation and parameters", () => {
  it("is a proper rotation", () => {
    const m = cloudRotationOf(cloudSeedOf("0200080020000000.0103"));
    for (let i = 0; i < 3; i += 1) {
      for (let j = 0; j < 3; j += 1) {
        let dot = 0;
        for (let k = 0; k < 3; k += 1) {
          dot += (m[3 * i + k] ?? 0) * (m[3 * j + k] ?? 0);
        }
        expect(dot).toBeCloseTo(i === j ? 1 : 0, 12);
      }
    }
    const [a = 0, b = 0, c = 0, d = 0, e = 0, f = 0, g = 0, h = 0, k = 0] = m;
    expect(a * (e * k - f * h) - b * (d * k - f * g) + c * (d * h - e * g)).toBeCloseTo(1, 12);
  });

  it("refuses an octave count outside 1 to 12", () => {
    expect(() => cloudFieldParams({ ...input(0), octaves: 0 })).toThrow(RangeError);
    expect(() => cloudFieldParams({ ...input(0), octaves: 13 })).toThrow(RangeError);
  });

  it("is a function of its input alone, its phases' weights summing to 1", () => {
    const a = cloudFieldParams(input(98_765, 4_321));
    expect(cloudFieldParams(input(98_765, 4_321))).toEqual(a);
    expect(a.phaseWeights[0] + a.phaseWeights[1]).toBeCloseTo(1, 12);
    expect(a.offsets.every((o) => o >= 0 && o < 1)).toBe(true);
  });
});

describe("the field", () => {
  const coverage = coverageTexels(uniformGrid(0.4), () => CALM);
  const params = cloudFieldParams(input(123_456, 500_000_000));

  it("is the calibrated mean over a footprint that resolves nothing", () => {
    const sample = cloudFieldAt(coverage, NOISE, params, onShell(12, 34), 1e9);
    expect(sample.unresolvedSd).toBe(1);
    expect(sample.fraction).toBeCloseTo(0.4, 6);
    expect(sample.inCloudTau).toBeCloseTo(8, 4);
  });

  it("is the calibrated mean under a time warp that fades every octave", () => {
    const warped = cloudFieldParams(input(123_456, 0, 1e9));
    expect(warped.octaveFades.every((fade) => fade === 0)).toBe(true);
    const sample = cloudFieldAt(coverage, NOISE, warped, onShell(-40, 100), 5_000);
    expect(sample.fraction).toBeCloseTo(0.4, 6);
    expect(sample.inCloudTau).toBeCloseTo(8, 4);
  });

  it("covers the coverage's share of the sphere and keeps τ̄ in cloud where it resolves the structure", () => {
    const points = 6_000;
    let covered = 0;
    let tau = 0;
    for (let i = 0; i < points; i += 1) {
      const sample = cloudFieldAt(coverage, NOISE, params, fibonacciPoint(i, points), 30_000);
      covered += sample.fraction;
      tau += sample.meanTau;
    }
    expect(covered / points).toBeGreaterThan(0.35);
    expect(covered / points).toBeLessThan(0.45);
    expect(tau / covered).toBeGreaterThan(7);
    expect(tau / covered).toBeLessThan(9);
  });

  it("leaves octaves past its cap unresolved, as the GPU's cheaper readers draw it", () => {
    const point = onShell(5, -60);
    const full = cloudFieldAt(coverage, NOISE, params, point, 500);
    const capped = cloudFieldAt(coverage, NOISE, params, point, 500, 4);
    expect(capped.unresolvedSd).toBeGreaterThan(full.unresolvedSd);
    // Four octaves of a field of four resolve it all; four of twelve leave the rest's variance.
    const fourOctaves = cloudFieldAt(coverage, NOISE, { ...params, octaves: 4 }, point, 500);
    expect(fourOctaves.unresolvedSd).toBe(1e-3);
    const drawn = (1 - 0.49 ** 4) / (1 - 0.49 ** 12);
    expect(capped.unresolvedSd).toBeCloseTo(Math.sqrt(1 - drawn), 9);
  });

  it("is continuous across a flow-map phase's restart", () => {
    const restart = FLOW_MAP_PERIOD_S / 2;
    const before = cloudFieldParams(input(restart - 1, 999_999_999));
    const after = cloudFieldParams(input(restart, 0));
    for (const [lat, lon] of [
      [10, 20],
      [-45, 170],
    ] as const) {
      const a = cloudFieldAt(coverage, NOISE, before, onShell(lat, lon), 2_000);
      const b = cloudFieldAt(coverage, NOISE, after, onShell(lat, lon), 2_000);
      expect(Math.abs(a.resolved - b.resolved)).toBeLessThan(1e-4);
    }
  });

  it("gives two clients the same field bit for bit", () => {
    const point = onShell(33, -77);
    const again = cloudFieldParams(input(123_456, 500_000_000));
    expect(cloudFieldAt(coverage, NOISE, again, point, 1_000)).toEqual(
      cloudFieldAt(coverage, NOISE, params, point, 1_000),
    );
  });

  it("differs between bodies", () => {
    const point = onShell(33, -77);
    const other = cloudFieldParams({ ...input(123_456, 500_000_000), seed: cloudSeedOf("other") });
    expect(cloudFieldAt(coverage, NOISE, other, point, 1_000).resolved).not.toBe(
      cloudFieldAt(coverage, NOISE, params, point, 1_000).resolved,
    );
  });
});
