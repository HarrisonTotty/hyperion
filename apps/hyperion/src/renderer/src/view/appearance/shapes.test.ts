import { describe, expect, it } from "vitest";

import {
  lambertPhase,
  lommelSeeligerPhase,
  phaseIntegral,
  shapeGeometricAlbedo,
  shapePhase,
} from "./shapes";

describe("the shapes' closed forms", () => {
  it("normalises both shapes to one at opposition and zero at conjunction", () => {
    expect(lambertPhase(0)).toBeCloseTo(1, 15);
    expect(lommelSeeligerPhase(0)).toBe(1);
    expect(lambertPhase(Math.PI)).toBeCloseTo(0, 15);
    expect(lommelSeeligerPhase(Math.PI)).toBe(0);
  });

  it("approaches the end values continuously", () => {
    expect(lommelSeeligerPhase(1e-9)).toBeCloseTo(1, 8);
    expect(lommelSeeligerPhase(Math.PI - 1e-6)).toBeCloseTo(0, 5);
  });

  it("gives Lambert's phase integral 3 ÷ 2", () => {
    expect(phaseIntegral(lambertPhase)).toBeCloseTo(1.5, 9);
  });

  it("gives Lommel–Seeliger's phase integral 16 ÷ 3 × (1 − ln 2)", () => {
    expect(phaseIntegral(lommelSeeligerPhase)).toBeCloseTo((16 / 3) * (1 - Math.LN2), 9);
  });

  it("reduces the mixture to each shape at its ends", () => {
    for (const alpha of [0.3, 1.2, 2.5]) {
      expect(shapePhase(0, alpha)).toBeCloseTo(lambertPhase(alpha), 15);
      expect(shapePhase(1, alpha)).toBeCloseTo(lommelSeeligerPhase(alpha), 15);
    }
  });

  it("gives the mixture's geometric albedo per unit A, ⅔ for Lambert and 1 for Lommel–Seeliger", () => {
    expect(shapeGeometricAlbedo(0)).toBeCloseTo(2 / 3, 15);
    expect(shapeGeometricAlbedo(1)).toBe(1);
    expect(shapeGeometricAlbedo(0.5)).toBeCloseTo(5 / 6, 15);
  });
});
