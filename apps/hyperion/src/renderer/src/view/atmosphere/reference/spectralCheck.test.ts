/**
 * R08.T17's spectral check (plan R08; `spectralCheck.ts`): the record `spectralCheck.json` is the
 * check `spectralCheck.ts` computes now on the four committed spectral references, the two
 * three-channel references and the twin's routed radiance, through `bless.ts`. Every figure is
 * recorded, never asserted: a miss is a finding.
 *
 * @remarks
 * A changed record is rewritten by `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/spectralCheck.test.ts`.
 */

import { describe, expect, it } from "vitest";

import { expectCommittedTable } from "../../../test/bless";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthSpectralDayReference from "./earth-spectral-day.reference.json" with { type: "json" };
import earthSpectralLowReference from "./earth-spectral-low.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import marsSpectralDayReference from "./mars-spectral-day.reference.json" with { type: "json" };
import marsSpectralLowReference from "./mars-spectral-low.reference.json" with { type: "json" };
import caseGateRecord from "./caseGate.json" with { type: "json" };
import committed from "./spectralCheck.json" with { type: "json" };
import vectorGate from "./vectorGate.json" with { type: "json" };
import type { Rgb } from "../../photometry/toneCurve";
import { CHANNEL_WAVELENGTHS_NM } from "../medium";
import { twinValueKey, twinValuesOfRecord } from "./caseGate";
import type { ReferenceAggregateFile, ReferenceFile, ReferenceGeometryFile } from "./references";
import {
  EARTH_SPECTRAL_DAY_CASE,
  EARTH_SPECTRAL_LOW_CASE,
  MARS_SPECTRAL_DAY_CASE,
  MARS_SPECTRAL_LOW_CASE,
  type SpectralReferenceCase,
} from "./spectralCases";
import {
  bodyCheck,
  bodyCheckRecord,
  routedTwinOf,
  SPECTRAL_CHECK_FORMAT,
  spectralReadingsOf,
  type SpectralTrace,
} from "./spectralCheck";

/** The bodies' parts with their references. */
const BODIES: ReadonlyArray<{
  readonly body: string;
  readonly traces: ReadonlyArray<SpectralTrace>;
  readonly channels: ReferenceFile;
}> = [
  {
    body: "earth",
    traces: [
      { spectral: EARTH_SPECTRAL_DAY_CASE, reference: earthSpectralDayReference },
      { spectral: EARTH_SPECTRAL_LOW_CASE, reference: earthSpectralLowReference },
    ],
    channels: earthReference,
  },
  {
    body: "mars",
    traces: [
      { spectral: MARS_SPECTRAL_DAY_CASE, reference: marsSpectralDayReference },
      { spectral: MARS_SPECTRAL_LOW_CASE, reference: marsSpectralLowReference },
    ],
    channels: marsReference,
  },
];

/** The record's head. */
const HEAD = {
  description:
    "R08.T17's spectral check: the reference traced in the 15 bake bins, taken to the render " +
    "channels by the spectral bakes' conversion, against the three-channel reference's vector I " +
    "and the twin's routed radiance (vectorGate.json), per image: Design note 10's tallies, " +
    "Δu′v′ (median and largest) beside its noise, the largest ΔE*ab and the relative RMS; the " +
    "sun's beam straight up and the fluxes by Δu′v′ and luminance; and each geometry's spectral " +
    "reading in the channels, its σ and its Δu′v′ noise. Recorded, never gated. " +
    "Written by spectralCheck.test.ts; do not edit by hand.",
  format: SPECTRAL_CHECK_FORMAT,
  writer: "spectralCheck.test.ts, through spectralCheck.ts (plan R08, R08.T17)",
} as const;

/** A geometry whose every value is `radiance`'s, its σ 1% of it, its sun unhidden. */
function flatGeometry(name: string, radiance: ReadonlyArray<number>): ReferenceGeometryFile {
  const sigma = radiance.map((l) => 0.01 * l);
  return {
    name,
    detectorHalfAngleDeg: 0.5,
    samples: 4096,
    quarterSamples: 1024,
    radiance,
    standardError: sigma,
    quarterStandardError: sigma.map((s) => 2 * s),
    sunOpticalDepth: [radiance.map(() => 0)],
  };
}

/** An aggregate whose fluxes are all `flux`'s, wholly diffuse, its σ 1% of it. */
function flatAggregate(name: string, flux: ReadonlyArray<number>): ReferenceAggregateFile {
  const sigma = flux.map((f) => 0.01 * f);
  return {
    name,
    samples: 4096,
    quarterSamples: 1024,
    incidentTop: flux,
    directGround: flux.map(() => 0),
    diffuseGround: flux,
    diffuseGroundStandardError: sigma,
    diffuseGroundQuarterStandardError: sigma.map((s) => 2 * s),
    upwellingTop: flux,
    upwellingTopStandardError: sigma,
    upwellingTopQuarterStandardError: sigma.map((s) => 2 * s),
  };
}

/**
 * A trace of `spectral` whose every bin carries the same light per unit of the sun's, `x`, at its
 * geometry `name` (its first by default) and, where asked, at its first aggregate.
 */
function flatTrace(
  spectral: SpectralReferenceCase,
  x: number,
  name?: string,
  withAggregate = false,
): SpectralTrace {
  const g =
    name === undefined
      ? spectral.geometries[0]
      : spectral.geometries.find((each) => each.name === name);
  const [a] = spectral.aggregates;
  if (g === undefined || (withAggregate && a === undefined)) {
    throw new Error(`${spectral.name} has the geometry and aggregate asked for`);
  }
  const light = spectral.sunIrradianceWPerM2Nm.map((e) => x * e);
  return {
    spectral,
    reference: {
      format: 1,
      case: spectral.name,
      polarisation: "stokes",
      samples: 4096,
      seed: spectral.seed,
      wavelengthsNm: [...spectral.wavelengthsNm],
      geometries: [flatGeometry(g.name, light)],
      aggregates: withAggregate && a !== undefined ? [flatAggregate(a.name, light)] : [],
    },
  };
}

describe("the spectral check", () => {
  it("takes light flat across the bins to the three-channel case's own colour", () => {
    const [reading] = spectralReadingsOf(flatTrace(EARTH_SPECTRAL_DAY_CASE, 0.25));
    if (reading === undefined) {
      throw new Error("one reading");
    }
    const sun = EARTH_SPECTRAL_DAY_CASE.base.sunIlluminanceLx;
    for (const c of [0, 1, 2] as const) {
      expect(reading.radiance[c]).toBeCloseTo(0.25 * sun[c], 6);
      expect(reading.standardError[c]).toBeGreaterThan(0);
      expect(reading.standardError[c]).toBeLessThan(0.01 * reading.radiance[c]);
    }
    expect(reading.uvNoise).toBeGreaterThan(0);
  });

  it("gives a flat sky, beam and flux the three-channel case's own colour and luminance", () => {
    const spectral = EARTH_SPECTRAL_DAY_CASE;
    const name = "ground-s30-v0-a0";
    const trace = flatTrace(spectral, 0.25, name, true);
    const sun = spectral.base.sunIlluminanceLx;
    const flat: Rgb = [0.25 * sun[0], 0.25 * sun[1], 0.25 * sun[2]];
    const [aggregate] = trace.reference.aggregates;
    if (aggregate === undefined) {
      throw new Error("the trace has its aggregate");
    }
    const channelReference: ReferenceFile = {
      ...trace.reference,
      case: spectral.base.name,
      wavelengthsNm: [...CHANNEL_WAVELENGTHS_NM],
      geometries: [flatGeometry(name, flat)],
      aggregates: [flatAggregate(aggregate.name, flat)],
    };
    const beamKey = twinValueKey({
      quantity: "beam",
      setting: "case",
      caseName: spectral.base.name,
      name,
    });
    const check = bodyCheck(
      [trace],
      channelReference,
      new Map([[name, flat]]),
      new Map([[beamKey, [1, 1, 1]]]),
    );
    const [beam] = check.beams;
    if (beam === undefined) {
      throw new Error("the zenith view has its beam");
    }
    for (const c of [0, 1, 2] as const) {
      expect(beam.binsLx[c] / sun[c]).toBeCloseTo(1, 9);
    }
    expect(beam.uv).toBeLessThan(1e-9);
    expect(beam.luminanceRatio).toBeCloseTo(1, 9);
    expect(beam.twinUv).toBeLessThan(1e-9);
    expect(check.fluxes.map((f) => f.flux)).toEqual([
      `${aggregate.name}/downwelling`,
      `${aggregate.name}/upwelling`,
    ]);
    for (const f of check.fluxes) {
      expect(f.uv).toBeLessThan(1e-9);
      expect(f.luminanceRatio).toBeCloseTo(1, 9);
    }
    const [image] = check.images;
    expect(image?.channels.largestUv).toBeLessThan(1e-9);
    expect(image?.twinTally.misses).toBe(0);
  });

  it("refuses a reference of other wavelengths", () => {
    const trace = flatTrace(MARS_SPECTRAL_LOW_CASE, 1);
    expect(() =>
      spectralReadingsOf({
        ...trace,
        reference: { ...trace.reference, wavelengthsNm: [645, 544, 433.625] },
      }),
    ).toThrow(/wavelengths/u);
  });

  it("spectralCheck.json is the check the committed references give now", async () => {
    const caseTwin = twinValuesOfRecord(caseGateRecord);
    const bodies = Object.fromEntries(
      BODIES.map(({ body, traces, channels }) => [
        body,
        bodyCheckRecord(bodyCheck(traces, channels, routedTwinOf(vectorGate, body), caseTwin)),
      ]),
    );
    await expect(
      expectCommittedTable({ ...HEAD, bodies }, committed, "./spectralCheck.json"),
    ).resolves.toBeUndefined();
  }, 120_000);
});
