/**
 * The metric's shared values (plan R08, R08.T13.a; `decision-r08-t13-scope.md`, ruling 6): `gate.ts`'s
 * named constants and its verdicts on synthetic readings, written through `bless.ts` to
 * `crates/hyperion-fit/src/atmosphere/metric_verdicts.json`, which the tracer's `metric` module
 * reads with `include_str!` and holds its mirror of the metric to, verdict for verdict.
 *
 * @remarks
 * The readings cover the floor (a dim geometry in a bright image, its T F L_max), the 4σ allowance
 * a hair inside and a hair outside on both sides, both refusals (`noisy` just past T ÷ 8 and just
 * inside it; `unscaled` past 25% of σ_q √(N_q ÷ N) above and below, and a σ exact at both counts),
 * and images of several geometries whose L_max another geometry sets. The sweep is scalar, so each
 * reading is one radiance, given to `gate.ts` in all three channels and read back from the first.
 *
 * Rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/metricVerdicts.test.ts`.
 */

import { describe, expect, it } from "vitest";

import committed from "../../../../../../../../crates/hyperion-fit/src/atmosphere/metric_verdicts.json" with { type: "json" };
import committedText from "../../../../../../../../crates/hyperion-fit/src/atmosphere/metric_verdicts.json?raw";
import { ADDED_FILE_LIMIT_BYTES } from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import type { Rgb } from "../../photometry/toneCurve";
import {
  AGGREGATE_SIGMA_CAP,
  gate,
  METRIC_FLOOR,
  METRIC_RELATIVE_TOLERANCE,
  METRIC_SIGMA_ALLOWANCE,
  METRIC_SIGMA_DIVISOR,
  metricTolerance,
  type ReferenceReading,
  type RefusalReason,
  SIGMA_SCALING_TOLERANCE,
} from "../gate";

/** The file, from this test. */
const FILE = "../../../../../../../../crates/hyperion-fit/src/atmosphere/metric_verdicts.json";

/** What the file says of itself. */
const DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/view/atmosphere/reference/metricVerdicts.test.ts (plan R08, R08.T13.a) from gate.ts's metric; read by crates/hyperion-fit/src/atmosphere/metric.rs. Do not edit by hand.";

/** One synthetic geometry: a scalar reading and the twin's radiance there. */
interface Synthetic {
  readonly name: string;
  readonly image: string;
  readonly radiance: number;
  readonly standardError: number;
  readonly samples: number;
  readonly quarterSamples: number;
  readonly quarterStandardError: number;
  readonly twin: number;
}

/** A hair: the relative step by which a reading stands inside or outside an edge. */
const HAIR = 1e-9;

/** σ_q for a σ that falls exactly as N^(−1/2) from a first quarter of N ÷ 4: 2σ. */
function scaling(sigma: number): number {
  return 2 * sigma;
}

/**
 * σ_q whose prediction σ_q ÷ 2 is σ ÷ `ratio`, a hair above it (`side` 1) or below (`side` −1):
 * the N^(−1/2) check's edges at ratios 1.25 and 0.75.
 */
function quarterAt(sigma: number, ratio: number, side: 1 | -1): number {
  return ((2 * sigma) / ratio) * (1 + side * 1e-6);
}

/** A reading of radiance `l` and σ `sigma` at 40,000 samples whose σ falls as N^(−1/2). */
function reading(
  name: string,
  image: string,
  l: number,
  sigma: number,
  twin: number,
  quarterStandardError: number = scaling(sigma),
): Synthetic {
  return {
    name,
    image,
    radiance: l,
    standardError: sigma,
    samples: 40_000,
    quarterSamples: 10_000,
    quarterStandardError,
    twin,
  };
}

/** The twin's radiance at the 4σ edge of `l` in an image of L_max `lMax`, `sign` its side. */
function atEdge(l: number, sigma: number, lMax: number, sign: 1 | -1, inside: boolean): number {
  const bound = metricTolerance(l, lMax) + METRIC_SIGMA_ALLOWANCE * sigma;
  return l + sign * bound * (inside ? 1 - HAIR : 1 + HAIR);
}

/** The σ at which a reading of `l` in an image of L_max `m` is refused as noisy: T ÷ 8. */
function cap(l: number, m: number): number {
  return metricTolerance(l, m) / METRIC_SIGMA_DIVISOR;
}

/** The synthetic cases, each an image or a few (the module's remarks). */
function cases(): ReadonlyArray<{ readonly name: string; readonly geometries: Synthetic[] }> {
  const lMax = 2;
  return [
    {
      name: "the 4σ allowance's edge",
      geometries: [
        reading("bright", "a", lMax, 0.004, lMax * 1.01),
        reading("above-inside", "a", 1, 0.005, atEdge(1, 0.005, lMax, 1, true)),
        reading("above-outside", "a", 1.1, 0.005, atEdge(1.1, 0.005, lMax, 1, false)),
        reading("below-inside", "a", 0.9, 0.004, atEdge(0.9, 0.004, lMax, -1, true)),
        reading("below-outside", "a", 0.8, 0.004, atEdge(0.8, 0.004, lMax, -1, false)),
      ],
    },
    {
      name: "the floor",
      geometries: [
        reading("bright", "b", 10, 0.05, 10),
        // Under 20F of the brightest: T is F L_max, 3 × 10⁻³, well past 5% of 0.01.
        reading("dim-inside", "b", 0.01, 1e-5, atEdge(0.01, 1e-5, 10, 1, true)),
        reading("dim-outside", "b", 0.02, 1e-5, atEdge(0.02, 1e-5, 10, -1, false)),
        reading("dim-far", "b", 0.005, 1e-5, 0),
      ],
    },
    {
      name: "the refusals",
      geometries: [
        reading("bright", "c", 4, 0.01, 4),
        reading("noisy", "c", 1, cap(1, 4) * (1 + HAIR), 1),
        reading("not-noisy", "c", 1.5, cap(1.5, 4) * (1 - HAIR), 1.5),
        reading("noisy-and-failing", "c", 2, cap(2, 4) * 3, 0),
        // σ_q √(N_q ÷ N) = σ_q ÷ 2 against σ: held from σ ÷ 1.25 to σ ÷ 0.75.
        reading(
          "unscaled-high",
          "c",
          1,
          0.002,
          1,
          quarterAt(0.002, 1 + SIGMA_SCALING_TOLERANCE, -1),
        ),
        reading("scaled-high", "c", 1, 0.002, 1, quarterAt(0.002, 1 + SIGMA_SCALING_TOLERANCE, 1)),
        reading("unscaled-low", "c", 1, 0.002, 1, quarterAt(0.002, 1 - SIGMA_SCALING_TOLERANCE, 1)),
        reading("scaled-low", "c", 1, 0.002, 1, quarterAt(0.002, 1 - SIGMA_SCALING_TOLERANCE, -1)),
        reading("exact", "c", 1, 0, 1.01, 0),
        reading("exact-failing", "c", 1, 0, 1.2, 0),
      ],
    },
    {
      name: "the images apart",
      geometries: [
        reading("noon-bright", "noon", 5, 0.01, 5.1),
        reading("noon-dim", "noon", 0.002, 1e-6, 0.0035),
        reading("twilight-bright", "twilight", 0.01, 2e-5, 0.0104),
        reading("twilight-dim", "twilight", 0.002, 1e-6, 0.0023),
      ],
    },
  ];
}

/** A scalar as `gate.ts`'s three equal channels. */
function triple(x: number): Rgb {
  return [x, x, x];
}

/** A synthetic geometry with `gate.ts`'s verdict on it. */
interface Verdict extends Synthetic {
  readonly tolerance: number;
  readonly bound: number;
  readonly passes: boolean;
  readonly refusal: RefusalReason | null;
}

/** One case's verdicts by `gate.ts`, channel 0's. */
function verdicts(geometries: ReadonlyArray<Synthetic>): Verdict[] {
  const readings: ReferenceReading[] = geometries.map((g) => ({
    name: g.name,
    image: g.image,
    radiance: triple(g.radiance),
    standardError: triple(g.standardError),
    samples: g.samples,
    quarterSamples: g.quarterSamples,
    quarterStandardError: triple(g.quarterStandardError),
  }));
  const client = new Map(geometries.map((g) => [g.name, triple(g.twin)] as const));
  const report = gate(readings, client);
  const first = report.comparisons.filter((c) => c.channel === 0);
  const reasons = new Map(
    report.refusals.filter((r) => r.channel === 0).map((r) => [r.name, r.reason]),
  );
  return geometries.map((g) => {
    const comparison = first.find((c) => c.name === g.name);
    if (comparison === undefined) {
      throw new Error(`gate compared no ${g.name}`);
    }
    return {
      ...g,
      tolerance: comparison.tolerance,
      bound: comparison.bound,
      passes: comparison.passes,
      refusal: reasons.get(g.name) ?? null,
    };
  });
}

/** The table the file holds. */
function table(): unknown {
  return {
    description: DESCRIPTION,
    constants: {
      relativeTolerance: METRIC_RELATIVE_TOLERANCE,
      floor: METRIC_FLOOR,
      sigmaAllowance: METRIC_SIGMA_ALLOWANCE,
      sigmaDivisor: METRIC_SIGMA_DIVISOR,
      sigmaScalingTolerance: SIGMA_SCALING_TOLERANCE,
      aggregateSigmaCap: AGGREGATE_SIGMA_CAP,
    },
    cases: cases().map((c) => ({ name: c.name, geometries: verdicts(c.geometries) })),
  };
}

describe("the metric's shared values", () => {
  it("are written to metric_verdicts.json, which the tracer holds its mirror to", async () => {
    await expect(expectCommittedTable(table(), committed, FILE)).resolves.toBeUndefined();
  });

  it("keep the file under the added-file hook", () => {
    expect(new TextEncoder().encode(committedText).length).toBeLessThan(ADDED_FILE_LIMIT_BYTES);
  });

  it("cover a pass and a failure, both refusals and a held exact reading", () => {
    const all = cases().flatMap((c) => verdicts(c.geometries));
    expect(new Set(all.map((v) => `${v.passes}/${v.refusal}`))).toEqual(
      new Set(["true/null", "false/null", "true/noisy", "false/noisy", "true/unscaled"]),
    );
    expect(
      all
        .filter((v) => v.refusal === "unscaled")
        .map((v) => v.name)
        .toSorted(),
    ).toEqual(["unscaled-high", "unscaled-low"]);
  });
});
