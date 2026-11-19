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
import { CloudMapAnchor, cloudMapTwinOf } from "./cloudMap";
import { type CloudMarchScene, marchCloudRay, marchEntry } from "./cloudMarch";
import {
  cloudMapTexelAt,
  cloudShapeFrame,
  cloudShapeKindOf,
  normalisationTable,
} from "./cloudShape";
import { cloudShapeNoises } from "./cloudShapeNoise";
import type { CloudLightInput } from "./cloudShadow";
import { marchColumn, octaveCorrectionAt, octaveCorrectionOf } from "./cloudVolume";
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
    kind: "march",
    originM: { x: 6.39e6, y: 0, z: 0 },
    direction: { x: -1, y: 0, z: 0 },
    sun: { x: 0.8, y: 0.6, z: 0 },
    t0: 9_000,
    t1: 11_000,
    viewSteps: 8,
    lightSteps: 3,
    detail: true,
    footprintPerM: 0.05,
    jitter: 0.25,
  },
];

/** The volume's scene over `clouds`' field, its map about a camera at 2 km over (a, 0, 0). */
function sceneOf(clouds: CloudLightInput): CloudMarchScene {
  const kind = cloudShapeKindOf({ phase: "liquid" });
  const frame = new CloudMapAnchor().frameFor(
    { x: 6.38e6, y: 0, z: 0 },
    clouds.shell.equatorialRadiusM,
  );
  return {
    map: cloudMapTwinOf(frame, clouds.shell, (point, footprintM) =>
      cloudMapTexelAt(clouds.coverage, clouds.noise, clouds.params, point, footprintM),
    ),
    field: { noise: clouds.noise, params: clouds.params },
    shellRadiusM: clouds.shell.equatorialRadiusM,
    noises: cloudShapeNoises(7),
    frame: cloudShapeFrame(2_000, { x: 0, y: 5, z: 0 }, 100, 1),
    inputs: {
      baseM: 1_000,
      depthM: 2_000,
      kind,
      table: normalisationTable(kind.profileExponent),
      flat: 0,
    },
    topM: 3_000,
    datumRadiusM: 6_378_137,
    axisRatio: clouds.shell.equatorialRadiusM / clouds.shell.polarRadiusM,
    optics: clouds.optics,
    correction: CORRECTION,
  };
}

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
    expect(Array.from(packed.subarray(20, 25))).toEqual([1, 10, 0.85, 1e-5, 0.5].map(Math.fround));
    expect(Array.from(packed.subarray(40, 48))).toEqual(
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
    const [field] = expectedCloudProbe(CASES.slice(0, 4), CLOUDS, CORRECTION);
    expect(field).toHaveLength(CLOUD_PROBE_OUTPUTS);
    expect(field?.[7]).toBeCloseTo(3, 2);
  });

  it("expects an optics case's δ-Eddington at its inputs in f32", () => {
    const [, optics] = expectedCloudProbe(CASES.slice(0, 4), CLOUDS, CORRECTION);
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

  it("packs a column case's seven arguments and a march case's sixteen", () => {
    const packed = packCloudProbeCases(CASES);
    expect(Array.from(packed.subarray(60, 68))).toEqual(
      [3, 10, 0.75, 1, -0.75, 0.8654, 5.43e-7, 32].map(Math.fround),
    );
    expect(Array.from(packed.subarray(80, 97))).toEqual(
      [4, 6.39e6, 0, 0, -1, 0, 0, 0.8, 0.6, 0, 9_000, 11_000, 8, 3, 1, 0.05, 0.25].map(Math.fround),
    );
  });

  it("expects a column case's uncorrected march and its correction at its inputs in f32", () => {
    const column = expectedCloudProbe(CASES.slice(0, 4), CLOUDS, CORRECTION)[3];
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

  it("expects a march case's march through the 3D density at its inputs in f32", () => {
    const scene = sceneOf(CLOUDS);
    const [march] = expectedCloudProbe(CASES.slice(4), CLOUDS, CORRECTION, scene);
    const originM = { x: 6.39e6, y: 0, z: 0 };
    const direction = { x: -1, y: 0, z: 0 };
    const footprintPerM = Math.fround(0.05);
    const entry = marchEntry(scene, originM, direction, 9_000, footprintPerM);
    const result = marchCloudRay(
      scene,
      { viewSteps: 8, lightSteps: 3, detail: true },
      {
        originM,
        direction,
        t0M: 9_000,
        t1M: 11_000,
        suns: [{ x: Math.fround(0.8), y: Math.fround(0.6), z: 0 }, null],
        footprintPerM,
        jitter: 0.25,
      },
      entry,
    );
    expect(march?.slice(0, 5)).toEqual([
      result.sun[0],
      result.sky,
      result.transmittance,
      result.meanDistanceM,
      result.share,
    ]);
  });

  it("refuses a march case with no volume's scene", () => {
    expect(() => expectedCloudProbe(CASES.slice(4), CLOUDS, CORRECTION)).toThrow(/scene/);
  });

  it("expects a place above the shell to keep all its light", () => {
    const [, , ground] = expectedCloudProbe(CASES.slice(0, 4), CLOUDS, CORRECTION);
    expect(ground).toEqual([1, 1, 0, 0, 0, 0, 0, 0]);
  });
});
