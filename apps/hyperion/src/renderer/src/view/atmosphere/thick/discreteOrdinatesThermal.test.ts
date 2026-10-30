import { describe, expect, it } from "vitest";

import {
  doubleGaussQuadrature,
  type GroundReflectance,
  groundCosinesOf,
  type OrdinateLayer,
  solveDiscreteOrdinates,
  type ThermalSolution,
} from "./discreteOrdinates";

const USER_COSINES = [0.1, 0.3, 0.5, 0.8, 1] as const;

/** Henyey–Greenstein's moments g^l to the solver's 16 streams. */
function henyeyGreenstein(g: number): number[] {
  return Array.from({ length: 16 }, (_, l) => g ** l);
}

/** A slab of `layers` equal layers, each of optical depth `tau` and albedo `omega`. */
function slab(
  layers: number,
  tau: number,
  omega: number,
  moments: number[] = [1],
): OrdinateLayer[] {
  return Array.from({ length: layers }, () => ({
    opticalDepth: tau,
    singleScatteringAlbedo: omega,
    moments,
  }));
}

/** ∫ₐᵇ f by Simpson's rule on 2,000 intervals. */
function simpson(f: (x: number) => number, a: number, b: number): number {
  const steps = 2_000;
  const h = (b - a) / steps;
  let sum = f(a) + f(b);
  for (let i = 1; i < steps; i += 1) {
    sum += (i % 2 === 0 ? 2 : 4) * f(a + i * h);
  }
  return (sum * h) / 3;
}

/** A solve's thermal field, or a test failure. */
function thermalOf(
  layers: ReadonlyArray<OrdinateLayer>,
  interfacePlanck: ArrayLike<number>,
  groundPlanck: number,
  groundAlbedo = 0,
): ThermalSolution {
  const solution = solveDiscreteOrdinates({
    layers,
    groundAlbedo,
    beamCosines: [],
    userCosines: USER_COSINES,
    maxMode: 0,
    thermal: { interfacePlanck, groundPlanck },
  });
  if (solution.thermal === undefined) {
    throw new Error("a solve with an emission has its field");
  }
  return solution.thermal;
}

/** The field's intensity leaving the top at user cosine u. */
function emergent(thermal: ThermalSolution, u: number): number {
  return thermal.mode.userUp[u] ?? Number.NaN;
}

describe("the solver's thermal emission (R08.T18.b)", () => {
  it("gives an absorbing isothermal column B(1 − e^(−τ ÷ μ)) at the top, which tends to B as τ grows", () => {
    for (const tau of [0.1, 1, 10, 100]) {
      // Four layers, so that the stack's interfaces carry it as one layer would.
      const thermal = thermalOf(slab(4, tau / 4, 0), [2, 2, 2, 2, 2], 0);
      for (const [u, mu] of USER_COSINES.entries()) {
        expect(emergent(thermal, u)).toBeCloseTo(2 * -Math.expm1(-tau / mu), 12);
      }
    }
    const opaque = thermalOf(slab(4, 25, 0), [2, 2, 2, 2, 2], 0);
    for (const u of USER_COSINES.keys()) {
      expect(Math.abs(emergent(opaque, u) / 2 - 1)).toBeLessThan(1e-12);
    }
  });

  it("gives a deep absorbing column of B linear in τ the Eddington–Barbier value b₀ + b₁μ", () => {
    // One layer of τ 60: B = 1 + 0.5τ through it, exactly linear.
    const thermal = thermalOf(slab(1, 60, 0), [1, 31], 0);
    for (const [u, mu] of USER_COSINES.entries()) {
      expect(emergent(thermal, u)).toBeCloseTo(1 + 0.5 * mu, 10);
    }
  });

  it("sums two absorbing layers of different B and an emitting ground as a quadrature does", () => {
    const layers = [
      { opticalDepth: 0.4, singleScatteringAlbedo: 0, moments: [1] },
      { opticalDepth: 1.1, singleScatteringAlbedo: 0, moments: [1] },
    ];
    const planck = [0.2, 0.7, 1.5];
    const groundPlanck = 2.4;
    const thermal = thermalOf(layers, planck, groundPlanck);
    // ∫ B(τ) e^(−τ ÷ μ) dτ ÷ μ by Simpson's rule on each layer, and the ground's B_g e^(−τ ÷ μ).
    for (const [u, mu] of USER_COSINES.entries()) {
      const first = simpson((t) => ((0.2 + (0.5 / 0.4) * t) * Math.exp(-t / mu)) / mu, 0, 0.4);
      const second = simpson(
        (t) => ((0.7 + (0.8 / 1.1) * (t - 0.4)) * Math.exp(-t / mu)) / mu,
        0.4,
        1.5,
      );
      const want = first + second + groundPlanck * Math.exp(-1.5 / mu);
      expect(Math.abs(emergent(thermal, u) - want)).toBeLessThan(1e-9 * Math.max(want, 1));
    }
  });

  it("holds Kirchhoff's law: a scattering column over a ground at its own temperature emits B(1 − r(μ))", () => {
    // ω 0.8, Henyey–Greenstein g 0.6, τ 2, over a Lambertian ground of albedo 0.25, isothermal.
    const layers = slab(4, 0.5, 0.8, henyeyGreenstein(0.6));
    const groundAlbedo = 0.25;
    const thermal = thermalOf(layers, [1, 1, 1, 1, 1], 1, groundAlbedo);
    // r(μ₀): the column's reflected flux of a beam of unit flux normal to itself, over μ₀.
    const beams = solveDiscreteOrdinates({
      layers,
      groundAlbedo,
      beamCosines: USER_COSINES,
      userCosines: USER_COSINES,
      maxMode: 0,
    });
    let worst = 0;
    for (const [u, mu] of USER_COSINES.entries()) {
      const reflected = (beams.beams[u]?.fluxes.diffuseUp[0] ?? Number.NaN) / mu;
      worst = Math.max(worst, Math.abs(emergent(thermal, u) - (1 - reflected)));
    }
    // The solve's reciprocity is its own to rounding (4 × 10⁻¹⁵ measured, here and at ω 0.99,
    // g 0.85, τ 10): the user stream's formal solution against the beam's flux on the nodes.
    expect(worst).toBeLessThan(1e-12);
  });

  it("holds an isothermal column's mean intensity at B deep inside, whatever it scatters", () => {
    // τ 100 above the middle: the top's leak reaches in as e^(−kτ), k = √(3(1 − ω)(1 − ωg)) ≈ 0.41
    // in diffusion, 10⁻¹⁸ there.
    const layers = slab(20, 10, 0.9, henyeyGreenstein(0.5));
    const thermal = thermalOf(
      layers,
      Array.from({ length: 21 }, () => 3),
      3,
      0.3,
    );
    const middle = thermal.fluxes.meanIntensity[10] ?? Number.NaN;
    expect(Math.abs(middle / 3 - 1)).toBeLessThan(1e-9);
    // Light leaves the top, so its mean intensity there falls short of B, and none enters.
    expect(thermal.fluxes.meanIntensity[0]).toBeLessThan(3);
    expect(thermal.fluxes.diffuseDown[0]).toBeCloseTo(0, 12);
  });

  it("emits as the albedo's ground from a ground given by its reflectance at the same albedo", () => {
    const layers = slab(3, 0.3, 0.6, henyeyGreenstein(0.4));
    const planck = [0.5, 0.8, 1.1, 1.4];
    const albedo = 0.4;
    const cosines = groundCosinesOf(16, USER_COSINES, []);
    const ground: GroundReflectance = {
      outgoingCosines: cosines.outgoingCosines,
      incidentCosines: cosines.incidentCosines,
      modes: [
        new Float64Array(cosines.outgoingCosines.length * cosines.incidentCosines.length).fill(
          albedo,
        ),
      ],
    };
    const lambertian = thermalOf(layers, planck, 2, albedo);
    const given = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0,
      groundReflectance: ground,
      beamCosines: [],
      userCosines: USER_COSINES,
      maxMode: 0,
      thermal: { interfacePlanck: planck, groundPlanck: 2 },
    }).thermal;
    for (const u of USER_COSINES.keys()) {
      expect(given?.mode.userUp[u]).toBeCloseTo(emergent(lambertian, u), 12);
    }
    const n = doubleGaussQuadrature(16).nodes.length;
    for (let i = 0; i < n; i += 1) {
      expect(given?.mode.quadratureUp[i]).toBeCloseTo(lambertian.mode.quadratureUp[i] ?? 0, 12);
    }
  });

  /** A scattering slab under one beam, without its emission. */
  const beamProblem = {
    layers: slab(2, 0.7, 0.9, henyeyGreenstein(0.5)),
    groundAlbedo: 0.2,
    beamCosines: [0.6],
    userCosines: USER_COSINES,
    maxMode: 2,
  };

  it("leaves the beams as they are beside it", () => {
    const emitting = solveDiscreteOrdinates({
      ...beamProblem,
      thermal: { interfacePlanck: [1, 2, 3], groundPlanck: 4 },
    });
    expect(emitting.beams).toEqual(solveDiscreteOrdinates(beamProblem).beams);
  });

  it("gives no field without it", () => {
    expect(solveDiscreteOrdinates(beamProblem).thermal).toBeUndefined();
  });

  it("gives a scattering layer of B linear in τ the field of the same layer split in two", () => {
    // ω 0.9 and Henyey–Greenstein g 0.7, B = 1 + 0.8τ through τ 1.5: z's slope term carries it.
    const moments = henyeyGreenstein(0.7);
    const whole = thermalOf(slab(1, 1.5, 0.9, moments), [1, 2.2], 0.5, 0.1);
    const halves = thermalOf(slab(2, 0.75, 0.9, moments), [1, 1.6, 2.2], 0.5, 0.1);
    for (const u of USER_COSINES.keys()) {
      expect(halves.mode.userUp[u]).toBeCloseTo(emergent(whole, u), 12);
    }
    const n = doubleGaussQuadrature(16).nodes.length;
    for (let i = 0; i < n; i += 1) {
      expect(halves.mode.quadratureUp[2 * n + i]).toBeCloseTo(
        whole.mode.quadratureUp[n + i] ?? Number.NaN,
        12,
      );
    }
  });

  it("refuses an emission without a radiance an interface, or with one negative or not finite", () => {
    const layers = slab(2, 0.5, 0.5);
    const solve = (interfacePlanck: number[], groundPlanck = 0): unknown =>
      solveDiscreteOrdinates({
        layers,
        groundAlbedo: 0,
        beamCosines: [],
        userCosines: [1],
        maxMode: 0,
        thermal: { interfacePlanck, groundPlanck },
      });
    expect(() => solve([1, 1])).toThrow(/3 of them, not 2/u);
    expect(() => solve([1, -1, 1])).toThrow(/interface 1's Planck radiance/u);
    expect(() => solve([1, 1, 1], Number.NaN)).toThrow(/groundPlanck/u);
  });
});
