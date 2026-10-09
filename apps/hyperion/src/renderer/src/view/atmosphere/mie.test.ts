import { describe, expect, it } from "vitest";

import { gaussLegendre } from "../lighting/quadrature";
import {
  type ComplexIndex,
  logarithmicDerivatives,
  MIE_MAX_SIZE_PARAMETER,
  MIE_MIN_SIZE_PARAMETER,
  type MieResult,
  mieSphere,
  mieTermCount,
} from "./mie";

/** A complex number as [re, im], as the reference tables list them. */
type Pair = readonly [number, number];

/** One of Wiscombe's MIEV0 test cases with his stored answers. */
interface WiscombeCase {
  /** MIEV0's case number. */
  readonly id: number;
  readonly x: number;
  readonly index: ComplexIndex;
  readonly qExt: number;
  readonly qSca: number;
  /** g Q_sca, MIEV0's GQSC. */
  readonly gQSca: number;
  /** S₁ at 0°, 30°, … 180°, in Wiscombe's sign convention, as {@link mieSphere} returns it. */
  readonly s1: ReadonlyArray<Pair>;
  /** S₂ at the same angles. */
  readonly s2: ReadonlyArray<Pair>;
  /**
   * The amplitudes, named `S1 90` or `S2 180`, where Wiscombe's stored answer is good to only a
   * few 10⁻⁶ ({@link REFERENCE_LIMITED_TOLERANCE}).
   */
  readonly limited: ReadonlyArray<string>;
}

/**
 * MIEV0's refractive-index test cases 5 to 19 and the answers its test driver checks against:
 * `BLOCK DATA CHEKMI` of `MVTstNew.f` in Wiscombe's MIEV distribution (the 1996 revision of
 * NCAR/TN-140+STR), "the answer obtained on a Cray computer using 14-digit precision", stored to 7
 * significant figures. miepython's tests (S. Prahl, MIT licence, `tests/test_mie.py`) tabulate the
 * same cases' Q and g to 6 decimals, and case 14's amplitudes.
 *
 * Case 15 (m = 1.5 − i, x = 100), which miepython checks only to 10⁻³ and the plan said to omit,
 * is kept: this code agrees with Wiscombe's own answer to 2.5 × 10⁻⁷.
 */
const WISCOMBE_CASES: ReadonlyArray<WiscombeCase> = [
  {
    id: 5,
    x: 0.099,
    index: { n: 0.75, k: 0 },
    qExt: 7.417859e-6,
    qSca: 7.417859e-6,
    gQSca: 1.074279e-8,
    s1: [
      [1.817558e-8, -1.654225e-4],
      [1.817558e-8, -1.653815e-4],
      [1.817558e-8, -1.652694e-4],
      [1.817558e-8, -1.651163e-4],
      [1.817558e-8, -1.649631e-4],
      [1.817558e-8, -1.64851e-4],
      [1.817558e-8, -1.6481e-4],
    ],
    s2: [
      [1.817558e-8, -1.654225e-4],
      [1.574051e-8, -1.432172e-4],
      [9.087788e-9, -8.261265e-5],
      [9.797186e-23, 2.938374e-8],
      [-9.087788e-9, 8.25036e-5],
      [-1.574051e-8, 1.427725e-4],
      [-1.817558e-8, 1.6481e-4],
    ],
    // From MIEV0's small-particle formulas (x max(1, |m|) ≤ 0.1), not its series; S₂(90°) is a
    // remainder 10⁻⁴ of the forward amplitude.
    limited: ["S2 90"],
  },
  {
    id: 6,
    x: 0.101,
    index: { n: 0.75, k: 0 },
    qExt: 8.033542e-6,
    qSca: 8.033542e-6,
    gQSca: 1.211e-8,
    s1: [
      [2.048754e-8, -1.756419e-4],
      [2.048754e-8, -1.755965e-4],
      [2.048753e-8, -1.754726e-4],
      [2.048751e-8, -1.753033e-4],
      [2.04875e-8, -1.751341e-4],
      [2.048749e-8, -1.750103e-4],
      [2.048749e-8, -1.74965e-4],
    ],
    s2: [
      [2.048754e-8, -1.756419e-4],
      [1.774273e-8, -1.520629e-4],
      [1.024377e-8, -8.771198e-5],
      [1.845057e-15, 3.24727e-8],
      [-1.024375e-8, 8.759147e-5],
      [-1.774269e-8, 1.515715e-4],
      [-2.048749e-8, 1.74965e-4],
    ],
    // MIEV0 sums x + 4 x^(1/3) + 1 = 2 terms here; the third, which N_stop keeps, moves S₂ at 60°
    // and 120° by 7.6 × 10⁻⁷ of the 1.1 × 10⁻⁶ found (the power-series test below agrees with this
    // code to 10⁻¹⁰).
    limited: ["S2 60", "S2 120"],
  },
  {
    id: 7,
    x: 10,
    index: { n: 0.75, k: 0 },
    qExt: 2.232265,
    qSca: 2.232265,
    gQSca: 2.001164,
    s1: [
      [5.580662e1, -9.758097],
      [-7.672879, 1.087317e1],
      [3.587894, -1.756177],
      [-1.785905, -5.232828e-2],
      [1.537971, -8.329374e-2],
      [-4.140427e-1, 1.876851e-1],
      [-1.078568, -3.608807e-2],
    ],
    s2: [
      [5.580662e1, -9.758097],
      [-1.092923e1, 9.629667],
      [3.427411, 8.082691e-2],
      [-5.148748e-1, -7.027288e-1],
      [-6.908338e-1, 2.152693e-1],
      [5.247557e-1, -1.923391e-1],
      [1.078568, 3.608807e-2],
    ],
    limited: [],
  },
  {
    id: 8,
    x: 1000,
    index: { n: 0.75, k: 0 },
    qExt: 1.997908,
    qSca: 1.997908,
    gQSca: 1.688121,
    s1: [
      [4.99477e5, -1.336502e4],
      [-3.999296e2, -3.316361e2],
      [-5.209852e2, -5.776614e2],
      [-1.600887e2, 1.348013e2],
      [8.43172e1, -1.209493e2],
      [-7.556092e1, -8.13481e1],
      [1.705778e1, 4.84251e2],
    ],
    s2: [
      [4.99477e5, -1.336502e4],
      [-3.946018e2, -1.147791e2],
      [-1.970767e2, -6.93747e2],
      [-4.152365e1, 1.143e2],
      [-4.261732e1, 5.535055e1],
      [4.218303e1, 9.100831e1],
      [-1.705778e1, -4.84251e2],
    ],
    limited: [],
  },
  {
    id: 9,
    x: 1,
    index: { n: 1.33, k: 1e-5 },
    qExt: 9.395198e-2,
    qSca: 9.39233e-2,
    gQSca: 1.733048e-2,
    s1: [
      [2.3488e-2, 2.281705e-1],
      [2.341722e-2, 2.217102e-1],
      [2.322408e-2, 2.046815e-1],
      [2.296081e-2, 1.828349e-1],
      [2.26982e-2, 1.625401e-1],
      [2.250635e-2, 1.48617e-1],
      [2.243622e-2, 1.437106e-1],
    ],
    s2: [
      [2.3488e-2, 2.281705e-1],
      [2.03483e-2, 1.938171e-1],
      [1.181704e-2, 1.075976e-1],
      [2.729533e-4, 6.702879e-3],
      [-1.114466e-2, -7.646326e-2],
      [-1.9423e-2, -1.271557e-1],
      [-2.243622e-2, -1.437106e-1],
    ],
    limited: [],
  },
  {
    id: 10,
    x: 100,
    index: { n: 1.33, k: 1e-5 },
    qExt: 2.101321,
    qSca: 2.096594,
    gQSca: 1.821854,
    s1: [
      [5.253302e3, -1.243188e2],
      [-5.534573e1, -2.971881e1],
      [1.710488e1, -1.520096e1],
      [-3.655758, 8.76986],
      [2.414318, 5.380874e-1],
      [-1.222996, 3.283917e1],
      [-5.659205e1, 4.650974e1],
    ],
    s2: [
      [5.253302e3, -1.243188e2],
      [-8.467204e1, -1.99947e1],
      [3.310764e1, -2.709787],
      [-6.550512, -4.67537],
      [6.039011, -1.169971e1],
      [-9.653812, 1.474455e1],
      [5.659205e1, -4.650974e1],
    ],
    limited: [],
  },
  {
    id: 11,
    x: 10000,
    index: { n: 1.33, k: 1e-5 },
    qExt: 2.004089,
    qSca: 1.723857,
    gQSca: 1.564987,
    s1: [
      [5.010222e7, -1.535815e5],
      [3.786814e3, -7.654293e3],
      [-2.731172e3, 1.326633e3],
      [-1.061003e3, -1.930155e2],
      [-1.05814e3, 2.298414e1],
      [-2.748855e3, 2.298181e3],
      [-1.821193e2, -9.519122e2],
    ],
    s2: [
      [5.010222e7, -1.535815e5],
      [5.074755e3, -7.515986e3],
      [-3.076558e3, -1.775975e2],
      [2.43092e2, 8.409836e1],
      [5.906487e1, -5.370283e2],
      [-8.036201e1, -4.939186],
      [1.821194e2, 9.519123e2],
    ],
    // MIEV0 sums x + 4 x^(1/3) + 2 = 10,088 terms here (his (50), which he finds low by 1 at 8 of
    // 135 points); the 10,089th, which N_stop keeps, is a resonance with |aₙ|² + |bₙ|² = 2.1 × 10⁻¹³
    // that moves S(180°) by 4.2 × 10⁻⁶. Wiscombe's answer is the 10,088-term sum to 10⁻⁷, and this
    // code is 4 × 10⁻⁷ from the converged series (a 40-digit evaluation; R08's Risks).
    limited: ["S1 180", "S2 180"],
  },
  {
    id: 12,
    x: 0.055,
    index: { n: 1.5, k: 1 },
    qExt: 1.01491e-1,
    qSca: 1.131687e-5,
    gQSca: 5.558541e-9,
    s1: [
      [7.675259e-5, 8.343879e-5],
      [7.674331e-5, 8.343495e-5],
      [7.671794e-5, 8.342445e-5],
      [7.668328e-5, 8.341012e-5],
      [7.664863e-5, 8.339578e-5],
      [7.662326e-5, 8.338529e-5],
      [7.661398e-5, 8.338145e-5],
    ],
    s2: [
      [7.675259e-5, 8.343879e-5],
      [6.646948e-5, 7.225169e-5],
      [3.838246e-5, 4.169695e-5],
      [3.132066e-8, -2.037399e-8],
      [-3.830082e-5, -4.171317e-5],
      [-6.634986e-5, -7.221887e-5],
      [-7.661398e-5, -8.338145e-5],
    ],
    // MIEV0's small-particle formulas, as in case 5.
    limited: ["S2 90"],
  },
  {
    id: 13,
    x: 0.056,
    index: { n: 1.5, k: 1 },
    qExt: 1.033467e-1,
    qSca: 1.216311e-5,
    gQSca: 6.193255e-9,
    s1: [
      [8.102381e-5, 8.807251e-5],
      [8.101364e-5, 8.80683e-5],
      [8.098587e-5, 8.805682e-5],
      [8.094795e-5, 8.804113e-5],
      [8.091003e-5, 8.802545e-5],
      [8.088228e-5, 8.801396e-5],
      [8.087213e-5, 8.800976e-5],
    ],
    s2: [
      [8.102381e-5, 8.807251e-5],
      [7.016844e-5, 7.626381e-5],
      [4.051865e-5, 4.401169e-5],
      [3.427277e-8, -2.229631e-8],
      [-4.042932e-5, -4.402945e-5],
      [-7.003755e-5, -7.62279e-5],
      [-8.087213e-5, -8.800976e-5],
    ],
    limited: [],
  },
  {
    id: 14,
    x: 1,
    index: { n: 1.5, k: 1 },
    qExt: 2.336321,
    qSca: 6.634538e-1,
    gQSca: 1.274736e-1,
    s1: [
      [5.840802e-1, 1.905153e-1],
      [5.65702e-1, 1.871997e-1],
      [5.175251e-1, 1.784426e-1],
      [4.563396e-1, 1.671665e-1],
      [4.002117e-1, 1.566427e-1],
      [3.621572e-1, 1.49391e-1],
      [3.488438e-1, 1.468286e-1],
    ],
    s2: [
      [5.840802e-1, 1.905153e-1],
      [5.00161e-1, 1.456112e-1],
      [2.879639e-1, 4.105398e-2],
      [3.622847e-2, -6.182646e-2],
      [-1.74875e-1, -1.229586e-1],
      [-3.056823e-1, -1.43846e-1],
      [-3.488438e-1, -1.468286e-1],
    ],
    limited: [],
  },
  {
    id: 15,
    x: 100,
    index: { n: 1.5, k: 1 },
    qExt: 2.097502,
    qSca: 1.283697,
    gQSca: 1.091466,
    s1: [
      [5.243754e3, -2.934167e2],
      [4.049055e1, -1.898456e1],
      [-2.646835e1, -1.929564e1],
      [1.26889e1, 2.397474e1],
      [5.149886, 2.290736e1],
      [-1.605395e1, 1.418642e1],
      [-2.02936e1, 4.384435],
    ],
    s2: [
      [5.243754e3, -2.934167e2],
      [2.019198e1, 3.110731],
      [9.152743, -7.470202],
      [-1.232914e1, -7.823167],
      [-7.173357, -1.655464e1],
      [1.448052e1, -1.393594e1],
      [2.02936e1, -4.384435],
    ],
    limited: [],
  },
  {
    id: 16,
    x: 10000,
    index: { n: 1.5, k: 1 },
    qExt: 2.004368,
    qSca: 1.236574,
    gQSca: 1.046525,
    s1: [
      [5.010919e7, -1.753404e5],
      [-3.690394e3, -1.573897e3],
      [2.391551e2, 3.247786e3],
      [-2.607463e3, 7.414859e2],
      [-6.183154e2, 2.26497e3],
      [-3.368019e2, 2.11575e3],
      [-2.184719e2, -2.06461e3],
    ],
    s2: [
      [5.010919e7, -1.753404e5],
      [-9.333175e2, -1.839736e3],
      [-1.202951e3, -1.899647e2],
      [1.013073e3, -1.064666e3],
      [1.334826e2, -1.800859e3],
      [2.293862e2, -1.996754e3],
      [2.184719e2, 2.06461e3],
    ],
    limited: [],
  },
  {
    id: 17,
    x: 1,
    index: { n: 10, k: 10 },
    qExt: 2.532993,
    qSca: 2.049405,
    gQSca: -2.267961e-1,
    s1: [
      [6.332483e-1, 4.179305e-1],
      [6.162264e-1, 4.597163e-1],
      [5.736317e-1, 5.602514e-1],
      [5.238628e-1, 6.675352e-1],
      [4.825816e-1, 7.434033e-1],
      [4.570214e-1, 7.809867e-1],
      [4.485464e-1, 7.912365e-1],
    ],
    s2: [
      [6.332483e-1, 4.179305e-1],
      [5.573186e-1, 2.954338e-1],
      [3.525107e-1, -5.921611e-3],
      [7.881172e-2, -3.435544e-1],
      [-1.881212e-1, -6.028739e-1],
      [-3.793898e-1, -7.473279e-1],
      [-4.485464e-1, -7.912365e-1],
    ],
    limited: [],
  },
  {
    id: 18,
    x: 100,
    index: { n: 10, k: 10 },
    qExt: 2.071124,
    qSca: 1.836785,
    gQSca: 1.021648,
    s1: [
      [5.177811e3, -2.633811e1],
      [5.227436e1, -1.270012e1],
      [-2.705712e1, -3.951751e1],
      [1.00886, 4.663027e1],
      [-1.50564e1, 4.333057e1],
      [-4.51077e1, 5.199554],
      [-4.145383e1, -1.821808e1],
    ],
    s2: [
      [5.177811e3, -2.633811e1],
      [-2.380252e1, -3.872567e-1],
      [2.585821e1, 3.323624e1],
      [-3.479935, -4.364245e1],
      [1.360634e1, -4.238302e1],
      [4.474564e1, -5.452513],
      [4.145383e1, 1.821808e1],
    ],
    limited: [],
  },
  {
    id: 19,
    x: 10000,
    index: { n: 10, k: 10 },
    qExt: 2.005914,
    qSca: 1.795393,
    gQSca: 9.842238e-1,
    s1: [
      [5.014786e7, -1.206004e5],
      [-4.08009e3, -2.664399e3],
      [-1.22404e3, 4.596569e3],
      [-4.57949e3, -8.590486e2],
      [-3.356286e3, 3.125121e3],
      [-3.149584e3, 3.270358e3],
      [2.25248e3, -3.924468e3],
    ],
    s2: [
      [5.014786e7, -1.206004e5],
      [3.351286e3, 7.291906e2],
      [4.497446e2, -4.072999e3],
      [4.313394e3, 4.969719e2],
      [3.17191e3, -3.129068e3],
      [3.105243e3, -3.269355e3],
      [-2.25248e3, 3.924468e3],
    ],
    limited: [],
  },
];

/** Wiscombe's seven test angles, 0° to 180° in steps of 30° (`MVTstNew.f`). */
const WISCOMBE_ANGLES_DEG: ReadonlyArray<number> = [0, 30, 60, 90, 120, 150, 180];
const WISCOMBE_MU = Float64Array.from(WISCOMBE_ANGLES_DEG, (deg) =>
  Math.cos((deg * Math.PI) / 180),
);

/**
 * The tolerance on Q_ext, Q_sca and g against Wiscombe's answers: 10⁻⁶ (the plan's, miepython's),
 * absolute at values of order one and relative below, where an absolute 10⁻⁶ would test nothing.
 */
const WISCOMBE_TOLERANCE = 1e-6;

/** The tolerance on the amplitudes Wiscombe's answers hold to only a few 10⁻⁶, each case's `limited`. */
const REFERENCE_LIMITED_TOLERANCE = 5e-6;

/** The amplitude at angle `j` of an interleaved `s1` or `s2`. */
function amplitude(values: Float64Array, j: number): Pair {
  return [values[2 * j] ?? Number.NaN, values[2 * j + 1] ?? Number.NaN];
}

/** |a − b| ÷ |b| for complex a and b. */
function relativeError(a: Pair, b: Pair): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1]) / Math.hypot(b[0], b[1]);
}

/** |S|² summed over both amplitudes at angle `j`. */
function intensity(result: MieResult, j: number): number {
  const [a, b] = amplitude(result.s1, j);
  const [c, d] = amplitude(result.s2, j);
  return a * a + b * b + c * c + d * d;
}

describe("mieSphere against Wiscombe's MIEV0 test cases", () => {
  it.each(WISCOMBE_CASES)(
    "case $id (x = $x) gives Q_ext, Q_sca and g to 10⁻⁶",
    ({ x, index, qExt, qSca, gQSca }) => {
      const result = mieSphere(x, index, new Float64Array(0));
      const g = gQSca / qSca;
      expect(Math.abs(result.qExt - qExt)).toBeLessThanOrEqual(
        WISCOMBE_TOLERANCE * Math.min(1, qExt),
      );
      expect(Math.abs(result.qSca - qSca)).toBeLessThanOrEqual(
        WISCOMBE_TOLERANCE * Math.min(1, qSca),
      );
      // g, a mean cosine, absolute: a small sphere's g is a higher-order remainder (x² of order
      // one), which Wiscombe's small-particle answers (cases 5, 12) hold to 2 × 10⁻⁹ absolute.
      expect(Math.abs(result.asymmetry - g)).toBeLessThanOrEqual(WISCOMBE_TOLERANCE);
    },
  );

  it.each(WISCOMBE_CASES)(
    "case $id (x = $x) gives S₁ and S₂ at 0° to 180° to 10⁻⁶ of each, 5 × 10⁻⁶ where limited",
    ({ x, index, s1, s2, limited }) => {
      const result = mieSphere(x, index, WISCOMBE_MU);
      WISCOMBE_ANGLES_DEG.forEach((deg, j) => {
        for (const [name, computed, reference] of [
          ["S1", result.s1, s1[j]],
          ["S2", result.s2, s2[j]],
        ] as const) {
          if (reference === undefined) {
            throw new Error(`case is missing ${name} at ${deg}°`);
          }
          const tolerance = limited.includes(`${name} ${deg}`)
            ? REFERENCE_LIMITED_TOLERANCE
            : WISCOMBE_TOLERANCE;
          expect(
            relativeError(amplitude(computed, j), reference),
            `${name} at ${deg}°`,
          ).toBeLessThan(tolerance);
        }
      });
    },
  );
});

describe("mieSphere against Bohren and Huffman's BHMIE sample", () => {
  it("reproduces m = 1.55 at λ = 0.6328 µm, r = 0.525 µm to the printed digits", () => {
    // Bohren and Huffman 1983, Appendix A, p. 482: QSCA = QEXT = 3.10543, QBACK = 2.92534, at
    // x = 2π × 0.525 ÷ 0.6328. BHMIE prints no g; 0.63314 is miepython's for the same case
    // (`test_03_bh_dielectric`). The tolerance is half a unit of the last printed digit.
    const x = (2 * Math.PI * 0.525) / 0.6328;
    const result = mieSphere(x, { n: 1.55, k: 0 }, new Float64Array([-1]));
    const [backRe, backIm] = amplitude(result.s1, 0);
    const qBack = (4 * (backRe * backRe + backIm * backIm)) / (x * x);
    const halfUnit = 5e-6;
    expect(Math.abs(result.qExt - 3.10543)).toBeLessThanOrEqual(halfUnit);
    expect(Math.abs(result.qSca - 3.10543)).toBeLessThanOrEqual(halfUnit);
    expect(Math.abs(qBack - 2.92534)).toBeLessThanOrEqual(halfUnit);
    expect(Math.abs(result.asymmetry - 0.63314)).toBeLessThanOrEqual(halfUnit);
  });
});

/** K = (m² − 1) ÷ (m² + 2) for m = n + ik (Bohren and Huffman's sign), as [re, im]. */
function polarisability({ n, k }: ComplexIndex): Pair {
  const m2Re = n * n - k * k;
  const m2Im = 2 * n * k;
  const denominator = (m2Re + 2) ** 2 + m2Im ** 2;
  return [
    ((m2Re - 1) * (m2Re + 2) + m2Im * m2Im) / denominator,
    (m2Im * (m2Re + 2) - (m2Re - 1) * m2Im) / denominator,
  ];
}

/** A small sphere for the Rayleigh-limit tests. */
interface RayleighCase {
  readonly index: ComplexIndex;
  readonly x: number;
}

describe("mieSphere in the Rayleigh limit", () => {
  // Bohren and Huffman 1983, §5.1, (5.4), (5.8) and (5.11): as x → 0, Q_sca → (8/3) x⁴ |K|²,
  // Q_abs → 4x Im K, g → 0, and the amplitudes are a dipole's, S₁ = (3/2) a₁ and
  // S₂ = (3/2) a₁ cos θ, so |S₁|² is isotropic and |S₂|² = μ² |S₁|². The corrections are of
  // order x², which bounds each comparison; at x = 10⁻⁵ that is 10⁻¹⁰, and ψ₁'s Taylor series is
  // what holds it (sin x ÷ x − cos x alone is 3 × 10⁻⁶ off there).
  const cases: ReadonlyArray<RayleighCase> = [
    { index: { n: 1.33, k: 0 }, x: 1e-3 },
    { index: { n: 1.33, k: 0 }, x: 1e-5 },
    { index: { n: 1.5, k: 0.1 }, x: 1e-3 },
    { index: { n: 1.5, k: 0.1 }, x: 1e-5 },
  ];

  it.each(cases)("scatters (8/3) x⁴ |K|² at m = $index.n + $index.k i, x = $x", ({ index, x }) => {
    const result = mieSphere(x, index, new Float64Array(0));
    const [kRe, kIm] = polarisability(index);
    const rayleighQSca = (8 / 3) * x ** 4 * (kRe * kRe + kIm * kIm);
    expect(Math.abs(result.qSca / rayleighQSca - 1)).toBeLessThan(x * x);
  });

  it.each(cases.filter(({ index }) => index.k > 0))(
    "absorbs 4x Im K at m = $index.n + $index.k i, x = $x",
    ({ index, x }) => {
      const result = mieSphere(x, index, new Float64Array(0));
      const [, kIm] = polarisability(index);
      expect(Math.abs((result.qExt - result.qSca) / (4 * x * kIm) - 1)).toBeLessThan(x * x);
    },
  );

  it.each(cases)(
    "scatters as a dipole, with g → 0, at m = $index.n + $index.k i, x = $x",
    ({ index, x }) => {
      const mu = 0.5;
      const result = mieSphere(x, index, new Float64Array([1, mu]));
      const [f1Re, f1Im] = amplitude(result.s1, 0);
      const [s1Re, s1Im] = amplitude(result.s1, 1);
      const [s2Re, s2Im] = amplitude(result.s2, 1);
      const forward = f1Re * f1Re + f1Im * f1Im;
      expect(Math.abs(result.asymmetry)).toBeLessThan(x * x);
      expect(Math.abs((s1Re * s1Re + s1Im * s1Im) / forward - 1)).toBeLessThan(x * x);
      expect(Math.abs((s2Re * s2Re + s2Im * s2Im) / forward - mu * mu)).toBeLessThan(x * x);
    },
  );
});

describe("mieSphere's conservation", () => {
  it("absorbs nothing when k = 0, to 10⁻¹²", () => {
    for (const x of [0.1, 1, 10, 100, 1_000, 10_000]) {
      for (const n of [0.75, 1.0001, 1.33, 1.55, 2.5, 10]) {
        const result = mieSphere(x, { n, k: 0 }, new Float64Array(0));
        expect(Math.abs(result.qExt - result.qSca), `x = ${x}, n = ${n}`).toBeLessThan(1e-12);
      }
    }
  });

  it.each([
    { x: 0.5, index: { n: 1.33, k: 0 } },
    { x: 10, index: { n: 1.33, k: 0 } },
    { x: 100, index: { n: 1.5, k: 0.01 } },
    { x: 1_000, index: { n: 1.55, k: 0.1 } },
  ])(
    "normalises the phase function and its mean cosine to 10⁻⁹ at x = $x, on N_stop + 1 nodes",
    ({ x, index }) => {
      // |S₁|² + |S₂|² is a polynomial in μ of degree 2 N_stop, which Gauss–Legendre on more than
      // N_stop nodes integrates exactly: ∫ (|S₁|² + |S₂|²) dμ = x² Q_sca, and with μ, x² Q_sca g.
      const rule = gaussLegendre(mieTermCount(x) + 1);
      const result = mieSphere(x, index, rule.x);
      let total = 0;
      let moment = 0;
      for (let j = 0; j < rule.x.length; j += 1) {
        const weighted = (rule.w[j] ?? 0) * intensity(result, j);
        total += weighted;
        moment += weighted * (rule.x[j] ?? 0);
      }
      expect(Math.abs(total / (x * x * result.qSca) - 1)).toBeLessThan(1e-9);
      expect(Math.abs(moment / (x * x * result.qSca) - result.asymmetry)).toBeLessThan(1e-9);
    },
  );
});

/** The spherical Bessel functions jₙ(z) and yₙ(z) at one order and argument. */
interface SphericalBessel {
  readonly j: number;
  readonly y: number;
}

/**
 * jₙ(z) and yₙ(z) of real z by their power series (Abramowitz and Stegun 10.1.2 and 10.1.3), which
 * are well conditioned at small z, with j₋₁ = cos z ÷ z and y₋₁ = sin z ÷ z.
 */
function sphericalBesselBySeries(order: number, z: number): SphericalBessel {
  if (order === -1) {
    return { j: Math.cos(z) / z, y: Math.sin(z) / z };
  }
  let doubleFactorialAbove = 1;
  for (let i = 2 * order + 1; i > 1; i -= 2) {
    doubleFactorialAbove *= i;
  }
  const doubleFactorialBelow = doubleFactorialAbove / (2 * order + 1);
  let jSum = 0;
  let ySum = 0;
  let jTerm = 1;
  let yTerm = 1;
  for (let k = 0; k < 30; k += 1) {
    if (k > 0) {
      jTerm *= -(z * z) / 2 / (k * (2 * order + 2 * k + 1));
      yTerm *= -(z * z) / 2 / (k * (2 * k - 1 - 2 * order));
    }
    jSum += jTerm;
    ySum += yTerm;
  }
  return {
    j: (z ** order / doubleFactorialAbove) * jSum,
    y: (-doubleFactorialBelow / z ** (order + 1)) * ySum,
  };
}

/** S₁ and S₂ from aₙ, bₙ built from the series Bessel functions, for a real index m. */
function amplitudesBySeries(x: number, m: number, mu: Float64Array): Pick<MieResult, "s1" | "s2"> {
  const psi = (order: number, z: number): number => z * sphericalBesselBySeries(order, z).j;
  const psiPrime = (order: number, z: number): number =>
    z * sphericalBesselBySeries(order - 1, z).j - order * sphericalBesselBySeries(order, z).j;
  const chi = (order: number, z: number): number => -z * sphericalBesselBySeries(order, z).y;
  const chiPrime = (order: number, z: number): number =>
    -(z * sphericalBesselBySeries(order - 1, z).y - order * sphericalBesselBySeries(order, z).y);
  const s1 = new Float64Array(2 * mu.length);
  const s2 = new Float64Array(2 * mu.length);
  mu.forEach((cosine, j) => {
    let piPrevious = 0;
    let piCurrent = 1;
    for (let order = 1; order <= mieTermCount(x); order += 1) {
      // aₙ = A ÷ (A + iB), bₙ = C ÷ (C + iD) for real m, with ζ = ψ + iχ (Wiscombe's (16)).
      const mx = m * x;
      const a = m * psi(order, mx) * psiPrime(order, x) - psi(order, x) * psiPrime(order, mx);
      const b = m * psi(order, mx) * chiPrime(order, x) - chi(order, x) * psiPrime(order, mx);
      const c = psi(order, mx) * psiPrime(order, x) - m * psi(order, x) * psiPrime(order, mx);
      const d = psi(order, mx) * chiPrime(order, x) - m * chi(order, x) * psiPrime(order, mx);
      const aRe = (a * a) / (a * a + b * b);
      const aIm = (-a * b) / (a * a + b * b);
      const bRe = (c * c) / (c * c + d * d);
      const bIm = (-c * d) / (c * c + d * d);
      const t = cosine * piCurrent - piPrevious;
      const tau = order * t - piPrevious;
      const weight = (2 * order + 1) / (order * (order + 1));
      s1[2 * j] = (s1[2 * j] ?? 0) + weight * (aRe * piCurrent + bRe * tau);
      s1[2 * j + 1] = (s1[2 * j + 1] ?? 0) + weight * (aIm * piCurrent + bIm * tau);
      s2[2 * j] = (s2[2 * j] ?? 0) + weight * (aRe * tau + bRe * piCurrent);
      s2[2 * j + 1] = (s2[2 * j + 1] ?? 0) + weight * (aIm * tau + bIm * piCurrent);
      const next = cosine * piCurrent + (1 + 1 / order) * t;
      piPrevious = piCurrent;
      piCurrent = next;
    }
  });
  return { s1, s2 };
}

describe("mieSphere for small spheres against the power series of jₙ and yₙ", () => {
  it.each([
    { x: 0.099, m: 0.75 },
    { x: 0.101, m: 0.75 },
    { x: 0.3, m: 1.33 },
    { x: 0.5, m: 1.55 },
  ])("agrees at x = $x, m = $m to 10⁻⁹ of each amplitude", ({ x, m }) => {
    // Wiscombe's cases 5 and 6, and two droplets: an evaluation of the same series that shares
    // none of the recurrences, so it checks aₙ and bₙ where Wiscombe's own answers fall short.
    const computed = mieSphere(x, { n: m, k: 0 }, WISCOMBE_MU);
    const reference = amplitudesBySeries(x, m, WISCOMBE_MU);
    WISCOMBE_ANGLES_DEG.forEach((deg, j) => {
      expect(
        relativeError(amplitude(computed.s1, j), amplitude(reference.s1, j)),
        `S1 at ${deg}°`,
      ).toBeLessThan(1e-9);
      expect(
        relativeError(amplitude(computed.s2, j), amplitude(reference.s2, j)),
        `S2 at ${deg}°`,
      ).toBeLessThan(1e-9);
    });
  });
});

/** cot z = (sin 2a − i sinh 2b) ÷ (cosh 2b − cos 2a) for z = a + ib. */
function cot(a: number, b: number): Pair {
  const denominator = Math.cosh(2 * b) - Math.cos(2 * a);
  return [Math.sin(2 * a) / denominator, -Math.sinh(2 * b) / denominator];
}

describe("logarithmicDerivatives", () => {
  it("recurs from Lentz's start to D₀ = cot z at z = 1,500, to 10⁻¹¹", () => {
    const { re, im } = logarithmicDerivatives(1_500, 0, mieTermCount(1_000));
    expect(relativeError([re[0] ?? Number.NaN, im[0] ?? Number.NaN], cot(1_500, 0))).toBeLessThan(
      1e-11,
    );
  });

  it.each([
    { a: 13.3, b: -0.5 },
    { a: 200, b: -20 },
    { a: 1_330, b: -0.0133 },
  ])("gives D₀ = cot z at z = $a + $b i, to 10⁻¹¹", ({ a, b }) => {
    const { re, im } = logarithmicDerivatives(a, b, mieTermCount(a / 1.33));
    expect(relativeError([re[0] ?? Number.NaN, im[0] ?? Number.NaN], cot(a, b))).toBeLessThan(
      1e-11,
    );
  });

  it("refuses a zero or non-finite argument", () => {
    expect(() => logarithmicDerivatives(0, 0, 3)).toThrow(RangeError);
    expect(() => logarithmicDerivatives(Number.NaN, 0, 3)).toThrow(RangeError);
  });

  it("refuses a highest index that is not an integer ≥ 1", () => {
    expect(() => logarithmicDerivatives(1.5, 0, 2.5)).toThrow(RangeError);
    expect(() => logarithmicDerivatives(1.5, 0, 0)).toThrow(RangeError);
  });
});

describe("Bohren and Huffman's BHMIE start", () => {
  it("is tens of percent off in D₀ at z = 1,500, as logarithmicDerivatives' remarks say", () => {
    // At real z = 1,500 (x = 1,000, m = 1.5), BHMIE starts D = 0 at
    // n = max(x + 4 x^(1/3) + 2, |z|) + 15 = 1,515 (Bohren and Huffman 1983, p. 478) and recurs
    // down; measured, D₀ comes out 27% off.
    const z = 1_500;
    let bhmie = 0;
    for (let n = 1_515; n >= 1; n -= 1) {
      bhmie = n / z - 1 / (bhmie + n / z);
    }
    const [expected] = cot(z, 0);
    expect(Math.abs(bhmie - expected) / Math.abs(expected)).toBeGreaterThan(0.1);
  });
});

/** a × b for complex a and b. */
function times(a: Pair, b: Pair): Pair {
  return [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]];
}

/** a ÷ b for complex a and b. */
function over(a: Pair, b: Pair): Pair {
  const norm = b[0] * b[0] + b[1] * b[1];
  return [(a[0] * b[0] + a[1] * b[1]) / norm, (a[1] * b[0] - a[0] * b[1]) / norm];
}

/**
 * S₁(180°) = ½ Σ (−1)ⁿ⁺¹ (2n + 1) (aₙ − bₙ), MIEV0's SBACK, with ψₙ(x) taken from
 * ψₙ = ψₙ₋₁ ÷ (Dₙ(x) + n ÷ x), Dₙ of the real x by downward recurrence, in place of ψₙ's upward
 * recurrence, whose error grows past n = x (Wiscombe's Table 1).
 */
function backwardAmplitudeWithPsiFromRatio(x: number, index: ComplexIndex): Pair {
  const terms = mieTermCount(x);
  const m: Pair = [index.n, -index.k];
  const ofMx = logarithmicDerivatives(index.n * x, -index.k * x, terms);
  const ofX = logarithmicDerivatives(x, 0, terms + 1);
  let psiPrevious = Math.sin(x);
  let chiPrevious = Math.cos(x);
  let psi = psiPrevious / ((ofX.re[1] ?? Number.NaN) + 1 / x);
  let chi = chiPrevious / x + psiPrevious;
  let back: Pair = [0, 0];
  for (let n = 1; n <= terms; n += 1) {
    const d: Pair = [ofMx.re[n] ?? Number.NaN, ofMx.im[n] ?? Number.NaN];
    const coefficient = (t: Pair): Pair =>
      over(
        [t[0] * psi - psiPrevious, t[1] * psi],
        [t[0] * psi - t[1] * chi - psiPrevious, t[0] * chi + t[1] * psi - chiPrevious],
      );
    const tA = over(d, m);
    const tB = times(m, d);
    const a = coefficient([tA[0] + n / x, tA[1]]);
    const b = coefficient([tB[0] + n / x, tB[1]]);
    const weight = ((n % 2 === 1 ? 1 : -1) * (2 * n + 1)) / 2;
    back = [back[0] + weight * (a[0] - b[0]), back[1] + weight * (a[1] - b[1])];
    const chiNext = ((2 * n + 1) / x) * chi - chiPrevious;
    const psiNext = psi / ((ofX.re[n + 1] ?? Number.NaN) + (n + 1) / x);
    chiPrevious = chi;
    chi = chiNext;
    psiPrevious = psi;
    psi = psiNext;
  }
  return back;
}

describe("mieSphere at the back of a large sphere", () => {
  it("gives case 11's S₁(180°) to 10⁻⁹ of a second evaluation with ψₙ from Dₙ(x)", () => {
    // The backward sum is the alternating one Wiscombe names the most sensitive to precision. The
    // upward recurrence's error in ψₙ, about 10⁻⁵ by n = N_stop at x = 10⁴, does not reach it; both
    // sum the same N_stop terms, so this checks the arithmetic, not the cut.
    const index: ComplexIndex = { n: 1.33, k: 1e-5 };
    const result = mieSphere(10_000, index, new Float64Array([-1]));
    expect(
      relativeError(amplitude(result.s1, 0), backwardAmplitudeWithPsiFromRatio(10_000, index)),
    ).toBeLessThan(1e-9);
  });
});

describe("mieTermCount", () => {
  it("is ⌊x + 4.05 x^(1/3) + 2⌋ at x = 10, 100 and 1,000", () => {
    expect(mieTermCount(10)).toBe(20);
    expect(mieTermCount(100)).toBe(120);
    expect(mieTermCount(1_000)).toBe(1_042);
  });

  it("is never below Wiscombe's three-branch criterion", () => {
    for (const x of [0.02, 0.1, 1, 8, 9, 4_199, 4_200, 20_000]) {
      const branch =
        x <= 8
          ? x + 4 * Math.cbrt(x) + 1
          : x < 4_200
            ? x + 4.05 * Math.cbrt(x) + 2
            : x + 4 * Math.cbrt(x) + 2;
      expect(mieTermCount(x), `x = ${x}`).toBeGreaterThanOrEqual(Math.floor(branch));
    }
  });
});

describe("mieSphere's inputs", () => {
  const index: ComplexIndex = { n: 1.33, k: 0 };
  const none = new Float64Array(0);

  it("refuses a size parameter outside [10⁻⁶, 20,000]", () => {
    expect(() => mieSphere(0, index, none)).toThrow(RangeError);
    expect(() => mieSphere(Number.NaN, index, none)).toThrow(RangeError);
    expect(() => mieSphere(MIE_MIN_SIZE_PARAMETER * 0.99, index, none)).toThrow(RangeError);
    expect(() => mieSphere(MIE_MAX_SIZE_PARAMETER * 1.01, index, none)).toThrow(RangeError);
  });

  it("accepts the smallest and largest size parameters, 10⁻⁶ and 20,000", () => {
    expect(() => mieSphere(MIE_MIN_SIZE_PARAMETER, index, none)).not.toThrow();
    expect(() => mieSphere(MIE_MAX_SIZE_PARAMETER, index, none)).not.toThrow();
  });

  it("refuses an index with n ≤ 0 or k < 0", () => {
    expect(() => mieSphere(1, { n: 0, k: 0 }, none)).toThrow(RangeError);
    expect(() => mieSphere(1, { n: 1.33, k: -1e-3 }, none)).toThrow(RangeError);
  });

  it("refuses an index that is not finite", () => {
    expect(() => mieSphere(1, { n: Number.POSITIVE_INFINITY, k: 0 }, none)).toThrow(RangeError);
    expect(() => mieSphere(1, { n: 1.33, k: Number.NaN }, none)).toThrow(RangeError);
  });

  it("refuses a cosine outside [−1, 1]", () => {
    expect(() => mieSphere(1, index, new Float64Array([1.0000001]))).toThrow(RangeError);
    expect(() => mieSphere(1, index, new Float64Array([Number.NaN]))).toThrow(RangeError);
  });

  it("returns empty amplitudes for no angles", () => {
    const result = mieSphere(1, index, none);
    expect(result.s1).toHaveLength(0);
    expect(result.s2).toHaveLength(0);
  });
});
