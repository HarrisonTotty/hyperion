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
import { STAND_IN_CONDENSATES } from "./condensates";

const CASES: ReadonlyArray<CloudProbeCase> = [
  { kind: "field", pointM: { x: 6.4e6, y: 1e5, z: -2e5 }, footprintM: 3_000, octaveCap: 9 },
  { kind: "optics", tau: 10, asymmetry: 0.85, coAlbedo: 1e-5, mu0: 0.5 },
  {
    kind: "ground",
    placeOverA: { x: 1.01, y: 0, z: 0 },
    sun: { x: 1, y: 0, z: 0 },
    sourceRadiusRad: 0.01,
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
    expect(Array.from(packed.subarray(12, 17))).toEqual([1, 10, 0.85, 1e-5, 0.5].map(Math.fround));
    expect(Array.from(packed.subarray(24, 32))).toEqual(
      [2, 1.01, 0, 0, 1, 0, 0, 0.01].map(Math.fround),
    );
  });

  it("reads the target's f32 bits back a case a row", () => {
    const values = new Float32Array(CLOUD_PROBE_OUTPUTS * 2).map((_, i) => i / 4);
    expect(decodeCloudProbe(values.buffer, 2)[1]).toEqual(
      Array.from({ length: CLOUD_PROBE_OUTPUTS }, (_, i) => (i + CLOUD_PROBE_OUTPUTS) / 4),
    );
  });

  it("expects each case's twin, a place above the shell keeping all its light", () => {
    const [field, optics, ground] = expectedCloudProbe(CASES, CLOUDS);
    expect(field).toHaveLength(CLOUD_PROBE_OUTPUTS);
    expect(field?.[7]).toBeCloseTo(3, 2);
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
    expect(ground).toEqual([1, 1, 0, 0, 0, 0, 0, 0]);
  });
});
