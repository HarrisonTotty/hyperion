/**
 * The albedo's quadrature and its reader (plan R08, R08.T5.e): eight nodes in μ, a Lambert plane's
 * 2A ÷ 3, and a reference without a node refused. The committed albedo case's checks are
 * `view/atmosphere/reference/venusAlbedo.test.ts`'s.
 */

import { describe, expect, it } from "vitest";

import { ALBEDO_NODES, geometricAlbedo, readAlbedoReference } from "./venusAlbedo";

describe("the albedo's quadrature", () => {
  it("has eight nodes in μ, ascending, inside (0, 1)", () => {
    const mus = ALBEDO_NODES.map((n) => n.mu);
    expect([mus.length, mus.toSorted((a, b) => a - b), mus[0] ?? 0, mus.at(-1) ?? 1]).toEqual([
      8,
      mus,
      expect.closeTo(0.0199, 4),
      expect.closeTo(0.9801, 4),
    ]);
  });

  it("gives a Lambert plane two thirds of its albedo", () => {
    const albedo = 0.37;
    const reference = readAlbedoReference({
      wavelengthsNm: [365],
      geometries: ALBEDO_NODES.map((node, k) => ({
        name: `albedo-n${k}`,
        // R = π I ÷ F = A μ for a Lambert plane, the sun of irradiance π.
        radiance: [albedo * node.mu],
        standardError: [0],
      })),
    });
    expect(geometricAlbedo(reference, 365)).toEqual({
      albedo: expect.closeTo((2 * albedo) / 3, 14),
      standardError: 0,
    });
  });

  it("refuses a reference without one of its nodes", () => {
    const reference = readAlbedoReference({ wavelengthsNm: [365], geometries: [] });
    expect(() => geometricAlbedo(reference, 365)).toThrow(/node 0/u);
  });
});
