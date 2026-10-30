import { describe, expect, it } from "vitest";

import type { AtmosphereMedium } from "./medium";
import { marchTwin, type TwinRay, type TwinSources } from "./tablesCpu";
import { blackBodyLuminance, type MediumTemperature, thermalSourceOf } from "./thermalSource";

const GROUND_RADIUS_M = 6_371_000;
const TOP_M = 100_000;
const SCALE_HEIGHT_M = 8_000;

/** A grey column of vertical optical depth `tau` and single-scattering albedo `omega`. */
function grey(tau: number, omega: number): AtmosphereMedium {
  const extinction = tau / (SCALE_HEIGHT_M * -Math.expm1(-TOP_M / SCALE_HEIGHT_M));
  const s = omega * extinction;
  const a = (1 - omega) * extinction;
  return {
    name: "grey",
    topHeightM: TOP_M,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "grey",
        density: { kind: "exponential", scaleHeightM: SCALE_HEIGHT_M },
        scattering: [s, s, s],
        absorption: [a, a, a],
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
      },
    ],
  };
}

/** An isothermal column at `temperatureK`. */
function isothermal(temperatureK: number): MediumTemperature {
  return {
    heightsM: Float64Array.of(0, TOP_M),
    temperaturesK: Float64Array.of(temperatureK, temperatureK),
  };
}

/** Sources that light nothing, beside the thermal source of `temperatureK` at the frame's `scale`. */
function darkSources(temperatureK: number, scale: number): TwinSources {
  return {
    bottomRadiusM: GROUND_RADIUS_M,
    sunTransmittance: (_h, _mu, out) => out.fill(0),
    multiScattering: (_h, _mu, out) => out.fill(0),
    absorberPathDepth: () => undefined,
    thermal: { source: thermalSourceOf(isothermal(temperatureK), TOP_M), scale },
  };
}

/** A vertical ray from the top to the ground, in 96 even steps, lit by no source. */
function downward(): { readonly ray: TwinRay; readonly steps: { tM: number; dtM: number }[] } {
  const dtM = TOP_M / 96;
  return {
    ray: {
      originM: { x: 0, y: 0, z: GROUND_RADIUS_M + TOP_M },
      direction: { x: 0, y: 0, z: -1 },
      sun: { x: 0, y: 0, z: 1 },
      tStartM: 0,
      tEndM: TOP_M,
      fromCamera: false,
      sources: [],
    },
    steps: Array.from({ length: 96 }, (_, i) => ({ tM: (i + 0.5) * dtM, dtM })),
  };
}

describe("the march's thermal emission in its twin (R08.T18.b)", () => {
  it("gives an isothermal column B̃(1 − ω)(1 − T) at the top, Kirchhoff's law step by step", () => {
    const scale = 2.5e-4;
    const { ray, steps } = downward();
    for (const [tau, omega] of [
      [0.1, 0],
      [1, 0.5],
      [10, 0.9],
    ] as const) {
      const march = marchTwin(
        grey(tau, omega),
        darkSources(2_500, scale),
        { kind: "sphere", bottomRadiusM: GROUND_RADIUS_M },
        ray,
        steps,
      );
      const b = blackBodyLuminance(2_500);
      for (const c of [0, 1, 2] as const) {
        const want = scale * Math.fround(b[c]) * (1 - omega) * (1 - march.transmittance[c]);
        expect(Math.abs(march.luminance[c] / want - 1)).toBeLessThan(1e-6);
      }
    }
  });

  it("tends to B̃ as the column's optical depth grows, an opaque absorbing column emitting it", () => {
    const { ray, steps } = downward();
    const b = blackBodyLuminance(2_500);
    const marches = [0.1, 1, 10, 100].map((tau) =>
      marchTwin(
        grey(tau, 0),
        darkSources(2_500, 1),
        { kind: "sphere", bottomRadiusM: GROUND_RADIUS_M },
        ray,
        steps,
      ),
    );
    const shares = marches.map((march) => march.luminance[0] / Math.fround(b[0]));
    // 1 − T of the march's own steps, its midpoints' quadrature of the column (6 × 10⁻⁴ off e^(−τ)).
    expect(shares[0]).toBeCloseTo(1 - (marches[0]?.transmittance[0] ?? Number.NaN), 9);
    for (let i = 1; i < shares.length; i += 1) {
      expect(shares[i]).toBeGreaterThan(shares[i - 1] ?? Number.POSITIVE_INFINITY);
    }
    expect(Math.abs((shares[3] ?? 0) - 1)).toBeLessThan(1e-6);
  });
});
