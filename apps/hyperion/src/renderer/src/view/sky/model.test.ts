import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { galacticTranslated } from "../coords/position";
import { decodedSky, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
import {
  bakedBeyondM,
  PARALLAX_BASELINE_M,
  type SkyCamera,
  type SkyModel,
  skyRequestReason,
} from "./model";

const LIGHT_YEAR_M = 9_460_730_472_580_800;

/** A sky asked at `years` with stars at `distancesLy`. */
function heldSky(years: number, distancesLy: ReadonlyArray<number>): SkyModel {
  const stars = distancesLy.map((distanceLy) => ({
    direction: [1, 0, 0] as const,
    distanceLy,
    vMag: 5,
  }));
  const payload = skyPayload(stars, 2, null);
  const request = skyRequest(years);
  return {
    request,
    response: skyResponse(request, payload, stars.length, 2),
    ...decodedSky(payload, stars.length),
    stale: false,
  };
}

const OBSERVER = skyRequest(0).observer;

const CAMERA: SkyCamera = { position: OBSERVER, fovDeg: 60, widthPx: 1_920 };

/** The camera moved `m` metres along +x from the sky's observer. */
function movedBy(m: number): SkyCamera {
  return { ...CAMERA, position: galacticTranslated(OBSERVER, vec3(m, 0, 0)) };
}

describe("skyRequestReason", () => {
  it("asks on arrival, with no sky held", () => {
    expect(skyRequestReason(null, { request: skyRequest(0), cameras: [CAMERA] })).toBe("arrival");
  });

  it("asks on arrival in another system", () => {
    const held = heldSky(0, [100]);
    expect(
      skyRequestReason(held, { request: skyRequest(0, "0200080020000001"), cameras: [CAMERA] }),
    ).toBe("arrival");
  });

  it("holds the sky within its validity and asks again past it", () => {
    const held = heldSky(0, [100]);
    expect(skyRequestReason(held, { request: skyRequest(0.5), cameras: [CAMERA] })).toBeNull();
    expect(skyRequestReason(held, { request: skyRequest(1.01), cameras: [CAMERA] })).toBe(
      "expired",
    );
  });

  it("asks again for a deeper camera limit, a larger N_max or the eye, never for less", () => {
    const held = heldSky(0, [100]);
    const ask = (request: ReturnType<typeof skyRequest>) =>
      skyRequestReason(held, { request, cameras: [CAMERA] });
    expect(ask({ ...skyRequest(0), camera_limit_v: 10.5 })).toBe("limits");
    expect(ask({ ...skyRequest(0), camera_limit_v: 9.52 })).toBeNull();
    expect(ask({ ...skyRequest(0), camera_limit_v: 8 })).toBeNull();
    expect(ask({ ...skyRequest(0), n_max: 5_000 })).toBe("limits");
    expect(ask({ ...skyRequest(0), n_max: 10 })).toBeNull();
    expect(
      ask({ ...skyRequest(0), eye: { field_factor: 2, age_years: 25, pigmentation: 0.5 } }),
    ).toBe("limits");
  });

  it("asks again on a jump back before the sky's time", () => {
    const held = heldSky(10, [100]);
    expect(skyRequestReason(held, { request: skyRequest(9), cameras: [CAMERA] })).toBe("jump");
  });

  it("asks again when a camera's move shifts the nearest baked star by a tenth of a pixel", () => {
    const held = heldSky(0, [20, 400]);
    const pixelRad = Math.PI / 3 / 1_920;
    // The 20 ly star is baked (beyond the parallax sprites' 9 ly or so); a tenth of a pixel of it.
    const shiftM = 0.1 * pixelRad * 20 * LIGHT_YEAR_M;
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(shiftM * 0.9)],
      }),
    ).toBeNull();
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(shiftM * 1.1)],
      }),
    ).toBe("parallax");
  });

  it("leaves the parallax sprites, within 30 au's tenth of a pixel, out of the rule", () => {
    // At 1080p and 60°, 30 au moves a star a tenth of a pixel at about 9 ly.
    const beyondLy = bakedBeyondM(CAMERA) / LIGHT_YEAR_M;
    expect(beyondLy).toBeGreaterThan(8);
    expect(beyondLy).toBeLessThan(10);
    const held = heldSky(0, [1, 400]);
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(PARALLAX_BASELINE_M)],
      }),
    ).toBeNull();
  });
});
