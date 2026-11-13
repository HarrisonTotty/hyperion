/**
 * The large-particle limit of a sphere's optics (plan R08, R08.T16.a): Bessel's J₁ and Fraunhofer's
 * pattern, the rays' power and matrix, and the crossover from Mie, measured and printed.
 */
import { describe, expect, it } from "vitest";

import {
  addGeometricSphere,
  AIRY_RING_AVERAGE_FROM,
  airyPattern,
  BESSEL_J1_FIRST_ZERO,
  besselJ1,
  CROSSOVER_ALBEDO_TOLERANCE,
  CROSSOVER_ASYMMETRY_TOLERANCE,
  CROSSOVER_EXTINCTION_TOLERANCE,
  diffractionAsymmetry,
  DIFFRACTION_DELTA_SIZE_PARAMETER,
  GEOMETRIC_OPTICS_SIZE_PARAMETER,
  geometricLimit,
  geometricMatrix,
  geometricMatrixSums,
  geometricSphere,
  meanReflectance,
} from "./geometricOptics";
import { gaussLegendre } from "../lighting/quadrature";
import { indexAt, type MaterialForm, resolveMaterial } from "./materials/materials";
import { AEROSOL_TERM_WAVELENGTHS_NM } from "./aerosol";
import { type ComplexIndex, mieSphere } from "./mie";
import { PHASE_TABLE_SIZE } from "./sizeDistribution";

const NO_ANGLES = new Float64Array(0);

describe("Bessel's J₁ and Fraunhofer's pattern", () => {
  it("gives J₁ to rounding: Abramowitz and Stegun's Table 9.1 and its first zero, Table 9.5", () => {
    expect(besselJ1(1)).toBeCloseTo(0.4400505857, 10);
    expect(besselJ1(2)).toBeCloseTo(0.5767248078, 10);
    expect(Math.abs(besselJ1(BESSEL_J1_FIRST_ZERO))).toBeLessThan(1e-14);
  });

  it("puts the pattern's power, ½ ∫ A(v) v dv, at 1 with its rings averaged past 4π", () => {
    const rule = gaussLegendre(16);
    const panels = 256;
    let inner = 0;
    for (let p = 0; p < panels; p += 1) {
      const from = (p * AIRY_RING_AVERAGE_FROM) / panels;
      const half = AIRY_RING_AVERAGE_FROM / panels / 2;
      for (let q = 0; q < rule.x.length; q += 1) {
        const v = from + half * (1 + (rule.x[q] ?? Number.NaN));
        inner += half * (rule.w[q] ?? Number.NaN) * airyPattern(v) * v;
      }
    }
    // Past 4π, the ring average holds Rayleigh's J₀² + J₁² there.
    const outer = 0.5 * airyPattern(AIRY_RING_AVERAGE_FROM) * AIRY_RING_AVERAGE_FROM ** 2;
    expect(0.5 * inner + outer).toBeCloseTo(1, 5);
    expect(airyPattern(0)).toBe(1);
  });

  it("makes the forward delta of a pattern whose first dark ring lies inside it, past x ≈ 4,957", () => {
    expect(DIFFRACTION_DELTA_SIZE_PARAMETER).toBeCloseTo(4956.8, 1);
    expect(1 - diffractionAsymmetry(DIFFRACTION_DELTA_SIZE_PARAMETER)).toBeLessThan(2e-4);
  });
});

describe("the rays", () => {
  it.each<[string, ComplexIndex]>([
    ["water's", { n: 1.334, k: 0 }],
    ["a dense silicate's", { n: 1.6, k: 0 }],
    ["zinc sulphide's", { n: 2.4, k: 0 }],
    ["an index below 1", { n: 0.8, k: 0 }],
  ])("keep a lossless sphere's power at %s index", (_, index) => {
    const sphere = geometricSphere(geometricLimit(index, false), 500);
    expect(sphere.qExt).toBe(2);
    expect(Math.abs(sphere.qSca - 2)).toBeLessThan(1e-12);
  });

  it("leave an opaque sphere only its reflection", () => {
    const index = { n: 2, k: 1 };
    const sphere = geometricSphere(geometricLimit(index, false), 1000);
    expect(sphere.qSca - 1).toBeCloseTo(meanReflectance(index), 12);
  });

  it("scatter as a sphere does: a₃ is a₁ forward and −a₁ backward, and b₂ is 0 for a lossless one", () => {
    const limit = geometricLimit({ n: 1.334, k: 0 }, true);
    const sums = geometricMatrixSums(limit, Float64Array.from([1, -1, 0]));
    addGeometricSphere(sums, 1000, 1);
    const matrix = geometricMatrix(sums);
    expect((matrix.a3[0] ?? 0) / (matrix.a1[0] ?? 1)).toBeCloseTo(1, 6);
    // The backward entry holds the rays within 1.4° of it.
    expect((matrix.a3[1] ?? 0) / (matrix.a1[1] ?? 1)).toBeLessThan(-0.99);
    expect(Math.abs(matrix.b2[2] ?? 1)).toBeLessThan(1e-15);
  });

  it("give an absorbing sphere's matrix as Mie does at x 1,000, its reflection alone left", () => {
    // n 1.5, k 0.1: the refracted rays pass exp(−400 cos r), so the side angles are the reflection's.
    const index = { n: 1.5, k: 0.1 };
    const mu = Float64Array.from([30, 60, 90, 120, 150], (deg) => Math.cos((deg * Math.PI) / 180));
    const mie = {
      a1: new Float64Array(5),
      b1: new Float64Array(5),
      a3: new Float64Array(5),
      b2: new Float64Array(5),
    };
    const count = 2000;
    for (let j = 0; j < count; j += 1) {
      const sphere = mieSphere(900 + (200 * (j + 0.5)) / count, index, mu);
      for (let q = 0; q < mu.length; q += 1) {
        const r1 = sphere.s1[2 * q] ?? 0;
        const i1 = sphere.s1[2 * q + 1] ?? 0;
        const r2 = sphere.s2[2 * q] ?? 0;
        const i2 = sphere.s2[2 * q + 1] ?? 0;
        const s1 = r1 * r1 + i1 * i1;
        const s2 = r2 * r2 + i2 * i2;
        // `sizeDistribution.ts`'s elements: b₂ ∝ −Im(S₁S₂*).
        mie.a1[q] = (mie.a1[q] ?? 0) + 0.5 * (s1 + s2);
        mie.b1[q] = (mie.b1[q] ?? 0) + 0.5 * (s2 - s1);
        mie.a3[q] = (mie.a3[q] ?? 0) + r1 * r2 + i1 * i2;
        mie.b2[q] = (mie.b2[q] ?? 0) + r1 * i2 - i1 * r2;
      }
    }
    const sums = geometricMatrixSums(geometricLimit(index, true), mu);
    addGeometricSphere(sums, 1000, 1);
    const limit = geometricMatrix(sums);
    for (let q = 0; q < mu.length; q += 1) {
      for (const element of ["b1", "a3", "b2"] as const) {
        const ofMie = (mie[element][q] ?? 0) / (mie.a1[q] ?? 1);
        const ofLimit = (limit[element][q] ?? 0) / (limit.a1[q] ?? 1);
        expect(Math.abs(ofLimit - ofMie)).toBeLessThan(0.02);
      }
    }
  });

  it("hold every ray's power on the phase table as read", () => {
    const index = { n: 1.436, k: 0 };
    const limit = geometricLimit(index, true);
    const grid = limit.grid;
    if (grid === undefined) {
      throw new Error("the limit has its angles");
    }
    // Every group's orders together: their table, as read, against their power.
    const entries = PHASE_TABLE_SIZE;
    const table = new Float64Array(entries);
    for (let q = 0; q < grid.a1.length; q += 1) {
      table[q % entries] = (table[q % entries] ?? 0) + (grid.a1[q] ?? 0);
    }
    const u = Float64Array.from({ length: entries }, (_, i) => i / (entries - 1));
    const rule = gaussLegendre(8);
    let read = 0;
    for (let i = 0; i + 1 < entries; i += 1) {
      const half = 0.5 * ((u[i + 1] ?? 0) - (u[i] ?? 0));
      for (let q = 0; q < rule.x.length; q += 1) {
        const t = 0.5 * (1 + (rule.x[q] ?? Number.NaN));
        const uu = (u[i] ?? 0) + t * 2 * half;
        const value = (table[i] ?? 0) + t * ((table[i + 1] ?? 0) - (table[i] ?? 0));
        read +=
          half * (rule.w[q] ?? Number.NaN) * value * Math.sin(Math.PI * uu * uu) * 2 * Math.PI * uu;
      }
    }
    expect(2 * Math.PI * read).toBeCloseTo(1, 10);
  });
});

/** A material's index at a wavelength, as a mode of it in that phase is drawn. */
function indexOf(material: string, form: MaterialForm, wavelengthNm: number): ComplexIndex {
  return indexAt(resolveMaterial(material, form), wavelengthNm);
}

/** Mie's Q_ext, Q_sca and Q_sca g, evenly averaged over x ± 10%, at most 0.05 apart. */
function mieWindow(x: number, index: ComplexIndex): readonly [number, number, number] {
  const from = 0.9 * x;
  const count = Math.max(400, Math.ceil((0.2 * x) / 0.05));
  let ext = 0;
  let sca = 0;
  let scaG = 0;
  for (let j = 0; j < count; j += 1) {
    const sphere = mieSphere(from + ((j + 0.5) * 0.2 * x) / count, index, NO_ANGLES);
    ext += sphere.qExt;
    sca += sphere.qSca;
    scaG += sphere.qSca * sphere.asymmetry;
  }
  return [ext / count, sca / count, scaG / count];
}

/** The limit's, averaged over the same window at 40 points. */
function limitWindow(x: number, index: ComplexIndex): readonly [number, number, number] {
  const limit = geometricLimit(index, false);
  let ext = 0;
  let sca = 0;
  let scaG = 0;
  for (let j = 0; j < 40; j += 1) {
    const sphere = geometricSphere(limit, 0.9 * x + ((j + 0.5) * 0.2 * x) / 40);
    ext += sphere.qExt;
    sca += sphere.qSca;
    scaG += sphere.qScaAsymmetry;
  }
  return [ext / 40, sca / 40, scaG / 40];
}

describe("the crossover from Mie", () => {
  it(`lies at x = ${GEOMETRIC_OPTICS_SIZE_PARAMETER}, the least of the grid where the limit meets Mie for the volatile decks`, () => {
    // The volatile decks' spheres: water's droplets, and the ices whose analogue takes Mie's
    // cross-sections and ω for their own index (`nonSpherical.ts`).
    const materials: ReadonlyArray<readonly [string, MaterialForm]> = [
      ["H2O", { phase: "liquid" }],
      ["NH3", { phase: "solid" }],
      ["NH4SH", { phase: "solid" }],
      ["CH4", { phase: "solid" }],
      ["H2S", { phase: "solid" }],
    ];
    const grid = [50, 75, 100, 150, 200];
    const lines: string[] = [];
    const worst = grid.map((x) => {
      let extinction = 0;
      let albedo = 0;
      let asymmetry = 0;
      for (const [material, form] of materials) {
        for (const nm of AEROSOL_TERM_WAVELENGTHS_NM) {
          const index = indexOf(material, form, nm);
          const [mieExt, mieSca, mieScaG] = mieWindow(x, index);
          const [ext, sca, scaG] = limitWindow(x, index);
          extinction = Math.max(extinction, Math.abs(ext / mieExt - 1));
          albedo = Math.max(albedo, Math.abs(sca / ext - mieSca / mieExt));
          asymmetry = Math.max(asymmetry, Math.abs(scaG / sca - mieScaG / mieSca));
        }
      }
      lines.push(
        `x ${String(x).padStart(4)}: Q_ext ${(100 * extinction).toFixed(2)}%, ω ${albedo.toExponential(2)}, g ${asymmetry.toFixed(4)}`,
      );
      return { x, extinction, albedo, asymmetry };
    });
    console.warn(
      `the large-particle limit against Mie, worst over the volatile decks' indices:\n${lines.join("\n")}`,
    );
    const meets = worst.find(
      (w) =>
        w.extinction <= CROSSOVER_EXTINCTION_TOLERANCE &&
        w.albedo <= CROSSOVER_ALBEDO_TOLERANCE &&
        w.asymmetry <= CROSSOVER_ASYMMETRY_TOLERANCE,
    );
    expect(meets?.x).toBe(GEOMETRIC_OPTICS_SIZE_PARAMETER);
  });
});
