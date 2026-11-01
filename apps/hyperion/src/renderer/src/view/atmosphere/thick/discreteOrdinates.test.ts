import { describe, expect, it } from "vitest";

import { bitsOf } from "../../../test/bits";
import { gaussLegendre } from "../irradianceQuadrature";
import {
  type BeamSolution,
  type DiscreteOrdinatesProblem,
  DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO,
  DISCRETE_ORDINATE_STREAMS,
  type OrdinateLayer,
  doubleGaussQuadrature,
  factorLayerStack,
  groundCosinesOf,
  type GroundReflectance,
  homogeneousSolution,
  intensityAtAzimuth,
  layerStackRightHandSide,
  normalizedAssociatedLegendre,
  planeParallelBeam,
  solveDiscreteOrdinates,
  solveLayerStack,
} from "./discreteOrdinates";
import { sphericalBeamPath } from "./sphericalBeam";

/** Henyey–Greenstein's moments χ_l = g^l, l < count: the phase function cut at l = count − 1. */
function henyeyGreenstein(g: number, count: number = DISCRETE_ORDINATE_STREAMS): Float64Array {
  return Float64Array.from({ length: count }, (_, l) => g ** l);
}

/** Scalar Rayleigh scattering's moments: χ₂ = 0.1, the rest past χ₀ zero. */
const RAYLEIGH = [1, 0, 0.1] as const;
const ISOTROPIC = [1] as const;

/** A homogeneous slab as `count` equal layers. */
function slab(
  tau: number,
  omega: number,
  moments: ArrayLike<number>,
  count: number = 1,
): OrdinateLayer[] {
  return Array.from({ length: count }, () => ({
    opticalDepth: tau / count,
    singleScatteringAlbedo: omega,
    moments,
  }));
}

function beamAt(
  solution: { readonly beams: ReadonlyArray<BeamSolution> },
  b: number,
): BeamSolution {
  const beam = solution.beams[b];
  if (beam === undefined) {
    throw new Error(`no beam ${b}`);
  }
  return beam;
}

function entry(values: Float64Array, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/** Reflected and transmitted (direct plus diffuse) flux of a beam, at the top and the ground. */
function reflectedAndTransmitted(beam: BeamSolution): { reflected: number; transmitted: number } {
  const { diffuseUp, diffuseDown, direct } = beam.fluxes;
  const last = direct.length - 1;
  return {
    reflected: entry(diffuseUp, 0),
    transmitted: entry(diffuseDown, last) + entry(direct, last),
  };
}

/** P_l(x) by the three-term recurrence. */
function legendre(l: number, x: number): number {
  let p0 = 1;
  let p1 = x;
  if (l === 0) {
    return 1;
  }
  for (let k = 1; k < l; k += 1) {
    const p2 = ((2 * k + 1) * x * p1 - k * p0) / (k + 1);
    p0 = p1;
    p1 = p2;
  }
  return p1;
}

/** The phase function per steradian its moments represent: Σ (2l + 1) ÷ 4π χ_l P_l(cos Θ). */
function phaseFromMoments(moments: ArrayLike<number>, cosTheta: number): number {
  let sum = 0;
  for (let l = 0; l < moments.length; l += 1) {
    sum += ((2 * l + 1) / (4 * Math.PI)) * (moments[l] ?? Number.NaN) * legendre(l, cosTheta);
  }
  return sum;
}

function relative(actual: number, expected: number): number {
  return Math.abs(actual - expected) / Math.abs(expected);
}

describe("the double-Gauss quadrature and the Legendre functions", () => {
  it("puts N = NSTR ÷ 2 Gauss nodes on each hemisphere, weights summing to 1", () => {
    const quadrature = doubleGaussQuadrature();
    expect(quadrature.streams).toBe(16);
    expect(quadrature.nodes).toHaveLength(8);
    const sum = quadrature.weights.reduce((a, b) => a + b, 0);
    expect(sum).toBeCloseTo(1, 14);
    // Exact on [0, 1] to degree 2N − 1 = 15: ∫₀¹ μ^15 dμ = 1 ÷ 16.
    const moment = quadrature.nodes.reduce(
      (a, mu, i) => a + entry(quadrature.weights, i) * mu ** 15,
      0,
    );
    expect(moment).toBeCloseTo(1 / 16, 14);
  });

  it("gives Λ_l^0 = P_l", () => {
    const mu = 0.37;
    const maxDegree = 15;
    const zeroth = normalizedAssociatedLegendre(0, maxDegree, mu);
    for (let l = 0; l <= maxDegree; l += 1) {
      expect(entry(zeroth, l)).toBeCloseTo(legendre(l, mu), 14);
    }
  });

  it("satisfies the addition theorem P_l(cos Θ) = Σ_m (2 − δ_m0) Λ_l^m Λ_l^m cos(mΔφ)", () => {
    const mu = 0.37;
    const muPrime = -0.62;
    const deltaPhi = 1.1;
    const maxDegree = 15;
    const cosTheta =
      mu * muPrime + Math.sqrt(1 - mu * mu) * Math.sqrt(1 - muPrime * muPrime) * Math.cos(deltaPhi);
    for (let l = 0; l <= maxDegree; l += 1) {
      let sum = 0;
      for (let m = 0; m <= l; m += 1) {
        const a = normalizedAssociatedLegendre(m, maxDegree, mu);
        const b = normalizedAssociatedLegendre(m, maxDegree, muPrime);
        sum += (m === 0 ? 1 : 2) * entry(a, l) * entry(b, l) * Math.cos(m * deltaPhi);
      }
      expect(sum).toBeCloseTo(legendre(l, cosTheta), 13);
    }
  });
});

describe("energy at ω = 1 (a)", () => {
  const depths = [1e-3, 0.1, 1, 10, 64, 1000];
  const phases = [
    { name: "Henyey–Greenstein g 0.85", moments: henyeyGreenstein(0.85) },
    { name: "Rayleigh", moments: RAYLEIGH },
  ];
  const beamCosines = [0.1, 0.5, 1];
  for (const { name, moments } of phases) {
    for (const count of [1, 64]) {
      it(`conserves the beam's flux to 10⁻⁶ on ${count} layer(s) of ${name}`, () => {
        for (const tau of depths) {
          const layers = slab(tau, 1, moments, count);
          const black = solveDiscreteOrdinates({
            layers,
            groundAlbedo: 0,
            beamCosines,
            userCosines: [],
            maxMode: 0,
          });
          const white = solveDiscreteOrdinates({
            layers,
            groundAlbedo: 1,
            beamCosines,
            userCosines: [],
            maxMode: 0,
          });
          beamCosines.forEach((mu0, b) => {
            const { reflected, transmitted } = reflectedAndTransmitted(beamAt(black, b));
            expect(Math.abs(reflected + transmitted - mu0)).toBeLessThanOrEqual(1e-6 * mu0);
            const whiteReflected = reflectedAndTransmitted(beamAt(white, b)).reflected;
            expect(Math.abs(whiteReflected - mu0)).toBeLessThanOrEqual(1e-6 * mu0);
          });
        }
      });
    }
  }
});

/**
 * Chandrasekhar's H-function for isotropic scattering at albedo ω, independently of the solver.
 *
 * @remarks
 * H = 1 + μH ∫₀¹ Ψ H(μ′) ÷ (μ + μ′) dμ′ with Ψ = ω ÷ 2 (S. Chandrasekhar, "The transfer of
 * radiation in stellar atmospheres", Bull. Amer. Math. Soc. 53(7) (1947) 641–711,
 * doi:10.1090/S0002-9904-1947-08825-X, eq. 168; _Radiative Transfer_, 1950, not read, and the
 * reflection law below its §9 convention, πF the beam's flux normal to itself). It is iterated in
 * the equivalent form
 * 1 ÷ H(μ) = √(1 − ω) + (ω ÷ 2) ∫₀¹ μ′ H(μ′) ÷ (μ + μ′) dμ′, which follows from it by
 * μ ÷ (μ + μ′) = 1 − μ′ ÷ (μ + μ′) and α₀ = ∫₀¹ H dμ = (2 ÷ ω)(1 − √(1 − ω)), on a 200-point Gauss
 * rule: 19 iterations at ω 0.5 and 49 at 0.9 to 10⁻¹⁵, the first form's residual then 10⁻¹⁵, and
 * H(1) = 1.25126 and 1.85010.
 */
function chandrasekharH(omega: number): {
  at: (mu: number) => number;
  alpha0: number;
  residual: (mu: number) => number;
} {
  const rule = gaussLegendre(200);
  const { nodes, weights } = rule;
  const h = new Float64Array(nodes.length).fill(1);
  const evaluate = (mu: number): number => {
    let integral = 0;
    nodes.forEach((x, k) => {
      integral += entry(weights, k) * ((x * entry(h, k)) / (mu + x));
    });
    return 1 / (Math.sqrt(1 - omega) + (omega / 2) * integral);
  };
  for (let iteration = 0; iteration < 5000; iteration += 1) {
    const next = Float64Array.from(nodes, evaluate);
    let change = 0;
    next.forEach((value, k) => {
      change = Math.max(change, Math.abs(value - entry(h, k)));
    });
    h.set(next);
    if (change < 1e-15) {
      break;
    }
  }
  const alpha0 = h.reduce((a, value, k) => a + entry(weights, k) * value, 0);
  // The first form's residual, H − 1 − (ω ÷ 2) μ H ∫₀¹ H(μ′) ÷ (μ + μ′) dμ′.
  const residual = (mu: number): number => {
    let integral = 0;
    nodes.forEach((x, k) => {
      integral += entry(weights, k) * (entry(h, k) / (mu + x));
    });
    const value = evaluate(mu);
    return value - 1 - (omega / 2) * mu * value * integral;
  };
  return { at: evaluate, alpha0, residual };
}

describe("Chandrasekhar's H-function (b)", () => {
  for (const omega of [0.5, 0.9]) {
    const h = chandrasekharH(omega);

    it(`computes an H at ω ${omega} that satisfies Chandrasekhar's first form to 10⁻¹²`, () => {
      for (const mu of [0.1, 0.5, 1]) {
        expect(Math.abs(h.residual(mu))).toBeLessThan(1e-12);
      }
    });

    // In the iterated form H(0) = 1 exactly when α₀ is right, so this one check covers both.
    it(`computes an H at ω ${omega} with α₀ = (2 ÷ ω)(1 − √(1 − ω))`, () => {
      expect(h.alpha0).toBeCloseTo((2 / omega) * (1 - Math.sqrt(1 - omega)), 12);
    });

    it(`reflects a semi-infinite isotropic medium at ω ${omega} as (ωF ÷ 4) μ₀ ÷ (μ + μ₀) H(μ) H(μ₀) to 10⁻³`, () => {
      const userCosines = [0.1, 0.2, 0.3, 0.45, 0.6, 0.75, 0.9, 1];
      const beamCosines = [0.1, 0.25, 0.5, 0.8, 1];
      const solution = solveDiscreteOrdinates({
        layers: slab(50, omega, ISOTROPIC),
        groundAlbedo: 0,
        beamCosines,
        userCosines,
        maxMode: 0,
      });
      const f = 1 / Math.PI;
      const nodes = Array.from(solution.quadrature.nodes);
      beamCosines.forEach((mu0, b) => {
        const field = beamAt(solution, b).modes[0];
        if (field === undefined) {
          throw new Error("no mode 0");
        }
        const expected = (mu: number): number =>
          ((omega * f) / 4) * (mu0 / (mu + mu0)) * h.at(mu) * h.at(mu0);
        userCosines.forEach((mu, u) => {
          expect(relative(entry(field.userUp, u), expected(mu))).toBeLessThan(1e-3);
        });
        const inRange = nodes.flatMap((mu, i) => (mu >= 0.1 ? [{ mu, i }] : []));
        inRange.forEach(({ mu, i }) => {
          expect(relative(entry(field.quadratureUp, i), expected(mu))).toBeLessThan(1e-3);
        });
      });
    });
  }
});

describe("single scattering (c)", () => {
  const tau = 1e-6;
  const omega = 0.9;
  const beamCosines = [0.3, 0.7];
  const userCosines = [0.2, 0.5, 0.9];
  const azimuths = [0, Math.PI / 3, (2 * Math.PI) / 3, Math.PI];
  const phases = [
    { name: "Rayleigh", moments: RAYLEIGH },
    { name: "Henyey–Greenstein g 0.5 cut at l = 15", moments: henyeyGreenstein(0.5) },
  ];
  for (const { name, moments } of phases) {
    it(`matches the single-scattering closed forms to 10⁻⁴ for ${name}`, () => {
      const solution = solveDiscreteOrdinates({
        layers: slab(tau, omega, moments),
        groundAlbedo: 0,
        beamCosines,
        userCosines,
        maxMode: DISCRETE_ORDINATE_STREAMS - 1,
      });
      beamCosines.forEach((mu0, b) => {
        const beam = beamAt(solution, b);
        const s0 = Math.sqrt(1 - mu0 * mu0);
        userCosines.forEach((mu, u) => {
          const s = Math.sqrt(1 - mu * mu);
          for (const deltaPhi of azimuths) {
            const reflectedPhase = phaseFromMoments(
              moments,
              -mu * mu0 + s * s0 * Math.cos(deltaPhi),
            );
            const reflected =
              omega * reflectedPhase * (mu0 / (mu + mu0)) * -Math.expm1(-tau * (1 / mu + 1 / mu0));
            const transmittedPhase = phaseFromMoments(
              moments,
              mu * mu0 + s * s0 * Math.cos(deltaPhi),
            );
            const transmitted =
              omega *
              transmittedPhase *
              (mu0 / (mu0 - mu)) *
              (Math.exp(-tau / mu0) - Math.exp(-tau / mu));
            expect(
              relative(intensityAtAzimuth(beam, 0, u, "up", deltaPhi), reflected),
            ).toBeLessThan(1e-4);
            expect(
              relative(intensityAtAzimuth(beam, 1, u, "down", deltaPhi), transmitted),
            ).toBeLessThan(1e-4);
          }
        });
      });
    });
  }
});

/**
 * The largest relative difference between two solves' outputs at the top and the bottom, a
 * solve's interfaces being 0 and L: each relative to the larger of the value and a floor, μ₀ for a
 * flux and μ₀ ÷ π for an intensity (a Lambertian field carrying the incident flux).
 */
function largestMiss(a: BeamSolution, b: BeamSolution): number {
  let worst = 0;
  const mu0 = a.beamCosine;
  const lastA = a.fluxes.direct.length - 1;
  const lastB = b.fluxes.direct.length - 1;
  const check = (x: number, y: number, floor: number): void => {
    worst = Math.max(worst, Math.abs(x - y) / Math.max(Math.abs(x), floor));
  };
  for (const key of ["direct", "diffuseDown", "diffuseUp", "meanIntensity"] as const) {
    check(entry(a.fluxes[key], 0), entry(b.fluxes[key], 0), mu0);
    check(entry(a.fluxes[key], lastA), entry(b.fluxes[key], lastB), mu0);
  }
  a.modes.forEach((fieldA, m) => {
    const fieldB = b.modes[m];
    if (fieldB === undefined) {
      throw new Error(`no mode ${m}`);
    }
    for (const key of ["quadratureUp", "quadratureDown", "userUp", "userDown"] as const) {
      const valuesA = fieldA[key];
      const valuesB = fieldB[key];
      const width = valuesA.length / (lastA + 1);
      for (let i = 0; i < width; i += 1) {
        check(entry(valuesA, i), entry(valuesB, i), mu0 / Math.PI);
        check(entry(valuesA, lastA * width + i), entry(valuesB, lastB * width + i), mu0 / Math.PI);
      }
    }
  });
  return worst;
}

describe("layering (d) and one factorisation (e)", () => {
  it("solves a slab as one layer and as 64 alike to 10⁻¹⁰", () => {
    const moments = henyeyGreenstein(0.5);
    const problem = (count: number): DiscreteOrdinatesProblem => ({
      layers: slab(2, 0.9, moments, count),
      groundAlbedo: 0.2,
      beamCosines: [0.15, 0.55, 0.95],
      userCosines: [0.1, 0.33, 0.66, 1],
      maxMode: 3,
    });
    const one = solveDiscreteOrdinates(problem(1));
    const many = solveDiscreteOrdinates(problem(64));
    one.beams.forEach((beam, b) => {
      expect(largestMiss(beam, beamAt(many, b))).toBeLessThanOrEqual(1e-10);
    });
  });

  it("solves every beam on one factorisation as it solves each alone, to 10⁻¹³", () => {
    const layers: OrdinateLayer[] = [
      { opticalDepth: 0.3, singleScatteringAlbedo: 0.99, moments: RAYLEIGH },
      { opticalDepth: 2, singleScatteringAlbedo: 0.95, moments: henyeyGreenstein(0.8) },
      { opticalDepth: 0.7, singleScatteringAlbedo: 0.5, moments: henyeyGreenstein(0.3) },
    ];
    const beamCosines = [0.1, 0.3, 0.5, 0.8, 1];
    const base = { layers, groundAlbedo: 0.3, userCosines: [0.2, 0.7], maxMode: 4 };
    const together = solveDiscreteOrdinates({ ...base, beamCosines });
    beamCosines.forEach((mu0, b) => {
      const alone = solveDiscreteOrdinates({ ...base, beamCosines: [mu0] });
      expect(largestMiss(beamAt(together, b), beamAt(alone, 0))).toBeLessThanOrEqual(1e-13);
    });
  });

  it("solves each beam among the others to the bit of it solved alone, one at an eigenvalue's reciprocal (R08.T14.b)", () => {
    const forward: OrdinateLayer = {
      opticalDepth: 2,
      singleScatteringAlbedo: 0.95,
      moments: henyeyGreenstein(0.8),
    };
    const layers: OrdinateLayer[] = [
      { opticalDepth: 0.3, singleScatteringAlbedo: 0.99, moments: RAYLEIGH },
      forward,
      { opticalDepth: 0.7, singleScatteringAlbedo: 0.5, moments: henyeyGreenstein(0.3) },
    ];
    const rates = homogeneousSolution(forward, 1, doubleGaussQuadrature()).decayRates;
    const rate = Array.from(rates).find((k) => k > 1.5 && k < 4);
    if (rate === undefined) {
      throw new Error("no decay rate between 1.5 and 4");
    }
    // Their right-hand sides solved together (`linearAlgebra.ts`'s `solveBandColumns`), the
    // resonant beam's two shifted solves among them.
    const beamCosines = [0.1, 1 / rate, 0.5, 0.8, 1];
    const base = { layers, groundAlbedo: 0.3, userCosines: [0.2, 0.7], maxMode: 3 };
    const together = solveDiscreteOrdinates({ ...base, beamCosines });
    beamCosines.forEach((mu0, b) => {
      const alone = beamAt(solveDiscreteOrdinates({ ...base, beamCosines: [mu0] }), 0);
      const among = beamAt(together, b);
      expect(bytesOf(among.fluxes)).toEqual(bytesOf(alone.fluxes));
      among.modes.forEach((mode, m) => {
        expect(bytesOf(mode)).toEqual(bytesOf(alone.modes[m] ?? mode));
      });
    });
  });
});

describe("the fields a solve makes (R08.T14.b)", () => {
  const base: DiscreteOrdinatesProblem = {
    layers: slab(2, 0.9, henyeyGreenstein(0.6), 4),
    groundAlbedo: 0.3,
    beamCosines: [0.2, 0.7],
    userCosines: [0.25, 0.8],
    maxMode: 2,
  };

  it("makes the node fields at the interfaces asked and the ground's, to the whole solve's bits, NaN elsewhere", () => {
    const whole = solveDiscreteOrdinates(base);
    const some = solveDiscreteOrdinates({ ...base, fieldInterfaces: [0, 2], userDown: false });
    whole.beams.forEach((beam, b) => {
      const kept = beamAt(some, b);
      beam.modes.forEach((mode, m) => {
        const keptMode = kept.modes[m];
        for (const key of ["quadratureUp", "quadratureDown"] as const) {
          mode[key].forEach((value, i) => {
            const k = Math.floor(i / 8);
            const keptValue = entry(keptMode?.[key] ?? new Float64Array(), i);
            expect(
              [0, 2, 4].includes(k) ? Object.is(keptValue, value) : Number.isNaN(keptValue),
            ).toBe(true);
          });
        }
        expect(keptMode?.userUp).toEqual(mode.userUp);
        expect(keptMode?.userDown.every(Number.isNaN)).toBe(true);
      });
    });
  });

  it("refuses an interface the solve does not have", () => {
    for (const k of [-1, 5, 1.5]) {
      expect(() => solveDiscreteOrdinates({ ...base, fieldInterfaces: [k] })).toThrow(RangeError);
    }
  });
});

/** Every typed array of `value`, by key, as its bytes: two equal are equal to the bit. */
function bytesOf(value: object): Record<string, Uint8Array> {
  const out: Record<string, Uint8Array> = {};
  for (const [key, field] of Object.entries(value)) {
    if (field instanceof Float64Array) {
      out[key] = new Uint8Array(field.buffer, field.byteOffset, field.byteLength);
    }
  }
  return out;
}

/** A slab of τ 1,000 at ω, as `count` layers, lit at three μ₀. */
function thickSlab(omega: number, count: number): ReadonlyArray<BeamSolution> {
  return solveDiscreteOrdinates({
    layers: slab(1000, omega, henyeyGreenstein(0.85), count),
    groundAlbedo: 0,
    beamCosines: [0.1, 0.6, 1],
    userCosines: [0.05, 0.5, 1],
    maxMode: 1,
  }).beams;
}

/** Whether every flux and intensity of a beam is finite. */
function allFinite(beam: BeamSolution): boolean {
  const { direct, diffuseDown, diffuseUp, meanIntensity } = beam.fluxes;
  const arrays: Float64Array[] = [direct, diffuseDown, diffuseUp, meanIntensity];
  for (const field of beam.modes) {
    arrays.push(field.quadratureUp, field.quadratureDown, field.userUp, field.userDown);
  }
  return arrays.every((values) => values.every((value) => Number.isFinite(value)));
}

describe("stability at τ 1,000 (f)", () => {
  for (const count of [1, 64]) {
    it(`stays finite and conserves the beam's flux to 10⁻⁶ at ω 1, on ${count} layer(s)`, () => {
      for (const beam of thickSlab(1, count)) {
        expect(allFinite(beam)).toBe(true);
        const { reflected, transmitted } = reflectedAndTransmitted(beam);
        const mu0 = beam.beamCosine;
        expect(Math.abs(reflected + transmitted - mu0)).toBeLessThanOrEqual(1e-6 * mu0);
      }
    });

    it(`stays finite and absorbs at ω 0.999, on ${count} layer(s)`, () => {
      for (const beam of thickSlab(0.999, count)) {
        expect(allFinite(beam)).toBe(true);
        const { reflected, transmitted } = reflectedAndTransmitted(beam);
        expect(reflected + transmitted).toBeLessThan(beam.beamCosine);
      }
    });
  }
});

describe("reciprocity (g)", () => {
  const layers = slab(1, 0.9, henyeyGreenstein(0.75));
  const quadrature = doubleGaussQuadrature();

  it("is reciprocal to rounding at the quadrature angles, mode by mode", () => {
    const nodes = Array.from(quadrature.nodes);
    const maxMode = 3;
    const solution = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0,
      beamCosines: nodes,
      userCosines: [],
      maxMode,
    });
    for (let m = 0; m <= maxMode; m += 1) {
      const rho = (i: number, j: number): number =>
        (Math.PI * entry(beamAt(solution, j).modes[m]?.quadratureUp ?? new Float64Array(), i)) /
        (nodes[j] ?? Number.NaN);
      let largest = 0;
      for (let i = 0; i < nodes.length; i += 1) {
        for (let j = 0; j < nodes.length; j += 1) {
          largest = Math.max(largest, Math.abs(rho(i, j)));
        }
      }
      for (let i = 0; i < nodes.length; i += 1) {
        for (let j = 0; j < i; j += 1) {
          expect(Math.abs(rho(i, j) - rho(j, i))).toBeLessThanOrEqual(1e-13 * largest);
        }
      }
    }
  });

  it("is reciprocal to rounding at user angles too, mode by mode", () => {
    const cosines = [0.15, 0.35, 0.6, 0.85, 1];
    const maxMode = 3;
    const solution = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0,
      beamCosines: cosines,
      userCosines: cosines,
      maxMode,
    });
    for (let m = 0; m <= maxMode; m += 1) {
      const rho = (u: number, b: number): number =>
        (Math.PI * entry(beamAt(solution, b).modes[m]?.userUp ?? new Float64Array(), u)) /
        (cosines[b] ?? Number.NaN);
      let largest = 0;
      for (let u = 0; u < cosines.length; u += 1) {
        for (let b = 0; b < cosines.length; b += 1) {
          largest = Math.max(largest, Math.abs(rho(u, b)));
        }
      }
      for (let u = 0; u < cosines.length; u += 1) {
        for (let b = 0; b < u; b += 1) {
          expect(Math.abs(rho(u, b) - rho(b, u))).toBeLessThanOrEqual(1e-13 * largest);
        }
      }
    }
  });
});

describe("a Lambertian ground by the adding identity (h)", () => {
  it("gives the plane albedo r + A t t̄ ÷ (1 − A r̄) to rounding", () => {
    const layers = slab(1, 0.9, henyeyGreenstein(0.75));
    const quadrature = doubleGaussQuadrature();
    const nodes = Array.from(quadrature.nodes);
    const tests = [0.2, 0.5, 0.9];
    const albedo = 0.3;
    const cosines = [...nodes, ...tests];
    const black = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0,
      beamCosines: cosines,
      userCosines: [],
      maxMode: 0,
    });
    const grey = solveDiscreteOrdinates({
      layers,
      groundAlbedo: albedo,
      beamCosines: tests,
      userCosines: [],
      maxMode: 0,
    });
    const planeAlbedo = (b: number): number => {
      const beam = beamAt(black, b);
      return reflectedAndTransmitted(beam).reflected / beam.beamCosine;
    };
    const transmittance = (b: number): number => {
      const beam = beamAt(black, b);
      return reflectedAndTransmitted(beam).transmitted / beam.beamCosine;
    };
    let rBar = 0;
    let tBar = 0;
    nodes.forEach((mu, j) => {
      const w = entry(quadrature.weights, j);
      rBar += 2 * w * mu * planeAlbedo(j);
      tBar += 2 * w * mu * transmittance(j);
    });
    tests.forEach((mu0, k) => {
      const b = nodes.length + k;
      const expected = planeAlbedo(b) + (albedo * transmittance(b) * tBar) / (1 - albedo * rBar);
      const actual = reflectedAndTransmitted(beamAt(grey, k)).reflected / mu0;
      expect(relative(actual, expected)).toBeLessThan(1e-12);
    });
  });
});

describe("the singular beam and the conservative limit", () => {
  it("solves a μ₀ at an eigenvalue's reciprocal as its neighbours imply", () => {
    const layer: OrdinateLayer = {
      opticalDepth: 1,
      singleScatteringAlbedo: 0.9,
      moments: henyeyGreenstein(0.7),
    };
    const quadrature = doubleGaussQuadrature();
    for (const mode of [0, 1]) {
      const rates = homogeneousSolution(layer, mode, quadrature).decayRates;
      const rate = Array.from(rates).find((k) => k > 1.5 && k < 4);
      if (rate === undefined) {
        throw new Error("no decay rate between 1.5 and 4");
      }
      const mu0 = 1 / rate;
      // Ten times the window, so that the neighbours are solved directly; their mean is within
      // about δ² ÷ 2 of the second derivative of the value at μ₀ (2 × 10⁻⁸, measured).
      const delta = 1e-4;
      const solution = solveDiscreteOrdinates({
        layers: [layer],
        groundAlbedo: 0.1,
        beamCosines: [mu0 * (1 - delta), mu0, mu0 * (1 + delta)],
        userCosines: [0.3, mu0, 0.8],
        maxMode: mode,
      });
      const fields = [0, 1, 2].map((b) => beamAt(solution, b).modes[mode]);
      const [below, at, above] = fields;
      if (below === undefined || at === undefined || above === undefined) {
        throw new Error("missing mode");
      }
      for (const key of ["quadratureUp", "quadratureDown", "userUp", "userDown"] as const) {
        at[key].forEach((value, i) => {
          expect(Number.isFinite(value)).toBe(true);
          const neighbours = (entry(below[key], i) + entry(above[key], i)) / 2;
          expect(Math.abs(value - neighbours)).toBeLessThan(1e-6 * Math.max(Math.abs(value), 1e-3));
        });
      }
    }
  });

  it("conserves energy at ω = 1 with μ₀ at an eigenvalue's reciprocal", () => {
    const layer: OrdinateLayer = {
      opticalDepth: 3,
      singleScatteringAlbedo: 1,
      moments: henyeyGreenstein(0.85),
    };
    const rates = homogeneousSolution(layer, 0, doubleGaussQuadrature()).decayRates;
    const resonant = Array.from(rates)
      .filter((k) => k >= 1)
      .map((k) => 1 / k);
    const solution = solveDiscreteOrdinates({
      layers: [layer],
      groundAlbedo: 0,
      beamCosines: resonant,
      userCosines: [],
      maxMode: 0,
    });
    resonant.forEach((mu0, b) => {
      const { reflected, transmitted } = reflectedAndTransmitted(beamAt(solution, b));
      expect(Math.abs(reflected + transmitted - mu0)).toBeLessThanOrEqual(1e-6 * mu0);
    });
  });

  it("joins the conservative solution continuously from ω just below 1", () => {
    const solve = (omega: number): BeamSolution =>
      beamAt(
        solveDiscreteOrdinates({
          layers: slab(10, omega, henyeyGreenstein(0.85)),
          groundAlbedo: 0,
          beamCosines: [0.5],
          userCosines: [],
          maxMode: 0,
        }),
        0,
      );
    const conservative = reflectedAndTransmitted(solve(1));
    for (const coAlbedo of [1e-6, 1e-8, 10 * DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO]) {
      const near = reflectedAndTransmitted(solve(1 - coAlbedo));
      const absorbed = 0.5 - near.reflected - near.transmitted;
      expect(absorbed).toBeGreaterThan(0);
      expect(absorbed).toBeLessThan(coAlbedo * 1e3);
      expect(Math.abs(near.reflected - conservative.reflected)).toBeLessThan(coAlbedo * 1e3);
    }
  });

  it("reproduces the quadrature intensities at user cosines on the nodes", () => {
    const quadrature = doubleGaussQuadrature();
    const layers: OrdinateLayer[] = [
      { opticalDepth: 0.5, singleScatteringAlbedo: 1, moments: RAYLEIGH },
      { opticalDepth: 4, singleScatteringAlbedo: 0.97, moments: henyeyGreenstein(0.8) },
    ];
    const solution = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0.4,
      beamCosines: [0.35, 0.9],
      userCosines: quadrature.nodes,
      maxMode: 2,
    });
    for (const beam of solution.beams) {
      for (const field of beam.modes) {
        field.userUp.forEach((value, i) => {
          expect(value).toBeCloseTo(entry(field.quadratureUp, i), 12);
        });
        field.userDown.forEach((value, i) => {
          expect(value).toBeCloseTo(entry(field.quadratureDown, i), 12);
        });
      }
    }
  });

  it("refuses a Henyey–Greenstein g 0.95's raw moments and solves their delta-M truncation", () => {
    // At 16 streams the raw χ_l = g^l give a discrete operator that is not a contraction from
    // g about 0.944 at ω 1 (the module's remarks); delta-M with f = χ₁₆ (Wiscombe 1977) cures it.
    const g = 0.95;
    const raw: OrdinateLayer = {
      opticalDepth: 2,
      singleScatteringAlbedo: 1,
      moments: henyeyGreenstein(g),
    };
    expect(() => homogeneousSolution(raw, 0, doubleGaussQuadrature())).toThrow(RangeError);
    const f = g ** DISCRETE_ORDINATE_STREAMS;
    const truncated: OrdinateLayer = {
      opticalDepth: (1 - f) * raw.opticalDepth,
      singleScatteringAlbedo: 1,
      moments: Array.from(henyeyGreenstein(g), (chi) => (chi - f) / (1 - f)),
    };
    const beam = beamAt(
      solveDiscreteOrdinates({
        layers: [truncated],
        groundAlbedo: 0,
        beamCosines: [0.6],
        userCosines: [],
        maxMode: 0,
      }),
      0,
    );
    const { reflected, transmitted } = reflectedAndTransmitted(beam);
    expect(Math.abs(reflected + transmitted - 0.6)).toBeLessThanOrEqual(1e-6 * 0.6);
  });
});

describe("the pseudo-spherical seam", () => {
  const layers = slab(2, 0.9, henyeyGreenstein(0.6), 4);
  const plane = planeParallelBeam(layers, 0.4);
  const base = { layers, groundAlbedo: 0.1, beamCosines: [0.4], userCosines: [0.5], maxMode: 1 };

  it("solves an explicit plane-parallel beam path exactly as the default one", () => {
    const implicit = beamAt(solveDiscreteOrdinates(base), 0);
    const explicit = beamAt(solveDiscreteOrdinates({ ...base, beamPaths: [plane] }), 0);
    expect(largestMiss(implicit, explicit)).toBe(0);
  });

  it("halves the diffuse field for a beam dimmed to half at every layer's top", () => {
    const implicit = beamAt(solveDiscreteOrdinates(base), 0);
    const dimmed = {
      topTransmission: Array.from(plane.topTransmission, (t) => t / 2),
      slantFactor: plane.slantFactor,
    };
    const half = beamAt(solveDiscreteOrdinates({ ...base, beamPaths: [dimmed] }), 0);
    half.fluxes.diffuseUp.forEach((value, k) => {
      expect(value).toBeCloseTo(entry(implicit.fluxes.diffuseUp, k) / 2, 14);
    });
  });
});

/** An exponential air's share of its column above a height: a scale height of 8 km. */
function exponentialAbove(heightM: number): number {
  return Math.exp(-heightM / 8_000);
}

describe("a beam below the horizontal (R08.T14.d.2)", () => {
  // An exponential air of 64 layers over Earth's radius, τ 0.3 in all, its scale height 8 km.
  const radiusM = 6_371_000;
  const heights = Float64Array.from({ length: 65 }, (_, k) => 100_000 * (1 - k / 64));
  const whole = exponentialAbove(0) - exponentialAbove(100_000);
  const depths = Float64Array.from(
    { length: 64 },
    (_, p) =>
      (0.3 * (exponentialAbove(entry(heights, p + 1)) - exponentialAbove(entry(heights, p)))) /
      whole,
  );
  // μ₀ −1/64 puts the shadow's edge in the lowest layer, −0.1 in the 44th.
  const beamCosines = [-1 / 64, -0.1];
  const beamPaths = beamCosines.map((mu0) => sphericalBeamPath(radiusM, heights, depths, mu0));
  /** Σ_p T_p (1 − e^(−λ_p τ_p)) ÷ λ_p: the beam each path deposits in the column. */
  const deposited = (b: number): number => {
    const path = beamPaths[b];
    if (path === undefined) {
      throw new Error(`no path ${b}`);
    }
    return Array.from(depths).reduce((sum, tau, p) => {
      const lambda = path.slantFactor[p] ?? Number.NaN;
      return sum + ((path.topTransmission[p] ?? Number.NaN) * -Math.expm1(-lambda * tau)) / lambda;
    }, 0);
  };
  const unpathed = (
    moments: ArrayLike<number>,
    groundAlbedo: number,
  ): DiscreteOrdinatesProblem => ({
    layers: Array.from(depths, (opticalDepth) => ({
      opticalDepth,
      singleScatteringAlbedo: 1,
      moments,
    })),
    groundAlbedo,
    beamCosines,
    userCosines: [0.5],
    maxMode: 0,
  });
  const problem = (moments: ArrayLike<number>, groundAlbedo: number): DiscreteOrdinatesProblem => ({
    ...unpathed(moments, groundAlbedo),
    beamPaths,
  });

  for (const { name, moments } of [
    { name: "Rayleigh", moments: RAYLEIGH },
    { name: "Henyey–Greenstein g 0.85", moments: henyeyGreenstein(0.85) },
  ]) {
    it(`conserves the beam its path deposits to 10⁻⁶ at ω 1 on ${name}, a black and a white ground`, () => {
      const black = solveDiscreteOrdinates(problem(moments, 0));
      const white = solveDiscreteOrdinates(problem(moments, 1));
      beamCosines.forEach((_, b) => {
        const beam = deposited(b);
        const { diffuseUp, diffuseDown } = beamAt(black, b).fluxes;
        const out = entry(diffuseUp, 0) + entry(diffuseDown, depths.length);
        expect(Math.abs(out - beam)).toBeLessThanOrEqual(1e-6 * beam);
        const whiteUp = entry(beamAt(white, b).fluxes.diffuseUp, 0);
        expect(Math.abs(whiteUp - beam)).toBeLessThanOrEqual(1e-6 * beam);
      });
    });
  }

  it("leaves the ground unlit by the beam, though the layer the shadow's edge cuts carries one to its base", () => {
    const solution = solveDiscreteOrdinates(problem(RAYLEIGH, 0.3));
    const path = beamPaths[0];
    const last = depths.length - 1;
    expect(path?.topTransmission[last]).toBeGreaterThan(0);
    for (const [b, beam] of solution.beams.entries()) {
      expect(Math.abs(entry(beam.fluxes.direct, depths.length))).toBe(0);
      // Upward, below 0, through each lit interface.
      expect(entry(beam.fluxes.direct, 0)).toBeLessThan(0);
      expect(entry(beam.fluxes.direct, 0)).toBe(
        (beamCosines[b] ?? Number.NaN) * (beamPaths[b]?.topTransmission[0] ?? Number.NaN),
      );
    }
  });

  it("reads no ground reflection of the beam, its ground cosines holding the beams above the horizontal alone", () => {
    const cosines = groundCosinesOf(DISCRETE_ORDINATE_STREAMS, [0.5], [0.4, ...beamCosines]);
    expect(Array.from(cosines.incidentCosines.subarray(-1))).toEqual([0.4]);
    // A Lambertian ground as ρ⁰ = A solves a beam below the horizontal as the albedo does.
    const albedo = 0.3;
    const size = cosines.outgoingCosines.length * cosines.incidentCosines.length;
    const ground: GroundReflectance = { ...cosines, modes: [new Float64Array(size).fill(albedo)] };
    const byAlbedo = solveDiscreteOrdinates(problem(RAYLEIGH, albedo));
    const byGround = solveDiscreteOrdinates({
      ...problem(RAYLEIGH, 0),
      groundReflectance: ground,
    });
    byAlbedo.beams.forEach((beam, b) => {
      const other = beamAt(byGround, b);
      expect(other.fluxes).toEqual(beam.fluxes);
      expect(largestModeMiss(other, beam, depths.length + 1, (mode) => mode.userUp)).toBeLessThan(
        1e-15,
      );
    });
  });

  it("refuses a cosine below the horizontal without its path, or below −1 with one", () => {
    const base = problem(RAYLEIGH, 0);
    expect(() => solveDiscreteOrdinates(unpathed(RAYLEIGH, 0))).toThrow(/without a path/u);
    const [path] = beamPaths;
    if (path === undefined) {
      throw new Error("no path");
    }
    expect(() =>
      solveDiscreteOrdinates({ ...base, beamCosines: [-1.01], beamPaths: [path] }),
    ).toThrow(/in \[−1, 1\]/u);
  });
});

/** A fixed, non-symmetric n × m matrix of order-one entries, by a formula, for the stack's test. */
function fixedMatrix(rows: number, columns: number, seed: number): Float64Array {
  return Float64Array.from(
    { length: rows * columns },
    (_, k) => Math.sin(seed + 1.7 * k) + (k % (columns + 1) === 0 ? 2 : 0),
  );
}

describe("the layer stack", () => {
  it("solves its boundary conditions for a general ground reflection", () => {
    // Three layers of n = 3 streams a hemisphere, arbitrary faces and particular solutions, and a
    // dense ground reflection R with a source S: the coefficients must satisfy every condition
    // `factorLayerStack` states.
    const n = 3;
    const span = 2 * n;
    const layerCount = 3;
    const faces = Array.from({ length: layerCount }, (_, p) => ({
      top: fixedMatrix(span, span, 10 * p),
      bottom: fixedMatrix(span, span, 10 * p + 5),
    }));
    const reflection = Float64Array.from(fixedMatrix(n, n, 99), (v) => 0.1 * v);
    const particularTop = Array.from({ length: layerCount }, (_, p) =>
      fixedMatrix(span, 1, 40 + p),
    );
    const particularBottom = Array.from({ length: layerCount }, (_, p) =>
      fixedMatrix(span, 1, 60 + p),
    );
    const groundSource = fixedMatrix(n, 1, 7);
    const stack = factorLayerStack(faces, n, reflection);
    const x = solveLayerStack(
      stack,
      layerStackRightHandSide(stack, particularTop, particularBottom, groundSource),
    );
    // Layer p's stream r at a face: its solutions times their coefficients, plus its particular.
    const intensity = (
      values: Float64Array | undefined,
      particular: Float64Array | undefined,
      p: number,
      r: number,
    ): number => {
      if (values === undefined || particular === undefined) {
        throw new Error(`no layer ${p}`);
      }
      let sum = entry(particular, r);
      for (let s = 0; s < span; s += 1) {
        sum += entry(values, r * span + s) * entry(x, span * p + s);
      }
      return sum;
    };
    const topOf = (p: number, r: number): number =>
      intensity(faces[p]?.top, particularTop[p], p, r);
    const bottomOf = (p: number, r: number): number =>
      intensity(faces[p]?.bottom, particularBottom[p], p, r);
    for (let i = 0; i < n; i += 1) {
      expect(Math.abs(topOf(0, n + i))).toBeLessThan(1e-12);
    }
    for (let p = 0; p + 1 < layerCount; p += 1) {
      for (let r = 0; r < span; r += 1) {
        expect(Math.abs(bottomOf(p, r) - topOf(p + 1, r))).toBeLessThan(1e-12);
      }
    }
    for (let i = 0; i < n; i += 1) {
      let reflected = entry(groundSource, i);
      for (let j = 0; j < n; j += 1) {
        reflected += entry(reflection, i * n + j) * bottomOf(layerCount - 1, n + j);
      }
      expect(Math.abs(bottomOf(layerCount - 1, i) - reflected)).toBeLessThan(1e-12);
    }
  });
});

/**
 * A slab's reflectance read the way `deck.ts` reads a deck's: ρ^m(μ, μ′) = π I^m(0, +μ) ÷ μ′ for a
 * plane-parallel beam at μ′, at each outgoing cosine (as a user cosine) and each incident one.
 */
function slabReflectance(
  layers: ReadonlyArray<OrdinateLayer>,
  groundAlbedo: number,
  cosines: { readonly outgoingCosines: Float64Array; readonly incidentCosines: Float64Array },
  maxMode: number,
): GroundReflectance {
  const { outgoingCosines, incidentCosines } = cosines;
  const solution = solveDiscreteOrdinates({
    layers,
    groundAlbedo,
    beamCosines: incidentCosines,
    userCosines: outgoingCosines,
    maxMode,
  });
  const columns = incidentCosines.length;
  const modes = Array.from({ length: maxMode + 1 }, (_, m) => {
    const rho = new Float64Array(outgoingCosines.length * columns);
    for (let j = 0; j < columns; j += 1) {
      const up = beamAt(solution, j).modes[m]?.userUp ?? new Float64Array();
      for (let i = 0; i < outgoingCosines.length; i += 1) {
        rho[i * columns + j] = (Math.PI * entry(up, i)) / entry(incidentCosines, j);
      }
    }
    return rho;
  });
  return { outgoingCosines, incidentCosines, modes };
}

/** The largest |a − b| over two beams' modes at the interfaces `interfaces`, of each mode's largest |b|. */
function largestModeMiss(
  actual: BeamSolution,
  expected: BeamSolution,
  interfaces: number,
  pick: (mode: BeamSolution["modes"][number]) => Float64Array,
): number {
  let worst = 0;
  expected.modes.forEach((mode, m) => {
    const other = actual.modes[m];
    if (other === undefined) {
      throw new Error(`no mode ${m}`);
    }
    const want = pick(mode);
    const got = pick(other);
    const width = want.length / expected.fluxes.direct.length;
    const count = interfaces * width;
    let largest = 0;
    for (let i = 0; i < count; i += 1) {
      largest = Math.max(largest, Math.abs(entry(want, i)));
    }
    // A mode that vanishes (any m > 0 under an overhead sun) is held absolutely.
    const scale = largest > 0 ? largest : 1;
    for (let i = 0; i < count; i += 1) {
      worst = Math.max(worst, Math.abs(entry(got, i) - entry(want, i)) / scale);
    }
  });
  return worst;
}

describe("a ground other than Lambertian (R08.T15)", () => {
  const beamCosines = [0.25, 0.6, 1];
  const userCosines = [0.15, 0.5, 0.9];
  const maxMode = 3;
  const cosines = groundCosinesOf(DISCRETE_ORDINATE_STREAMS, userCosines, beamCosines);
  const base = { beamCosines, userCosines, maxMode };

  // Measured 2026-10-29: the user cosines 1.5 × 10⁻¹⁷ of each mode's largest; the split 2.3 × 10⁻¹³.
  it("reproduces a Lambertian ground given as ρ⁰ = A: the nodes to the bit, the user cosines to 10⁻¹⁵", () => {
    const layers = [...slab(0.4, 0.95, RAYLEIGH, 2), ...slab(3, 0.9, henyeyGreenstein(0.7), 3)];
    const albedo = 0.3;
    const size = cosines.outgoingCosines.length * cosines.incidentCosines.length;
    const lambertian: GroundReflectance = {
      ...cosines,
      modes: [new Float64Array(size).fill(albedo)],
    };
    const albedoSolve = solveDiscreteOrdinates({ ...base, layers, groundAlbedo: albedo });
    const groundSolve = solveDiscreteOrdinates({
      ...base,
      layers,
      groundAlbedo: 0,
      groundReflectance: lambertian,
    });
    let userMiss = 0;
    albedoSolve.beams.forEach((beam, b) => {
      const other = beamAt(groundSolve, b);
      expect(other.fluxes).toEqual(beam.fluxes);
      beam.modes.forEach((mode, m) => {
        const twin = other.modes[m];
        expect([twin?.quadratureUp, twin?.quadratureDown, twin?.userDown]).toEqual([
          mode.quadratureUp,
          mode.quadratureDown,
          mode.userDown,
        ]);
      });
      userMiss = Math.max(
        userMiss,
        largestModeMiss(other, beam, layers.length + 1, (mode) => mode.userUp),
      );
    });
    expect(userMiss).toBeLessThan(1e-15);
  });

  it("gives a slab's reflectance as the ground the solve of the whole column, to 10⁻¹²", () => {
    const upper = slab(0.5, 0.97, henyeyGreenstein(0.5), 3);
    const lower = slab(6, 0.95, henyeyGreenstein(0.75), 4);
    const albedo = 0.2;
    const deck = slabReflectance(lower, albedo, cosines, maxMode);
    const split = solveDiscreteOrdinates({
      ...base,
      layers: upper,
      groundAlbedo: 0,
      groundReflectance: deck,
    });
    const whole = solveDiscreteOrdinates({
      ...base,
      layers: [...upper, ...lower],
      groundAlbedo: albedo,
    });
    let worst = 0;
    whole.beams.forEach((beam, b) => {
      const other = beamAt(split, b);
      for (const pick of [
        (mode: BeamSolution["modes"][number]) => mode.quadratureUp,
        (mode: BeamSolution["modes"][number]) => mode.quadratureDown,
        (mode: BeamSolution["modes"][number]) => mode.userUp,
        (mode: BeamSolution["modes"][number]) => mode.userDown,
      ]) {
        worst = Math.max(worst, largestModeMiss(other, beam, upper.length + 1, pick));
      }
    });
    expect(worst).toBeLessThan(1e-12);
  });

  it("refuses a ground reflectance beside an albedo, missing a cosine, of the wrong size or not finite", () => {
    const layers = slab(1, 0.9, RAYLEIGH);
    const size = cosines.outgoingCosines.length * cosines.incidentCosines.length;
    const ground: GroundReflectance = { ...cosines, modes: [new Float64Array(size).fill(0.2)] };
    const solve = (groundAlbedo: number, groundReflectance: GroundReflectance): unknown =>
      solveDiscreteOrdinates({ ...base, layers, groundAlbedo, groundReflectance });
    expect(() => solve(0.1, ground)).toThrow(RangeError);
    const fewer = groundCosinesOf(DISCRETE_ORDINATE_STREAMS, userCosines.slice(1), beamCosines);
    const fewerSize = fewer.outgoingCosines.length * fewer.incidentCosines.length;
    expect(() => solve(0, { ...fewer, modes: [new Float64Array(fewerSize).fill(0.2)] })).toThrow(
      RangeError,
    );
    expect(() => solve(0, { ...cosines, modes: [new Float64Array(size - 1)] })).toThrow(RangeError);
    expect(() =>
      solve(0, { ...cosines, modes: [new Float64Array(size).fill(Number.NaN)] }),
    ).toThrow(RangeError);
  });
});

describe("the refusals", () => {
  const valid: DiscreteOrdinatesProblem = {
    layers: slab(1, 0.9, RAYLEIGH),
    groundAlbedo: 0.1,
    beamCosines: [0.5],
    userCosines: [0.5],
    maxMode: 1,
  };

  it("refuses a stream count that is odd, too small or not whole", () => {
    for (const streams of [15, 0, 1, 7.5, -2]) {
      expect(() => solveDiscreteOrdinates({ ...valid, streams })).toThrow(RangeError);
      expect(() => doubleGaussQuadrature(streams)).toThrow(RangeError);
    }
  });

  it("refuses an optical depth that is negative or not finite", () => {
    for (const opticalDepth of [-1e-9, Number.NaN, Number.POSITIVE_INFINITY]) {
      const layers = [{ opticalDepth, singleScatteringAlbedo: 0.5, moments: RAYLEIGH }];
      expect(() => solveDiscreteOrdinates({ ...valid, layers })).toThrow(RangeError);
    }
  });

  it("refuses a solve with no layers", () => {
    expect(() => solveDiscreteOrdinates({ ...valid, layers: [] })).toThrow(RangeError);
  });

  it("refuses a single-scattering albedo outside [0, 1]", () => {
    for (const singleScatteringAlbedo of [-0.01, 1.000001, Number.NaN]) {
      const layers = [{ opticalDepth: 1, singleScatteringAlbedo, moments: RAYLEIGH }];
      expect(() => solveDiscreteOrdinates({ ...valid, layers })).toThrow(RangeError);
    }
  });

  it("refuses χ₀ away from 1 and moments that are not finite", () => {
    for (const moments of [[0.99], [1 + 1e-9, 0.5], [], [1, Number.NaN]]) {
      const layers = [{ opticalDepth: 1, singleScatteringAlbedo: 0.5, moments }];
      expect(() => solveDiscreteOrdinates({ ...valid, layers })).toThrow(RangeError);
    }
  });

  it("refuses a beam or user cosine outside (0, 1]", () => {
    for (const cosine of [0, -0.5, 1.0001, Number.NaN]) {
      expect(() => solveDiscreteOrdinates({ ...valid, beamCosines: [cosine] })).toThrow(RangeError);
      expect(() => solveDiscreteOrdinates({ ...valid, userCosines: [cosine] })).toThrow(RangeError);
    }
  });

  it("refuses a ground albedo outside [0, 1]", () => {
    for (const groundAlbedo of [-0.1, 1.5, Number.NaN]) {
      expect(() => solveDiscreteOrdinates({ ...valid, groundAlbedo })).toThrow(RangeError);
    }
  });

  it("refuses a mode past NSTR − 1, negative or not whole", () => {
    for (const maxMode of [16, -1, 1.5]) {
      expect(() => solveDiscreteOrdinates({ ...valid, maxMode })).toThrow(RangeError);
    }
    expect(() => solveDiscreteOrdinates({ ...valid, streams: 4, maxMode: 4 })).toThrow(RangeError);
  });

  it("refuses beam paths that do not fit the beams and layers", () => {
    const path = planeParallelBeam(valid.layers, 0.5);
    const bad = [
      [],
      [path, path],
      [{ topTransmission: [1.5], slantFactor: [2] }],
      [{ topTransmission: [1], slantFactor: [0] }],
      [{ topTransmission: [1, 1], slantFactor: [2, 2] }],
    ];
    for (const beamPaths of bad) {
      expect(() => solveDiscreteOrdinates({ ...valid, beamPaths })).toThrow(RangeError);
    }
  });
});

describe("the helpers' refusals", () => {
  const quadrature = doubleGaussQuadrature();

  it("refuses a Legendre order or degree that is not whole, or a cosine outside [−1, 1]", () => {
    for (const [order, degree, mu] of [
      [-1, 4, 0.5],
      [0.5, 4, 0.5],
      [0, -1, 0.5],
      [0, 2.5, 0.5],
      [0, 4, 1.01],
      [0, 4, Number.NaN],
    ] as const) {
      expect(() => normalizedAssociatedLegendre(order, degree, mu)).toThrow(RangeError);
    }
  });

  it("refuses moments whose odd part is not a contraction, the factorisation's error as cause", () => {
    // χ₁ = 1.5 at ω 1 puts ωχ₁ past 1, so −P is not positive definite.
    const layer = { opticalDepth: 1, singleScatteringAlbedo: 1, moments: [1, 1.5] };
    let caught: unknown;
    try {
      homogeneousSolution(layer, 0, quadrature);
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(RangeError);
    expect(caught instanceof Error ? caught.cause : undefined).toBeInstanceOf(RangeError);
  });

  it("refuses moments whose even part gives a negative k²", () => {
    // χ₂ = 1.5 at ω 0.9 puts ωχ₂ past 1, so Lᵀ(−Q)L has a negative eigenvalue.
    const layer = { opticalDepth: 1, singleScatteringAlbedo: 0.9, moments: [1, 0, 1.5] };
    expect(() => homogeneousSolution(layer, 0, quadrature)).toThrow(RangeError);
  });

  it("refuses a plane-parallel beam at a cosine outside (0, 1]", () => {
    const layers = slab(1, 0.5, RAYLEIGH);
    for (const cosine of [0, 1.5, Number.NaN]) {
      expect(() => planeParallelBeam(layers, cosine)).toThrow(RangeError);
    }
  });

  it("refuses a plane-parallel beam over a layer of negative optical depth", () => {
    const bad = [{ opticalDepth: -1, singleScatteringAlbedo: 0.5, moments: RAYLEIGH }];
    expect(() => planeParallelBeam(bad, 0.5)).toThrow(RangeError);
  });

  it("refuses a stack with no layers, faces of the wrong size or a reflection of the wrong size", () => {
    const n = 2;
    const face = { top: new Float64Array(16), bottom: new Float64Array(16) };
    const short = { top: new Float64Array(15), bottom: new Float64Array(16) };
    for (const [faces, reflection] of [
      [[], undefined],
      [[short], undefined],
      [[face], new Float64Array(3)],
    ] as const) {
      expect(() => factorLayerStack(faces, n, reflection)).toThrow(RangeError);
    }
  });

  it("refuses a stack's right-hand side of the wrong count or size", () => {
    const face = { top: Float64Array.from([1, 0, 0, 1]), bottom: Float64Array.from([2, 0, 0, 3]) };
    const stack = factorLayerStack([face], 1, undefined);
    const two = new Float64Array(2);
    for (const [top, bottom, ground] of [
      [[], [two], undefined],
      [[new Float64Array(3)], [two], undefined],
      [[two], [two], new Float64Array(2)],
    ] as const) {
      expect(() => layerStackRightHandSide(stack, top, bottom, ground)).toThrow(RangeError);
    }
  });

  it("refuses an intensity at an interface or user cosine out of range", () => {
    const beam = beamAt(
      solveDiscreteOrdinates({
        layers: slab(1, 0.9, RAYLEIGH),
        groundAlbedo: 0,
        beamCosines: [0.5],
        userCosines: [0.5, 1],
        maxMode: 1,
      }),
      0,
    );
    for (const [k, u] of [
      [2, 0],
      [-1, 1],
      [0, 2],
      [0, -1],
      [0.5, 0],
    ] as const) {
      expect(() => intensityAtAzimuth(beam, k, u, "up", 0)).toThrow(RangeError);
    }
  });
});

/**
 * Henyey–Greenstein's χ_l = g^l to l = 15 by repeated products, which are the same bits on every
 * platform, where `**` need not be (`lib/pinnedMathPaths.test.ts`).
 */
function powersOf(g: number): Float64Array {
  const out = new Float64Array(DISCRETE_ORDINATE_STREAMS);
  let power = 1;
  for (let l = 0; l < out.length; l += 1) {
    out[l] = power;
    power *= g;
  }
  return out;
}

describe("a solve's bits (R08.T14.b)", () => {
  it("gives a fixed column every field to the bits it gave before the bake's levers", async () => {
    // A conservative layer, one that absorbs alone, forward peaks, a beam at an eigenvalue's
    // reciprocal, one below the horizontal on its spherical path, a Lambertian ground and an
    // emitting column: every path of the solve, pinned to R08.T14.a's arithmetic.
    const forward: OrdinateLayer = {
      opticalDepth: 1.3,
      singleScatteringAlbedo: 0.8,
      moments: powersOf(0.5),
    };
    const layers: OrdinateLayer[] = [
      { opticalDepth: 0.05, singleScatteringAlbedo: 1, moments: RAYLEIGH },
      { opticalDepth: 0.4, singleScatteringAlbedo: 0.97, moments: powersOf(0.7) },
      forward,
      { opticalDepth: 0.2, singleScatteringAlbedo: 0, moments: ISOTROPIC },
      { opticalDepth: 2.5, singleScatteringAlbedo: 0.999, moments: powersOf(0.85) },
      { opticalDepth: 0.6, singleScatteringAlbedo: 0.6, moments: RAYLEIGH },
    ];
    const rates = homogeneousSolution(forward, 1, doubleGaussQuadrature()).decayRates;
    const rate = Array.from(rates).find((k) => k > 1.5 && k < 4);
    if (rate === undefined) {
      throw new Error("no decay rate between 1.5 and 4");
    }
    const beamCosines = [-0.05, 0.1, 1 / rate, 0.55, 1];
    const heights = Float64Array.from({ length: 7 }, (_, k) => 60_000 * (1 - k / 6));
    const depths = Float64Array.from(layers, (layer) => layer.opticalDepth);
    const solution = solveDiscreteOrdinates({
      layers,
      groundAlbedo: 0.2,
      beamCosines,
      userCosines: [0.1, 0.45, 0.9],
      maxMode: 3,
      beamPaths: beamCosines.map((mu0) => sphericalBeamPath(6_371_000, heights, depths, mu0)),
      thermal: { interfacePlanck: [0.1, 0.2, 0.4, 0.5, 0.9, 1.1, 1.4], groundPlanck: 1.6 },
    });
    expect(solution.thermal).toBeDefined();
    expect(await bitsOf(solution)).toBe(
      "d9da446ba8e60802a6053a0b589a755e0ae84505d5b07ffe352e4e28557ade58",
    );
  });
});
