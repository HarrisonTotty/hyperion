import { describe, expect, it } from "vitest";

import { decodedSky, skyPayload } from "../../test/skyFixtures";
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
