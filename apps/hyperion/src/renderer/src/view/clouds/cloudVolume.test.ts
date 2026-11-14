import { describe, expect, it } from "vitest";

import { cloudFieldAt, cloudFieldParams, cloudSeedOf, coverageTexels } from "./cloudDensity";
import { cloudShellOf } from "./cloudLayer";
import { cloudNoiseTexels } from "./cloudNoise";
import { deltaEddington } from "./cloudOptics";
import type { CloudLightInput } from "./cloudShadow";
import {
  BACKWARD_LOBE_ASYMMETRY,
  CLOUD_OCTAVES,
  CLOUD_VOLUME_LEAST_MU,
  correctedColumn,
  CORRECTION_LEAST_MU,
  CORRECTION_MAX,
  ESCAPE_SERIES,
  LEAST_VIEW_MU,
  CORRECTION_MUS,
  CORRECTION_TAU_RANGE,
  CORRECTION_TAUS,
  correctionMu,
  correctionTau,
  FORWARD_LOBE_SHARE,
  fluxReflectance,
  marchColumn,
  marchDeckRay,
  octaveCorrectionAt,
  octaveCorrectionOf,
  octaveReflectance,
  twoLobeAsymmetry,
  twoLobeOf,
  twoLobePhase,
} from "./cloudVolume";
import cloudVolumeWgsl from "./shaders/cloudVolume.wgsl?raw";
import { STAND_IN_CONDENSATES } from "./condensates";

const WATER = STAND_IN_CONDENSATES.liquidWater;
const CORRECTION = octaveCorrectionOf(WATER);
const TAUS = [1, 2, 5, 10, 20, 50];
const MUS = [0.5, 0.75, 1];

/** δ-Eddington's flux reflectance of liquid water's slab. */
function slabReflectance(tau: number, mu0: number): number {
  return deltaEddington({
    tau,
    asymmetry: WATER.asymmetry,
    singleScatterAlbedo: 1 - WATER.coAlbedo,
    mu0,
  }).reflectance;
}

/** The corrected march's flux reflectance of a column, `steps` steps deep. */
function marchedReflectance(tau: number, mu0: number, steps: number): number {
  const k = octaveCorrectionAt(CORRECTION, tau, mu0);
  return fluxReflectance(mu0, (muView, cosTheta) =>
    correctedColumn(marchColumn({ tau, mu0, muView, cosTheta, optics: WATER, steps }), k),
  );
}

/** The octaves' flux reflectance, single and multiple together. */
function octaveTotal(tau: number, mu0: number, optics: typeof WATER = WATER): number {
  const flux = octaveReflectance(tau, mu0, optics);
  return flux.single + flux.multiple;
}

/** The value of a WGSL `const NAME : type = value;`, as a number. */
function wgslConst(name: string): number {
  const found = new RegExp(String.raw`const ${name} : [a-z0-9]+ = ([-0-9.e]+)u?;`, "u").exec(
    cloudVolumeWgsl,
  );
  if (found?.[1] === undefined) {
    throw new Error(`no const ${name}`);
  }
  return Number(found[1]);
}

describe("the two lobes", () => {
  it("weigh to the condensate's asymmetry", () => {
    for (const g of [0, 0.5, 0.75, WATER.asymmetry, 0.9]) {
      expect(twoLobeAsymmetry(twoLobeOf(g))).toBeCloseTo(g, 12);
    }
  });

  it("integrate to 1 over the sphere, with a mean cosine of g", () => {
    for (const g of [0.3, WATER.asymmetry]) {
      const lobes = twoLobeOf(g);
      let total = 0;
      let mean = 0;
      const n = 200_000;
      for (let i = 0; i < n; i += 1) {
        const c = -1 + (2 * (i + 0.5)) / n;
        const p = twoLobePhase(c, lobes) * 2 * Math.PI * (2 / n);
        total += p;
        mean += c * p;
      }
      expect(total).toBeCloseTo(1, 5);
      expect(mean).toBeCloseTo(g, 4);
    }
  });

  it("scale each lobe's asymmetry with an octave's c^i", () => {
    const lobes = twoLobeOf(0.8);
    expect(twoLobePhase(0.3, lobes, 0)).toBeCloseTo(1 / (4 * Math.PI), 12);
  });
});

describe("the octaves' fit to δ-Eddington", () => {
  it("misses δ-Eddington by its stated 59% alone", () => {
    let worst = 0;
    for (const tau of TAUS) {
      for (const mu0 of MUS) {
        worst = Math.max(worst, Math.abs(octaveTotal(tau, mu0) / slabReflectance(tau, mu0) - 1));
      }
    }
    expect(worst).toBeLessThan(0.6);
  });

  it("adds no light: no octave scatters more than it takes out", () => {
    expect(CLOUD_OCTAVES.scattering).toBeLessThanOrEqual(CLOUD_OCTAVES.extinction);
  });

  it("corrects to δ-Eddington exactly at the table's samples", () => {
    for (const [i, j] of [
      [3, 2],
      [17, 15],
      [28, 0],
    ] as const) {
      const tau = correctionTau(i);
      const mu0 = correctionMu(j);
      const flux = octaveReflectance(tau, mu0, WATER);
      const corrected = flux.single + octaveCorrectionAt(CORRECTION, tau, mu0) * flux.multiple;
      expect(corrected).toBeCloseTo(slabReflectance(tau, mu0), 6);
    }
  });

  it("holds a finite correction within its bounds at every sample", () => {
    expect(
      CORRECTION.factors.every((k) => Number.isFinite(k) && k >= 0 && k <= CORRECTION_MAX),
    ).toBe(true);
  });

  it("reflects no more than the sun brings, corrected, over the table's whole range", () => {
    for (const tau of [0.05, 1, 50, 500, 10_000]) {
      for (const mu0 of [CLOUD_VOLUME_LEAST_MU, 0.3, 1]) {
        const flux = octaveReflectance(tau, mu0, WATER);
        const corrected = flux.single + octaveCorrectionAt(CORRECTION, tau, mu0) * flux.multiple;
        expect(corrected).toBeLessThanOrEqual(1);
      }
    }
  });
});

describe("the reference march", () => {
  it("converges to the octaves' closed form", () => {
    for (const [tau, mu0] of [
      [1, 1],
      [10, 0.5],
      [50, 0.75],
    ] as const) {
      const marched = fluxReflectance(mu0, (muView, cosTheta) => {
        const light = marchColumn({ tau, mu0, muView, cosTheta, optics: WATER, steps: 512 });
        return light.single + light.multiple;
      });
      expect(marched / octaveTotal(tau, mu0)).toBeCloseTo(1, 3);
    }
  });

  it("matches δ-Eddington for each column's τ to 3% at μ₀ ≥ 0.5, corrected", () => {
    for (const tau of TAUS) {
      for (const mu0 of [0.5, 0.6, 0.85, 1]) {
        expect(
          Math.abs(marchedReflectance(tau, mu0, 64) / slabReflectance(tau, mu0) - 1),
        ).toBeLessThan(0.03);
      }
    }
  });

  it("stays finite along a grazing view", () => {
    const light = marchColumn({
      tau: 8,
      mu0: 0.5,
      muView: 0,
      cosTheta: 0.2,
      optics: WATER,
      steps: 32,
    });
    expect(Number.isFinite(light.single + light.multiple)).toBe(true);
  });
});

/** A body's clouds of one coverage everywhere at 1.5 km, liquid water's optics, τ̄ 8. */
function uniformClouds(fraction: number, rate: number): CloudLightInput {
  const widthCells = 64;
  const heightCells = 32;
  return {
    coverage: coverageTexels(
      {
        widthCells,
        heightCells,
        fractions: new Float64Array(widthCells * heightCells).fill(fraction),
        offset: 0,
      },
      () => ({ eastMps: 0, northMps: 0 }),
    ),
    noise: cloudNoiseTexels(),
    params: cloudFieldParams({
      seed: cloudSeedOf("0200080020000000.0103"),
      condensate: { inCloudOpticalDepth550: 8 },
      radiusM: 6_378_137,
      time: { seconds: 77_777, nanos: 0 },
      rate,
      windMps: 20,
      octaves: 12,
    }),
    shell: cloudShellOf({ equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.3 }, 1_500),
    optics: WATER,
    datumRadiusM: 6_378_137,
  };
}

describe("a coverage cell", () => {
  it("is as bright on the mean of its texels by δ-Eddington as by the march, to 2%", () => {
    const clouds = uniformClouds(0.6, 1);
    const r = clouds.shell.equatorialRadiusM;
    // A cell of 1.4° about 10° N, 30° E, its texels 2 km apart.
    let byLayer = 0;
    let byMarch = 0;
    const n = 12;
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        const lat = ((10 + (1.4 * (i + 0.5)) / n) * Math.PI) / 180;
        const lon = ((30 + (1.4 * (j + 0.5)) / n) * Math.PI) / 180;
        const point = {
          x: r * Math.cos(lat) * Math.cos(lon),
          y: r * Math.cos(lat) * Math.sin(lon),
          z: r * Math.sin(lat),
        };
        const tau = cloudFieldAt(
          clouds.coverage,
          clouds.noise,
          clouds.params,
          point,
          2_000,
        ).meanTau;
        byLayer += slabReflectance(tau, 0.8);
        byMarch += marchedReflectance(tau, 0.8, 32);
      }
    }
    expect(byLayer).toBeGreaterThan(0);
    expect(Math.abs(byMarch / byLayer - 1)).toBeLessThan(0.02);
  });
});

describe("marchDeckRay", () => {
  it("sees a uniform overcast from above as its column's corrected march", () => {
    const clouds = uniformClouds(1, 1e9);
    const deck = { baseM: 1_000, topM: 3_000 };
    const origin = { x: 6_378_137 + 20_000, y: 0, z: 0 };
    const down = { x: -1, y: 0, z: 0 };
    const sun = { x: 1, y: 0, z: 0 };
    const marched = marchDeckRay(clouds, deck, CORRECTION, {
      originM: origin,
      direction: down,
      sun,
      steps: 64,
      footprintPerM: 1e-3,
    });
    const column = correctedColumn(
      marchColumn({ tau: 8, mu0: 1, muView: 1, cosTheta: -1, optics: WATER, steps: 64 }),
      octaveCorrectionAt(CORRECTION, 8, 1),
    );
    expect(marched.radiance / column).toBeCloseTo(1, 3);
  });

  it("brings a ray grazing the deck no more than twice a white surface's light", () => {
    // Tangent to the shell 2 km up at the equator, the sun overhead there.
    const r = 6_378_137 + 2_000;
    const grazing = marchDeckRay(uniformClouds(1, 1e9), { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: { x: r, y: -300_000, z: 0 },
      direction: { x: 0, y: 1, z: 0 },
      sun: { x: 1, y: 0, z: 0 },
      steps: 256,
      footprintPerM: 1e-3,
    });
    expect(grazing.radiance).toBeGreaterThan(0);
    // About 0.37 sr⁻¹ (2026-11-14), where octaves that added light gave 7.25.
    expect(grazing.radiance).toBeLessThan(2 / Math.PI);
  });

  it("takes no sun on its night side", () => {
    const night = marchDeckRay(uniformClouds(1, 1e9), { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: { x: 6_378_137 + 20_000, y: 0, z: 0 },
      direction: { x: -1, y: 0, z: 0 },
      sun: { x: -1, y: 0, z: 0 },
      steps: 32,
      footprintPerM: 1e-3,
    });
    expect(night.radiance).toBe(0);
  });

  it("lets a uniform overcast's column transmit e^(−τ̄) of a ray straight through it", () => {
    const marched = marchDeckRay(uniformClouds(1, 1e9), { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: { x: 6_378_137 + 20_000, y: 0, z: 0 },
      direction: { x: -1, y: 0, z: 0 },
      sun: { x: 1, y: 0, z: 0 },
      steps: 64,
      footprintPerM: 1e-3,
    });
    expect(marched.transmittance).toBeCloseTo(Math.exp(-8), 4);
  });

  it("passes a ray that misses the deck untouched", () => {
    const clouds = uniformClouds(1, 1e9);
    const away = marchDeckRay(clouds, { baseM: 1_000, topM: 3_000 }, CORRECTION, {
      originM: { x: 6_378_137 + 20_000, y: 0, z: 0 },
      direction: { x: 1, y: 0, z: 0 },
      sun: { x: 1, y: 0, z: 0 },
      steps: 16,
      footprintPerM: 1e-3,
    });
    expect(away).toEqual({ radiance: 0, transmittance: 1 });
  });
});

describe("cloudVolume.wgsl", () => {
  it("holds the CPU's octaves, lobes and table", () => {
    expect(wgslConst("CLOUD_OCTAVE_COUNT")).toBe(CLOUD_OCTAVES.count);
    expect(wgslConst("CLOUD_OCTAVE_EXTINCTION")).toBe(CLOUD_OCTAVES.extinction);
    expect(wgslConst("CLOUD_OCTAVE_SCATTERING")).toBe(CLOUD_OCTAVES.scattering);
    expect(wgslConst("CLOUD_OCTAVE_PHASE")).toBe(CLOUD_OCTAVES.phase);
    expect(wgslConst("CLOUD_ESCAPE_SERIES")).toBe(ESCAPE_SERIES);
    expect(wgslConst("CLOUD_LEAST_VIEW_MU")).toBe(LEAST_VIEW_MU);
    expect(wgslConst("CLOUD_BACKWARD_LOBE")).toBe(BACKWARD_LOBE_ASYMMETRY);
    expect(wgslConst("CLOUD_FORWARD_SHARE")).toBe(FORWARD_LOBE_SHARE);
    expect(wgslConst("CLOUD_CORRECTION_TAUS")).toBe(CORRECTION_TAUS);
    expect(wgslConst("CLOUD_CORRECTION_MUS")).toBe(CORRECTION_MUS);
    expect(wgslConst("CLOUD_CORRECTION_TAU_LEAST")).toBe(CORRECTION_TAU_RANGE[0]);
    expect(wgslConst("CLOUD_CORRECTION_TAU_MOST")).toBe(CORRECTION_TAU_RANGE[1]);
    expect(wgslConst("CLOUD_CORRECTION_LEAST_MU")).toBe(CORRECTION_LEAST_MU);
    expect(wgslConst("CLOUD_VOLUME_LEAST_MU")).toBe(CLOUD_VOLUME_LEAST_MU);
  });
});
