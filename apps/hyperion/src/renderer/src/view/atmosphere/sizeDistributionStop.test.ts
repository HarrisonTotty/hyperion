/**
 * The size quadrature's stop on resonant spheres (plan R08, R08.T5.f; decision-r08-t6d-findings.md
 * §4): `sizeDistribution.ts`'s `sphereModeOptics` takes a level only after two successive
 * agreements, and past its cap gives the finest level, marked unconverged.
 */

import { describe, expect, it } from "vitest";

import { type MaterialForm, refractiveIndex } from "./materials/materials";
import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM } from "./medium";
import { type ComplexIndex, mieSphere } from "./mie";
import {
  modeOptics,
  SIZE_CONVERGENCE,
  SIZE_LEVEL_MAX,
  SIZE_NODES_MAX,
  type SizeDistribution,
  sizeQuadrature,
  sphereModeOptics,
} from "./sizeDistribution";

/**
 * Whether the slow sweep runs: by hand, `HYPERION_SLOW_TESTS=1 pnpm --filter hyperion exec vitest
 * run src/renderer/src/view/atmosphere/sizeDistributionStop.test.ts` (no recipe sets it for this
 * file).
 */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

function at(values: Float64Array, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no element ${i}`);
  }
  return value;
}

/** Venus's mode-2 droplets, as `sizeDistribution.test.ts` holds them. */
const VENUS_MODE_2: SizeDistribution = {
  kind: "gamma",
  effectiveRadiusUm: 1.05,
  effectiveVariance: 0.07,
};

/** No scattering angles: the efficiencies alone, which are what the quadrature's stop tests. */
const NO_ANGLES = new Float64Array(0);

/** `aerosol.test.ts`'s lossless TiO₂ stand-in: n 2.65, k 0, a gamma mode of r_eff 0.5 µm, v_eff 0.1. */
const STAND_IN: SizeDistribution = {
  kind: "gamma",
  effectiveRadiusUm: 0.5,
  effectiveVariance: 0.1,
};
const STAND_IN_INDEX: ComplexIndex = { n: 2.65, k: 0 };

/** One level's mean efficiencies and asymmetry. */
interface LevelOptics {
  readonly nodes: number;
  readonly qExt: number;
  readonly qSca: number;
  readonly asymmetry: number;
}

/**
 * A level's optics summed here from `sizeQuadrature` and `mieSphere`, apart from
 * `sphereModeOptics` (whose cut at Mie's smallest size parameter does not reach these modes).
 */
function levelOptics(
  sizes: SizeDistribution,
  index: ComplexIndex,
  wavelengthNm: number,
  level: number,
): LevelOptics {
  const { radiiUm, areaWeights } = sizeQuadrature(sizes, wavelengthNm, level);
  const wavenumber = (2 * Math.PI * 1000) / wavelengthNm;
  let area = 0;
  let ext = 0;
  let sca = 0;
  let scaG = 0;
  for (let i = 0; i < radiiUm.length; i += 1) {
    const w = at(areaWeights, i);
    const result = mieSphere(wavenumber * at(radiiUm, i), index, NO_ANGLES);
    area += w;
    ext += w * result.qExt;
    sca += w * result.qSca;
    scaG += w * result.qSca * result.asymmetry;
  }
  return { nodes: radiiUm.length, qExt: ext / area, qSca: sca / area, asymmetry: scaG / sca };
}

/** A refinement's change as the quadrature measures it: Q relative, g absolute, the largest. */
function change(coarse: LevelOptics, fine: LevelOptics): number {
  return Math.max(
    Math.abs(coarse.qExt - fine.qExt) / fine.qExt,
    Math.abs(coarse.qSca - fine.qSca) / fine.qSca,
    Math.abs(coarse.asymmetry - fine.asymmetry),
  );
}

/**
 * Where T5.b's rule stopped, on the first level whose change fell under the tolerance: that
 * level, its change, and the next level's change from it.
 */
function firstAgreement(
  sizes: SizeDistribution,
  index: ComplexIndex,
  wavelengthNm: number,
): { readonly taken: LevelOptics; readonly change: number; readonly nextChange: number } {
  let coarse = levelOptics(sizes, index, wavelengthNm, 0);
  for (let level = 1; level < SIZE_LEVEL_MAX; level += 1) {
    const fine = levelOptics(sizes, index, wavelengthNm, level);
    const first = change(coarse, fine);
    if (first < SIZE_CONVERGENCE) {
      const next = levelOptics(sizes, index, wavelengthNm, level + 1);
      return { taken: fine, change: first, nextChange: change(fine, next) };
    }
    coarse = fine;
  }
  throw new Error(`no level agreed by ${SIZE_LEVEL_MAX}`);
}

/** The largest change, in the optics the quadrature's stop tests, from an earlier result. */
function bulkChange(
  was: { readonly qExt: number; readonly qSca: number; readonly asymmetry: number },
  now: { readonly qExt: number; readonly qSca: number; readonly asymmetry: number },
): number {
  return Math.max(
    Math.abs(now.qExt / was.qExt - 1),
    Math.abs(now.qSca / was.qSca - 1),
    Math.abs(now.asymmetry - was.asymmetry),
  );
}

/**
 * Rutile's ordinary index by J. R. DeVore's fit (J. Opt. Soc. Am. 41, 416–419, 1951; 0.43–1.53 µm),
 * k 0, as `nonSpherical.test.ts` takes it.
 */
function rutile(wavelengthNm: number): ComplexIndex {
  return { n: Math.sqrt(5.913 + 0.2441 / ((wavelengthNm / 1000) ** 2 - 0.0803)), k: 0 };
}

/** A registry material's index, in a form. */
function material(key: string, form?: MaterialForm): (wavelengthNm: number) => ComplexIndex {
  return (wavelengthNm) => refractiveIndex(key, wavelengthNm, form).index;
}

/** One index at every wavelength. */
function fixed(index: ComplexIndex): (wavelengthNm: number) => ComplexIndex {
  return () => index;
}

function gamma(effectiveRadiusUm: number, effectiveVariance: number): SizeDistribution {
  return { kind: "gamma", effectiveRadiusUm, effectiveVariance };
}

function logNormal(effectiveRadiusUm: number, effectiveVariance: number): SizeDistribution {
  return { kind: "logNormal", effectiveRadiusUm, effectiveVariance };
}

/** A mode the tests held before R08.T5.f, with the optics T5.b's single agreement gave it. */
interface HeldMode {
  readonly name: string;
  readonly sizes: SizeDistribution;
  readonly index: (wavelengthNm: number) => ComplexIndex;
  readonly wavelengthNm: number;
  /** T5.b's node count. */
  readonly nodes: number;
  readonly qExt: number;
  readonly qSca: number;
  readonly asymmetry: number;
}

/**
 * Every sphere-Mie mode that `sizeDistribution.test.ts`, `aerosol.test.ts` and
 * `nonSpherical.test.ts` evaluated before R08.T5.f, with T5.b's optics, computed at 3ce63f6d (the
 * cosines do not move the efficiencies). The stand-in at 440 nm is apart: T5.b stopped there on a
 * chance agreement.
 */
const HELD_MODES: ReadonlyArray<HeldMode> = [
  {
    name: "a narrow mode of x = 10⁻³ spheres",
    sizes: logNormal((1e-3 * 0.55) / (2 * Math.PI), 1e-6),
    index: fixed({ n: 1.44, k: 0 }),
    wavelengthNm: 550,
    nodes: 8,
    qExt: 1.852249939020193e-13,
    qSca: 1.852249939020193e-13,
    asymmetry: 1.9278428527902013e-7,
  },
  {
    name: "Hansen and Travis's Fig. 16 mode",
    sizes: gamma((14 * 0.55) / (2 * Math.PI), 0.07),
    index: fixed({ n: 1.33, k: 0 }),
    wavelengthNm: 550,
    nodes: 768,
    qExt: 2.3305714526827956,
    qSca: 2.3305714526827956,
    asymmetry: 0.7605730889722752,
  },
  {
    name: "Venus's mode 2 at n 1.44",
    sizes: VENUS_MODE_2,
    index: fixed({ n: 1.44, k: 0 }),
    wavelengthNm: 550,
    nodes: 1328,
    qExt: 2.3946534453830215,
    qSca: 2.3946534453830215,
    asymmetry: 0.7180455991607831,
  },
  {
    name: "Venus's mode 2 at k 10⁻³",
    sizes: VENUS_MODE_2,
    index: fixed({ n: 1.44, k: 1e-3 }),
    wavelengthNm: 550,
    nodes: 1328,
    qExt: 2.394594862129252,
    qSca: 2.334162494862271,
    asymmetry: 0.725593294889105,
  },
  {
    name: "a narrow 0.6 µm mode",
    sizes: logNormal(0.6, 1e-8),
    index: fixed({ n: 1.33, k: 1e-3 }),
    wavelengthNm: 550,
    nodes: 8,
    qExt: 3.7926655227426793,
    qSca: 3.7634050443774005,
    asymmetry: 0.8436234539625923,
  },
  {
    name: "Venus's mode 2 on H2SO4's file",
    sizes: VENUS_MODE_2,
    index: material("H2SO4"),
    wavelengthNm: 550,
    nodes: 1328,
    qExt: 2.390469202806494,
    qSca: 2.390467901953085,
    asymmetry: 0.718691994109505,
  },
  {
    name: "the generic stand-in's mode",
    sizes: gamma(0.5, 0.1),
    index: fixed({ n: 1.5, k: 0 }),
    wavelengthNm: 550,
    nodes: 464,
    qExt: 3.101820164059571,
    qSca: 3.101820164059571,
    asymmetry: 0.673379056286321,
  },
  {
    name: "Mars dust spheres of 1.5 µm",
    sizes: gamma(1.5, 0.3),
    index: material("mars_dust"),
    wavelengthNm: 550,
    nodes: 2800,
    qExt: 2.3905273719246143,
    qSca: 2.177766636563953,
    asymmetry: 0.7533568630892354,
  },
  {
    name: "Mars dust spheres of 1.5 µm",
    sizes: gamma(1.5, 0.3),
    index: material("mars_dust"),
    wavelengthNm: 645,
    nodes: 4848,
    qExt: 2.4438546662564735,
    qSca: 2.3618912084392143,
    asymmetry: 0.7265118774506889,
  },
  {
    name: "Mars dust spheres of 1.5 µm",
    sizes: gamma(1.5, 0.3),
    index: material("mars_dust"),
    wavelengthNm: 434,
    nodes: 3488,
    qExt: 2.3246229914140515,
    qSca: 1.821390390089591,
    asymmetry: 0.8073855022032624,
  },
  {
    // T5.b's water.json at 550 nm, Hale and Querry's n and k: the held optics are T5.b's, and since
    // R08.T5.h the file's n is IAPWS R9-97's (decision-r08-t5g ruling 1), so the index is pinned.
    name: "water droplets of 1.5 µm at T5.b's index",
    sizes: gamma(1.5, 0.3),
    index: fixed({ n: 1.333, k: 1.96e-9 }),
    wavelengthNm: 550,
    nodes: 5600,
    qExt: 2.4400399227005933,
    qSca: 2.440039767799159,
    asymmetry: 0.7929965571312556,
  },
  {
    name: "water ice spheres of 1.5 µm",
    sizes: gamma(1.5, 0.3),
    index: material("H2O", { phase: "solid" }),
    wavelengthNm: 550,
    nodes: 5600,
    qExt: 2.450724526376215,
    qSca: 2.450724351012741,
    asymmetry: 0.8050555082252997,
  },
  {
    name: "Mars dust spheres of 1 µm",
    sizes: gamma(1, 0.1),
    index: material("mars_dust"),
    wavelengthNm: 550,
    nodes: 1632,
    qExt: 2.4222002876687396,
    qSca: 2.2546258425643537,
    asymmetry: 0.7207209016056303,
  },
  {
    name: "H2SO4 droplets of 1 µm",
    sizes: gamma(1, 0.1),
    index: material("H2SO4"),
    wavelengthNm: 550,
    nodes: 1632,
    qExt: 2.425257238266094,
    qSca: 2.425256000771062,
    asymmetry: 0.7117413907701637,
  },
  {
    name: "liquid iron of 0.5 µm",
    sizes: gamma(0.5, 0.1),
    index: material("Fe", { phase: "liquid" }),
    wavelengthNm: 550,
    nodes: 232,
    qExt: 2.627542303067907,
    qSca: 1.7920185519250191,
    asymmetry: 0.6369928208590993,
  },
  {
    name: "the sulphate",
    sizes: logNormal(0.2, 0.2),
    index: material("H2SO4"),
    wavelengthNm: 645,
    nodes: 352,
    qExt: 1.2288240927688134,
    qSca: 1.2288239285558369,
    asymmetry: 0.6741164161173182,
  },
  {
    name: "the sulphate",
    sizes: logNormal(0.2, 0.2),
    index: material("H2SO4"),
    wavelengthNm: 544,
    nodes: 392,
    qExt: 1.6546898779842678,
    qSca: 1.6546896726566842,
    asymmetry: 0.7051725037029876,
  },
  {
    name: "the sulphate",
    sizes: logNormal(0.2, 0.2),
    index: material("H2SO4"),
    wavelengthNm: 433.625,
    nodes: 936,
    qExt: 2.2674420635425303,
    qSca: 2.267441790480149,
    asymmetry: 0.7327288143978189,
  },
  {
    name: "the TiO2 stand-in",
    sizes: STAND_IN,
    index: fixed(STAND_IN_INDEX),
    wavelengthNm: 680,
    nodes: 50720,
    qExt: 2.6420142297586806,
    qSca: 2.6420142297586806,
    asymmetry: 0.42325380428966275,
  },
  {
    name: "the TiO2 stand-in",
    sizes: STAND_IN,
    index: fixed(STAND_IN_INDEX),
    wavelengthNm: 550,
    nodes: 58776,
    qExt: 2.5617415076501597,
    qSca: 2.5617415076501606,
    asymmetry: 0.4495643472533773,
  },
  {
    name: "TiO2 of 3 µm",
    sizes: gamma(3, 0.1),
    index: fixed(STAND_IN_INDEX),
    wavelengthNm: 550,
    nodes: 37520,
    qExt: 2.1816285695570627,
    qSca: 2.1816285695570627,
    asymmetry: 0.5797485676345534,
  },
  {
    // T5.b's ammonia-ice.json at 550 nm, Martonchik et al.'s n and k: the held optics are T5.b's,
    // and since R08.T5.i the file's k is 0 (decision-p14-t49c-split §5.3), so the index is pinned.
    name: "NH3 ice of 10 µm at T5.b's index",
    sizes: gamma(10, 0.1),
    index: fixed({ n: 1.43606, k: 2.88714e-5 }),
    wavelengthNm: 550,
    nodes: 3912,
    qExt: 2.0891048075491843,
    qSca: 2.076230543129823,
    asymmetry: 0.8320819595479251,
  },
  {
    name: "CH4 ice of 10 µm",
    sizes: gamma(10, 0.1),
    index: material("CH4", { phase: "solid" }),
    wavelengthNm: 550,
    nodes: 3912,
    qExt: 2.0898873169222876,
    qSca: 2.0898873169222876,
    asymmetry: 0.8676211870610973,
  },
  {
    name: "CO2 ice of 10 µm",
    sizes: gamma(10, 0.1),
    index: material("CO2", { phase: "solid" }),
    wavelengthNm: 550,
    nodes: 7816,
    qExt: 2.0893646465730145,
    qSca: 2.089364437894714,
    asymmetry: 0.8373815586088129,
  },
  {
    name: "water ice of 0.03 µm",
    sizes: gamma(0.03, 0.1),
    index: material("H2O", { phase: "solid" }),
    wavelengthNm: 650,
    nodes: 128,
    qExt: 0.001164322519993009,
    qSca: 0.0011643127118202746,
    asymmetry: 0.03164767330527265,
  },
  {
    name: "dispersive rutile of 3 µm",
    sizes: gamma(3, 0.1),
    index: rutile,
    wavelengthNm: 550,
    nodes: 18760,
    qExt: 2.181755896449359,
    qSca: 2.1817558964493586,
    asymmetry: 0.5799281660367377,
  },
  {
    // Half nonSpherical.ts's SMALL_GRAIN_SIZE_PARAMETER of 0.22, as nonSpherical.test.ts's grain.
    name: "a small rutile grain",
    sizes: gamma((0.5 * 0.22 * 0.55) / (2 * Math.PI), 0.1),
    index: rutile,
    wavelengthNm: 550,
    nodes: 128,
    qExt: 0.00030325992857143595,
    qSca: 0.0003032599285714359,
    asymmetry: 0.009034492813470043,
  },
  {
    name: "an absorbing mineral of 3 µm",
    sizes: gamma(3, 0.1),
    index: fixed({ n: 1.5, k: 0.01 }),
    wavelengthNm: 550,
    nodes: 1176,
    qExt: 2.199308757917674,
    qSca: 1.4667017257065658,
    asymmetry: 0.8844716303115467,
  },
  {
    name: "a transparent high-index mineral of 8 µm",
    sizes: gamma(8, 0.1),
    index: fixed({ n: 1.8, k: 2e-5 }),
    wavelengthNm: 550,
    nodes: 6256,
    qExt: 2.1016928457904056,
    qSca: 2.0937986455349304,
    asymmetry: 0.7341171066221748,
  },
];

/** The lossless spheres the stop must converge on at the fitted channels (R08.T5.f). */
const LOSSLESS: ReadonlyArray<{ readonly n: number; readonly effectiveRadiusUm: number }> = [
  1.33, 1.5, 2,
].flatMap((n) => [0.5, 5].map((effectiveRadiusUm) => ({ n, effectiveRadiusUm })));

describe("the size quadrature's stop (R08.T5.f)", () => {
  it("does not take the stand-in's first agreement at 435 nm, but a level after a second", () => {
    const first = firstAgreement(STAND_IN, STAND_IN_INDEX, 435);
    const optics = sphereModeOptics(STAND_IN, STAND_IN_INDEX, 435, NO_ANGLES);
    expect(optics.sizeConvergence.converged).toBe(true);
    expect(optics.sizeConvergence.nodes).toBeGreaterThan(first.taken.nodes);
    expect(optics.sizeConvergence.lastChange).toBeLessThan(SIZE_CONVERGENCE);
  });

  it("does not stop at the stand-in's chance agreement at 435.125 nm", () => {
    // T5.b stopped at 2,200 nodes on a change of 6.9 × 10⁻⁵; the next level moves by 4.9 × 10⁻⁴.
    const chance = firstAgreement(STAND_IN, STAND_IN_INDEX, 435.125);
    expect(chance.change).toBeLessThan(SIZE_CONVERGENCE);
    expect(chance.nextChange).toBeGreaterThan(3 * SIZE_CONVERGENCE);
    const optics = sphereModeOptics(STAND_IN, STAND_IN_INDEX, 435.125, NO_ANGLES);
    expect(optics.sizeConvergence.converged).toBe(true);
    expect(optics.sizeConvergence.nodes).toBeGreaterThan(chance.taken.nodes);
    expect(bulkChange(chance.taken, optics)).toBeGreaterThan(SIZE_CONVERGENCE);
  });

  it.each(LOSSLESS)(
    "converges for lossless spheres of n $n at r_eff $effectiveRadiusUm µm at the fitted channels",
    ({ n, effectiveRadiusUm }) => {
      const ends = CHANNEL_WAVELENGTHS_NM.map(
        (nm) =>
          sphereModeOptics(gamma(effectiveRadiusUm, 0.1), { n, k: 0 }, nm, NO_ANGLES)
            .sizeConvergence,
      );
      expect(ends.map((end) => end.converged)).toEqual([true, true, true]);
      expect(Math.max(...ends.map((end) => end.lastChange))).toBeLessThan(SIZE_CONVERGENCE);
    },
  );

  it.each(HELD_MODES)(
    "leaves $name at $wavelengthNm nm as T5.b gave it, to 10⁻⁴",
    ({ sizes, index, wavelengthNm, nodes, ...was }) => {
      const optics = sphereModeOptics(sizes, index(wavelengthNm), wavelengthNm, NO_ANGLES);
      expect(optics.sizeConvergence.converged).toBe(true);
      expect(optics.sizeConvergence.nodes).toBeGreaterThanOrEqual(nodes);
      expect(bulkChange(was, optics)).toBeLessThan(SIZE_CONVERGENCE);
    },
  );

  it("gives the finest level within a cap, marked unconverged with its last change", () => {
    // The stand-in at the fitted blue channel, whose changes stay above the tolerance to 8,824 nodes.
    const cap = 10_000;
    const optics = sphereModeOptics(STAND_IN, STAND_IN_INDEX, 433.625, NO_ANGLES, cap);
    let level = 0;
    while (sizeQuadrature(STAND_IN, 433.625, level + 1).radiiUm.length <= cap) {
      level += 1;
    }
    const finest = levelOptics(STAND_IN, STAND_IN_INDEX, 433.625, level);
    const before = levelOptics(STAND_IN, STAND_IN_INDEX, 433.625, level - 1);
    expect(optics.sizeConvergence.converged).toBe(false);
    expect(optics.sizeConvergence.nodes).toBe(finest.nodes);
    expect(optics.sizeConvergence.lastChange).toBeGreaterThan(SIZE_CONVERGENCE);
    expect(optics.sizeConvergence.lastChange).toBeCloseTo(change(before, finest), 12);
    expect(optics.qExt).toBeCloseTo(finest.qExt, 12);
    expect(optics.asymmetry).toBeCloseTo(finest.asymmetry, 12);
  });

  it("gives the coarsest level, with no change to report, under a cap below it", () => {
    const optics = sphereModeOptics(STAND_IN, STAND_IN_INDEX, 433.625, NO_ANGLES, 100);
    expect(optics.sizeConvergence).toEqual({
      converged: false,
      nodes: sizeQuadrature(STAND_IN, 433.625, 0).radiiUm.length,
      lastChange: Number.POSITIVE_INFINITY,
    });
  });

  it("refuses a node cap that is not a positive integer", () => {
    for (const cap of [0, -8, 1.5, Number.NaN]) {
      expect(() => sphereModeOptics(VENUS_MODE_2, { n: 1.44, k: 0 }, 550, NO_ANGLES, cap)).toThrow(
        RangeError,
      );
    }
  });

  it("carries the quadrature's end into a registry mode's optics", () => {
    const sizes = gamma(0.5, 0.1);
    const optics = modeOptics({ material: "H2SO4", sizes }, 550);
    const spheres = sphereModeOptics(sizes, refractiveIndex("H2SO4", 550).index, 550, NO_ANGLES);
    expect(optics.sizeConvergence).toEqual(spheres.sizeConvergence);
    expect(optics.sizeConvergence?.converged).toBe(true);
  });

  it("caps the quadrature at 2²⁰ nodes, and its levels with it", () => {
    expect(SIZE_NODES_MAX).toBe(2 ** 20);
    expect(SIZE_LEVEL_MAX).toBe(20);
  });

  it.runIf(slowTests())(
    "converges for the stand-in at every 0.125 nm from 430 to 437 nm, at the fitted channels and at the bake bins",
    { timeout: 3_600_000 },
    () => {
      const sweep = Array.from({ length: 57 }, (_, i) => 430 + 0.125 * i);
      const unconverged = [...sweep, ...CHANNEL_WAVELENGTHS_NM, ...BAKE_WAVELENGTHS_NM].filter(
        (nm) =>
          !sphereModeOptics(STAND_IN, STAND_IN_INDEX, nm, NO_ANGLES).sizeConvergence.converged,
      );
      expect(unconverged).toEqual([]);
    },
  );

  it.runIf(slowTests())(
    "moves the stand-in at 440 nm, where T5.b stopped on a chance agreement, by over 10⁻⁴",
    { timeout: 600_000 },
    () => {
      // T5.b took 8,720 nodes there; two agreements take 278,984.
      const was = {
        qExt: 2.4864529454392446,
        qSca: 2.4864529454392446,
        asymmetry: 0.475086421495458,
      };
      const optics = sphereModeOptics(STAND_IN, STAND_IN_INDEX, 440, NO_ANGLES);
      expect(optics.sizeConvergence.converged).toBe(true);
      expect(bulkChange(was, optics)).toBeGreaterThan(SIZE_CONVERGENCE);
    },
  );
});
