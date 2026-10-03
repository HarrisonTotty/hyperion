import { describe, expect, it } from "vitest";

import { decodedSky, skyPayload } from "../../test/skyFixtures";
import { cubeTexelOf } from "./cube";
import { eyeLimitAt, fieldFactorOffsetMag } from "./limits";

const DIRECTIONS: ReadonlyArray<readonly [number, number, number]> = [
  [1, 0, 0],
  [0, -1, 0.2],
  [0.3, 0.4, -0.8],
];

describe("eyeLimitAt", () => {
  it("reads the band texel's limit at the request's field factor", () => {
    const { band } = decodedSky(skyPayload([], 4, 6.6), 0);
    const source = { band, faceTexels: 4, requestFieldFactor: 1.4 };
    for (const direction of DIRECTIONS) {
      expect(eyeLimitAt(source, direction, 1.4)).toBeCloseTo(6.6, 5);
    }
  });

  it("lowers every limit by 0.387 mag for a field factor of 2", () => {
    const { band } = decodedSky(skyPayload([], 4, 6.6), 0);
    const source = { band, faceTexels: 4, requestFieldFactor: 1.4 };
    for (const direction of DIRECTIONS) {
      expect(eyeLimitAt(source, direction, 1.4) - eyeLimitAt(source, direction, 2)).toBeCloseTo(
        0.387,
        3,
      );
    }
  });

  it("is NaN where the eye was not asked", () => {
    const { band } = decodedSky(skyPayload([], 4, null), 0);
    expect(eyeLimitAt({ band, faceTexels: 4, requestFieldFactor: null }, [1, 0, 0], 1.4)).toBeNaN();
  });

  it("refuses a field factor that is not positive", () => {
    expect(() => fieldFactorOffsetMag(0, 1.4)).toThrow(RangeError);
  });
});

describe("eyeLimitAt's texel", () => {
  it("reads each direction's own texel, as the cube's face, row and column give it", () => {
    const { band } = decodedSky(skyPayload([], 4, 6.6), 0);
    // Give every texel its own limit: 5 + its index ÷ 100.
    band.eyeLimitMag.forEach((_, index) => {
      band.eyeLimitMag[index] = 5 + index / 100;
    });
    const source = { band, faceTexels: 4, requestFieldFactor: 1.4 };
    for (const direction of DIRECTIONS) {
      const { face, row, column } = cubeTexelOf(direction[0], direction[1], direction[2], 4);
      const index = (face * 4 + row) * 4 + column;
      expect(eyeLimitAt(source, direction, 1.4)).toBeCloseTo(5 + index / 100, 5);
    }
  });
});
