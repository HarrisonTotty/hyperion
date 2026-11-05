/**
 * The checks on the committed references (plan R08, R08.T12.b.1 and b.2): each is its case's,
 * traced with `--stokes` at the case's seed and counts; its `sunOpticalDepth` is the client's own
 * `f64` quadrature of its case along the same chord to 10⁻⁶ max(1, τ) (Design note 10's beam
 * check); and its σ meets the metric's caps wherever `gate.ts` does not refuse it, in the vector and
 * the scalar radiance, every refusal pinned in `refusals.json` as R08's record lists it
 * (`decision-r08-t12b-scope.md`, rulings 6, 8 and 9). The Venus-class and Titan-class cases'
 * sanity ranges, pinned in `sanityRanges.json`. And the readers and the beam quadrature
 * themselves, on small inputs. R08.T16.a.2's `jupiter-envelope` and `saturn-envelope`
 * (`test/jupiterEnvelopeCase.ts`, `test/saturnEnvelopeCase.ts`) are among them, their disc and limb
 * alone.
 *
 * @remarks
 * `refusals.json` and `sanityRanges.json` are rewritten with the references, by
 * `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/references.test.ts`, and the record's list follows
 * them. The thick cases' Mie runs for about a minute, once.
 */

import { describe, expect, it } from "vitest";

import type { Rgb } from "../../photometry/toneCurve";
import {
  caseJson,
  REFERENCE_DEFAULT_SAMPLES,
  type SampleCounts,
} from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import { JUPITER_ENVELOPE_CASE, jupiterEnvelopeCase } from "../../../test/jupiterEnvelopeCase";
import { SATURN_ENVELOPE_CASE, saturnEnvelopeCase } from "../../../test/saturnEnvelopeCase";
import {
  aggregateRefusal,
  beamDepthAgrees,
  type Channel,
  luminanceOf,
  refusals,
  sanityRange,
} from "../gate";
import { columnLengthM, extinction } from "../medium";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthHighLimbReference from "./earth-high-limb.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import earthOzoneSlabHighLimbReference from "./earth-ozone-slab-high-limb.reference.json" with { type: "json" };
import giantDeckReference from "./giant-deck.reference.json" with { type: "json" };
import jupiterEnvelopeReference from "./jupiter-envelope.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import saturnEnvelopeReference from "./saturn-envelope.reference.json" with { type: "json" };
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import titanClassReference from "./titan-class.reference.json" with { type: "json" };
import venus58barReference from "./venus-58bar.reference.json" with { type: "json" };
import venus92barReference from "./venus-92bar.reference.json" with { type: "json" };
import committedRefusals from "./refusals.json" with { type: "json" };
import sampleCounts from "./sampleCounts.json" with { type: "json" };
import committedSanity from "./sanityRanges.json" with { type: "json" };
import { HIGH_LIMB_CASES, MARS_CASE, REFERENCE_CASES, type ReferenceCase } from "./cases";
import { THICK_CASE_NAMES, thickReferenceCases } from "./thickCases";
import type { ReferenceGeometry } from "./geometries";
import {
  clientBeam,
  fluxReadingsOf,
  type RadianceKind,
  readingsOf,
  type ReferenceAggregateFile,
  type ReferenceFile,
  type ReferenceGeometryFile,
} from "./references";

/** The committed references, by case. */
const REFERENCES: Readonly<Record<string, ReferenceFile>> = {
  earth: earthReference,
  "earth-ozone-slab": earthOzoneSlabReference,
  mars: marsReference,
  "saturn-oblate": saturnOblateReference,
  "earth-high-limb": earthHighLimbReference,
  "earth-ozone-slab-high-limb": earthOzoneSlabHighLimbReference,
  "venus-92bar": venus92barReference,
  "venus-58bar": venus58barReference,
  "titan-class": titanClassReference,
  "giant-deck": giantDeckReference,
  [JUPITER_ENVELOPE_CASE]: jupiterEnvelopeReference,
  [SATURN_ENVELOPE_CASE]: saturnEnvelopeReference,
};

/** Each case's own counts. */
const COUNTS: Readonly<Record<string, SampleCounts>> = sampleCounts;

function referenceOf(name: string): ReferenceFile {
  const reference = REFERENCES[name];
  if (reference === undefined) {
    throw new Error(`no reference is committed for ${name}`);
  }
  return reference;
}

/** The channels' letters: red, green and blue. */
const CHANNEL_LETTERS = ["r", "g", "b"] as const;

/**
 * Every refusal of a reference, in the vector and the scalar radiance and their fluxes, as
 * `<kind>/<name>/<reason>/<channels>`: kind `vector` or `scalar`, the reason `gate.ts`'s, and the
 * channels refused for it, of `r`, `g` and `b`. A reference whose σ misses its cap there is refused,
 * not held, and nothing waits for a ruling (ruling 8).
 */
function refusedIn(referenceCase: ReferenceCase, reference: ReferenceFile): string[] {
  const refused = new Map<string, string>();
  const add = (key: string, channel: Channel): void => {
    refused.set(key, `${refused.get(key) ?? ""}${CHANNEL_LETTERS[channel]}`);
  };
  for (const kind of ["vector", "scalar"] as const satisfies readonly RadianceKind[]) {
    for (const r of refusals(readingsOf(referenceCase, reference, kind))) {
      add(`${kind}/${r.name}/${r.reason}`, r.channel);
    }
    for (const flux of fluxReadingsOf(reference, kind)) {
      for (const c of [0, 1, 2] as const satisfies readonly Channel[]) {
        const reason = aggregateRefusal(flux, c);
        if (reason !== undefined) {
          add(`${kind}/${flux.name}/${reason}`, c);
        }
      }
    }
  }
  return [...refused].map(([key, channels]) => `${key}/${channels}`);
}

/** The cloud-free cases and the higher limb, built cheaply, by name. */
const CHEAP: ReadonlyMap<string, ReferenceCase> = new Map(
  [...REFERENCE_CASES, ...HIGH_LIMB_CASES].map((c) => [c.name, c] as const),
);

/** Every case with a committed reference, the thick ones by name until first asked. */
const NAMES: ReadonlyArray<string> = [
  ...CHEAP.keys(),
  ...THICK_CASE_NAMES,
  JUPITER_ENVELOPE_CASE,
  SATURN_ENVELOPE_CASE,
];

/** A case by name: a thick one's Mie runs once, when a test first asks. */
function caseNamed(name: string): ReferenceCase {
  const cheap = CHEAP.get(name);
  if (cheap !== undefined) {
    return cheap;
  }
  if (name === JUPITER_ENVELOPE_CASE) {
    return jupiterEnvelopeCase().referenceCase;
  }
  if (name === SATURN_ENVELOPE_CASE) {
    return saturnEnvelopeCase().referenceCase;
  }
  const thick = thickReferenceCases();
  const found = THICK_CASE_NAMES.find((n) => n === name);
  if (found === undefined) {
    throw new Error(`no case ${name}`);
  }
  return thick[found];
}

/** Each case by name, for the tests over every committed reference. */
const CASES = NAMES.map((name) => [name] as const);

/** A thick case's Mie and a beam's quadrature, ms. */
const SLOW_MS = 600_000;

describe("the committed references", () => {
  it.each(CASES)(
    "%s is its case's, traced with --stokes at its seed and the default count",
    { timeout: SLOW_MS },
    (name) => {
      const referenceCase = caseNamed(name);
      const reference = referenceOf(name);
      expect([
        reference.format,
        reference.case,
        reference.polarisation,
        reference.seed,
        reference.samples,
      ]).toEqual([1, name, "stokes", referenceCase.seed, REFERENCE_DEFAULT_SAMPLES]);
    },
  );

  it.each(CASES)(
    "%s traces its case's geometries at their counts and cones",
    { timeout: SLOW_MS },
    (name) => {
      const reference = referenceOf(name);
      const written = caseJson(caseNamed(name), COUNTS[name]);
      expect(reference.geometries.map((g) => [g.name, g.samples, g.detectorHalfAngleDeg])).toEqual(
        written.geometries.map((g) => [
          g.name,
          g.samples ?? reference.samples,
          g.detectorHalfAngleDeg ?? written.detectorHalfAngleDeg,
        ]),
      );
    },
  );

  it.each(CASES)(
    "%s traces its case's aggregates at their counts",
    { timeout: SLOW_MS },
    (name) => {
      const reference = referenceOf(name);
      const written = caseJson(caseNamed(name), COUNTS[name]);
      expect(reference.aggregates.map((a) => [a.name, a.samples])).toEqual(
        written.aggregates.map((a) => [a.name, a.samples ?? reference.samples]),
      );
    },
  );

  it.each(CASES)(
    "%s's sunOpticalDepth is the client's own quadrature to 10⁻⁶ max(1, τ)",
    { timeout: SLOW_MS },
    (name) => {
      const referenceCase = caseNamed(name);
      const traced = new Map(referenceOf(name).geometries.map((g) => [g.name, g] as const));
      const mismatches: string[] = [];
      for (const geometry of referenceCase.geometries) {
        const depths = traced.get(geometry.name)?.sunOpticalDepth[0];
        const beam = clientBeam(referenceCase, geometry);
        for (const c of [0, 1, 2] as const) {
          const depth = depths?.[c];
          const mine = beam?.opticalDepth[c];
          const agrees =
            depth === null || mine === undefined
              ? depth === null && mine === undefined
              : depth !== undefined && beamDepthAgrees(depth, mine);
          if (!agrees) {
            mismatches.push(`${geometry.name}/${c}: ${String(depth)} against ${String(mine)}`);
          }
        }
      }
      expect(mismatches).toEqual([]);
    },
  );

  it(
    "meet their σ caps wherever gate.ts does not refuse them, every refusal listed",
    { timeout: SLOW_MS },
    async () => {
      const table = {
        description:
          "The references' refusals by gate.ts, written by references.test.ts (plan R08, R08.T12.b.1 and b.2): <kind>/<name>/<reason>/<channels>. Do not edit by hand.",
        ...Object.fromEntries(
          NAMES.map((name) => [name, refusedIn(caseNamed(name), referenceOf(name))] as const),
        ),
      };
      await expect(
        expectCommittedTable(table, committedRefusals, "./refusals.json"),
      ).resolves.toBeUndefined();
    },
  );
});

/** Each channel's weight in a linear colour's luminance, Rec. 709's Y. */
const LUMINANCE_WEIGHTS: Rgb = [
  luminanceOf([1, 0, 0]),
  luminanceOf([0, 1, 0]),
  luminanceOf([0, 0, 1]),
];

/** An aggregate's downwelling flux over the top's per channel, and its σ. */
function downwellingFractions(
  reference: ReferenceFile,
  aggregate: string,
): { readonly fraction: Rgb; readonly sigma: Rgb } {
  const a = reference.aggregates.find((each) => each.name === aggregate);
  if (a === undefined) {
    throw new Error(`${reference.case} has no aggregate ${aggregate}`);
  }
  const of = (c: Channel): number =>
    ((a.directGround[c] ?? Number.NaN) + (a.diffuseGround[c] ?? Number.NaN)) /
    (a.incidentTop[c] ?? Number.NaN);
  const sigmaOf = (c: Channel): number =>
    (a.diffuseGroundStandardError[c] ?? Number.NaN) / (a.incidentTop[c] ?? Number.NaN);
  return { fraction: [of(0), of(1), of(2)], sigma: [sigmaOf(0), sigmaOf(1), sigmaOf(2)] };
}

/** One of the references' sanity ranges (Design note 10, _Statistics_; R08.T12.b). */
interface SanityCase {
  readonly name: string;
  readonly caseName: string;
  readonly aggregate: string;
  readonly low: number;
  readonly high: number;
}

/**
 * The sanity ranges, each on its aggregate's downwelling luminance over the top's
 * (R08.T12.b's text): the Venus-class case's 2–4% at the Pioneer Venus large probe's LSFR's sun
 * (Tomasko et al. 1980, 2.5%; a global-mean fraction absorbed at the ground, by the science check,
 * held here as the plan states it), and the Titan-class case's 5–15% under its sun 60° from the
 * zenith (Tomasko et al. 2008b, Planet. Space Sci. 56, 648: 11% at the surface, a diurnal mean at
 * 10°S, whose sun no source here gives); `venus-58bar`'s held to the same range, plan 14's Venus.
 */
const SANITY: ReadonlyArray<SanityCase> = [
  {
    name: "venus-92bar-surface",
    caseName: "venus-92bar",
    aggregate: "flux-lsfr",
    low: 0.02,
    high: 0.04,
  },
  {
    name: "venus-58bar-surface",
    caseName: "venus-58bar",
    aggregate: "flux-lsfr",
    low: 0.02,
    high: 0.04,
  },
  {
    name: "titan-class-surface",
    caseName: "titan-class",
    aggregate: "flux-s60",
    low: 0.05,
    high: 0.15,
  },
];

/** A sanity range's reading: the luminance fraction, its σ, and gate.ts's verdict. */
function sanityReading(range: SanityCase): {
  readonly fraction: number;
  readonly sigma: number;
  readonly refused: boolean;
  readonly within: boolean;
} {
  const { fraction, sigma } = downwellingFractions(referenceOf(range.caseName), range.aggregate);
  const value = luminanceOf(fraction);
  const error = Math.hypot(
    LUMINANCE_WEIGHTS[0] * sigma[0],
    LUMINANCE_WEIGHTS[1] * sigma[1],
    LUMINANCE_WEIGHTS[2] * sigma[2],
  );
  return { fraction: value, sigma: error, ...sanityRange(value, error, range.low, range.high) };
}

/**
 * The Venus-class surface's red shift: each channel's downwelling fraction over a bluer one's, in σ
 * of the two together (R08.T12.b's text: "with the surface sky red-shifted").
 */
function redShift(): {
  readonly redOverGreen: number;
  readonly greenOverBlue: number;
  readonly redOverBlue: number;
} {
  const { fraction, sigma } = downwellingFractions(referenceOf("venus-92bar"), "flux-lsfr");
  const step = (a: Channel, b: Channel): number =>
    (fraction[a] - fraction[b]) / Math.hypot(sigma[a], sigma[b]);
  return { redOverGreen: step(0, 1), greenOverBlue: step(1, 2), redOverBlue: step(0, 2) };
}

describe("the sanity ranges", () => {
  // Each range is held at 4σ; whether its σ meets (b − a) ÷ 16 is the verdict sanityRanges.json
  // pins (the Venus-class surface's heavy tails refuse it: R08's records).
  it.each(SANITY.map((r) => [r.name, r] as const))(
    "hold %s's surface flux within its range at 4σ",
    (_, range) => {
      expect(sanityReading(range).within).toBe(true);
    },
  );

  it("red-shift the Venus-class surface's light, its red past its blue by 4σ", () => {
    expect(redShift().redOverBlue).toBeGreaterThan(4);
  });

  it("are pinned in sanityRanges.json with their readings and verdicts", async () => {
    const table = {
      description:
        "The references' sanity ranges, written by references.test.ts (plan R08, R08.T12.b.2): each probe's downwelling luminance over the top's, its σ, and gate.ts's verdict, and the Venus-class surface's red shift in σ. Do not edit by hand.",
      ...Object.fromEntries(
        SANITY.map((range) => {
          const { fraction } = downwellingFractions(referenceOf(range.caseName), range.aggregate);
          return [
            range.name,
            { ...sanityReading(range), channels: fraction, low: range.low, high: range.high },
          ] as const;
        }),
      ),
      "venus-92bar-red-shift": redShift(),
    };
    await expect(
      expectCommittedTable(table, committedSanity, "./sanityRanges.json"),
    ).resolves.toBeUndefined();
  });
});

/** A ground geometry of Mars's case at a sun zenith, looking straight up. */
function marsGround(sunZenithDeg: number, heightM = 1): ReferenceGeometry {
  return {
    name: "probe",
    image: "probe",
    observer: { heightM, latitudeDeg: 0 },
    view: { zenithDeg: 0, azimuthDeg: 0 },
    sun: { zenithDeg: sunZenithDeg, azimuthDeg: 0 },
  };
}

describe("the client's beam quadrature", () => {
  it("is the vertical column's optical depth for a sun overhead", () => {
    const medium = MARS_CASE.medium;
    const vertical = ([0, 1, 2] as const).map((c) =>
      medium.terms.reduce(
        (sum, t) =>
          sum +
          extinction(t)[c] *
            (columnLengthM(t.density, medium.topHeightM) - columnLengthM(t.density, 1)),
        0,
      ),
    );
    const beam = clientBeam(MARS_CASE, marsGround(0));
    expect(beam?.opticalDepth.map((tau, c) => tau / (vertical[c] ?? Number.NaN) - 1)).toEqual([
      expect.closeTo(0, 7),
      expect.closeTo(0, 7),
      expect.closeTo(0, 7),
    ]);
  });

  it("is none where the ground hides the sun", () => {
    expect(clientBeam(MARS_CASE, marsGround(95))).toBeNull();
  });

  it("is nothing from above the top towards a sun the atmosphere does not lie before", () => {
    expect(clientBeam(MARS_CASE, marsGround(30, 1e6))?.opticalDepth).toEqual([0, 0, 0]);
  });
});

/** A geometry's file entry of radiance `l` and σ `sigma` in every channel, scalar alone. */
function scalarGeometryFile(name: string, l: number, sigma: number): ReferenceGeometryFile {
  return {
    name,
    detectorHalfAngleDeg: 0.5,
    samples: 4000,
    quarterSamples: 1000,
    radiance: [l, l, l],
    standardError: [sigma, sigma, sigma],
    quarterStandardError: [2 * sigma, 2 * sigma, 2 * sigma],
    sunOpticalDepth: [[0.1, 0.2, 0.3]],
  };
}

/** {@link scalarGeometryFile} in the Stokes mode, its scalar radiance 0.9 of its vector one. */
function geometryFile(name: string, l: number, sigma: number): ReferenceGeometryFile {
  return {
    ...scalarGeometryFile(name, l, sigma),
    stokes: {
      scalarRadiance: [0.9 * l, 0.9 * l, 0.9 * l],
      scalarStandardError: [sigma, sigma, sigma],
      scalarQuarterStandardError: [2 * sigma, 2 * sigma, 2 * sigma],
    },
  };
}

/** A reference aggregate's file entry. */
const AGGREGATE: ReferenceAggregateFile = {
  name: "flux-s0",
  samples: 4000,
  quarterSamples: 1000,
  incidentTop: [10, 10, 10],
  directGround: [6, 5, 4],
  diffuseGround: [2, 3, 4],
  diffuseGroundStandardError: [0.01, 0.01, 0.01],
  diffuseGroundQuarterStandardError: [0.02, 0.02, 0.02],
  upwellingTop: [1, 1, 1],
  upwellingTopStandardError: [0.001, 0.001, 0.001],
  upwellingTopQuarterStandardError: [0.002, 0.002, 0.002],
  scalar: {
    diffuseGround: [1, 2, 3],
    diffuseGroundStandardError: [0.01, 0.01, 0.01],
    diffuseGroundQuarterStandardError: [0.02, 0.02, 0.02],
    upwellingTop: [0.9, 0.9, 0.9],
    upwellingTopStandardError: [0.001, 0.001, 0.001],
    upwellingTopQuarterStandardError: [0.002, 0.002, 0.002],
  },
};

/** A small reference of Mars's case: its first geometry, and one aggregate. */
const SMALL: ReferenceFile = {
  format: 1,
  case: "mars",
  polarisation: "stokes",
  samples: 4000,
  seed: 2,
  wavelengthsNm: [645, 544, 433.625],
  geometries: [geometryFile(MARS_CASE.geometries[0]?.name ?? "", 2, 0.01)],
  aggregates: [AGGREGATE],
};

describe("the reference's readers", () => {
  it("read a geometry in its case's image", () => {
    const [reading] = readingsOf(MARS_CASE, SMALL);
    expect([reading?.image, reading?.radiance]).toEqual([
      MARS_CASE.geometries[0]?.image,
      [2, 2, 2],
    ]);
  });

  it("read the scalar radiance of the same paths where asked", () => {
    expect(readingsOf(MARS_CASE, SMALL, "scalar")[0]?.radiance).toEqual([1.8, 1.8, 1.8]);
  });

  it("refuse a scalar reading of a reference traced without --stokes", () => {
    const name = MARS_CASE.geometries[0]?.name ?? "";
    const scalarOnly = {
      ...SMALL,
      polarisation: "scalar",
      geometries: [scalarGeometryFile(name, 1, 0)],
    };
    expect(() => readingsOf(MARS_CASE, scalarOnly, "scalar")).toThrow(/--stokes/u);
  });

  it("refuse a geometry its case does not have", () => {
    expect(() =>
      readingsOf(MARS_CASE, { ...SMALL, geometries: [geometryFile("nowhere", 1, 0)] }),
    ).toThrow(/no geometry/u);
  });

  it("give the downwelling flux as the direct beam and the diffuse flux together", () => {
    const [down, up] = fluxReadingsOf(SMALL);
    expect([down?.flux, up?.flux]).toEqual([
      [8, 8, 8],
      [1, 1, 1],
    ]);
  });

  it("give the scalar fluxes of the same paths where asked", () => {
    const [down, up] = fluxReadingsOf(SMALL, "scalar");
    expect([down?.flux, up?.flux]).toEqual([
      [7, 7, 7],
      [0.9, 0.9, 0.9],
    ]);
  });
});
