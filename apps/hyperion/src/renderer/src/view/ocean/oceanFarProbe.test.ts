import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { OceanFarOptics } from "./oceanFar";
import { OCEAN_FAR_FLOATS, packOceanFar, packOceanFarProbe, packOceanRays } from "./oceanFarProbe";

/** An airless sea seen with no cap: both infinite. */
const OPTICS: OceanFarOptics = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_356_752,
  pixelAngleRad: 2e-4,
  whitecaps: true,
  sunDirection: vec3(0, 0.6, 0.8),
  glintCapCdM2: Number.POSITIVE_INFINITY,
  sunLux: [1, 2, 3],
  skyLux: [4, 5, 6],
  sea: { thresholdMps: Number.POSITIVE_INFINITY, equivalentPerMps: 0, coxMunkPerMps: 0 },
  refractiveIndex: [1.33, 1.34, 1.35],
  attenuationPerM: null,
  deepReflectance: [0, 0, 0],
  skyReflectance: 0.066,
  bedAlbedo: [0.3, 0.3, 0.25],
};

describe("the far ocean's uniform (R11.T8.b)", () => {
  it("packs the optics in OceanFar's order, ∞ as 10³⁰ and an opaque column's flag 0", () => {
    const packed = packOceanFar(OPTICS);
    expect(packed.length).toBe(OCEAN_FAR_FLOATS);
    expect([...packed.subarray(0, 4)]).toEqual([6_378_137, 6_356_752, Math.fround(2e-4), 1]);
    expect(packed[7]).toBe(Math.fround(1e30));
    expect(packed[11]).toBe(Math.fround(1e30));
    expect([...packed.subarray(20, 24)]).toEqual([0, 0, 0, 0]);
    expect(packed[27]).toBe(Math.fround(0.066));
    expect([...packed.subarray(28, 31)]).toEqual([0.3, 0.3, 0.25].map(Math.fround));
  });

  it("follows the optics with the ray count, and packs two vec4f a ray", () => {
    expect(packOceanFarProbe(OPTICS, 7)[OCEAN_FAR_FLOATS]).toBe(7);
    const rays = packOceanRays([{ originM: vec3(1, 2, 3), direction: vec3(0, 0, -1) }]);
    expect([...rays]).toEqual([1, 2, 3, 0, 0, 0, -1, 0]);
  });
});
