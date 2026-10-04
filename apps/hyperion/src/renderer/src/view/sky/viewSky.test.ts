import { describe, expect, it } from "vitest";

import { decodedSky, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
import { DEFAULT_EXPOSURE } from "../photometry/exposure";
import type { SkyModel } from "./model";
import { viewSkyLimit, viewSkyRequest } from "./viewSky";

const INPUT = {
  universe: "000000000000002a",
  observer: skyRequest(0).observer,
  system: "0200080020000000",
  time: { seconds: 0, nanos: 0 },
  exposure: DEFAULT_EXPOSURE,
  fovDeg: 60,
  nMax: 300_000,
} as const;

describe("viewSkyRequest", () => {
  it("asks the eye's limits for an eye view, and no camera limit", () => {
    const request = viewSkyRequest({ ...INPUT, role: "eye" });
    expect(request.eye).toEqual({ field_factor: 1.4, age_years: 25, pigmentation: 0.5 });
    expect(request.camera_limit_v).toBeNull();
    expect(request.exclude_system).toBe("0200080020000000");
  });

  it("asks a camera view's noise-floor limit, at most V 11", () => {
    const request = viewSkyRequest({ ...INPUT, role: "camera" });
    expect(request.eye).toBeNull();
    expect(request.camera_limit_v).toBeLessThanOrEqual(11);
    expect(request.camera_limit_v).toBeGreaterThan(9);
  });
});

describe("viewSkyLimit", () => {
  it("states the eye's deepest limit over the sky", () => {
    const payload = skyPayload([], 2, 7.4);
    const request = skyRequest(0);
    const model: SkyModel = {
      request,
      response: skyResponse(request, payload, 0, 2),
      ...decodedSky(payload, 0),
      stale: false,
    };
    model.band.eyeLimitMag[3] = 7.9;
    const { limit, labelV } = viewSkyLimit(model, "eye", DEFAULT_EXPOSURE, 60);
    expect(labelV).toBeCloseTo(7.9, 5);
    expect(limit.kind).toBe("eye");
  });
});
