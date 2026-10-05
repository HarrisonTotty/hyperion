import { MAX_CUT_V } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { autoAt, manualAt } from "../../test/exposureFixtures";
import { decodedSky, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
import { pixelSolidAngle } from "../camera/projection";
import {
  DEFAULT_EXPOSURE,
  type ExposureControl,
  exposureScale,
  programTriple,
  VIEW_CAMERA,
} from "../photometry/exposure";
import {
  illuminanceLx,
  pixelLuminance,
  PSF_QUAD_PX,
  psfPixelWeights,
} from "../photometry/magnitude";
import { PLANCKIAN_FIT_RANGE_K, starColour } from "../photometry/starColour";
import {
  AGX_MIN_EV,
  type Rgb,
  spriteToneCurve,
  TONE_CURVE_BLACK,
  toneCurve,
} from "../photometry/toneCurve";
import { srgbEncode } from "../post/tonemap";
import { cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";
import type { SkyModel } from "./model";
import {
  DARK_SKY_CD_M2,
  deepestTriple,
  viewSkyLabelV,
  viewSkyLimit,
  viewSkyRequest,
} from "./viewSky";

const INPUT = {
  universe: "000000000000002a",
  observer: skyRequest(0).observer,
  system: "0200080020000000",
  time: { seconds: 0, nanos: 0 },
  fovDeg: 60,
  nMax: 300_000,
} as const;

/** A held sky whose band texels' eye limits are all V 7.4 but one, at 7.9. */
function heldSky(): SkyModel {
  const payload = skyPayload([], 2, 7.4);
  const request = skyRequest(0);
  const model: SkyModel = {
    request,
    response: skyResponse(request, payload, 0, 2),
    ...decodedSky(payload, 0),
    stale: false,
  };
  model.band.eyeLimitMag[3] = 7.9;
  return model;
}

/** EV100 from −14 to 42, `MAN`'s span, by tenths. */
const EV100_SPAN = Array.from({ length: 561 }, (_, i) => -14 + i / 10);

/** A camera view's limit at 60° over the dark sky, V, at the program's triple at an EV100. */
const programLimitV = (ev100: number): number =>
  cameraLimitV(DEFAULT_VIEW_CAMERA, programTriple(VIEW_CAMERA, ev100), 60, DARK_SKY_CD_M2);

describe("deepestTriple", () => {
  it("is the view camera's frame shutter at its top gain: f/1.4, 1/30 s, ISO 409,600, clear", () => {
    const triple = deepestTriple(VIEW_CAMERA);
    expect([
      triple.aperture,
      triple.shutterS,
      Math.abs(triple.iso / 409_600 - 1) < 1e-12,
      triple.ndEv,
    ]).toEqual([1.4, 1 / 30, true, undefined]);
  });

  it("gives a limit no exposure in MAN's span reaches deeper", () => {
    const deepest = cameraLimitV(
      DEFAULT_VIEW_CAMERA,
      deepestTriple(VIEW_CAMERA),
      60,
      DARK_SKY_CD_M2,
    );
    expect(EV100_SPAN.filter((ev100) => programLimitV(ev100) > deepest + 1e-12)).toEqual([]);
  });
});

describe("viewSkyRequest", () => {
  it("asks the eye's limits for an eye view, and no camera limit", () => {
    const request = viewSkyRequest({ ...INPUT, role: "eye" });
    expect(request.eye).toEqual({ field_factor: 1.4, age_years: 25, pigmentation: 0.5 });
    expect(request.camera_limit_v).toBeNull();
    expect(request.exclude_system).toBe("0200080020000000");
  });

  it("asks a camera view's deepest limit, V 10.06 at 60°", () => {
    const request = viewSkyRequest({ ...INPUT, role: "camera" });
    expect(request.eye).toBeNull();
    expect(Math.abs((request.camera_limit_v ?? Number.NaN) - 10.06)).toBeLessThan(0.01);
  });

  it("cuts a narrower camera's request at V 11, where its limit is 11.72 at 30° and 13.58 at 13°", () => {
    const limits = [30, 13].map(
      (fovDeg) => viewSkyRequest({ ...INPUT, role: "camera", fovDeg }).camera_limit_v,
    );
    expect(limits).toEqual([MAX_CUT_V, MAX_CUT_V]);
  });
});

describe("viewSkyLimit", () => {
  it("culls a camera view at its deepest limit, V 10.06, 11.72 and 13.58 at 60°, 30° and 13°", () => {
    const limits = [60, 30, 13].map((fovDeg) => {
      const limit = viewSkyLimit(heldSky(), "camera", fovDeg);
      return limit.kind === "camera" ? limit.limitV : Number.NaN;
    });
    expect(limits.map((limitV) => Math.round(limitV * 100) / 100)).toEqual([10.06, 11.72, 13.58]);
  });

  it("culls an eye view by its band texels' limits", () => {
    expect(viewSkyLimit(heldSky(), "eye", 60).kind).toBe("eye");
  });
});

describe("viewSkyLabelV", () => {
  it("states the eye's deepest limit over the sky", () => {
    expect(viewSkyLabelV(heldSky(), "eye", DEFAULT_EXPOSURE, 60)).toBeCloseTo(7.9, 5);
  });

  it("states a camera's limit at the default MAN exposure, V 10.06 at 60°", () => {
    expect(Math.abs(viewSkyLabelV(heldSky(), "camera", DEFAULT_EXPOSURE, 60) - 10.06)).toBeLessThan(
      0.01,
    );
  });

  it("states a camera's limit at AUTO's exposure, V 2.56 at EV100 15", () => {
    expect(Math.abs(viewSkyLabelV(heldSky(), "camera", autoAt(15), 60) - 2.56)).toBeLessThan(0.01);
  });

  it("states one limit for one EV100 at every level: one camera", () => {
    const sky = heldSky();
    const at15: ReadonlyArray<ExposureControl> = [
      manualAt(15),
      { kind: "inhibited", ev100: 15, reason: "operator" },
      { kind: "inhibited", ev100: 15, reason: "no_image_to_meter" },
    ];
    const reference = viewSkyLabelV(sky, "camera", autoAt(15), 60);
    expect(
      at15.map((control) => Math.abs(viewSkyLabelV(sky, "camera", control, 60) - reference)),
    ).toEqual([0, 0, 0]);
  });
});

/** The largest camera band term of R06.T3.c's colour table, mag: A0V's +0.14. */
const LARGEST_BAND_TERM = 0.14;

/** The centre pixel's share of a centred star's light (R02's point-spread function). */
const CENTRED_PEAK_WEIGHT =
  psfPixelWeights({ xPx: 0, yPx: 0 })[(PSF_QUAD_PX * PSF_QUAD_PX - 1) / 2] ?? Number.NaN;

/** `MAN`'s EV100s at which the label's limit is 0.05 mag or more shallower than the cull's. */
function shallowerExposures(): ReadonlyArray<number> {
  const sky = heldSky();
  const cull = viewSkyLimit(sky, "camera", 60);
  const cullV = cull.kind === "camera" ? cull.limitV : Number.NaN;
  return EV100_SPAN.filter(
    (ev100) => cullV - viewSkyLabelV(sky, "camera", autoAt(ev100), 60) >= 0.05,
  );
}

describe("the deeper cull", () => {
  it("parts from the label's limit by 0.05 mag or more from EV100 3.5 up", () => {
    const shallower = shallowerExposures();
    expect([shallower[0], shallower.length]).toEqual([
      expect.closeTo(3.5, 9),
      EV100_SPAN.filter((ev100) => ev100 >= 3.45).length,
    ]);
  });

  it.each([
    { name: "1080p", widthPx: 1_920, heightPx: 1_080 },
    { name: "4K", widthPx: 3_840, heightPx: 2_160 },
  ])(
    "draws a star at the label's limit, centred, below AgX's floor and as black at $name and 60°",
    ({ widthPx, heightPx }) => {
      const centreSr = pixelSolidAngle(
        vec3(0, 0, -1),
        { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
        { widthPx, heightPx },
      );
      const sky = heldSky();
      const exposed = shallowerExposures().map(
        (ev100) =>
          pixelLuminance(
            illuminanceLx(viewSkyLabelV(sky, "camera", autoAt(ev100), 60)),
            CENTRED_PEAK_WEIGHT,
            centreSr,
          ) * exposureScale(ev100),
      );
      expect([
        exposed.filter((value) => !(value < 2 ** AGX_MIN_EV)),
        exposed.filter((value) => {
          const out = toneCurve([value, value, value]);
          return out.some((channel, at) => channel !== TONE_CURVE_BLACK[at]);
        }),
      ]).toEqual([[], []]);
    },
  );

  it("moves no 8-bit code at a 4K corner for the bluest star at the label's limit", () => {
    const tan = Math.tan(Math.PI / 6);
    const cornerSr = pixelSolidAngle(
      vec3(tan, (tan * 9) / 16, -1),
      { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
      { widthPx: 3_840, heightPx: 2_160 },
    );
    // The bluest colour the fit gives with the largest camera band term, A0V's +0.14 (Pickles
    // 1998 through the default sensor, decision-camera-eta), which lets the star in at
    // V = limit − 0.14: a bound no one star reaches, some 0.3 EV above AgX's floor at EV100 3.5.
    const [r, g, b] = starColour(PLANCKIAN_FIT_RANGE_K[1]);
    const sky = heldSky();
    const codes = shallowerExposures().flatMap((ev100) => {
      const exposedPerLx =
        pixelLuminance(
          illuminanceLx(viewSkyLabelV(sky, "camera", autoAt(ev100), 60) - LARGEST_BAND_TERM),
          CENTRED_PEAK_WEIGHT,
          cornerSr,
        ) * exposureScale(ev100);
      const light: Rgb = [exposedPerLx * r, exposedPerLx * g, exposedPerLx * b];
      const sprite = spriteToneCurve(light);
      const pass = toneCurve(light);
      return [0, 1, 2].flatMap((at) => [
        255 * srgbEncode(sprite[at] ?? Number.NaN),
        255 * Math.abs(srgbEncode(pass[at] ?? Number.NaN) - srgbEncode(TONE_CURVE_BLACK[at] ?? 0)),
      ]);
    });
    expect(codes.filter((code) => !(code < 0.01))).toEqual([]);
  });
});
