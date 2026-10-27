/**
 * The bake's delta-M (plan R08, R08.T14.b's first test): a forward-peaked layer truncated at the
 * bake's 16 streams gives the fluxes of the same layer solved whole at 64 streams, to 1%.
 */
import { describe, expect, it } from "vitest";

import { aerosolModeOptics, aerosolPhaseTable } from "../aerosol";
import { deltaMLayer, THICK_BAKE_STREAMS } from "./bake";
import { VENUS_CLOUD_PARTICLES } from "./bakeFixtures";
import { type BeamSolution, type OrdinateLayer, solveDiscreteOrdinates } from "./discreteOrdinates";
import { phaseMoments } from "./spectralColumn";

/** The streams of the whole solve, past which the layers' moments are below 10⁻⁴. */
const FULL_STREAMS = 64;

/** The beams and grounds each layer is held under. */
const SUNS = [0.2, 0.5, 1];
const GROUNDS = [0, 0.3];
const DEPTHS = [1, 10, 30];

/** Up at the top, and up and down (direct and diffuse) at the ground, of a beam. */
function fluxesOf(beam: BeamSolution): readonly [number, number, number] {
  const { diffuseUp, diffuseDown, direct } = beam.fluxes;
  const last = direct.length - 1;
  return [
    diffuseUp[0] ?? Number.NaN,
    diffuseUp[last] ?? Number.NaN,
    (diffuseDown[last] ?? Number.NaN) + (direct[last] ?? Number.NaN),
  ];
}

/** The fluxes of one layer, the m = 0 solve alone. */
function solve(
  layer: OrdinateLayer,
  streams: number,
  ground: number,
): ReadonlyArray<readonly [number, number, number]> {
  const solution = solveDiscreteOrdinates({
    layers: [layer],
    groundAlbedo: ground,
    beamCosines: SUNS,
    userCosines: [1],
    maxMode: 0,
    streams,
  });
  return solution.beams.map(fluxesOf);
}

/** The largest gap of the truncated solve's fluxes from the whole one's, relative to the beam's μ₀. */
/**
 * How the 16-stream solve takes the moments: delta-M truncated (`deltaMLayer`), or cut at l = 15
 * with no truncation.
 */
type Truncation = "delta-M" | "cut";

function worstGap(moments: Float64Array, omega: number, truncation: Truncation): number {
  let worst = 0;
  for (const tau of DEPTHS) {
    for (const ground of GROUNDS) {
      const whole: OrdinateLayer = {
        opticalDepth: tau,
        singleScatteringAlbedo: omega,
        moments: moments.subarray(0, FULL_STREAMS),
      };
      let layer: OrdinateLayer;
      switch (truncation) {
        case "delta-M":
          ({ layer } = deltaMLayer(
            tau,
            omega * tau,
            moments.subarray(0, THICK_BAKE_STREAMS + 1),
            0,
          ));
          break;
        case "cut":
          layer = {
            opticalDepth: tau,
            singleScatteringAlbedo: omega,
            moments: moments.subarray(0, THICK_BAKE_STREAMS),
          };
          break;
      }
      const full = solve(whole, FULL_STREAMS, ground);
      const truncated = solve(layer, THICK_BAKE_STREAMS, ground);
      for (const [b, mu0] of SUNS.entries()) {
        const expected = full[b] ?? [Number.NaN, Number.NaN, Number.NaN];
        const got = truncated[b] ?? [Number.NaN, Number.NaN, Number.NaN];
        for (const [i, value] of got.entries()) {
          const reference = expected[i] ?? Number.NaN;
          // Relative to the flux, or to 1% of the incident μ₀ for a flux that small.
          worst = Math.max(worst, Math.abs(value - reference) / Math.max(reference, 0.01 * mu0));
        }
      }
    }
  }
  return worst;
}

describe("the bake's delta-M", () => {
  // Measured, the worst gaps on, then off: 1.8 × 10⁻⁴ and 4.8 × 10⁻³ for the cloud, 1.9 × 10⁻⁴ and
  // 3.1 × 10⁻³ for the droplets (2026-10-26).
  it.for(["delta-M", "cut"] as const)(
    "gives a Henyey–Greenstein g 0.85 cloud the whole solve's fluxes within 1%, its moments by %s",
    (truncation) => {
      const g = 0.85;
      const moments = Float64Array.from({ length: FULL_STREAMS + 1 }, (_, l) => g ** l);
      expect(
        Math.max(...[1, 0.999].map((omega) => worstGap(moments, omega, truncation))),
      ).toBeLessThan(0.01);
    },
  );

  it.for(["delta-M", "cut"] as const)(
    "gives the Venus-class sulphuric acid droplets' table the whole solve's fluxes within 1%, its moments by %s",
    (truncation) => {
      const optics = aerosolModeOptics(VENUS_CLOUD_PARTICLES, 550);
      const table = aerosolPhaseTable([optics, optics, optics]);
      const moments = phaseMoments({ kind: "tabulated", table }, FULL_STREAMS);
      expect([
        (moments[FULL_STREAMS] ?? Number.NaN) < 1e-4,
        worstGap(moments, optics.singleScatteringAlbedo, truncation) < 0.01,
      ]).toEqual([true, true]);
    },
  );

  it("truncates at f = χ₁₆, held at the forward delta and at 0, and scales τ, ω and χ", () => {
    const moments = Float64Array.from({ length: 17 }, (_, l) => 0.9 ** l);
    const f = 0.9 ** 16;
    const { layer, truncated } = deltaMLayer(2, 1.8, moments, 0);
    expect(truncated).toBe(f);
    expect(Math.abs(layer.opticalDepth - 2 * (1 - 0.9 * f))).toBeLessThan(1e-15);
    expect(Math.abs(layer.singleScatteringAlbedo - (0.9 * (1 - f)) / (1 - 0.9 * f))).toBeLessThan(
      1e-15,
    );
    expect(layer.moments.length).toBe(16);
    expect(Math.abs((layer.moments[1] ?? Number.NaN) - (0.9 - f) / (1 - f))).toBeLessThan(1e-15);
    expect(deltaMLayer(2, 1.8, moments, 0.5).truncated).toBe(0.5);
    const rayleigh = phaseMoments({ kind: "rayleigh", depolarisation: 0 }, 16);
    expect(deltaMLayer(1, 1, rayleigh, 0).truncated).toBe(0);
  });

  it("refuses moments shorter than χ₀ … χ₁₆, and a fraction of 1 or more", () => {
    expect(() => deltaMLayer(1, 1, Float64Array.of(1, 0.5), 0)).toThrow(RangeError);
    const delta = new Float64Array(17).fill(1);
    expect(() => deltaMLayer(1, 1, delta, 0)).toThrow(RangeError);
  });
});
