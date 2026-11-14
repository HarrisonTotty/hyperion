import { describe, expect, it } from "vitest";

import { cloudFieldParams, cloudSeedOf, coverageTexels } from "./cloudDensity";
import { cloudShellOf } from "./cloudLayer";
import { cloudNoiseTexels } from "./cloudNoise";
import { deltaEddington } from "./cloudOptics";
import {
  CLOUD_PROBE_CASE_ROWS,
  CLOUD_PROBE_OUTPUTS,
  type CloudProbeCase,
  decodeCloudProbe,
  expectedCloudProbe,
  packCloudProbeCases,
} from "./cloudProbe";
import type { CloudLightInput } from "./cloudShadow";
import { marchColumn, marchDeckRay, octaveCorrectionAt, octaveCorrectionOf } from "./cloudVolume";
import { STAND_IN_CONDENSATES } from "./condensates";

const CORRECTION = octaveCorrectionOf(STAND_IN_CONDENSATES.liquidWater);

const CASES: ReadonlyArray<CloudProbeCase> = [
  { kind: "field", pointM: { x: 6.4e6, y: 1e5, z: -2e5 }, footprintM: 3_000, octaveCap: 9 },
  { kind: "optics", tau: 10, asymmetry: 0.85, coAlbedo: 1e-5, mu0: 0.5 },
  {
    kind: "ground",
    placeOverA: { x: 1.01, y: 0, z: 0 },
    sun: { x: 1, y: 0, z: 0 },
    sourceRadiusRad: 0.01,
  },
  {
    kind: "column",
    tau: 10,
    mu0: 0.75,
    muView: 1,
    cosTheta: -0.75,
    asymmetry: 0.8654,
    coAlbedo: 5.43e-7,
    steps: 32,
  },
  {
    kind: "deck",
    originM: { x: 6.39e6, y: 0, z: 0 },
    direction: { x: -1, y: 0, z: 0 },
    sun: { x: 0.8, y: 0.6, z: 0 },
    baseM: 1_000,
    topM: 3_000,
    steps: 8,
    footprintPerM: 0.05,
  },
];

const CLOUDS: CloudLightInput = {
  coverage: coverageTexels(
    { widthCells: 8, heightCells: 4, fractions: new Float64Array(32).fill(0.5), offset: 0 },
    () => ({ eastMps: 3, northMps: 0 }),
  ),
  noise: cloudNoiseTexels(),
  params: cloudFieldParams({
    seed: cloudSeedOf("body"),
    condensate: { inCloudOpticalDepth550: 8 },
    radiusM: 6.378e6,
    time: { seconds: 0, nanos: 0 },
    rate: 1,
    windMps: 3,
    octaves: 12,
  }),
  shell: cloudShellOf({ equatorialRadiusM: 6.378e6, polarRadiusM: 6.357e6 }, 1_500),
  optics: STAND_IN_CONDENSATES.liquidWater,
  datumRadiusM: 6.378e6,
};

describe("the clouds' probe", () => {
  it("packs each case's kind and arguments into its rows", () => {
    const packed = packCloudProbeCases(CASES);
    expect(packed.length).toBe(4 * CLOUD_PROBE_CASE_ROWS * CASES.length);
    expect(Array.from(packed.subarray(0, 6))).toEqual([0, 6.4e6, 1e5, -2e5, 3_000, 9]);
    expect(Array.from(packed.subarray(16, 21))).toEqual([1, 10, 0.85, 1e-5, 0.5].map(Math.fround));
    expect(Array.from(packed.subarray(32, 40))).toEqual(
      [2, 1.01, 0, 0, 1, 0, 0, 0.01].map(Math.fround),
    );
  });

  it("reads the target's f32 bits back a case a row", () => {
    const values = new Float32Array(CLOUD_PROBE_OUTPUTS * 2).map((_, i) => i / 4);
    expect(decodeCloudProbe(values.buffer, 2)[1]).toEqual(
      Array.from({ length: CLOUD_PROBE_OUTPUTS }, (_, i) => (i + CLOUD_PROBE_OUTPUTS) / 4),
    );
  });

  it("expects a field case's sample and coverage cell", () => {
    const [field] = expectedCloudProbe(CASES, CLOUDS, CORRECTION);
    expect(field).toHaveLength(CLOUD_PROBE_OUTPUTS);
    expect(field?.[7]).toBeCloseTo(3, 2);
  });

  it("expects an optics case's δ-Eddington at its inputs in f32", () => {
    const [, optics] = expectedCloudProbe(CASES, CLOUDS, CORRECTION);
    const slab = deltaEddington({
      tau: 10,
      asymmetry: Math.fround(0.85),
      singleScatterAlbedo: 1 - Math.fround(1e-5),
      mu0: 0.5,
    });
    expect(optics?.slice(0, 3)).toEqual([
      slab.reflectance,
      slab.directTransmittance,
      slab.diffuseTransmittance,
    ]);
  });

  it("packs a column case's seven arguments and a deck case's thirteen", () => {
    const packed = packCloudProbeCases(CASES);
    expect(Array.from(packed.subarray(48, 56))).toEqual(
      [3, 10, 0.75, 1, -0.75, 0.8654, 5.43e-7, 32].map(Math.fround),
    );
    expect(Array.from(packed.subarray(64, 78))).toEqual(
      [4, 6.39e6, 0, 0, -1, 0, 0, 0.8, 0.6, 0, 1_000, 3_000, 8, 0.05].map(Math.fround),
    );
  });

  it("expects a column case's uncorrected march and its correction at its inputs in f32", () => {
    const column = expectedCloudProbe(CASES, CLOUDS, CORRECTION)[3];
    const optics = { asymmetry: Math.fround(0.8654), coAlbedo: Math.fround(5.43e-7) };
    const light = marchColumn({
      tau: 10,
      mu0: 0.75,
      muView: 1,
      cosTheta: -0.75,
      optics,
      steps: 32,
    });
    expect(column?.slice(0, 3)).toEqual([
      light.single,
      light.multiple,
      octaveCorrectionAt(CORRECTION, 10, 0.75),
    ]);
  });

  it("expects a deck case's march through the deck at its inputs in f32", () => {
    const deck = expectedCloudProbe(CASES, CLOUDS, CORRECTION)[4];
    const march = marchDeckRay(CLOUDS, { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: { x: 6.39e6, y: 0, z: 0 },
      direction: { x: -1, y: 0, z: 0 },
      sun: { x: Math.fround(0.8), y: Math.fround(0.6), z: 0 },
      steps: 8,
      footprintPerM: Math.fround(0.05),
    });
    expect(march.radiance).toBeGreaterThan(0);
    expect(deck?.slice(0, 2)).toEqual([march.radiance, march.transmittance]);
  });

  it("expects a deck case that misses the deck to pass untouched", () => {
    const deck: CloudProbeCase = {
      kind: "deck",
      originM: { x: 6.4e6, y: 0, z: 0 },
      direction: { x: 1, y: 0, z: 0 },
      sun: { x: 1, y: 0, z: 0 },
      baseM: 1_000,
      topM: 3_000,
      steps: 8,
      footprintPerM: 1e-3,
    };
    expect(expectedCloudProbe([deck], CLOUDS, CORRECTION)[0]?.slice(0, 2)).toEqual([0, 1]);
  });

  it("expects a place above the shell to keep all its light", () => {
    const [, , ground] = expectedCloudProbe(CASES, CLOUDS, CORRECTION);
    expect(ground).toEqual([1, 1, 0, 0, 0, 0, 0, 0]);
  });
});
