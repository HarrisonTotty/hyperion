import { describe, expect, it } from "vitest";

import { cross, norm, scale, type Vec3 } from "../../geometry/vec3";
import { CLOUD_OCTAVES_MAX, cloudFieldParams, cloudSeedOf, coverageTexels } from "./cloudDensity";
import { cloudNoiseTexels } from "./cloudNoise";
import {
  baseNoiseAt,
  CELL_LEAST_HEIGHT,
  cellHeightNoiseAt,
  cellNoiseAt,
  cellOf,
  cloudColumnAt,
  cloudShapeFrame,
  cloudShapeKindOf,
  cloudShapeNormalisation,
  columnPlace,
  deckTopShare,
  type DensityInputs,
  densityPerM,
  detailNoiseAt,
  normalisationAt,
  normalisationTable,
  noiseShare,
  sampleTile,
  shareAbove,
  STRATIFORM_LEAST_TOP,
} from "./cloudShape";
import { cloudShapeNoises } from "./cloudShapeNoise";
import type { CoverageGrid } from "./coverage";

const NOISE = cloudNoiseTexels();
const SHAPES = cloudShapeNoises(cloudSeedOf("0200080020000000.0103"));
const EARTH_M = 6_378_137;
const BASE_M = 1_000;
const TOP_M = 3_000;
const SHELL_M = EARTH_M + 1_500;
const LIQUID = cloudShapeKindOf({ phase: "liquid" });
const ICE = cloudShapeKindOf({ phase: "solid" });

/** A coverage grid of one fraction everywhere. */
function uniformGrid(fraction: number): CoverageGrid {
  return {
    widthCells: 64,
    heightCells: 32,
    fractions: new Float64Array(64 * 32).fill(fraction),
    offset: 0,
  };
}

const PARAMS = cloudFieldParams({
  seed: cloudSeedOf("0200080020000000.0103"),
  condensate: { inCloudOpticalDepth550: 8 },
  radiusM: EARTH_M,
  time: { seconds: 123_456, nanos: 0 },
  rate: 1,
  windMps: 20,
  octaves: CLOUD_OCTAVES_MAX,
});

/** Point i of n spread evenly over the sphere (a Fibonacci lattice), on the shell. */
function fibonacciPoint(i: number, n: number): Vec3 {
  const z = 1 - (2 * (i + 0.5)) / n;
  const r = Math.sqrt(1 - z * z);
  const angle = i * Math.PI * (3 - Math.sqrt(5));
  return { x: SHELL_M * r * Math.cos(angle), y: SHELL_M * r * Math.sin(angle), z: SHELL_M * z };
}

/** The steps of a column's integral through the deck. */
const COLUMN_STEPS = 400;

/** A column's τ by the density integrated through the deck at `point`, and the field's mean τ. */
function columnIntegral(
  coverageFraction: number,
  kind: typeof LIQUID,
  point: Vec3,
  footprintM: number,
  shares: { readonly base: number; readonly detail: number },
  flat = 0,
): { readonly integral: number; readonly meanTau: number } {
  const coverage = coverageTexels(uniformGrid(coverageFraction), () => ({
    eastMps: 0,
    northMps: 0,
  }));
  const column = cloudColumnAt(coverage, NOISE, PARAMS, kind, point, footprintM);
  const inputs: DensityInputs = {
    baseM: BASE_M,
    depthM: TOP_M - BASE_M,
    kind,
    table: normalisationTable(kind.profileExponent),
    flat,
  };
  const frame = cloudShapeFrame(TOP_M - BASE_M, { x: 0, y: 0, z: 0 }, 0, 0);
  const up = scale(point, 1 / norm(point));
  const east = scale(cross({ x: 0, y: 0, z: 1 }, up), 1 / norm(cross({ x: 0, y: 0, z: 1 }, up)));
  const north = cross(up, east);
  const dh = (TOP_M - BASE_M) / COLUMN_STEPS;
  let integral = 0;
  for (let k = 0; k < COLUMN_STEPS; k += 1) {
    const h = BASE_M + (k + 0.5) * dh;
    const place = scale(up, EARTH_M + h);
    const plane = { x: place.x * east.x + place.y * east.y, y: place.z };
    const sigma = densityPerM(
      column.meanTau,
      columnPlace(h, column.topShare, inputs),
      {
        base: baseNoiseAt(SHAPES.base, frame, place),
        detail: detailNoiseAt(SHAPES.detail, SHAPES.curl, frame, place, plane, east, north),
        baseShare: shares.base,
        detailShare: shares.detail,
      },
      inputs,
    );
    integral += sigma * dh;
  }
  return { integral, meanTau: column.meanTau };
}

describe("the volume's density through a column", () => {
  it("normalises the erosion to its mean, by a quadrature within 10⁻³ of a finer one", () => {
    // With no noise, G = ∫₀¹ min(3(1 − u), 1) du = 2/3 + 1/6 for a uniform profile.
    expect(cloudShapeNormalisation(0, 0, 0)).toBeCloseTo(5 / 6, 3);
    const table = normalisationTable(LIQUID.profileExponent);
    for (const [wb, wd] of [
      [1, 1],
      [0.5, 1],
      [1, 0],
    ] as const) {
      expect(normalisationAt(table, wb, wd)).toBeCloseTo(
        cloudShapeNormalisation(wb, wd, LIQUID.profileExponent),
        5,
      );
    }
    // The erosion takes its share of the cloud: about half with both noises.
    expect(normalisationAt(table, 1, 1)).toBeGreaterThan(0.25);
    expect(normalisationAt(table, 1, 1)).toBeLessThan(0.75);
  });

  it("lays exactly the column's τ through the deck with no noise", () => {
    for (const kind of [LIQUID, ICE]) {
      const { integral, meanTau } = columnIntegral(0.6, kind, fibonacciPoint(7, 100), 200, {
        base: 0,
        detail: 0,
      });
      expect(meanTau).toBeGreaterThan(0);
      expect(integral / meanTau).toBeCloseTo(1, 2);
    }
  });

  it("keeps the layer's mean τ over many columns within 2%, the noises at full share", () => {
    // The field's mean τ over 2,000 places against the density's integrals there: the layer is
    // the volume's column integral (Design note 10's independent-pixel rule) in the mean, and
    // each column strays by the noises' vertical mean. Measured 2026-11-19: 0.998, 1.014 and
    // 1.009 of the layer's mean, and 0.43, 0.41 and 0.26 RMS a column, the billows as tall as the
    // deck, so that a column's vertical mean of them does not average out.
    for (const [fraction, kind] of [
      [0.4, LIQUID],
      [0.9, LIQUID],
      [0.6, ICE],
    ] as const) {
      let integrals = 0;
      let means = 0;
      let squares = 0;
      let cloudy = 0;
      const n = 2_000;
      for (let i = 0; i < n; i += 1) {
        const { integral, meanTau } = columnIntegral(fraction, kind, fibonacciPoint(i, n), 100, {
          base: 1,
          detail: 1,
        });
        integrals += integral;
        means += meanTau;
        if (meanTau > 0.5) {
          squares += (integral / meanTau - 1) ** 2;
          cloudy += 1;
        }
      }
      expect(Math.abs(integrals / means - 1)).toBeLessThan(0.02);
      // A column's own τ strays from the layer's by less than half of it, typically.
      expect(Math.sqrt(squares / cloudy)).toBeLessThan(0.5);
    }
  });

  it("is the layer's uniform slab when flattened", () => {
    const inputs: DensityInputs = {
      baseM: BASE_M,
      depthM: 2_000,
      kind: LIQUID,
      table: normalisationTable(LIQUID.profileExponent),
      flat: 1,
    };
    for (const h of [1_010, 1_900, 2_990]) {
      const place = columnPlace(h, 0.3, inputs);
      const sigma = densityPerM(
        8,
        place,
        { base: 0.1, detail: 0.9, baseShare: 1, detailShare: 1 },
        inputs,
      );
      expect(sigma).toBeCloseTo(8 / 2_000, 9);
      expect(shareAbove(place, inputs)).toBeCloseTo((3_000 - h) / 2_000, 9);
    }
  });
});

describe("a column's top", () => {
  it("rises as an adiabatic cumulus's, H ∝ τ^(3/5), where the cover is broken", () => {
    expect(deckTopShare(2, 0.3, true)).toBeCloseTo(1, 9);
    expect(deckTopShare(0.5, 0.3, true)).toBeCloseTo(0.25 ** 0.6, 9);
    expect(deckTopShare(0, 0.3, true)).toBeGreaterThan(0);
  });

  it("fills most of the deck where the cover closes, and always for ice", () => {
    expect(deckTopShare(0, 0.9, true)).toBeCloseTo(STRATIFORM_LEAST_TOP, 9);
    expect(deckTopShare(0, 0.3, false)).toBeCloseTo(STRATIFORM_LEAST_TOP, 9);
    expect(deckTopShare(1, 0.675, true)).toBeGreaterThan(deckTopShare(1, 0.3, true));
  });
});

describe("the shape noises' reading", () => {
  it("fades a noise between 4 and 2 footprints a feature", () => {
    expect(noiseShare(1_000, 250)).toBe(1);
    expect(noiseShare(1_000, 500)).toBe(0);
    expect(noiseShare(1_000, 353.6)).toBeCloseTo(0.5, 3);
  });

  it("repeats a tile", () => {
    const u = { x: 0.31, y: 0.77, z: 0.05 };
    expect(sampleTile(SHAPES.base, 128, { x: u.x + 3, y: u.y - 2, z: u.z + 1 })).toBeCloseTo(
      sampleTile(SHAPES.base, 128, u),
      9,
    );
  });
});

describe("a column's cumulus cells (R11.T7.c)", () => {
  it("cover the region's share and keep its mean τ over many places, to 3%", () => {
    // Measured 2026-11-19 over 2 × 10⁵ places, the cells at their own heights: cover 0.198, 0.397
    // and 0.699 of 0.2, 0.4 and 0.7, mean τ 0.983, 0.989 and 0.995 of the region's, the heights'
    // noise's mean 0.500: the filtered noise's tails a little light
    // (its deviation 1.002 and mean 0.0004), so that the high thresholds of thin cover take a
    // little less.
    const frame = cloudShapeFrame(2_000, { x: 0, y: 0, z: 0 }, 0, 0);
    for (const fraction of [0.2, 0.4, 0.7]) {
      const region = { fraction, meanTau: 8 * fraction, excess: 0.8 };
      let covered = 0;
      let tau = 0;
      const n = 20_000;
      for (let i = 0; i < n; i += 1) {
        const p = fibonacciPoint(i, n);
        const cell = cellOf(
          region,
          region,
          cellNoiseAt(SHAPES.base, frame, p),
          cellHeightNoiseAt(SHAPES.base, frame, p),
          8,
        );
        covered += cell.inCloudTau > 0 ? 1 : 0;
        tau += cell.inCloudTau;
      }
      expect(Math.abs(covered / n - fraction)).toBeLessThan(0.01);
      expect(Math.abs(tau / n / region.meanTau - 1)).toBeLessThan(0.03);
    }
  });

  it("follow the field's own masses by half, never solid where the region is broken", () => {
    const region = { fraction: 0.4, meanTau: 3.2, excess: 0.8 };
    // In a mass the cells cover 0.7 of it, beyond u = 0.3; in a clearing 0.2, beyond 0.8.
    expect(cellOf(region, { fraction: 1 }, 0.6, 0.5, 8).inCloudTau).toBeGreaterThan(0);
    expect(cellOf(region, { fraction: 0 }, 0.6, 0.5, 8).inCloudTau).toBe(0);
    expect(cellOf(region, { fraction: 1 }, 0.25, 0.5, 8).inCloudTau).toBe(0);
  });

  it("keep at least 0.6 τ̄ in cloud about a mass's thin edge, fewer, their mean τ still the region's", () => {
    const frame = cloudShapeFrame(2_000, { x: 0, y: 0, z: 0 }, 0, 0);
    const region = { fraction: 0.5, meanTau: 1, excess: 0.2 };
    let covered = 0;
    let tau = 0;
    const n = 20_000;
    for (let i = 0; i < n; i += 1) {
      const p = fibonacciPoint(i, n);
      const cell = cellOf(
        region,
        region,
        cellNoiseAt(SHAPES.base, frame, p),
        cellHeightNoiseAt(SHAPES.base, frame, p),
        8,
      );
      covered += cell.inCloudTau > 0 ? 1 : 0;
      tau += cell.inCloudTau;
    }
    // The region's in-cloud τ is 2: the cells take 0.6 of τ̄ 8, 4.8, and 2 ÷ 4.8 of its cover.
    expect(Math.abs(covered / n - (0.5 * 2) / 4.8)).toBeLessThan(0.01);
    expect(tau / covered).toBeGreaterThan(4.5);
    expect(Math.abs(tau / n / region.meanTau - 1)).toBeLessThan(0.05);
  });

  it("rise steeply from their edges to a plateau, their tops flat at their cores", () => {
    const region = { fraction: 0.4, meanTau: 3.2, excess: 0.8 };
    // The cells cover 0.4, beyond u = 0.6.
    const edge = cellOf(region, region, 0.62, 0.5, 8);
    const core = cellOf(region, region, 0.9, 0.5, 8);
    const deeper = cellOf(region, region, 0.99, 0.5, 8);
    expect(edge.inCloudTau).toBeGreaterThan(0);
    expect(core.inCloudTau).toBeGreaterThan(2 * edge.inCloudTau);
    expect(deeper.inCloudTau).toBe(core.inCloudTau);
    expect(deeper.topShare).toBe(core.topShare);
    expect(core.topShare).toBeGreaterThan(edge.topShare);
    expect(cellOf(region, region, 0.5, 0.5, 8).inCloudTau).toBe(0);
  });

  it("stand at their own heights, 0.4 to 1 of the tall cell's, a taller cell the deeper", () => {
    const region = { fraction: 0.6, meanTau: 4.8, excess: 0.8 };
    const low = cellOf(region, region, 0.99, 0, 8);
    const tall = cellOf(region, region, 0.99, 1, 8);
    expect(low.topShare / tall.topShare).toBeCloseTo(CELL_LEAST_HEIGHT, 12);
    expect(low.inCloudTau / tall.inCloudTau).toBeCloseTo(CELL_LEAST_HEIGHT, 12);
    // Neighbouring cells, a cell's feature apart, stand at different heights.
    const frame = cloudShapeFrame(2_000, { x: 0, y: 0, z: 0 }, 0, 0);
    const heights = Array.from({ length: 64 }, (_, i) =>
      cellHeightNoiseAt(SHAPES.base, frame, { x: 6.4e6, y: (i * frame.baseTileM) / 4, z: 0 }),
    );
    expect(Math.max(...heights) - Math.min(...heights)).toBeGreaterThan(0.5);
  });

  it("stand lower where sparse: their excess is at most their cover", () => {
    const sparse = cellOf(
      { fraction: 0.1, meanTau: 0.8, excess: 0.8 },
      { fraction: 0.1 },
      0.999,
      0.5,
      8,
    );
    const dense = cellOf(
      { fraction: 0.6, meanTau: 4.8, excess: 0.8 },
      { fraction: 0.6 },
      0.999,
      0.5,
      8,
    );
    expect(sparse.topShare).toBeLessThan(dense.topShare);
  });

  it("hide their noise's tile: two readings a tile apart differ", () => {
    const frame = cloudShapeFrame(2_000, { x: 0, y: 0, z: 0 }, 0, 0);
    let same = 0;
    for (let i = 0; i < 200; i += 1) {
      const p = { x: 6.4e6, y: 137 * i, z: 291 * i };
      const q = { ...p, y: p.y + frame.baseTileM };
      same +=
        Math.abs(cellNoiseAt(SHAPES.base, frame, p) - cellNoiseAt(SHAPES.base, frame, q)) < 1e-6
          ? 1
          : 0;
    }
    expect(same).toBe(0);
  });
});
