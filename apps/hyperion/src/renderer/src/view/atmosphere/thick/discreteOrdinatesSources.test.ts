import { describe, expect, it } from "vitest";

import {
  type DiscreteOrdinatesProblem,
  doubleGaussQuadrature,
  type ModeIntensities,
  normalizedAssociatedLegendre,
  type OrdinateLayer,
  solveDiscreteOrdinates,
  solveDiscreteOrdinatesWithSources,
  type StreamSource,
  type StreamSourceMode,
} from "./discreteOrdinates";

const USER_COSINES = [0.1, 0.3, 0.5, 0.8, 1] as const;

/** Henyey–Greenstein's moments g^l to the solver's 16 streams. */
function henyeyGreenstein(g: number): number[] {
  return Array.from({ length: 16 }, (_, l) => g ** l);
}

/** `layers` equal layers making up a slab of optical depth `tau`, albedo `omega`. */
function slab(layers: number, tau: number, omega: number, moments: number[]): OrdinateLayer[] {
  return Array.from({ length: layers }, () => ({
    opticalDepth: tau / layers,
    singleScatteringAlbedo: omega,
    moments,
  }));
}

/** The cosines of a solve's streams: its nodes, +μ then −μ, and its user cosines, +μ then −μ. */
function streamCosines(): { readonly nodes: number[]; readonly users: number[] } {
  const { nodes } = doubleGaussQuadrature();
  return {
    nodes: [...nodes, ...Array.from(nodes, (mu) => -mu)],
    users: [...USER_COSINES, ...USER_COSINES.map((mu) => -mu)],
  };
}

/**
 * A stream source mode whose value on a stream of cosine μ at depth τ is `value(τ, μ)`, at each
 * layer's faces.
 */
function sampledSource(
  layers: ReadonlyArray<OrdinateLayer>,
  value: (tau: number, mu: number) => number,
): StreamSourceMode {
  const { nodes, users } = streamCosines();
  const at = (cosines: number[], face: "top" | "bottom"): Float64Array => {
    const out = new Float64Array(layers.length * cosines.length);
    let above = 0;
    layers.forEach((layer, p) => {
      const tau = face === "top" ? above : above + layer.opticalDepth;
      cosines.forEach((mu, r) => {
        out[p * cosines.length + r] = value(tau, mu);
      });
      above += layer.opticalDepth;
    });
    return out;
  };
  return {
    nodesTop: at(nodes, "top"),
    nodesBottom: at(nodes, "bottom"),
    usersTop: at(users, "top"),
    usersBottom: at(users, "bottom"),
  };
}

/** A signed source of both τ and μ, for a conservative layer's test. */
function signedSource(tau: number, mu: number): number {
  return 0.4 - 0.7 * tau * mu + 0.2 * mu * mu;
}

/** The largest |a − b| over the entries both hold made (not NaN), of the largest |b|. */
function relativeDifference(a: Float64Array, b: Float64Array): number {
  let largest = 0;
  let difference = 0;
  b.forEach((value, i) => {
    const other = a[i] ?? Number.NaN;
    if (Number.isNaN(value) && Number.isNaN(other)) {
      return;
    }
    largest = Math.max(largest, Math.abs(value));
    difference = Math.max(difference, Math.abs(other - value));
  });
  return difference / largest;
}

/** The largest relative difference over a field's four arrays. */
function fieldDifference(a: ModeIntensities, b: ModeIntensities): number {
  return Math.max(
    relativeDifference(a.quadratureUp, b.quadratureUp),
    relativeDifference(a.quadratureDown, b.quadratureDown),
    relativeDifference(a.userUp, b.userUp),
    relativeDifference(a.userDown, b.userDown),
  );
}

/** One source's field at mode m, or a test failure. */
function sourcedField(
  problem: DiscreteOrdinatesProblem,
  source: StreamSource,
  m: number,
): ModeIntensities {
  const field = solveDiscreteOrdinatesWithSources(problem, () => [source]).sourced[0]?.[m];
  if (field === undefined) {
    throw new Error(`no field at mode ${m}`);
  }
  return field;
}

describe("solveDiscreteOrdinatesWithSources", () => {
  it("reproduces the thermal emission's field from its isotropic source (1 − ω) B", () => {
    const layers: OrdinateLayer[] = [0.3, 1.2, 0.05, 2].map((tau, p) => ({
      opticalDepth: tau,
      singleScatteringAlbedo: [0.9, 0.5, 0.99, 0.7][p] ?? Number.NaN,
      moments: henyeyGreenstein([0.6, 0.85, 0, 0.3][p] ?? Number.NaN),
    }));
    const planck = [0.2, 0.5, 1.1, 1.2, 2];
    const problem = {
      layers,
      groundAlbedo: 0.3,
      beamCosines: [],
      userCosines: USER_COSINES,
      maxMode: 0,
    };
    const thermal = solveDiscreteOrdinates({
      ...problem,
      thermal: { interfacePlanck: planck, groundPlanck: 0 },
    }).thermal?.mode;
    if (thermal === undefined) {
      throw new Error("a solve with an emission has its field");
    }
    const { nodes, users } = streamCosines();
    const face = (index: number, width: number): Float64Array =>
      Float64Array.from({ length: layers.length * width }, (_, i) => {
        const p = Math.floor(i / width);
        const omega = layers[p]?.singleScatteringAlbedo ?? Number.NaN;
        return (1 - omega) * (planck[p + index] ?? Number.NaN);
      });
    const source: StreamSourceMode = {
      nodesTop: face(0, nodes.length),
      nodesBottom: face(1, nodes.length),
      usersTop: face(0, users.length),
      usersBottom: face(1, users.length),
    };
    expect(fieldDifference(sourcedField(problem, [source], 0), thermal)).toBeLessThan(1e-12);
  });

  it("reproduces a beam's own diffuse field from its source sampled on thin layers", () => {
    const mu0 = 0.6;
    const omega = 0.95;
    const moments = henyeyGreenstein(0.7);
    const layers = slab(400, 1, omega, moments);
    const problem = { layers, groundAlbedo: 0, userCosines: USER_COSINES, maxMode: 2 };
    const beam = solveDiscreteOrdinates({ ...problem, beamCosines: [mu0] }).beams[0];
    // Q^m(τ, μ) = (ω ÷ 4π)(2 − δ_m0) Σ (2l + 1) χ_l Λ_l^m(μ) Λ_l^m(−μ₀) e^(−τ ÷ μ₀).
    const beamSource =
      (m: number) =>
      (tau: number, mu: number): number => {
        const atMu = normalizedAssociatedLegendre(m, 15, mu);
        const atSun = normalizedAssociatedLegendre(m, 15, -mu0);
        let sum = 0;
        for (let l = m; l < 16; l += 1) {
          sum += (2 * l + 1) * (moments[l] ?? 0) * (atMu[l] ?? 0) * (atSun[l] ?? 0);
        }
        return ((omega * (m === 0 ? 1 : 2)) / (4 * Math.PI)) * sum * Math.exp(-tau / mu0);
      };
    const source = [0, 1, 2].map((m) => sampledSource(layers, beamSource(m)));
    const sourced = solveDiscreteOrdinatesWithSources({ ...problem, beamCosines: [] }, () => [
      source,
    ]).sourced[0];
    for (const m of [0, 1, 2]) {
      const own = beam?.modes[m];
      const field = sourced?.[m];
      if (own === undefined || field === undefined) {
        throw new Error(`no mode ${m}`);
      }
      // The piecewise-linear source's error, Δτ² ÷ (8 μ₀²) of e^(−τ ÷ μ₀): about 2 × 10⁻⁶.
      expect(fieldDifference(field, own)).toBeLessThan(1e-5);
    }
  });

  it("solves a conservative layer's source as the limit of a small co-albedo", () => {
    const moments = henyeyGreenstein(0.8);
    const field = (omega: number): ModeIntensities => {
      const layers = slab(3, 3, omega, moments);
      return sourcedField(
        { layers, groundAlbedo: 0.2, beamCosines: [], userCosines: USER_COSINES, maxMode: 0 },
        [sampledSource(layers, signedSource)],
        0,
      );
    };
    // Measured 2026-11-01: 5.9 × 10⁻⁶, 5.9 × 10⁻⁷ and 3.3 × 10⁻⁸ at co-albedos of 10⁻⁶ to 10⁻⁸, the
    // physics' own share; below that the near-singular operator's rounding grows as 1 ÷ (1 − ω).
    expect(fieldDifference(field(1), field(1 - 1e-8))).toBeLessThan(1e-7);
  });

  it("leaves the beams' solution as solveDiscreteOrdinates gives it, to the bit", () => {
    const layers = slab(5, 2, 0.9, henyeyGreenstein(0.75));
    const problem = {
      layers,
      groundAlbedo: 0.25,
      beamCosines: [0.2, 0.7],
      userCosines: USER_COSINES,
      maxMode: 2,
    };
    const { solution } = solveDiscreteOrdinatesWithSources(problem, () => [
      [sampledSource(layers, (tau, mu) => tau - mu)],
    ]);
    expect(solution).toEqual(solveDiscreteOrdinates(problem));
  });

  it("gives a source no field at a mode it has none at", () => {
    const layers = slab(4, 1, 0.9, henyeyGreenstein(0.5));
    const sourced = solveDiscreteOrdinatesWithSources(
      { layers, groundAlbedo: 0.1, beamCosines: [], userCosines: USER_COSINES, maxMode: 2 },
      () => [[null, sampledSource(layers, (tau, mu) => mu * (1 + tau))]],
    ).sourced[0];
    for (const m of [0, 2]) {
      const zero = sourced?.[m];
      expect(zero?.userUp.every((value) => value === 0)).toBe(true);
      expect(zero?.quadratureDown.every((value) => value === 0)).toBe(true);
    }
    expect(sourced?.[1]?.userUp.some((value) => value !== 0)).toBe(true);
  });

  it("refuses a source whose faces hold the wrong count of values", () => {
    const layers = slab(2, 1, 0.9, henyeyGreenstein(0.5));
    const source = sampledSource(layers, () => 1);
    const short = { ...source, usersBottom: source.usersBottom.subarray(1) };
    expect(() =>
      solveDiscreteOrdinatesWithSources(
        { layers, groundAlbedo: 0, beamCosines: [], userCosines: USER_COSINES, maxMode: 0 },
        () => [[short]],
      ),
    ).toThrow(RangeError);
  });
});
