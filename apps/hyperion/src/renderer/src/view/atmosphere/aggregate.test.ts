import { describe, expect, it } from "vitest";

import titan from "./fixtures/optool-titan.json" with { type: "json" };
import {
  aggregateOptics,
  gammaFunction,
  kummerNegative,
  MMF_PHASE_SHIFT_LIMIT,
  mmfAggregate,
  mmfPhaseBasis,
  mmfPhaseShift,
} from "./aggregate";
import { refractiveIndex } from "./materials/materials";
import { mieSphere } from "./mie";

/**
 * The plan's Titan-like aggregate (R08.T5.c; Tomasko et al. 2008): tholin monomers of 0.05 µm,
 * 3,000 of them, D_f 2, with optool's k_f = (5 ÷ 3)^(D_f ÷ 2).
 */
const TITAN = { monomerRadiusUm: 0.05, monomerCount: 3000, fractalDimension: 2 } as const;

/**
 * The comparison's tolerances, optool's own numerical accuracy as its code states it
 * (`optool_fractal.f90`): its phase function's normalisation and its correlation function's
 * unitary condition are checked to 10⁻³; past qR_g = 26 it takes S(q)'s leading asymptotic term,
 * whose relative error D_f ÷ (qR_g)² is 0.3% there, which bounds F₁₁'s shape; and its g is a
 * midpoint sum over 1° bins, which reads 0.0007–0.0023 high against this module's quadrature on a
 * forward peak of this width (measured at the five wavelengths).
 */
const TOLERANCE = { crossSection: 1e-3, f11: 3e-3, ratio: 1e-3, asymmetry: 3e-3 } as const;

/** The 181 bin edges of optool's 1° bins, 0° to 180°. */
const EDGES = Float64Array.from({ length: 181 }, (_, i) => Math.cos((i * Math.PI) / 180));

/** optool's bin value: the mean of the two edges. */
function bins(values: Float64Array): number[] {
  return Array.from({ length: 180 }, (_, j) => 0.5 * ((values[j] ?? 0) + (values[j + 1] ?? 0)));
}

/** F₁₁ over the bins normalised to Σ F₁₁ sin θ = 1, for comparing shapes. */
function shape(f11: ReadonlyArray<number>): number[] {
  let sum = 0;
  for (const [j, v] of f11.entries()) {
    sum += v * Math.sin(((j + 0.5) * Math.PI) / 180);
  }
  return f11.map((v) => v / sum);
}

/** Kummer's M(a, b, −z) by its direct alternating series, accurate at small z. */
function direct(a: number, b: number, z: number): number {
  let term = 1;
  let sum = 1;
  for (let k = 0; k < 200; k += 1) {
    term *= ((a + k) * -z) / ((b + k) * (k + 1));
    sum += term;
  }
  return sum;
}

describe("the MMF against optool", () => {
  for (const [w, lambdaUm] of titan.wavelengthsUm.entries()) {
    const nm = Math.round(lambdaUm * 1000);
    let cached: ReturnType<typeof mmfAggregate> | undefined;
    const optics = (): ReturnType<typeof mmfAggregate> => {
      cached ??= mmfAggregate(TITAN, refractiveIndex("tholin", nm).index, nm, EDGES);
      return cached;
    };

    it(`gives optool's cross-sections at ${nm} nm`, () => {
      const ext = titan.extinctionCrossSectionUm2[w] ?? Number.NaN;
      const sca = titan.scatteringCrossSectionUm2[w] ?? Number.NaN;
      expect(Math.abs(optics().extinctionCrossSectionUm2 / ext - 1)).toBeLessThan(
        TOLERANCE.crossSection,
      );
      expect(Math.abs(optics().scatteringCrossSectionUm2 / sca - 1)).toBeLessThan(
        TOLERANCE.crossSection,
      );
    });

    it(`gives optool's asymmetry at ${nm} nm`, () => {
      expect(Math.abs(optics().asymmetry - (titan.asymmetry[w] ?? Number.NaN))).toBeLessThan(
        TOLERANCE.asymmetry,
      );
    });

    it(`gives optool's F11 shape and ratios at ${nm} nm`, () => {
      const { matrix } = optics();
      const a1 = bins(matrix.a1);
      const ours = shape(a1);
      const theirs = shape(titan.F11[w] ?? []);
      const b1 = bins(matrix.b1);
      const a3 = bins(matrix.a3);
      const b2 = bins(matrix.b2);
      for (let j = 0; j < 180; j += 1) {
        expect(Math.abs((ours[j] ?? 0) / (theirs[j] ?? 1) - 1)).toBeLessThan(TOLERANCE.f11);
        const f11 = titan.F11[w]?.[j] ?? Number.NaN;
        const ratio = (e: number[], j2: number): number => (e[j2] ?? 0) / (a1[j2] ?? 1);
        expect(Math.abs(ratio(b1, j) - (titan.F12[w]?.[j] ?? 0) / f11)).toBeLessThan(
          TOLERANCE.ratio,
        );
        expect(Math.abs(ratio(a3, j) - (titan.F33[w]?.[j] ?? 0) / f11)).toBeLessThan(
          TOLERANCE.ratio,
        );
        // optool's MMF writes S₃₄ = Im(S₂S₁*) in Bohren and Huffman's amplitudes, the opposite sign
        // of T5.b's b₂ and of optool's own Mie path (−D₂₁): b₂ = −F₃₄ here.
        expect(Math.abs(ratio(b2, j) + (titan.F34[w]?.[j] ?? 0) / f11)).toBeLessThan(
          TOLERANCE.ratio,
        );
      }
    });
  }
});

describe("the phase-shift gate (science-r08-mmf.md §1.4)", () => {
  const at = (nm: number): ReturnType<typeof mmfAggregate> =>
    mmfAggregate(TITAN, refractiveIndex("tholin", nm).index, nm, new Float64Array(0));

  it("gives the Titan-like aggregate Δφ 1.09, 0.80 and 0.54 by (9), aggregate terms 0.92, 0.67 and 0.45", () => {
    for (const [nm, total, aggregate] of [
      [380, 1.09, 0.92],
      [550, 0.8, 0.67],
      [780, 0.54, 0.45],
    ] as const) {
      const result = at(nm);
      expect(Math.abs(mmfPhaseShift(result) - total)).toBeLessThan(0.01);
      expect(Math.abs(result.phaseShift - aggregate)).toBeLessThan(0.01);
    }
  });

  it("makes it a model at 550 and 780 nm and an analogue at 380 nm", () => {
    expect(mmfPhaseBasis(at(550))).toBe("model");
    expect(mmfPhaseBasis(at(780))).toBe("model");
    expect(mmfPhaseBasis(at(380))).toBe("analogue");
  });

  it("keeps the MMF table and matrix, optool's -mmfss, for the analogue at 380 nm", () => {
    const optics = aggregateOptics({ material: "tholin", ...TITAN }, 380);
    expect(optics.phaseBasis).toBe("analogue");
    expect(optics.phaseModel).toBe("mmf");
    expect(optics.matrix).toBeDefined();
    expect(optics.phaseNote).toContain("analogue");
  });

  it("falls back for a compact large aggregate, keeping its cross-sections", () => {
    const compact = {
      material: "tholin",
      monomerRadiusUm: 0.1,
      monomerCount: 5000,
      fractalDimension: 2.5,
    } as const;
    const optics = aggregateOptics(compact, 550);
    expect(optics.phaseShift).toBeGreaterThanOrEqual(MMF_PHASE_SHIFT_LIMIT);
    expect(optics.phaseBasis).toBe("fallback");
    expect(optics.phaseModel).toBe("henyeyGreenstein");
    expect(optics.matrix).toBeUndefined();
    expect(optics.extinctionCrossSectionM2).toBeGreaterThan(0);
    expect(optics.asymmetry).toBeGreaterThan(0);
  });

  it("gives the Titan-like aggregate as a model with its matrix", () => {
    const optics = aggregateOptics({ material: "tholin", ...TITAN }, 550);
    expect(optics.phaseBasis).toBe("model");
    expect(optics.phaseModel).toBe("mmf");
    expect(optics.matrix).toBeDefined();
    expect(optics.matrix?.a2).toEqual(optics.matrix?.a1);
    expect(optics.matrix?.a4).toEqual(optics.matrix?.a3);
  });
});

describe("the MMF's limits and parts", () => {
  it("reduces to Mie for a single monomer", () => {
    const index = { n: 1.7, k: 0.03 };
    const x = (2 * Math.PI * 0.05 * 1000) / 550;
    const one = mmfAggregate(
      { monomerRadiusUm: 0.05, monomerCount: 1, fractalDimension: 2 },
      index,
      550,
      new Float64Array(0),
    );
    const mie = mieSphere(x, index, new Float64Array(0));
    const geometric = Math.PI * 0.05 * 0.05;
    expect(one.extinctionCrossSectionUm2 / (mie.qExt * geometric)).toBeCloseTo(1, 10);
    expect(one.asymmetry).toBeCloseTo(mie.asymmetry, 6);
  });

  it("normalises the matrix to ∫ a₁ dΩ ÷ 4π = 1", () => {
    // 8-point Gauss–Legendre on 1° panels in θ, which resolve the forward peak.
    const nodes = [
      -0.960_289_856_497_536_2, -0.796_666_477_413_626_7, -0.525_532_409_916_329,
      -0.183_434_642_495_649_8,
    ];
    const weights = [
      0.101_228_536_290_376_3, 0.222_381_034_453_374_5, 0.313_706_645_877_887_3,
      0.362_683_783_378_362,
    ];
    const theta: number[] = [];
    const w: number[] = [];
    const half = Math.PI / 360;
    for (let p = 0; p < 180; p += 1) {
      for (const [i, x] of [...nodes, ...nodes.map((v) => -v)].entries()) {
        const t = (2 * p + 1 + x) * half;
        theta.push(t);
        w.push(half * (weights[i % 4] ?? 0) * Math.sin(t));
      }
    }
    const result = mmfAggregate(
      TITAN,
      refractiveIndex("tholin", 550).index,
      550,
      Float64Array.from(theta, Math.cos),
    );
    const sum = w.reduce((s, wi, i) => s + wi * (result.matrix.a1[i] ?? 0), 0);
    expect((2 * Math.PI * sum) / (4 * Math.PI)).toBeCloseTo(1, 9);
  });

  it("evaluates Γ to 10⁻¹⁴", () => {
    expect(gammaFunction(0.5)).toBeCloseTo(Math.sqrt(Math.PI), 14);
    expect(gammaFunction(5)).toBeCloseTo(24, 12);
    expect(gammaFunction(1.25)).toBeCloseTo(0.906_402_477_055_477, 14);
  });

  it("evaluates Kummer's M(a, b, −z) on both sides of its switch", () => {
    expect(kummerNegative(1, 1.5, 2)).toBeCloseTo(direct(1, 1.5, 2), 12);
    // M(½, 3/2, −z) = √π erf(√z) ÷ (2√z) (DLMF 13.6.7), √π ÷ (2√z) to e^(−z) at large z.
    for (const z of [59.9, 60.1, 400]) {
      expect(kummerNegative(0.5, 1.5, z) / (Math.sqrt(Math.PI) / (2 * Math.sqrt(z)))).toBeCloseTo(
        1,
        12,
      );
    }
  });

  it("refuses a fractal dimension past the inventory's cap", () => {
    expect(() =>
      mmfAggregate(
        { monomerRadiusUm: 0.05, monomerCount: 100, fractalDimension: 2.6 },
        { n: 1.6, k: 0.01 },
        550,
      ),
    ).toThrow(RangeError);
  });
});
