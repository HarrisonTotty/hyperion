/**
 * The vector discrete-ordinates mode (plan R08, R08.T14.e): the generalised spherical functions and
 * the matrices' moments, the azimuthal modes against the matrix rotated geometrically, Natraj, Li
 * and Yung 2009's τ 0.5 table, single scattering, energy at ω = 1, the depolarisers against the
 * scalar solver, delta-M on the matrix, layering and one factorisation, and the refusals. Every
 * check is deterministic.
 */
import { describe, expect, it } from "vitest";

import { gaussLegendre } from "../../lighting/quadrature";
import { phaseTable } from "../medium";
import { mieSphere } from "../mie";
import { PHASE_TABLE_MU, PHASE_TABLE_U, sphereScatteringMatrix } from "../sizeDistribution";
import { phaseTableMoment } from "../forwardPeak";
import {
  normalizedAssociatedLegendre,
  solveDiscreteOrdinates,
  type StreamDirection,
} from "./discreteOrdinates";
import {
  depolarisingMoments,
  type MatrixMoments,
  meridianPhaseMatrix,
  type PolarisedBeamSolution,
  type PolarisedLayer,
  type PolarisedProblem,
  rayleighMatrixMoments,
  solvePolarised,
  stokesAtAzimuth,
  tableMatrixMoments,
  truncateMatrixMoments,
  wignerD,
} from "./polarisation";
import { phaseMoments } from "./spectralColumn";

/** An array's entry, or a test failure. */
function entry(values: ArrayLike<number>, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/** A solution's one beam, or a test failure. */
function beamOf(
  solution: { readonly beams: ReadonlyArray<PolarisedBeamSolution> },
  b = 0,
): PolarisedBeamSolution {
  const beam = solution.beams[b];
  if (beam === undefined) {
    throw new Error(`no beam ${b}`);
  }
  return beam;
}

/** Rayleigh's moments to l = 32 at ρ. */
function rayleigh(rho: number, order = 32): MatrixMoments {
  return rayleighMatrixMoments(
    order,
    rho,
    phaseMoments({ kind: "rayleigh", depolarisation: rho }, order),
  );
}

// ---------------------------------------------------------------------------------------------
// The geometry of the meridian frames, as the reference tracer's (east, north, up; azimuth from
// north toward east): built here from vectors, independent of the module's expansions.

type Vec = readonly [number, number, number];

const dot = (a: Vec, b: Vec): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec, b: Vec): Vec => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

/** The direction of travel of cosine μ and azimuth φ from north toward east. */
function travelDirection(mu: number, phi: number): Vec {
  const s = Math.sqrt(1 - mu * mu);
  return [s * Math.sin(phi), s * Math.cos(phi), mu];
}

/** e₁ = ∂k ÷ ∂θ, in k's vertical plane. */
function firstAxis(mu: number, phi: number): Vec {
  const s = Math.sqrt(1 - mu * mu);
  return [mu * Math.sin(phi), mu * Math.cos(phi), -s];
}

/** The Stokes rotation for a frame turned by α, (cos α, sin α), e₁′ = cos α e₁ + sin α e₂. */
function rotation(c: number, s: number): number[] {
  const c2 = c * c - s * s;
  const s2 = 2 * s * c;
  return [1, 0, 0, 0, 0, c2, s2, 0, 0, -s2, c2, 0, 0, 0, 0, 1];
}

function times(a: ReadonlyArray<number>, b: ReadonlyArray<number>): number[] {
  const out = Array.from({ length: 16 }, () => 0);
  for (let i = 0; i < 4; i += 1) {
    for (let j = 0; j < 4; j += 1) {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) {
        sum += entry(a, i * 4 + k) * entry(b, k * 4 + j);
      }
      out[i * 4 + j] = sum;
    }
  }
  return out;
}

/**
 * The phase matrix from the scattering matrix F(cos Θ) (the scattering plane's frame, e∥ = n × k,
 * e⊥ = n = k′ × k ÷ |k′ × k|), rotated from the incident direction's meridian frame and back into
 * the scattered one's.
 */
function geometricPhaseMatrix(
  scattering: (cosTheta: number) => number[],
  mu: number,
  phi: number,
  muPrime: number,
  phiPrime: number,
): number[] {
  const k = travelDirection(mu, phi);
  const kPrime = travelDirection(muPrime, phiPrime);
  const across = cross(kPrime, k);
  const length = Math.hypot(...across);
  const n: Vec = [across[0] / length, across[1] / length, across[2] / length];
  const parallelIn = cross(n, kPrime);
  const parallelOut = cross(n, k);
  const e1In = firstAxis(muPrime, phiPrime);
  const e2In = cross(kPrime, e1In);
  const e1Out = firstAxis(mu, phi);
  const toScattering = rotation(dot(parallelIn, e1In), dot(parallelIn, e2In));
  const toMeridian = rotation(dot(e1Out, parallelOut), dot(e1Out, n));
  return times(toMeridian, times(scattering(dot(k, kPrime)), toScattering));
}

/** Hansen and Travis 1974's Rayleigh matrix (eqs. 2.15–2.16), normalised as a₁. */
function rayleighScattering(rho: number): (c: number) => number[] {
  const big = (1 - rho) / (1 + rho / 2);
  const circular = (1 - 2 * rho) / (1 - rho);
  return (c) => {
    const a1 = big * 0.75 * (1 + c * c) + (1 - big);
    const a2 = big * 0.75 * (1 + c * c);
    const a3 = big * 1.5 * c;
    const a4 = big * circular * 1.5 * c;
    const b1 = -big * 0.75 * (1 - c * c);
    return [a1, b1, 0, 0, b1, a2, 0, 0, 0, 0, a3, 0, 0, 0, 0, a4];
  };
}

/** The scattering matrix a set of moments describes, to l = `order`. */
function synthesised(moments: MatrixMoments, order: number): (c: number) => number[] {
  return (c) => {
    const d00 = wignerD(0, 0, order, c);
    const d22 = wignerD(2, 2, order, c);
    const d2m2 = wignerD(2, -2, order, c);
    const d02 = wignerD(0, 2, order, c);
    let a1 = 0;
    let a4 = 0;
    let plus = 0;
    let minus = 0;
    let b1 = 0;
    let b2 = 0;
    for (let l = 0; l <= order; l += 1) {
      const f = 2 * l + 1;
      const at = (values: ArrayLike<number>): number => values[l] ?? 0;
      a1 += f * at(moments.a1) * entry(d00, l);
      a4 += f * at(moments.a4) * entry(d00, l);
      plus += f * (at(moments.a2) + at(moments.a3)) * entry(d22, l);
      minus += f * (at(moments.a2) - at(moments.a3)) * entry(d2m2, l);
      b1 += f * at(moments.b1) * entry(d02, l);
      b2 += f * at(moments.b2) * entry(d02, l);
    }
    const a2 = (plus + minus) / 2;
    const a3 = (plus - minus) / 2;
    return [a1, b1, 0, 0, b1, a2, 0, 0, 0, 0, a3, b2, 0, 0, -b2, a4];
  };
}

/** A fixed matrix's moments with every element and b₂, l ≤ 5: a test's, not a physical one. */
const SYNTHETIC: MatrixMoments = {
  a1: [1, 0.3, 0.2, 0.1, 0.05, 0.02],
  a2: [0, 0, 0.6, 0.3, 0.1, 0.04],
  a3: [0, 0, 0.4, 0.2, 0.15, 0.03],
  a4: [0.2, 0.25, 0.3, 0.1, 0.05, 0.01],
  b1: [0, 0, -0.2, 0.1, -0.05, 0.02],
  b2: [0, 0, 0.1, -0.05, 0.03, 0.01],
};

/**
 * A water-like sphere's matrix moments, x 3 and m 1.33 + 10⁻⁴ i (Mie, `mie.ts`), on the phase
 * tables' u as `aerosol.ts` lays one, delta-M truncated at `streams` with the identity's delta:
 * a matrix with b₂, which the vector mode carries V for.
 */
function sphereMoments(streams: number): MatrixMoments {
  const mie = mieSphere(3, { n: 1.33, k: 1e-4 }, PHASE_TABLE_MU);
  const matrix = sphereScatteringMatrix(mie, 3, PHASE_TABLE_MU);
  const quarter = 1 / (4 * Math.PI);
  const scaled = (values: Float64Array): Float64Array => values.map((v) => v * quarter);
  const a1 = scaled(matrix.a1);
  const norm = 1 / phaseTableMoment(PHASE_TABLE_U, a1, () => 1);
  const normalised = (values: Float64Array): Float64Array => scaled(values).map((v) => v * norm);
  const values = a1.map((v) => v * norm);
  const elements = {
    a2: normalised(matrix.a2),
    a3: normalised(matrix.a3),
    a4: normalised(matrix.a4),
    b1: normalised(matrix.b1),
    b2: normalised(matrix.b2),
  };
  const table = phaseTable(PHASE_TABLE_U, [values, values, values], {
    a2: [elements.a2, elements.a2, elements.a2],
    a3: [elements.a3, elements.a3, elements.a3],
    a4: [elements.a4, elements.a4, elements.a4],
    b1: [elements.b1, elements.b1, elements.b1],
    b2: [elements.b2, elements.b2, elements.b2],
  });
  const full = tableMatrixMoments(
    streams,
    PHASE_TABLE_U,
    elements,
    phaseMoments({ kind: "tabulated", table }, streams),
  );
  const f = entry(full.a1, streams);
  const diagonal = (element: ArrayLike<number>, from: number): number[] =>
    Array.from({ length: streams }, (_, l) => (l < from ? 0 : (entry(element, l) - f) / (1 - f)));
  const off = (element: ArrayLike<number>): number[] =>
    Array.from({ length: streams }, (_, l) => entry(element, l) / (1 - f));
  return {
    a1: Array.from({ length: streams }, (_, l) =>
      l === 0 ? 1 : (entry(full.a1, l) - f) / (1 - f),
    ),
    a2: diagonal(full.a2, 2),
    a3: diagonal(full.a3, 2),
    a4: diagonal(full.a4, 0),
    b1: off(full.b1),
    b2: off(full.b2),
  };
}

// ---------------------------------------------------------------------------------------------

describe("the generalised spherical functions", () => {
  const cosines = [-1, -0.83, -0.2, 0, 0.35, 0.91, 1];

  it("give d^l_m0 = (−1)^m Λ_l^m, the scalar solver's normalised Legendre functions", () => {
    for (const mu of cosines) {
      for (let m = 0; m <= 4; m += 1) {
        const d = wignerD(m, 0, 12, mu);
        const lambda = normalizedAssociatedLegendre(m, 12, mu);
        for (let l = 0; l <= 12; l += 1) {
          expect(entry(d, l)).toBeCloseTo((m % 2 === 0 ? 1 : -1) * entry(lambda, l), 13);
        }
      }
    }
  });

  it("give the closed forms of degree 2: ((1 ± μ) ÷ 2)² and √(3 ÷ 8)(1 − μ²)", () => {
    for (const mu of cosines) {
      expect(entry(wignerD(2, 2, 4, mu), 2)).toBeCloseTo(((1 + mu) / 2) ** 2, 15);
      expect(entry(wignerD(2, -2, 4, mu), 2)).toBeCloseTo(((1 - mu) / 2) ** 2, 15);
      expect(entry(wignerD(0, 2, 4, mu), 2)).toBeCloseTo(Math.sqrt(3 / 8) * (1 - mu * mu), 15);
      expect(entry(wignerD(1, 0, 4, mu), 1)).toBeCloseTo(-Math.sqrt((1 - mu * mu) / 2), 15);
    }
  });

  it("are orthogonal, ∫ d^l_mn d^k_mn dμ = 2 δ_lk ÷ (2l + 1), and mirror as (−1)^{l+m} d^l_m,−n", () => {
    const rule = gaussLegendre(40);
    for (const [m, n] of [
      [0, 2],
      [2, 2],
      [2, -2],
      [3, 2],
      [1, -2],
    ] as const) {
      const tables = Array.from(rule.x, (x) => wignerD(m, n, 18, x));
      for (let l = 0; l <= 18; l += 1) {
        for (let k = 0; k <= 18; k += 1) {
          let sum = 0;
          tables.forEach((d, i) => {
            sum += entry(rule.w, i) * entry(d, l) * entry(d, k);
          });
          const lowest = Math.max(Math.abs(m), Math.abs(n));
          const expected = l === k && l >= lowest ? 2 / (2 * l + 1) : 0;
          expect(Math.abs(sum - expected)).toBeLessThan(1e-13);
        }
      }
      for (const mu of cosines) {
        const here = wignerD(m, n, 18, mu);
        const mirrored = wignerD(m, -n, 18, -mu);
        for (let l = 0; l <= 18; l += 1) {
          expect(entry(mirrored, l)).toBeCloseTo(
            (l + m) % 2 === 0 ? entry(here, l) : -entry(here, l),
            13,
          );
        }
      }
    }
  });

  it("refuses an order that is not whole, a negative degree or a cosine outside [−1, 1]", () => {
    expect(() => wignerD(0.5, 0, 4, 0)).toThrow(RangeError);
    expect(() => wignerD(0, 0, -1, 0)).toThrow(RangeError);
    expect(() => wignerD(0, 2, 4, 1.01)).toThrow(RangeError);
  });
});

describe("the matrices' moments", () => {
  it("gives Rayleigh's closed form, held to a 32-point quadrature of Hansen and Travis's matrix", () => {
    const rule = gaussLegendre(32);
    for (const rho of [0, 0.0279, 0.09]) {
      const moments = rayleigh(rho, 8);
      const f = rayleighScattering(rho);
      for (let l = 0; l <= 8; l += 1) {
        let a1 = 0;
        let plus = 0;
        let minus = 0;
        let a4 = 0;
        let b1 = 0;
        rule.x.forEach((x, i) => {
          const w = entry(rule.w, i) / 2;
          const m = f(x);
          a1 += w * entry(m, 0) * entry(wignerD(0, 0, l, x), l);
          plus += w * (entry(m, 5) + entry(m, 10)) * entry(wignerD(2, 2, Math.max(l, 2), x), l);
          minus += w * (entry(m, 5) - entry(m, 10)) * entry(wignerD(2, -2, Math.max(l, 2), x), l);
          a4 += w * entry(m, 15) * entry(wignerD(0, 0, l, x), l);
          b1 += w * entry(m, 1) * entry(wignerD(0, 2, Math.max(l, 2), x), l);
        });
        expect(Math.abs(entry(moments.a1, l) - a1)).toBeLessThan(1e-14);
        expect(Math.abs(entry(moments.a2, l) - (plus + minus) / 2)).toBeLessThan(1e-14);
        expect(Math.abs(entry(moments.a3, l) - (plus - minus) / 2)).toBeLessThan(1e-14);
        expect(Math.abs(entry(moments.a4, l) - a4)).toBeLessThan(1e-14);
        expect(Math.abs(entry(moments.b1, l) - b1)).toBeLessThan(1e-14);
        expect(entry(moments.b2, l)).toBe(0);
      }
    }
  });

  it("gives a table's moments by its own quadrature: Rayleigh's matrix laid on the tables' u, to 10⁻⁴", () => {
    const rho = 0.0279;
    const f = rayleighScattering(rho);
    const quarter = 1 / (4 * Math.PI);
    const element = (k: number): Float64Array =>
      PHASE_TABLE_MU.map((mu) => entry(f(mu), k) * quarter);
    const a1 = element(0);
    const table = phaseTable(PHASE_TABLE_U, [a1, a1, a1]);
    const moments = tableMatrixMoments(
      8,
      PHASE_TABLE_U,
      { a2: element(5), a3: element(10), a4: element(15), b1: element(1), b2: element(11) },
      phaseMoments({ kind: "tabulated", table }, 8),
    );
    const closed = rayleigh(rho, 8);
    // Measured 1.2 × 10⁻⁵ (2026-10-27), the linear-in-u interpolant's error on 256 entries.
    for (const name of ["a1", "a2", "a3", "a4", "b1", "b2"] as const) {
      for (let l = 0; l <= 8; l += 1) {
        expect(Math.abs(entry(moments[name], l) - entry(closed[name], l))).toBeLessThan(1e-4);
      }
    }
  });

  it("gives a total depolariser a₁ alone", () => {
    const moments = depolarisingMoments(4, [1, 0.5, 0.25, 0.1, 0.05]);
    expect(Array.from(moments.a1)).toEqual([1, 0.5, 0.25, 0.1, 0.05]);
    for (const name of ["a2", "a3", "a4", "b1", "b2"] as const) {
      expect(Array.from(moments[name]).every((v) => v === 0)).toBe(true);
    }
  });
});

describe("the azimuthal modes", () => {
  const geometries = [
    [0.3, 1.1, -0.6, 0.2],
    [-0.8, 2.0, 0.5, -1.0],
    [0.9, 0.1, 0.2, 2.5],
    [-0.2, -2.2, -0.7, 0.4],
    [0.05, 3.0, -0.95, -2.9],
  ] as const;

  it("sum to the scattering matrix rotated into the meridian frames, to 10⁻¹³", () => {
    const cases: ReadonlyArray<readonly [MatrixMoments, number, (c: number) => number[]]> = [
      [rayleigh(0.03, 2), 2, rayleighScattering(0.03)],
      [SYNTHETIC, 5, synthesised(SYNTHETIC, 5)],
    ];
    let worst = 0;
    for (const [moments, order, scattering] of cases) {
      for (const [mu, phi, muPrime, phiPrime] of geometries) {
        const modes = meridianPhaseMatrix(moments, order, mu, muPrime, phi - phiPrime);
        const rotated = geometricPhaseMatrix(scattering, mu, phi, muPrime, phiPrime);
        for (let i = 0; i < 16; i += 1) {
          worst = Math.max(worst, Math.abs(entry(modes, i) - entry(rotated, i)));
        }
      }
    }
    // Measured 1.0 × 10⁻¹⁵ (2026-10-27).
    expect(worst).toBeLessThan(1e-13);
  });
});

/** Natraj, Li and Yung's level: the top's upwelling or the bottom's downwelling light. */
type Level = "top" | "bottom";

/**
 * Natraj, Li and Yung 2009 (ApJ 691, 1909), `CDS/{I,Q,U}_{UP,DN}_TAU_0.5`, as R08.T12.c's
 * `crates/hyperion-fit/src/atmosphere/benchmarks.rs` holds them (`NATRAJ_2009_TAU_0_5`): Rayleigh
 * of ρ = 0, ω = 1, τ 0.5 over a black ground, μ₀ 0.2, the diffuse light's I, Q and U as printed for
 * πF₀ = π, at the level, μ and relative azimuth φ (degrees). The values asserted only
 * (decision-r08-licences row 6b); `NOTICE` lists the tables.
 */
const NATRAJ_2009_TAU_0_5: ReadonlyArray<readonly [Level, number, number, number, number, number]> =
  [
    ["top", 0.1, 0, 0.35082997, -0.01524525, 0],
    ["top", 0.1, 90, 0.21348201, -0.16821107, 0.06357361],
    ["top", 0.1, 180, 0.36354469, -0.02795997, 0],
    ["top", 0.2, 0, 0.26939388, -0.00608288, 0],
    ["top", 0.2, 90, 0.17169133, -0.13222805, 0.04872177],
    ["top", 0.2, 180, 0.28888259, -0.02557158, 0],
    ["top", 0.52, 0, 0.13097041, 0.0196893, 0],
    ["top", 0.52, 90, 0.09759154, -0.07273772, 0.02406753],
    ["top", 0.52, 180, 0.15600064, -0.00534093, 0],
    ["top", 0.84, 0, 0.06783499, 0.03668441, 0],
    ["top", 0.84, 90, 0.06393739, -0.04623546, 0.01057373],
    ["top", 0.84, 180, 0.08559886, 0.01892054, 0],
    ["top", 1, 0, 0.05300496, 0.03755859, 0],
    ["bottom", 0.1, 0, 0.13485741, -0.01538451, 0],
    ["bottom", 0.1, 90, 0.08801144, -0.0589805, 0.02077195],
    ["bottom", 0.1, 180, 0.13070302, -0.01123012, 0],
    ["bottom", 0.2, 0, 0.1583861, -0.01767297, 0],
    ["bottom", 0.2, 90, 0.10033646, -0.07022742, 0.02479837],
    ["bottom", 0.2, 180, 0.14846675, -0.00775363, 0],
    ["bottom", 0.52, 0, 0.12178394, -0.00542392, 0],
    ["bottom", 0.52, 90, 0.07796226, -0.05573965, 0.01823144],
    ["bottom", 0.52, 180, 0.10282324, 0.01353678, 0],
    ["bottom", 0.84, 0, 0.07348081, 0.01547851, 0],
    ["bottom", 0.84, 90, 0.0553467, -0.03883915, 0.00889191],
    ["bottom", 0.84, 180, 0.0585424, 0.03041692, 0],
    ["bottom", 1, 0, 0.04682203, 0.03225729, 0],
  ];

/** Natraj's cosines, the solve's user cosines. */
const NATRAJ_COSINES = [0.1, 0.2, 0.52, 0.84, 1];

/** The solver's (I, Q, U) at each of Natraj's rows, at `streams`: one layer. */
function natrajStokes(streams: number): ReadonlyArray<readonly [number, number, number, number]> {
  const solution = solvePolarised({
    layers: [{ opticalDepth: 0.5, singleScatteringAlbedo: 1, moments: rayleigh(0) }],
    groundAlbedo: 0,
    beamCosines: [0.2],
    userCosines: NATRAJ_COSINES,
    maxMode: 2,
    streams,
  });
  const beam = beamOf(solution);
  return NATRAJ_2009_TAU_0_5.map(([level, mu, phi]) => {
    const direction: StreamDirection = level === "top" ? "up" : "down";
    return stokesAtAzimuth(
      beam,
      level === "top" ? 0 : 1,
      NATRAJ_COSINES.indexOf(mu),
      direction,
      (phi * Math.PI) / 180,
    );
  });
}

/**
 * The largest miss of I, Q and U against Natraj's rows at `streams`, each of the row's I: the
 * tables' Q and U are −Q and −U of the tracer's frame, and their beam carries π where the solver's
 * carries 1.
 */
function natrajMisses(streams: number): readonly [number, number, number] {
  const stokes = natrajStokes(streams);
  const misses: [number, number, number] = [0, 0, 0];
  NATRAJ_2009_TAU_0_5.forEach(([, , , i, q, u], row) => {
    const expected = [i, -q, -u].map((value) => value / Math.PI);
    const scale = i / Math.PI;
    for (let c = 0; c < 3; c += 1) {
      misses[c] = Math.max(
        entry(misses, c),
        Math.abs(entry(stokes[row] ?? [], c) - entry(expected, c)) / scale,
      );
    }
  });
  return misses;
}

describe("Natraj, Li and Yung 2009's τ 0.5 Rayleigh table (test 1)", () => {
  it("is reproduced at 16 streams, I, Q and U each within 10⁻³ of the geometry's I", () => {
    const [i, q, u] = natrajMisses(16);
    // Measured 1.6 × 10⁻⁴, 1.2 × 10⁻⁴ and 1.9 × 10⁻⁵ of I (2026-10-27), each at the bottom's μ 0.1,
    // φ 90°, its grazing view; at 32 streams 6.4 × 10⁻⁷,
    // 4.3 × 10⁻⁷ and 8.8 × 10⁻⁸, and at 8 streams 4.0 × 10⁻³, 3.1 × 10⁻³ and 6.3 × 10⁻⁴.
    expect(i).toBeLessThan(1e-3);
    expect(q).toBeLessThan(1e-3);
    expect(u).toBeLessThan(1e-3);
  });

  it("moves I, Q and U by under 10⁻³ of I from 16 streams to 32 on the same slab (recorded)", () => {
    const at16 = natrajStokes(16);
    const at32 = natrajStokes(32);
    let worst = 0;
    at16.forEach((stokes, row) => {
      const fine = at32[row] ?? [];
      for (let c = 0; c < 3; c += 1) {
        worst = Math.max(worst, Math.abs(entry(stokes, c) - entry(fine, c)) / entry(fine, 0));
      }
    });
    // Measured 1.6 × 10⁻⁴ of I (2026-10-27), in I at the bottom's μ 0.1, φ 90°.
    expect(worst).toBeLessThan(1e-3);
  });
});

describe("single scattering (exact)", () => {
  /** Upward at the top and downward at the bottom of a slab of τ, the closed form of its single scattering. */
  function singleScattering(
    scattering: (c: number) => number[],
    omega: number,
    tau: number,
    mu0: number,
    mu: number,
    phi: number,
    level: Level,
  ): number[] {
    // The beam travels down at azimuth 0; the scattered light up (+μ) or down (−μ) at φ.
    const travel = level === "top" ? mu : -mu;
    const z = geometricPhaseMatrix(scattering, travel, phi, -mu0, 0);
    const path =
      level === "top"
        ? (mu0 / (mu0 + mu)) * -Math.expm1(-tau * (1 / mu + 1 / mu0))
        : (mu0 / (mu0 - mu)) * (Math.exp(-tau / mu0) - Math.exp(-tau / mu));
    return [0, 1, 2, 3].map((c) => (omega / (4 * Math.PI)) * entry(z, c * 4) * path);
  }

  const cases: ReadonlyArray<readonly [string, MatrixMoments, number, (c: number) => number[]]> = [
    ["Rayleigh of ρ 0.03", rayleigh(0.03), 2, rayleighScattering(0.03)],
    ["a matrix with b₂, V carried", SYNTHETIC, 5, synthesised(SYNTHETIC, 5)],
  ];
  for (const [name, moments, order, scattering] of cases) {
    it(`matches the closed form to 10⁻⁴ of I for ${name}, which pins the frame and the azimuth`, () => {
      const tau = 1e-6;
      const omega = 0.9;
      const mu0 = 0.3;
      const users = [0.15, 0.5, 0.9];
      const solution = solvePolarised({
        layers: [{ opticalDepth: tau, singleScatteringAlbedo: omega, moments }],
        groundAlbedo: 0,
        beamCosines: [mu0],
        userCosines: users,
        maxMode: order,
        streams: 16,
      });
      const beam = beamOf(solution);
      let worst = 0;
      for (const [u, mu] of users.entries()) {
        for (const degrees of [0, 45, 90, 135, 180]) {
          const phi = (degrees * Math.PI) / 180;
          for (const level of ["top", "bottom"] as const) {
            const expected = singleScattering(scattering, omega, tau, mu0, mu, phi, level);
            const stokes = stokesAtAzimuth(
              beam,
              level === "top" ? 0 : 1,
              u,
              level === "top" ? "up" : "down",
              phi,
            );
            const scale = Math.abs(entry(expected, 0));
            for (let c = 0; c < 4; c += 1) {
              worst = Math.max(worst, Math.abs(entry(stokes, c) - entry(expected, c)) / scale);
            }
          }
        }
      }
      // Measured 3.5 × 10⁻⁶ (Rayleigh) and 4.7 × 10⁻⁶ (b₂) (2026-10-27): double scattering at τ
      // 10⁻⁶.
      expect(worst).toBeLessThan(1e-4);
    });
  }
});

/** The I fluxes' balance at ω = 1: reflected, transmitted and absorbed by the ground, of μ₀. */
function balance(problem: PolarisedProblem): number {
  const solution = solvePolarised(problem);
  let worst = 0;
  for (const beam of solution.beams) {
    const { direct, diffuseDown, diffuseUp } = beam.fluxes;
    const last = direct.length - 1;
    const incident = beam.beamCosine;
    const absorbed = (1 - problem.groundAlbedo) * (entry(direct, last) + entry(diffuseDown, last));
    worst = Math.max(worst, Math.abs(entry(diffuseUp, 0) + absorbed - incident) / incident);
  }
  return worst;
}

/** A conservative layer of τ. */
function conservative(moments: MatrixMoments, tau: number): PolarisedLayer {
  return { opticalDepth: tau, singleScatteringAlbedo: 1, moments };
}

describe("energy at ω = 1 (test 2)", () => {
  const sphere = sphereMoments(16);
  const mixed = Array.from({ length: 12 }, (_, p) =>
    conservative(p % 2 === 0 ? rayleigh(0.03) : sphere, [0.01, 0.4, 2, 0.1, 5, 0.7][p % 6] ?? 1),
  );
  const cases: ReadonlyArray<readonly [string, ReadonlyArray<PolarisedLayer>]> = [
    ["one Rayleigh layer of τ 1", [conservative(rayleigh(0.03), 1)]],
    ["one sphere layer of τ 3, V carried", [conservative(sphere, 3)]],
    ["twelve layers of Rayleigh and spheres, τ 16.4 in all", mixed],
  ];
  for (const [name, layers] of cases) {
    for (const groundAlbedo of [0, 1]) {
      it(`conserves the beam's flux to 10⁻⁶ on ${name} over a ground of albedo ${groundAlbedo}`, () => {
        const worst = balance({
          layers,
          groundAlbedo,
          beamCosines: [0.1, 0.5, 1],
          userCosines: [0.5],
          maxMode: 3,
          streams: 16,
        });
        // Measured at most 2.5 × 10⁻¹⁴ (2026-10-27).
        expect(worst).toBeLessThan(1e-6);
      });
    }
  }

  it("meets the nonsymmetric path on the sphere's layers past m = 0, where V is carried", () => {
    const solution = solvePolarised({
      layers: [conservative(sphere, 3)],
      groundAlbedo: 0,
      beamCosines: [0.5],
      userCosines: [0.5],
      maxMode: 2,
      streams: 16,
    });
    expect(solution.circular).toBe(true);
    expect(solution.eigen.map((record) => [record.components, record.nonsymmetricLayers])).toEqual([
      [2, 0],
      [4, 1],
      [4, 1],
    ]);
  });
});

describe("the depolarisers (test 3)", () => {
  it("give the scalar solver's I to 10⁻¹² at every node and user cosine, mode and interface", () => {
    const hg = Array.from({ length: 16 }, (_, l) => 0.75 ** l);
    const layers: OrdinateLayerPair[] = [
      [0.3, 0.999, phaseMoments({ kind: "rayleigh", depolarisation: 0.03 }, 16)],
      [1.5, 0.9, hg],
      [0.05, 1, hg],
      [2, 0.7, phaseMoments({ kind: "rayleigh", depolarisation: 0 }, 16)],
    ];
    const common = {
      groundAlbedo: 0.3,
      beamCosines: [0.15, 0.6, 1],
      userCosines: [0.05, 0.4, 0.95],
      maxMode: 3,
      streams: 16,
    };
    const scalar = solveDiscreteOrdinates({
      ...common,
      layers: layers.map(([tau, omega, moments]) => ({
        opticalDepth: tau,
        singleScatteringAlbedo: omega,
        moments,
      })),
    });
    const vector = solvePolarised({
      ...common,
      layers: layers.map(([tau, omega, moments]) => ({
        opticalDepth: tau,
        singleScatteringAlbedo: omega,
        moments: depolarisingMoments(15, moments),
      })),
    });
    let worst = 0;
    let largest = 0;
    scalar.beams.forEach((beam, b) => {
      beam.modes.forEach((mode, m) => {
        const polarised = beamOf(vector, b).modes[m];
        if (polarised === undefined) {
          throw new Error(`no mode ${m}`);
        }
        const c = polarised.components;
        const pairs: ReadonlyArray<readonly [Float64Array, Float64Array]> = [
          [mode.quadratureUp, polarised.quadratureUp],
          [mode.quadratureDown, polarised.quadratureDown],
          [mode.userUp, polarised.userUp],
          [mode.userDown, polarised.userDown],
        ];
        for (const [one, other] of pairs) {
          one.forEach((value, k) => {
            largest = Math.max(largest, Math.abs(value));
            worst = Math.max(worst, Math.abs(value - entry(other, k * c)));
            for (let x = 1; x < c; x += 1) {
              worst = Math.max(worst, Math.abs(entry(other, k * c + x)));
            }
          });
        }
      });
    });
    // Measured 1.1 × 10⁻¹⁴ of the largest I (2026-10-27).
    expect(worst).toBeLessThan(1e-12 * largest);
  });
});

/** A mixed layer of optics, before delta-M. */
interface MixedSlab {
  readonly opticalDepth: number;
  readonly singleScatteringAlbedo: number;
  readonly moments: MatrixMoments;
}

/**
 * Air at ρ 0.0279 and τ 0.3 with a depolarising Henyey–Greenstein aerosol of g 0.95, τ 0.3 and
 * ω 1, mixed by their scattering, to l = `order`.
 */
function airAndDepolariser(order: number): MixedSlab {
  const air = rayleigh(0.0279, order);
  const aerosol = depolarisingMoments(
    order,
    Array.from({ length: order + 1 }, (_, l) => 0.95 ** l),
  );
  const airScattering = 0.3;
  const aerosolScattering = 0.3;
  const mix = (a: ArrayLike<number>, b: ArrayLike<number>): number[] =>
    Array.from(
      { length: order + 1 },
      (_, l) =>
        (airScattering * entry(a, l) + aerosolScattering * entry(b, l)) /
        (airScattering + aerosolScattering),
    );
  return {
    opticalDepth: 0.6,
    singleScatteringAlbedo: 1,
    moments: {
      a1: mix(air.a1, aerosol.a1),
      a2: mix(air.a2, aerosol.a2),
      a3: mix(air.a3, aerosol.a3),
      a4: mix(air.a4, aerosol.a4),
      b1: mix(air.b1, aerosol.b1),
      b2: mix(air.b2, aerosol.b2),
    },
  };
}

/** f = a₁_NSTR and a₁′, as the thick bake's `deltaMLayer` gives them with no forward delta. */
function truncation(slab: MixedSlab, streams: number): { f: number; a1: number[] } {
  const f = Math.max(entry(slab.moments.a1, streams), 0);
  const a1 = Array.from({ length: streams }, (_, l) => (entry(slab.moments.a1, l) - f) / (1 - f));
  return { f, a1 };
}

/**
 * The slab's polarisation of I, the vector solve's I less its depolarised solve's, at the top
 * (upward) and the bottom (downward) of each user cosine, beam and mode m ≤ 2.
 */
function polarisationOfI(slab: MixedSlab, streams: number): number[] {
  const { f, a1 } = truncation(slab, streams);
  const omega = slab.singleScatteringAlbedo;
  const scaled = {
    opticalDepth: (1 - omega * f) * slab.opticalDepth,
    singleScatteringAlbedo: ((1 - f) * omega) / (1 - omega * f),
  };
  const userCosines = [1, 0.5, 0.2, 0.1];
  const common = { groundAlbedo: 0, beamCosines: [0.5, 0.2], userCosines, maxMode: 2, streams };
  const vector = solvePolarised({
    ...common,
    layers: [{ ...scaled, moments: truncateMatrixMoments(slab.moments, f, a1, streams) }],
  });
  const scalar = solvePolarised({
    ...common,
    layers: [{ ...scaled, moments: depolarisingMoments(streams - 1, a1) }],
  });
  const out: number[] = [];
  vector.beams.forEach((beam, b) => {
    beam.modes.forEach((mode, m) => {
      const depolarised = beamOf(scalar, b).modes[m];
      if (depolarised === undefined) {
        throw new Error(`no mode ${m}`);
      }
      const c = mode.components;
      const views = userCosines.length;
      for (let u = 0; u < views; u += 1) {
        out.push(entry(mode.userUp, u * c) - entry(depolarised.userUp, u * c));
        const bottom = (views + u) * c;
        out.push(entry(mode.userDown, bottom) - entry(depolarised.userDown, bottom));
      }
    });
  });
  return out;
}

describe("delta-M on the matrix", () => {
  it("keeps every element's moments below NSTR, the removed peak the identity", () => {
    const slab = airAndDepolariser(16);
    const streams = 8;
    const { f, a1 } = truncation(slab, streams);
    const truncated = truncateMatrixMoments(slab.moments, f, a1, streams);
    let worst = 0;
    for (let l = 0; l < streams; l += 1) {
      for (const element of ["a1", "a2", "a3", "a4"] as const) {
        if (l >= 2 || element === "a1" || element === "a4") {
          const kept = (1 - f) * entry(truncated[element], l) + f;
          worst = Math.max(worst, Math.abs(kept - entry(slab.moments[element], l)));
        }
      }
      for (const element of ["b1", "b2"] as const) {
        const kept = (1 - f) * entry(truncated[element], l);
        worst = Math.max(worst, Math.abs(kept - entry(slab.moments[element], l)));
      }
    }
    expect(worst).toBeLessThan(1e-15);
  });

  it("holds an 8-stream polarisation of air with a depolariser to 128 streams, within 2% of its largest", () => {
    const slab = airAndDepolariser(128);
    const reference = polarisationOfI(slab, 128);
    const eight = polarisationOfI(slab, 8);
    let largest = 0;
    let worst = 0;
    reference.forEach((value, i) => {
      largest = Math.max(largest, Math.abs(value));
      worst = Math.max(worst, Math.abs(entry(eight, i) - value));
    });
    // Measured 1.3% of the largest, 1.7 × 10⁻³ (2026-10-27). Taking the depolariser's peak as
    // the identity with its a₂ … a₄ kept at 0 misses by 24%.
    expect(worst).toBeLessThan(0.02 * largest);
  });
});

/** A layer's τ, ω and χ_l. */
type OrdinateLayerPair = readonly [number, number, ArrayLike<number>];

describe("layering, one factorisation and V", () => {
  const sphere = sphereMoments(16);

  it("solves a slab as one layer and as 32 alike to 10⁻¹⁰ of I", () => {
    const common = {
      groundAlbedo: 0.2,
      beamCosines: [0.4],
      userCosines: [0.2, 0.7],
      maxMode: 2,
      streams: 16,
    };
    const one = solvePolarised({
      ...common,
      layers: [{ opticalDepth: 2, singleScatteringAlbedo: 0.95, moments: sphere }],
    });
    const many = solvePolarised({
      ...common,
      layers: Array.from({ length: 32 }, () => ({
        opticalDepth: 2 / 32,
        singleScatteringAlbedo: 0.95,
        moments: sphere,
      })),
    });
    let worst = 0;
    let largest = 0;
    for (const [interface1, interface32] of [
      [0, 0],
      [1, 32],
    ] as const) {
      for (const direction of ["up", "down"] as const) {
        for (const u of [0, 1]) {
          for (const phi of [0, 1, 2.5]) {
            const a = stokesAtAzimuth(beamOf(one), interface1, u, direction, phi);
            const b = stokesAtAzimuth(beamOf(many), interface32, u, direction, phi);
            largest = Math.max(largest, Math.abs(entry(a, 0)));
            for (let c = 0; c < 4; c += 1) {
              worst = Math.max(worst, Math.abs(entry(a, c) - entry(b, c)));
            }
          }
        }
      }
    }
    // Measured 1.3 × 10⁻¹⁵ of the largest I (2026-10-27).
    expect(worst).toBeLessThan(1e-10 * largest);
  });

  it("solves every beam on one factorisation exactly as it solves each alone", () => {
    const layers = [
      { opticalDepth: 0.4, singleScatteringAlbedo: 1, moments: rayleigh(0.03) },
      { opticalDepth: 1.2, singleScatteringAlbedo: 0.9, moments: sphere },
    ];
    const cosines = [0.12, 0.5, 0.93];
    const common = { layers, groundAlbedo: 0.1, userCosines: [0.3], maxMode: 2, streams: 16 };
    const together = solvePolarised({ ...common, beamCosines: cosines });
    cosines.forEach((mu0, b) => {
      const alone = beamOf(solvePolarised({ ...common, beamCosines: [mu0] }));
      expect(beamOf(together, b).modes).toEqual(alone.modes);
    });
  });

  it("leaves V 0 and I as it was where no layer has b₂, carried or not", () => {
    const layers = [{ opticalDepth: 0.8, singleScatteringAlbedo: 0.97, moments: rayleigh(0.03) }];
    const common = { layers, groundAlbedo: 0, beamCosines: [0.3], userCosines: [0.6], maxMode: 2 };
    const without = solvePolarised({ ...common, streams: 16 });
    const with4 = solvePolarised({ ...common, streams: 16, circular: true });
    expect(without.circular).toBe(false);
    for (const phi of [0.3, 1.9]) {
      const a = stokesAtAzimuth(beamOf(without), 0, 0, "up", phi);
      const b = stokesAtAzimuth(beamOf(with4), 0, 0, "up", phi);
      for (let c = 0; c < 3; c += 1) {
        expect(Math.abs(entry(a, c) - entry(b, c))).toBeLessThan(1e-13);
      }
      expect(entry(b, 3)).toBe(0);
    }
  });
});

describe("the refusals", () => {
  const good: PolarisedProblem = {
    layers: [{ opticalDepth: 1, singleScatteringAlbedo: 0.9, moments: rayleigh(0) }],
    groundAlbedo: 0,
    beamCosines: [0.5],
    userCosines: [0.5],
    maxMode: 2,
    streams: 8,
  };
  const layerWith = (change: Partial<PolarisedLayer>): PolarisedProblem => ({
    ...good,
    layers: [{ ...entryLayer(good), ...change }],
  });

  it("solves the problem the refusals vary", () => {
    expect(() => solvePolarised(good)).not.toThrow();
  });

  it("refuses a stream count that is odd or too small, a mode past NSTR − 1, and no layers", () => {
    for (const problem of [
      { ...good, streams: 7 },
      { ...good, streams: 0 },
      { ...good, maxMode: 8 },
      { ...good, maxMode: -1 },
      { ...good, layers: [] },
    ]) {
      expect(() => solvePolarised(problem)).toThrow(RangeError);
    }
  });

  it("refuses a layer's τ, ω or moments out of range, and a₁_0 away from 1", () => {
    for (const problem of [
      layerWith({ opticalDepth: -1 }),
      layerWith({ singleScatteringAlbedo: 1.1 }),
      layerWith({ moments: { ...rayleigh(0), a1: [0.9, 0, 0.1] } }),
      layerWith({ moments: { ...rayleigh(0), b1: [0, 0, Number.NaN] } }),
    ]) {
      expect(() => solvePolarised(problem)).toThrow(RangeError);
    }
  });

  it("refuses cosines outside (0, 1], a ground albedo outside [0, 1] and beam paths that do not fit", () => {
    for (const problem of [
      { ...good, beamCosines: [0] },
      { ...good, userCosines: [1.5] },
      { ...good, groundAlbedo: -0.1 },
      { ...good, beamPaths: [] },
    ]) {
      expect(() => solvePolarised(problem)).toThrow(RangeError);
    }
  });

  it("refuses a Henyey–Greenstein g 0.95's raw moments, as the scalar solver does, and solves them truncated", () => {
    const raw = Array.from({ length: 16 }, (_, l) => 0.95 ** l);
    const f = 0.95 ** 16;
    const truncated = raw.map((chi, l) => (l === 0 ? 1 : (chi - f) / (1 - f)));
    const common = { groundAlbedo: 0, beamCosines: [0.5], userCosines: [0.5], maxMode: 1 };
    expect(() => solvePolarised({ ...common, layers: [depolarisedSlab(raw, 1)] })).toThrow(
      /delta-M/u,
    );
    // Delta-M keeps ω′ = 1 at ω = 1.
    expect(() =>
      solvePolarised({ ...common, layers: [depolarisedSlab(truncated, 1)] }),
    ).not.toThrow();
  });

  it("refuses a Stokes vector at an interface or user cosine it does not have", () => {
    const beam = beamOf(solvePolarised(good));
    expect(() => stokesAtAzimuth(beam, 2, 0, "up", 0)).toThrow(RangeError);
    expect(() => stokesAtAzimuth(beam, 0, 1, "down", 0)).toThrow(RangeError);
  });
});

/** A slab of τ 2 whose matrix is a₁ alone, of the moments χ_l. */
function depolarisedSlab(a1: ReadonlyArray<number>, omega: number): PolarisedLayer {
  return { opticalDepth: 2, singleScatteringAlbedo: omega, moments: depolarisingMoments(15, a1) };
}

function entryLayer(problem: PolarisedProblem): PolarisedLayer {
  const layer = problem.layers[0];
  if (layer === undefined) {
    throw new Error("the problem has a layer");
  }
  return layer;
}
