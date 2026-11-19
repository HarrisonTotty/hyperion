import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import {
  CLOUD_OCTAVES_MAX,
  cloudFieldParams,
  cloudSeedOf,
  coverageTexels,
  thresholdOf,
} from "./cloudDensity";
import { cloudShellOf } from "./cloudLayer";
import { CloudMapAnchor, CloudMapTwin } from "./cloudMap";
import {
  backgroundTransmittance,
  type CloudMarchScene,
  deckStretch,
  diffuseReflectance,
  marchCloudRay,
  marchEntry,
  rayShare,
  stepWarp,
} from "./cloudMarch";
import { cloudNoiseTexels } from "./cloudNoise";
import {
  type CloudColumn,
  cloudShapeFrame,
  cloudShapeKindOf,
  normalisationTable,
} from "./cloudShape";
import { cloudShapeNoises } from "./cloudShapeNoise";
import { marchDeckRay, octaveCorrectionOf } from "./cloudVolume";
import { STAND_IN_CONDENSATES } from "./condensates";

const A = 6_378_137;
const C = 6_356_752.3;
const K = A / C;
const WATER = STAND_IN_CONDENSATES.liquidWater;
const CORRECTION = octaveCorrectionOf(WATER);
const NOISES = cloudShapeNoises(cloudSeedOf("0200080020000000.0103"));
const FIELD_NOISE = cloudNoiseTexels();
const LIQUID = cloudShapeKindOf({ phase: "liquid" });

/** A place over the equator at 0° E, `heightM` above the datum. */
function over(heightM: number): Vec3 {
  return { x: A + heightM, y: 0, z: 0 };
}

const UP = { x: 1, y: 0, z: 0 };
const DOWN = { x: -1, y: 0, z: 0 };

/**
 * A scene whose every column covers `cover` of its footprint at in-cloud τ `tau`, the deck 1–3 km,
 * flattened by `flat`: a map of still air at `cover`'s threshold whose field's octaves a time rate
 * fades out, so that each column is the calibrated mean.
 */
function uniformScene(cover: number, tau: number, flat: number): CloudMarchScene {
  const frame = new CloudMapAnchor().frameFor(over(2_000), A + 1_500);
  const { threshold } = thresholdOf(cover);
  const params = cloudFieldParams({
    seed: 1,
    condensate: { inCloudOpticalDepth550: tau },
    radiusM: A,
    time: { seconds: 0, nanos: 0 },
    rate: 1e9,
    windMps: 20,
    octaves: CLOUD_OCTAVES_MAX,
  });
  return {
    map: new CloudMapTwin(frame, () => ({ resolvedSum: 0, threshold, eastMps: 0, northMps: 0 })),
    field: { noise: FIELD_NOISE, params },
    shellRadiusM: A + 1_500,
    noises: NOISES,
    frame: cloudShapeFrame(2_000, { x: 0, y: 0, z: 0 }, 0, 0),
    inputs: {
      baseM: 1_000,
      depthM: 2_000,
      kind: LIQUID,
      table: normalisationTable(LIQUID.profileExponent),
      flat,
    },
    topM: 3_000,
    datumRadiusM: A,
    axisRatio: K,
    optics: WATER,
    correction: CORRECTION,
  };
}

/** A ray's march through its deck's stretch, at a footprint that leaves the noises out. */
function marched(
  scene: CloudMarchScene,
  originM: Vec3,
  direction: Vec3,
  sun: Vec3,
  options: {
    readonly steps?: number;
    readonly jitter?: number;
    readonly footprintPerM?: number;
  } = {},
) {
  const stretch = deckStretch(originM, direction, K, A, 1_000, 3_000);
  if (stretch === null) {
    throw new Error("the ray misses the deck");
  }
  const footprintPerM = options.footprintPerM ?? 10;
  const entry = marchEntry(scene, originM, direction, stretch[0], footprintPerM);
  return marchCloudRay(
    scene,
    { viewSteps: options.steps ?? 64, lightSteps: 6, detail: true },
    {
      originM,
      direction,
      t0M: stretch[0],
      t1M: stretch[1],
      suns: [sun, null],
      footprintPerM,
      jitter: options.jitter ?? 0.5,
    },
    entry,
  );
}

describe("a ray's stretch of the deck (R11.T7.c)", () => {
  it("runs from the top to the base from above, from the base out of the top from below, and from the camera within", () => {
    const above = deckStretch(over(10_000), DOWN, K, A, 1_000, 3_000);
    const below = deckStretch(over(2), UP, K, A, 1_000, 3_000);
    const within = deckStretch(over(2_000), UP, K, A, 1_000, 3_000);
    expect(above?.[0]).toBeCloseTo(7_000, 3);
    expect(above?.[1]).toBeCloseTo(9_000, 3);
    expect(below?.[0]).toBeCloseTo(998, 3);
    expect(below?.[1]).toBeCloseTo(2_998, 3);
    expect(within).toEqual([0, expect.closeTo(1_000, 3)]);
    expect(deckStretch(over(10_000), UP, K, A, 1_000, 3_000)).toBeNull();
  });

  it("warps a long stretch's steps towards the camera, a short one's not", () => {
    expect(stepWarp(2_000, 2_000)).toBe(1);
    expect(stepWarp(16_000, 2_000)).toBe(1);
    expect(stepWarp(128_000, 2_000)).toBe(2);
    expect(stepWarp(1e9, 2_000)).toBe(2);
  });
});

/** An entry's column of share `fraction` and unresolved deviation `unresolvedSd`. */
function entryOf(fraction: number, unresolvedSd: number): CloudColumn {
  return { fraction, meanTau: 4, topShare: 1, unresolvedSd, excess: 1, cover: fraction };
}

describe("a cloud's background (R11.T7.c)", () => {
  it("shows through a thin edge by its δ-scaled share, and none through a thick core", () => {
    const g = WATER.asymmetry;
    const thin = backgroundTransmittance(Math.exp(-1), g);
    expect(thin).toBeGreaterThan(Math.exp(-1));
    expect(thin).toBeLessThan(Math.exp(-(1 - g * g)));
    expect(backgroundTransmittance(1, g)).toBe(1);
    expect(backgroundTransmittance(3e-3, g)).toBe(0);
    expect(backgroundTransmittance(Math.exp(-3), g)).toBeGreaterThan(
      backgroundTransmittance(Math.exp(-4), g),
    );
  });
});

describe("a ray's share (R11.T7.c)", () => {
  it("takes each column's own τ where its entry resolves the field, the layer's share where not", () => {
    const at = entryOf;
    expect(rayShare(at(0.3, 0.01), 0)).toBe(1);
    expect(rayShare(at(0.3, 0.9), 0)).toBeCloseTo(0.3, 12);
    expect(rayShare(at(0.3, 0.01), 1)).toBeCloseTo(0.3, 12);
  });
});

describe("the volume's march through its 3D density (R11.T7.c)", () => {
  it("is the uniform deck's march when flattened, the hand-over's form", () => {
    // marchDeckRay reads the field itself: under a time rate that fades every octave, an
    // overcast's calibrated mean, f 1 and τ̄ 4, everywhere; τ 4 leaves the view's transmittance
    // above the march's stop.
    const clouds = {
      coverage: coverageTexels(
        {
          widthCells: 64,
          heightCells: 32,
          fractions: new Float64Array(64 * 32).fill(1),
          offset: 0,
        },
        () => ({ eastMps: 0, northMps: 0 }),
      ),
      noise: cloudNoiseTexels(),
      params: cloudFieldParams({
        seed: 1,
        condensate: { inCloudOpticalDepth550: 4 },
        radiusM: A,
        time: { seconds: 0, nanos: 0 },
        rate: 1e9,
        windMps: 20,
        octaves: CLOUD_OCTAVES_MAX,
      }),
      shell: cloudShellOf({ equatorialRadiusM: A, polarRadiusM: C }, 1_500),
      optics: WATER,
      datumRadiusM: A,
    };
    const sun = { x: 0.8, y: 0.6, z: 0 };
    const reference = marchDeckRay(clouds, { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: over(10_000),
      direction: DOWN,
      sun,
      steps: 48,
      footprintPerM: 10,
    });
    const flat = marched(uniformScene(1, 4, 1), over(10_000), DOWN, sun, {
      steps: 48,
    });
    expect(flat.sun[0]).toBeCloseTo(reference.radiance, 6);
    // The background through it is the δ-scaled share of the same view transmittance.
    expect(flat.transmittance).toBeCloseTo(
      backgroundTransmittance(reference.transmittance, WATER.asymmetry),
      6,
    );
    expect(flat.share).toBeCloseTo(1, 6);
  });

  it.each([0.3, 0.6])(
    "is the uniform deck's march when flattened under a broken cover of %s, the layer's share of it",
    (fraction) => {
      // Under broken cover the cells give way to the layer's slab as the volume flattens, so that
      // at the hand-over the march is the layer's independent-pixel form, its share the field's.
      const clouds = {
        coverage: coverageTexels(
          {
            widthCells: 64,
            heightCells: 32,
            fractions: new Float64Array(64 * 32).fill(fraction),
            offset: 0,
          },
          () => ({ eastMps: 0, northMps: 0 }),
        ),
        noise: cloudNoiseTexels(),
        params: cloudFieldParams({
          seed: 1,
          condensate: { inCloudOpticalDepth550: 4 },
          radiusM: A,
          time: { seconds: 0, nanos: 0 },
          rate: 1e9,
          windMps: 20,
          octaves: CLOUD_OCTAVES_MAX,
        }),
        shell: cloudShellOf({ equatorialRadiusM: A, polarRadiusM: C }, 1_500),
        optics: WATER,
        datumRadiusM: A,
      };
      const sun = { x: 0.8, y: 0.6, z: 0 };
      // The reference marches the covered part at its in-cloud τ, the layer's form.
      const reference = marchDeckRay(clouds, { baseM: 1_000, topM: 3_000 }, CORRECTION, {
        originM: over(10_000),
        direction: DOWN,
        sun,
        steps: 48,
        footprintPerM: 10,
        share: fraction,
      });
      const flat = marched(uniformScene(fraction, 4, 1), over(10_000), DOWN, sun, { steps: 48 });
      expect(flat.share).toBeCloseTo(fraction, 6);
      expect(flat.sun[0]).toBeCloseTo(reference.radiance, 6);
    },
  );

  it("finds the sun's path through a uniform deck by its march towards it to 5% of the plane-parallel's", () => {
    // Unflattened, the profile's u^(2/3) and the march towards the source; the plane-parallel twin
    // is the same with the source's path taken through the column alone.
    const sun = { x: 0.8, y: 0.6, z: 0 };
    const scene = uniformScene(1, 8, 0);
    const marchedSun = marched(scene, over(10_000), DOWN, sun);
    const planeParallel = marched(
      { ...scene, inputs: { ...scene.inputs, flat: 1e-9 } },
      over(10_000),
      DOWN,
      sun,
    );
    expect(marchedSun.sun[0]).toBeGreaterThan(0);
    expect(Math.abs(marchedSun.sun[0] / planeParallel.sun[0] - 1)).toBeLessThan(0.05);
  });

  it("darkens a cloud's base seen from beneath against its top seen from above, and lets the sky through its gaps", () => {
    const sun = { x: 0.8, y: 0.6, z: 0 };
    const scene = uniformScene(1, 20, 0);
    const top = marched(scene, over(10_000), DOWN, sun);
    const base = marched(scene, over(2), UP, sun);
    expect(base.sun[0]).toBeLessThan(0.5 * top.sun[0]);
    expect(base.transmittance).toBeLessThan(0.01);
    const clear = marched(uniformScene(0, 8, 0), over(2), UP, sun);
    expect(clear.transmittance).toBeCloseTo(1, 6);
    expect(clear.sun[0]).toBeLessThan(1e-6);
  });

  it("brightens a thin edge towards the sun, its forward lobe's silver lining", () => {
    const thin = uniformScene(1, 0.5, 0);
    const towards = marched(thin, over(2), UP, { x: 0.95, y: 0.312, z: 0 });
    const away = marched(thin, over(2), UP, { x: 0.3, y: -0.954, z: 0 });
    expect(towards.sun[0]).toBeGreaterThan(3 * away.sun[0]);
  });

  it("lights its sky by the slab's reflectance at the top and its transmittance at the base", () => {
    // The opacity is weighed a little within the cloud, where the share of its τ above grows:
    // measured 0.91 of R from above and 1.04 of 1 − R from beneath (2026-11-19).
    const scene = uniformScene(1, 20, 0);
    const sun = { x: 0.8, y: 0.6, z: 0 };
    const top = marched(scene, over(10_000), DOWN, sun);
    const base = marched(scene, over(2), UP, sun);
    const r = diffuseReflectance(20, WATER.asymmetry);
    expect(Math.abs(top.sky / r - 1)).toBeLessThan(0.15);
    expect(Math.abs(base.sky / (1 - r) - 1)).toBeLessThan(0.1);
  });

  it("weighs its opacity where it meets the cloud, within the stretch", () => {
    const scene = uniformScene(1, 20, 0);
    const from = marched(scene, over(2), UP, { x: 0.8, y: 0.6, z: 0 });
    expect(from.meanDistanceM).toBeGreaterThan(998);
    expect(from.meanDistanceM).toBeLessThan(2_998);
  });

  it("stays finite and within [0, 1] in transmittance with the noises at full share", () => {
    const scene = uniformScene(0.8, 6, 0);
    for (const [origin, direction] of [
      [over(2), UP],
      [over(2), { x: Math.SQRT1_2, y: Math.SQRT1_2, z: 0 }],
      [over(2_000), { x: 0, y: 1, z: 0 }],
      [over(10_000), DOWN],
    ] as const) {
      const result = marched(
        scene,
        origin,
        direction,
        { x: 0.8, y: 0.6, z: 0 },
        { footprintPerM: 1e-3 },
      );
      expect(Number.isFinite(result.sun[0]) && Number.isFinite(result.sky)).toBe(true);
      expect(result.transmittance).toBeGreaterThanOrEqual(0);
      expect(result.transmittance).toBeLessThanOrEqual(1);
    }
  });
});
