import { describe, expect, it } from "vitest";

import { wgslFunction } from "../../test/wgslText";
import classEdgeWgsl from "../shaders/classEdge.wgsl?raw";
import {
  classPurity,
  EDGE_BASE_WAVELENGTH_RAD,
  EDGE_FADE_FROM_PX,
  EDGE_FADE_TO_PX,
  EDGE_GRAZING_MU,
  EDGE_NOISE_RMS,
  EDGE_NOISE_SLOPE_RMS,
  EDGE_NOISE_WEIGHT,
  EDGE_OCTAVE_GAIN,
  EDGE_OCTAVES,
  EDGE_SHARPNESS_MAX,
  EDGE_WIDTH_PX,
  edgeGate,
  edgeHash,
  edgeKey,
  edgeNoise,
  edgePlan,
  edgeSeedOf,
  edgeShapedShares,
  edgeSharpness,
  type EdgeSite,
  gradientNoise,
} from "./classEdge";

/** A `u32` or `f32` constant of a WGSL source, its digits as written. */
function wgslConstant(source: string, name: string): number {
  const found = new RegExp(`const ${name} : (?:u32|f32) = ([0-9.e-]+)u?;`, "u").exec(source);
  if (found === null) {
    throw new Error(`no constant ${name}`);
  }
  return Number(found[1]);
}

/** A unit direction from its components. */
function unit(x: number, y: number, z: number): readonly [number, number, number] {
  const n = Math.hypot(x, y, z);
  return [x / n, y / n, z / n];
}

/** Points spread over a box of `side` lattice units by a Kronecker sequence, `count` of them. */
function latticePoints(count: number, side: number): Array<readonly [number, number, number]> {
  return Array.from({ length: count }, (_, a) => [
    ((a * 0.7548776662466927) % 1) * side,
    ((a * 0.5698402909980532) % 1) * side,
    ((a * 0.3191043427737574) % 1) * side + 0.5,
  ]);
}

describe("the edge noise (lane fix-jagged-biome-edges)", () => {
  it("has the shader's constants", () => {
    expect([
      wgslConstant(classEdgeWgsl, "EDGE_BASE_WAVELENGTH_RAD"),
      wgslConstant(classEdgeWgsl, "EDGE_OCTAVES"),
      wgslConstant(classEdgeWgsl, "EDGE_OCTAVE_GAIN"),
      wgslConstant(classEdgeWgsl, "EDGE_FADE_FROM_PX"),
      wgslConstant(classEdgeWgsl, "EDGE_FADE_TO_PX"),
      wgslConstant(classEdgeWgsl, "EDGE_NOISE_WEIGHT"),
      wgslConstant(classEdgeWgsl, "EDGE_WIDTH_PX"),
      wgslConstant(classEdgeWgsl, "EDGE_SHARPNESS_MAX"),
      wgslConstant(classEdgeWgsl, "EDGE_NOISE_SLOPE_RMS"),
      wgslConstant(classEdgeWgsl, "EDGE_GRAZING_MU"),
    ]).toEqual([
      EDGE_BASE_WAVELENGTH_RAD,
      EDGE_OCTAVES,
      EDGE_OCTAVE_GAIN,
      EDGE_FADE_FROM_PX,
      EDGE_FADE_TO_PX,
      EDGE_NOISE_WEIGHT,
      EDGE_WIDTH_PX,
      EDGE_SHARPNESS_MAX,
      EDGE_NOISE_SLOPE_RMS,
      EDGE_GRAZING_MU,
    ]);
    const sigma =
      EDGE_NOISE_RMS *
      Math.sqrt(
        Array.from({ length: EDGE_OCTAVES }, (_, o) => EDGE_OCTAVE_GAIN ** (2 * o)).reduce(
          (a, b) => a + b,
          0,
        ),
      );
    expect(wgslConstant(classEdgeWgsl, "EDGE_NOISE_SIGMA")).toBeCloseTo(sigma, 12);
    expect(wgslConstant(classEdgeWgsl, "EDGE_RISE_LOGIT")).toBeCloseTo(2 * Math.log(9), 12);
  });

  it("hashes as the shader does, Wellons' lowbias32 three times", () => {
    expect(collapse(wgslFunction(classEdgeWgsl, "edge_mix32"))).toContain("0x7feb352du");
    expect(collapse(wgslFunction(classEdgeWgsl, "edge_mix32"))).toContain("0x846ca68bu");
    // The hash's words of a few corners, negative indices as their two's complement, so that a
    // change to the twin shows.
    expect([edgeHash(0, 0, 0, 0), edgeHash(1, -1, 2, 7), edgeHash(-5, 3, -9, 0xffffff)]).toEqual([
      0, 779864421, 2640180077,
    ]);
    expect(new Set([edgeHash(0, 0, 0, 1), edgeHash(0, 0, 0, 2), edgeHash(1, 0, 0, 1)]).size).toBe(
      3,
    );
  });

  it("is zero at every lattice corner", () => {
    expect(gradientNoise(3, -2, 7, 11)).toBe(0);
  });

  it("lies within [−1, 1] between the corners", () => {
    const values = latticePoints(4096, 16).map(([x, y, z]) => gradientNoise(x, y, z, 11));
    expect(Math.max(...values.map(Math.abs))).toBeLessThanOrEqual(1);
  });

  /** The noise's root mean square and its x slope's over 2¹⁶ points of a box 64 units on a side. */
  const moments = ((): { readonly rms: number; readonly slope: number } => {
    const points = latticePoints(1 << 16, 64);
    const h = 1e-4;
    let squares = 0;
    let slopes = 0;
    for (const [x, y, z] of points) {
      const v = gradientNoise(x, y, z, 12345);
      const dx = (gradientNoise(x + h, y, z, 12345) - gradientNoise(x - h, y, z, 12345)) / (2 * h);
      squares += v * v;
      slopes += dx * dx;
    }
    return { rms: Math.sqrt(squares / points.length), slope: Math.sqrt(slopes / points.length) };
  })();

  it("has the root mean square it is normalised by, to 2%", () => {
    expect(Math.abs(moments.rms / EDGE_NOISE_RMS - 1)).toBeLessThan(0.02);
  });

  it("has the slope its sharpness is reckoned by, to 2%", () => {
    expect(Math.abs(moments.slope / EDGE_NOISE_SLOPE_RMS - 1)).toBeLessThan(0.02);
  });

  it("draws an octave only where its wavelength spans two pixels or more, whole from four", () => {
    // A footprint of a sixteenth of the first octave's wavelength: octaves 0 to 2 whole, 3 faded
    // in (2 px), 4 not drawn.
    const plan = edgePlan(EDGE_BASE_WAVELENGTH_RAD / 16);
    expect(plan.weights.slice(0, 5)).toEqual([1, 1, 1, 0, 0]);
    const between = edgePlan(EDGE_BASE_WAVELENGTH_RAD / 24);
    expect(between.weights[3]).toBeGreaterThan(0);
    expect(between.weights[3]).toBeLessThan(1);
    expect(edgePlan(1).weights.every((w) => w === 0)).toBe(true);
    expect(edgePlan(1).slopePerPx).toBe(0);
  });

  it("is the same at a point for one key", () => {
    const d = unit(0.3, -0.7, 0.2);
    expect(edgeNoise(d, 5, edgePlan(1e-6))).toBe(edgeNoise(d, 5, edgePlan(1e-6)));
  });

  it("differs from one key to another", () => {
    const d = unit(0.3, -0.7, 0.2);
    expect(edgeNoise(d, 5, edgePlan(1e-6))).not.toBe(edgeNoise(d, 6, edgePlan(1e-6)));
  });

  it("moves continuously as its octaves fade in", () => {
    const d = unit(0.3, -0.7, 0.2);
    const steps = Array.from({ length: 64 }, (_, k) =>
      edgeNoise(d, 5, edgePlan((EDGE_BASE_WAVELENGTH_RAD / 8) * 0.97 ** k)),
    );
    const jumps = steps.slice(1).map((v, k) => Math.abs(v - (steps[k] ?? v)));
    expect(Math.max(...jumps)).toBeLessThan(0.15);
  });
});

/** Collapses a WGSL body's whitespace. */
function collapse(text: string): string {
  return text.replaceAll(/\s+/gu, " ");
}

describe("the edges' gate and sharpness (lane fix-jagged-biome-edges)", () => {
  it("opens across a boundary between lattice points of one class each", () => {
    // Two pure points either side: their mean purity 1, the mix's ½ at the midpoint.
    expect(edgeGate(1, 0.5)).toBe(1);
  });

  it("stays shut over lattice points of one mix", () => {
    const mix = classPurity([0, 0.6, 0.4]);
    expect(edgeGate(mix, mix)).toBe(0);
  });

  it("stays shut where one class holds all", () => {
    expect(edgeGate(1, 1)).toBe(0);
  });

  it("is no sharper than the bilinear blend where a lattice spacing is a pixel", () => {
    expect(edgeSharpness(1, edgePlan(1e-3))).toBe(1);
  });

  it("is held to its most where a lattice spacing is a thousand pixels", () => {
    expect(edgeSharpness(1000, edgePlan(1))).toBe(EDGE_SHARPNESS_MAX);
  });

  it("draws an edge EDGE_WIDTH_PX wide across a lattice of 10 px and no noise", () => {
    expect(edgeSharpness(10, edgePlan(1))).toBeCloseTo(
      (2 * Math.log(9)) / (EDGE_WIDTH_PX * 0.4),
      9,
    );
  });
});

/** A boundary's site at `d` on a lattice of `latticePx` pixels a spacing, across pure points. */
function boundarySite(d: readonly [number, number, number], latticePx: number): EdgeSite {
  const footprintRad = 2e-5;
  return { direction: d, footprintRad, latticeRad: latticePx * footprintRad, meanPurity: 1 };
}

describe("the shaped shares (lane fix-jagged-biome-edges)", () => {
  const KEYS = [101, 202, 0];

  it("leaves a point of one keyed class as it was", () => {
    const site = boundarySite(unit(1, 0.2, 0.1), 50);
    expect(edgeShapedShares([0, 1, 0, 0], KEYS, site)).toEqual([0, 1, 0, 0]);
  });

  it("leaves a point of one keyed and one unkeyed class as it was", () => {
    const site = boundarySite(unit(1, 0.2, 0.1), 50);
    expect(edgeShapedShares([0, 0.5, 0, 0.5], KEYS, site)).toEqual([0, 0.5, 0, 0.5]);
  });

  it("leaves a mix whose lattice points agree as it was", () => {
    const mixed = [0, 0.6, 0.4, 0];
    const site = { ...boundarySite(unit(1, 0.2, 0.1), 50), meanPurity: classPurity(mixed) };
    expect(edgeShapedShares(mixed, KEYS, site)).toEqual(mixed);
  });

  /** A point of law 0, two keyed classes and an unkeyed one, across a wide lattice. */
  const mixedPoint = (): number[] =>
    edgeShapedShares([0.1, 0.35, 0.3, 0.25], KEYS, boundarySite(unit(1, 2, 3), 50));

  it("keeps law 0's share", () => {
    expect(mixedPoint()[0]).toBe(0.1);
  });

  it("keeps an unkeyed class's share", () => {
    expect(mixedPoint()[3]).toBe(0.25);
  });

  it("keeps the keyed classes' sum", () => {
    const [, a = 0, b = 0] = mixedPoint();
    expect(a + b).toBeCloseTo(0.65, 12);
  });

  it("draws a boundary across a wide lattice spacing crisp, not as a blend", () => {
    // Points across a quarter of a lattice spacing either side of the midpoint, 200 px a spacing.
    const shaped = Array.from({ length: 200 }, (_, k) => {
      const s = 0.25 + (0.5 * k) / 199;
      const d = unit(1, 0.01 * k, 0.37);
      return edgeShapedShares([0, s, 1 - s, 0], KEYS, boundarySite(d, 200))[1] ?? 0;
    });
    const blended = shaped.filter((s) => s > 0.1 && s < 0.9).length;
    expect(blended).toBeLessThan(20);
  });

  it("never moves a boundary past its lattice points, where one class holds all", () => {
    for (let k = 0; k < 50; k += 1) {
      const d = unit(1, 0.03 * k, -0.2);
      const atA = edgeShapedShares([0, 0.999, 0.001, 0], KEYS, boundarySite(d, 200));
      expect(atA[1]).toBeGreaterThan(0.5);
    }
  });

  it("is the bilinear blend itself where the pixel spans the lattice", () => {
    const shares = [0, 0.3, 0.7, 0];
    const site = {
      direction: unit(0.5, 0.5, 0.7),
      footprintRad: EDGE_BASE_WAVELENGTH_RAD,
      latticeRad: EDGE_BASE_WAVELENGTH_RAD / 4,
      meanPurity: 1,
    };
    const shaped = edgeShapedShares(shares, KEYS, site);
    shaped.forEach((s, m) => {
      expect(s).toBeCloseTo(shares[m] ?? 0, 12);
    });
  });
});

describe("the edge keys (lane fix-jagged-biome-edges)", () => {
  const seed = edgeSeedOf("00000000000004d2");
  const keys = [edgeKey(seed, 3, 0), edgeKey(seed, 3, 1), edgeKey(seed, 4, 0)];

  it("are nonzero 24-bit words, which an f32 holds exactly", () => {
    expect(keys.every((key) => key > 0 && key < 2 ** 24 && Math.fround(key) === key)).toBe(true);
  });

  it("differ from class to class", () => {
    expect(new Set(keys).size).toBe(3);
  });

  it("differ from world to world", () => {
    expect(edgeKey(edgeSeedOf("00000000000004d3"), 3, 0)).not.toBe(keys[0]);
  });

  it("are one class's alone on one world", () => {
    expect(edgeKey(seed, 3, 0)).toBe(keys[0]);
  });

  it("refuses a detail seed that is not 16 hexadecimal digits", () => {
    expect(() => edgeSeedOf("4d2")).toThrow(/16 hexadecimal digits/u);
  });
});
