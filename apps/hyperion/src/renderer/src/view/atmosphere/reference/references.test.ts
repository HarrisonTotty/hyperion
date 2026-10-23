/**
 * The checks on the committed references (plan R08, R08.T12.b.1): each is its case's, traced with
 * `--stokes` at the case's seed and counts; its `sunOpticalDepth` is the client's own `f64`
 * quadrature of its case along the same chord to 10⁻⁶ max(1, τ) (Design note 10's beam check); and
 * its σ meets the metric's caps wherever `gate.ts` does not refuse it, in the vector and the scalar
 * radiance, every refusal pinned in `refusals.json` as R08's record lists it
 * (`decision-r08-t12b-scope.md`, rulings 6, 8 and 9). And the readers and the beam quadrature
 * themselves, on small inputs.
 *
 * @remarks
 * `refusals.json` is rewritten with the references, by `HYPERION_BLESS=1 pnpm --filter hyperion
 * exec vitest run src/renderer/src/view/atmosphere/reference/references.test.ts`, and the record's
 * list follows it.
 */

import { describe, expect, it } from "vitest";

import {
  caseJson,
  REFERENCE_DEFAULT_SAMPLES,
  type SampleCounts,
} from "../../../test/atmosphereCases";
import { expectCommittedTable } from "../../../test/bless";
import { aggregateRefusal, beamDepthAgrees, type Channel, refusals } from "../gate";
import { columnLengthM, extinction } from "../medium";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import committedRefusals from "./refusals.json" with { type: "json" };
import sampleCounts from "./sampleCounts.json" with { type: "json" };
import { MARS_CASE, REFERENCE_CASES, type ReferenceCase } from "./cases";
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

const CASES = REFERENCE_CASES.map((c) => [c.name, c] as const);

describe("the committed references", () => {
  it.each(CASES)(
    "%s is its case's, traced with --stokes at its seed and the default count",
    (name, referenceCase) => {
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

  it.each(CASES)("%s traces its case's geometries at their counts and cones", (name, c) => {
    const reference = referenceOf(name);
    const written = caseJson(c, COUNTS[name]);
    expect(reference.geometries.map((g) => [g.name, g.samples, g.detectorHalfAngleDeg])).toEqual(
      written.geometries.map((g) => [
        g.name,
        g.samples ?? reference.samples,
        g.detectorHalfAngleDeg ?? written.detectorHalfAngleDeg,
      ]),
    );
  });

  it.each(CASES)("%s traces its case's aggregates at their counts", (name, c) => {
    const reference = referenceOf(name);
    const written = caseJson(c, COUNTS[name]);
    expect(reference.aggregates.map((a) => [a.name, a.samples])).toEqual(
      written.aggregates.map((a) => [a.name, a.samples ?? reference.samples]),
    );
  });

  it.each(CASES)(
    "%s's sunOpticalDepth is the client's own quadrature to 10⁻⁶ max(1, τ)",
    { timeout: 300_000 },
    (name, referenceCase: ReferenceCase) => {
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

  it("meet their σ caps wherever gate.ts does not refuse them, every refusal listed", async () => {
    const table = {
      description:
        "The references' refusals by gate.ts, written by references.test.ts (plan R08, R08.T12.b.1): <kind>/<name>/<reason>/<channels>. Do not edit by hand.",
      ...Object.fromEntries(
        REFERENCE_CASES.map((c) => [c.name, refusedIn(c, referenceOf(c.name))] as const),
      ),
    };
    await expect(
      expectCommittedTable(table, committedRefusals, "./refusals.json"),
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
