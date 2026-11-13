import { describe, expect, it } from "vitest";

import type { Rgb } from "../photometry/toneCurve";
import {
  aggregatePasses,
  aggregateRefusal,
  beamDepthAgrees,
  beamPasses,
  beamRadiance,
  coneAverage,
  deltaEab,
  gate,
  imageDiagnostics,
  imageMaxima,
  METRIC_FLOOR,
  metricTolerance,
  type ReferenceReading,
  refusals,
  sanityRange,
  sigmaScales,
  uvPrimeDifference,
} from "./gate";

const grey = (v: number): Rgb => [v, v, v];

/** A reading at `radiance` in `image` whose σ is `sigma` and falls as N^(−1/2) from its quarter. */
function reading(name: string, image: string, radiance: number, sigma = 0): ReferenceReading {
  return {
    name,
    image,
    radiance: grey(radiance),
    standardError: grey(sigma),
    samples: 400_000,
    quarterSamples: 100_000,
    quarterStandardError: grey(2 * sigma),
  };
}

/** The client's radiances: each reading's radiance plus its offset in `offsets`, 0 where none. */
function clientOf(
  readings: ReadonlyArray<ReferenceReading>,
  offsets: Readonly<Record<string, number>> = {},
): ReadonlyMap<string, Rgb> {
  return new Map(
    readings.map((r) => [r.name, grey(r.radiance[0] + (offsets[r.name] ?? 0))] as const),
  );
}

describe("the metric's images and floors", () => {
  // The twilight sky's dim point is 1% of its own brightest: its floor is F × 1, its T 3 × 10⁻⁴.
  // Off by 5 × 10⁻⁴, it fails there; under the noon image's brightest it would pass.
  const twilight = [
    reading("twilight-bright", "twilight", 1),
    reading("twilight-dim", "twilight", 0.01),
  ];
  const noon = [reading("noon-bright", "noon", 1000), reading("noon-dim", "noon", 100)];
  const dimOffset = { "twilight-dim": 5e-4 };

  it("holds a twilight image's dim sky to its own brightest, not the noon image's", () => {
    const report = gate([...twilight, ...noon], clientOf([...twilight, ...noon], dimOffset));
    expect(report.failures.map((f) => f.name)).toEqual([
      "twilight-dim",
      "twilight-dim",
      "twilight-dim",
    ]);
  });

  it("passes the same dim point where it shares the noon image's brightest", () => {
    const mixed = [reading("noon-bright", "noon", 1000), reading("twilight-dim", "noon", 0.01)];
    expect(gate(mixed, clientOf(mixed, dimOffset)).failures).toEqual([]);
  });

  it("takes each image's brightest radiance as its own L_max", () => {
    expect(imageMaxima([...twilight, ...noon]).get("twilight")).toEqual(grey(1));
  });

  it("takes the floor below 20F of the image's brightest, and 5% above it", () => {
    expect(metricTolerance(1, 1)).toBe(0.05);
    expect(metricTolerance(20 * METRIC_FLOOR, 1)).toBeCloseTo(METRIC_FLOOR, 15);
    expect(metricTolerance(1e-3, 1)).toBe(METRIC_FLOOR);
    expect(metricTolerance(0.5, 1)).toBe(0.025);
  });

  it("allows four of the reference's standard errors beside T", () => {
    // T = 0.05 at radiance 1; σ 0.005 is T ÷ 10, under the cap.
    const at = (offset: number): boolean =>
      gate([reading("g", "i", 1, 0.005)], new Map([["g", grey(1 + offset)]])).failures.length === 0;
    expect(at(0.05 + 3.99 * 0.005)).toBe(true);
    expect(at(-(0.05 + 3.99 * 0.005))).toBe(true);
    expect(at(0.05 + 4.01 * 0.005)).toBe(false);
  });

  it("refuses a reference whose σ passes T ÷ 8", () => {
    // T = 0.05: σ 0.007 passes T ÷ 8 = 0.00625; σ 0.006 does not.
    expect(refusals([reading("noisy", "i", 1, 0.007)]).map((r) => r.reason)).toEqual([
      "noisy",
      "noisy",
      "noisy",
    ]);
    expect(refusals([reading("quiet", "i", 1, 0.006)])).toEqual([]);
  });

  it("gives a refusal the count at which its σ would meet T ÷ 8", () => {
    const [first] = refusals([reading("noisy", "i", 1, 0.007)]);
    expect(first?.neededSamples).toBe(Math.ceil(400_000 * (0.007 / 0.00625) ** 2));
  });

  it("reports a refused comparison rather than failing it", () => {
    const report = gate([reading("noisy", "i", 1, 0.007)], new Map([["noisy", grey(1.2)]]));
    expect([report.failures, report.comparisons.map((c) => [c.refused, c.passes])]).toEqual([
      [],
      [
        [true, false],
        [true, false],
        [true, false],
      ],
    ]);
  });

  it("refuses a reference whose σ does not fall as N^(−1/2) from its first quarter's", () => {
    // N 400,000 and N_q 100,000: σ is σ_q ÷ 2 when it falls as N^(−1/2).
    const scaled = { ...reading("g", "i", 1, 0.004), quarterStandardError: grey(0.008) };
    const slow = { ...scaled, quarterStandardError: grey(0.006) };
    expect([refusals([scaled]), refusals([slow]).map((r) => r.reason)]).toEqual([
      [],
      ["unscaled", "unscaled", "unscaled"],
    ]);
  });

  it.each([
    [0.0049, true],
    [0.0051, false],
    [0.0031, true],
    [0.0029, false],
  ])("holds a σ of %d against a quarter's 0.008 at N ÷ 4 to 25%: %s", (sigma, scales) => {
    expect(sigmaScales(sigma, 0.008, 400_000, 100_000)).toBe(scales);
  });

  it("takes a value exact at both counts as scaling", () => {
    expect(sigmaScales(0, 0, 400_000, 100_000)).toBe(true);
  });
});

describe("the aggregates and the sanity ranges", () => {
  const flux = {
    name: "noon",
    flux: grey(100),
    standardError: grey(0.2),
    samples: 400_000,
    quarterSamples: 100_000,
    quarterStandardError: grey(0.4),
  };

  it("hold an aggregate to 2% and 4σ_F", () => {
    // 2 + 4 × 0.2 = 2.8 either side of 100.
    expect([
      aggregatePasses(102.79, flux, 0),
      aggregatePasses(102.81, flux, 0),
      aggregatePasses(97.21, flux, 1),
    ]).toEqual([true, false, true]);
  });

  it("refuse an aggregate whose σ_F passes 0.25% of the flux, or does not scale", () => {
    expect([
      aggregateRefusal(flux, 2),
      aggregateRefusal({ ...flux, standardError: grey(0.26) }, 0),
      aggregateRefusal({ ...flux, quarterStandardError: grey(0.6) }, 0),
    ]).toEqual([undefined, "noisy", "unscaled"]);
  });

  // The Venus-class 2–4% (R08.T12.b.2's, as numbers here): σ at most 0.125%.
  it("hold a sanity range [a, b] as a − 4σ ≤ F ≤ b + 4σ", () => {
    expect([
      sanityRange(0.0195, 0.001, 0.02, 0.04),
      sanityRange(0.0155, 0.001, 0.02, 0.04),
      sanityRange(0.0445, 0.001, 0.02, 0.04),
    ]).toEqual([
      { refused: false, within: true },
      { refused: false, within: false },
      { refused: false, within: false },
    ]);
  });

  it("refuse a sanity range's reference whose σ passes (b − a) ÷ 16", () => {
    expect(sanityRange(0.03, 0.0013, 0.02, 0.04).refused).toBe(true);
  });
});

describe("the cone average", () => {
  const axis = { x: 0.2, y: -0.4, z: Math.sqrt(1 - 0.2) };
  const alpha = (1 * Math.PI) / 180;

  it("is a uniform radiance itself", () => {
    expect(coneAverage(() => [1, 2, 3], axis, alpha)).toEqual([
      expect.closeTo(1, 14),
      expect.closeTo(2, 14),
      expect.closeTo(3, 14),
    ]);
  });

  it("is the axis's radiance for a cone of no width", () => {
    const radiance = (d: { readonly x: number }): Rgb => grey(d.x);
    expect(coneAverage(radiance, axis, 0)).toEqual(grey(axis.x));
  });

  it("averages uniformly in solid angle, as the tracer draws its cone", () => {
    // ⟨μᵏ⟩ over the cone, μ the cosine with the axis: (1 − cᵏ⁺¹) ÷ ((k + 1)(1 − c)), c = cos α.
    const c = Math.cos(alpha);
    const moments = [1, 2, 5].map((k) => {
      const radiance = (d: { readonly x: number; readonly y: number; readonly z: number }): Rgb =>
        grey((d.x * axis.x + d.y * axis.y + d.z * axis.z) ** k);
      return coneAverage(radiance, axis, alpha)[0] / ((1 - c ** (k + 1)) / ((k + 1) * (1 - c)));
    });
    expect(moments).toEqual([expect.closeTo(1, 12), expect.closeTo(1, 12), expect.closeTo(1, 12)]);
  });

  it("averages a radiance linear across the cone to its value at the mean direction", () => {
    // The mean direction over the cone is ⟨μ⟩k, ⟨μ⟩ = (1 + c) ÷ 2, k the axis: a linear radiance
    // averages to its value there, not on the axis itself.
    const c = Math.cos(alpha);
    const tilted = (d: { readonly x: number; readonly y: number; readonly z: number }): Rgb =>
      grey(1 + 30 * d.y);
    const coneMean = 1 + 30 * axis.y * ((1 + c) / 2);
    expect(coneAverage(tilted, axis, alpha)[0]).toBeCloseTo(coneMean, 12);
  });
});

describe("the direct beam's checks", () => {
  it("gives the beam as the radiance of a white Lambertian surface facing the sun", () => {
    expect(beamRadiance(Math.PI, 0)).toBeCloseTo(1, 15);
    expect(beamRadiance(2, 1)).toBeCloseTo((2 * Math.exp(-1)) / Math.PI, 15);
  });

  it("holds the client's beam under T with no σ term, a beam under the floor passing", () => {
    // A low sun's beam, 10⁻⁶ of the image's brightest diffuse radiance: under the floor, off by
    // half itself, it passes; a bright beam off by 6% fails.
    const reference = beamRadiance(1, 2);
    expect(beamPasses(1.06 * reference, reference, 0.1)).toBe(false);
    expect(beamPasses(1.04 * reference, reference, 0.1)).toBe(true);
    const dim = 1e-6;
    expect(beamPasses(1.5 * dim, dim, 1)).toBe(true);
    expect(beamPasses(dim + 1.01 * METRIC_FLOOR, dim, 1)).toBe(false);
  });

  it("holds the reference's sunOpticalDepth to the client's quadrature to 10⁻⁶ max(1, τ)", () => {
    expect(beamDepthAgrees(0.5, 0.5 + 0.9e-6)).toBe(true);
    expect(beamDepthAgrees(0.5, 0.5 + 1.1e-6)).toBe(false);
    expect(beamDepthAgrees(20, 20 - 1.9e-5)).toBe(true);
    expect(beamDepthAgrees(20, 20 - 2.1e-5)).toBe(false);
  });
});

describe("the recorded diagnostics", () => {
  const image = [reading("a", "i", 10), reading("b", "i", 2), reading("c", "i", 0.5)];

  it("give no ΔE*ab or RMS difference for a client that equals the reference", () => {
    const diagnostics = imageDiagnostics(image, clientOf(image)).get("i");
    expect([diagnostics?.largestDeltaE, diagnostics?.relativeRms]).toEqual([0, grey(0)]);
  });

  it("give each image's brightest radiance over its mean", () => {
    const diagnostics = imageDiagnostics(image, clientOf(image)).get("i");
    expect(diagnostics?.maxOverMean[0]).toBeCloseTo(10 / (12.5 / 3), 14);
  });

  it("give no Δu′v′ between two colours of one chromaticity", () => {
    expect(uvPrimeDifference([1, 0.8, 0.6], [2, 1.6, 1.2])).toBeCloseTo(0, 14);
  });

  it("give some Δu′v′ between two colours of two chromaticities", () => {
    expect(uvPrimeDifference([1, 0.8, 0.6], [1, 0.8, 0.7])).toBeGreaterThan(0);
  });

  it("gives IPRT's relative RMS difference over the image", () => {
    const pair = [reading("a", "i", 3), reading("b", "i", 4)];
    const diagnostics = imageDiagnostics(pair, clientOf(pair, { a: 0.3, b: -0.4 })).get("i");
    // (0.3² + 0.4²)^½ ÷ (3² + 4²)^½ = 0.5 ÷ 5.
    expect(diagnostics?.relativeRms[0]).toBeCloseTo(0.1, 14);
  });

  it("finds 5% at most one just-noticeable ΔL* anywhere on AgX's curve at the AVG exposure", () => {
    // Design note 10: at the AVG meter's exposure, the image's mean at 0.104, 5% is at most
    // 0.96 ΔL* anywhere on the curve, for a grey from 10⁻⁴ to 10³ times the mean.
    let largest = 0;
    for (let k = -40; k <= 30; k += 1) {
      const radiance = 10 ** (k / 10);
      largest = Math.max(largest, deltaEab(grey(1.05 * radiance), grey(radiance), 1));
    }
    expect(largest).toBeGreaterThan(0.9);
    expect(largest).toBeLessThanOrEqual(0.96);
  });
});
