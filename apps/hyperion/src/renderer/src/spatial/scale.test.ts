import { describe, expect, it } from "vitest";

import { AU_PER_LY, formatScaleLength, SCALE_AU_BELOW_LY } from "../lib/format";
import { between, seededRandom } from "../test/seededRandom";
import { ceil125, floor125, gridSpacing, RADIUS_STEPS_LY, scaleBar, type ScaleUnit } from "./scale";

const LY_THEN_AU: ReadonlyArray<ScaleUnit> = [
  { perSceneUnit: 1, minSceneLength: SCALE_AU_BELOW_LY },
  { perSceneUnit: AU_PER_LY, minSceneLength: 0 },
];

describe("floor125 and ceil125", () => {
  it.each([
    [0.01, 0.01, 0.01],
    [0.0100000001, 0.01, 0.02],
    [0.015, 0.01, 0.02],
    [0.02, 0.02, 0.02],
    [0.03, 0.02, 0.05],
    [0.05, 0.05, 0.05],
    [0.3, 0.2, 0.5],
    [0.1 * 3, 0.2, 0.5],
    [1, 1, 1],
    [7, 5, 10],
    [50, 50, 50],
    [80, 50, 100],
    [499, 200, 500],
    [1_000, 1_000, 1_000],
    [65_536, 50_000, 100_000],
  ])("brackets %f between %f and %f", (x, below, above) => {
    expect(floor125(x)).toBe(below);
    expect(ceil125(x)).toBe(above);
  });

  it("returns the literal doubles for hundredths", () => {
    expect(floor125(0.011)).toBe(0.01);
    expect(floor125(0.021)).toBe(0.02);
    expect(floor125(0.051)).toBe(0.05);
  });

  it("takes a value a hair off a step as that step", () => {
    expect(floor125(0.05 / 2.5)).toBe(0.02);
    expect(ceil125(0.02 * (1 + 1e-15))).toBe(0.02);
    expect(floor125(0.02 * (1 - 1e-15))).toBe(0.02);
  });

  it.each([0, -1, Number.NaN, Number.POSITIVE_INFINITY])("refuses %f", (x) => {
    expect(() => floor125(x)).toThrow(RangeError);
    expect(() => ceil125(x)).toThrow(RangeError);
  });
});

describe("scaleBar", () => {
  it.each([
    [4, 200, 50, 200],
    [4, 199, 20, 80],
    [0.5, 120, 200, 100],
    [2_000, 150, 0.05, 100],
  ])("at %f px per unit and %f px gives %f units over %f px", (pxPerUnit, maxBarPx, length, px) => {
    const bar = scaleBar(pxPerUnit, maxBarPx);

    expect(bar.length).toBe(length);
    expect(bar.lengthPx).toBeCloseTo(px, 9);
  });

  it("never exceeds its maximum and is at least 40% of it", () => {
    const random = seededRandom(125);
    for (let i = 0; i < 2_000; i += 1) {
      const pxPerUnit = 10 ** between(random, -3, 6);
      const maxBarPx = between(random, 40, 240);

      const bar = scaleBar(pxPerUnit, maxBarPx, LY_THEN_AU);

      expect(bar.lengthPx).toBeLessThanOrEqual(maxBarPx);
      expect(bar.lengthPx).toBeGreaterThanOrEqual(0.4 * maxBarPx);
    }
  });

  it("stays in light-years down to 0.01 ly", () => {
    const bar = scaleBar(12_000, 150, LY_THEN_AU);

    expect(bar.length).toBe(0.01);
  });

  it("steps to astronomical units below 0.01 ly", () => {
    const bar = scaleBar(16_000, 150, LY_THEN_AU);

    expect(bar.length * AU_PER_LY).toBeCloseTo(500, 9);
    expect(bar.lengthPx).toBeLessThanOrEqual(150);
  });

  it("steps its label 20 ly, 10 ly, 5 ly … 0.01 ly, then 500 AU as it zooms in", () => {
    const labels: string[] = [];
    for (let pxPerUnit = 4; pxPerUnit < 40_000; pxPerUnit *= 1.25) {
      const label = formatScaleLength(scaleBar(pxPerUnit, 100, LY_THEN_AU).length);
      if (labels.at(-1) !== label) {
        labels.push(label);
      }
    }

    expect(labels.slice(0, 3)).toEqual(["20 ly", "10 ly", "5 ly"]);
    expect(labels).toContain("0.01 ly");
    expect(labels[labels.indexOf("0.01 ly") + 1]).toBe("500 AU");
  });
});

describe("gridSpacing", () => {
  it.each([
    [50, 20],
    [80, 20],
    [100, 20],
    [500, 200],
    [0.05, 0.02],
    [0.01, 0.002],
    [1, 0.2],
  ])("spaces the grid for a radius of %f at %f", (radius, spacing) => {
    expect(gridSpacing(radius)).toBe(spacing);
  });

  it("gives 2.5 to 5 rings out to every offered radius", () => {
    for (const radius of RADIUS_STEPS_LY) {
      const rings = radius / gridSpacing(radius);

      expect(rings).toBeGreaterThanOrEqual(2.5 - 1e-9);
      expect(rings).toBeLessThanOrEqual(5 + 1e-9);
    }
  });
});

describe("RADIUS_STEPS_LY", () => {
  it("runs from 0.01 ly to 500 ly in 15 steps", () => {
    expect(RADIUS_STEPS_LY).toHaveLength(15);
    expect(RADIUS_STEPS_LY[0]).toBe(0.01);
    expect(RADIUS_STEPS_LY.at(-1)).toBe(500);
  });

  it("is strictly increasing and each step is its own floor125", () => {
    for (const radius of RADIUS_STEPS_LY) {
      expect(floor125(radius)).toBe(radius);
      expect(ceil125(radius)).toBe(radius);
    }
    for (let index = 1; index < RADIUS_STEPS_LY.length; index += 1) {
      expect(RADIUS_STEPS_LY[index]).toBeGreaterThan(RADIUS_STEPS_LY[index - 1] ?? Infinity);
    }
  });

  it("holds the literal values", () => {
    expect(RADIUS_STEPS_LY).toEqual([
      0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500,
    ]);
  });
});
