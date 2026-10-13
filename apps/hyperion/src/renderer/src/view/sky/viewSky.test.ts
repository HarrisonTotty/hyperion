import { MAX_CUT_V, type SkyStars } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { autoAt, manualAt } from "../../test/exposureFixtures";
import { decodedSky, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
import { FOV_STEPS_DEG, pixelSolidAngle } from "../camera/projection";
import {
  DEFAULT_EXPOSURE,
  type ExposureControl,
  exposureScale,
  programTriple,
  shownTriple,
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
import { bandTexels } from "./band";
import { cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";
import { cubeTexelOf, texelSolidAnglesSr } from "./cube";
import { cullSky } from "./cull";
import type { SkyModel } from "./model";
import { starIlluminanceRgbLx } from "./photometry";
import {
  cameraTexelLimitsV,
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

/** A sky of surface brightness μ, mag arcsec⁻², as a luminance: B = 10^((12.58 − μ) ÷ 2.5). */
function skyOf(mu: number): number {
  return 10 ** ((12.58 - mu) / 2.5);
}

/**
 * μ 24 mag arcsec⁻², Leinert et al. 1998's integrated starlight at the galactic pole: the uniform
 * sky a camera's request and cull read before R06.T13.i.
 */
const MU_24_CD_M2 = skyOf(24);

/** The view camera's limit at its deepest triple, V, at a field of view over a background. */
function deepestLimitV(fovDeg: number, backgroundCdM2: number): number {
  return cameraLimitV(DEFAULT_VIEW_CAMERA, deepestTriple(VIEW_CAMERA), fovDeg, backgroundCdM2);
}

/** The band's face side of the camera skies below, texels. */
const FACE = 4;

/** What a camera sky of {@link cameraSky} is asked at and states, besides its band. */
interface CameraSkyOptions {
  readonly fovDeg?: number;
  /** The reply's cut, V; the request's where absent, as the server cuts a camera's sky. */
  readonly cutV?: number;
  readonly overflow?: number;
}

/**
 * A camera view's held sky, asked at `fovDeg` (60° unless given), its band texels lit at
 * `luminanceOf` each texel's index, face after face, rows from the top.
 */
function cameraSky(
  luminanceOf: (texel: number) => number,
  options: CameraSkyOptions = {},
): SkyModel {
  const payload = skyPayload([], FACE, null, luminanceOf);
  const request = viewSkyRequest({ ...INPUT, role: "camera", fovDeg: options.fovDeg ?? 60 });
  const response = skyResponse(request, payload, 0, FACE);
  const decoded = decodedSky(payload, 0);
  return {
    request,
    response: {
      ...response,
      cut_v: options.cutV ?? response.cut_v,
      overflow: options.overflow ?? 0,
    },
    ...decoded,
    stale: false,
  };
}

/** A star to place: its direction, V and camera band term. */
interface PlacedStar {
  readonly direction: readonly [number, number, number];
  readonly vMag: number;
  readonly cameraBandMag: number;
}

/** The stars as a decoded payload holds them, white, with no eye offset. */
function starsOf(placed: ReadonlyArray<PlacedStar>): SkyStars {
  return {
    count: placed.length,
    directions: Float32Array.from(placed.flatMap((star) => [...star.direction])),
    distanceLy: new Float32Array(placed.length).fill(100),
    vMag: Float32Array.from(placed.map((star) => star.vMag)),
    chroma: new Float32Array(placed.length * 2).fill(1 / 3),
    eyeOffsetMag: new Float32Array(placed.length),
    cameraBandMag: Float32Array.from(placed.map((star) => star.cameraBandMag)),
  };
}

/** A texel's index in a band of {@link FACE}² faces, face after face, rows from the top. */
function texelOf(direction: readonly [number, number, number]): number {
  const { face, column, row } = cubeTexelOf(direction[0], direction[1], direction[2], FACE);
  return (face * FACE + row) * FACE + column;
}

/** A direction in the Milky Way's brightest texel of the sky below, and one in its darkest. */
const BRIGHT_DIRECTION = [1, 0.2, 0.1] as const;
const DARK_DIRECTION = [-1, 0.2, 0.1] as const;

/**
 * T17's camera band near the Sun at its two ends (Design note 18): μ 21.26 in the +X face, where
 * {@link BRIGHT_DIRECTION} lies, and μ 25.25 elsewhere.
 */
function nearSunBand(texel: number): number {
  return texel < FACE * FACE ? skyOf(21.26) : skyOf(25.25);
}

/** 2,000 stars over the sky on a Fibonacci lattice, V 9.4 to 10.4, band terms −0.3 to +0.14. */
function latticeStars(): SkyStars {
  const count = 2_000;
  const terms = [-0.3, -0.1, 0, 0.14];
  return starsOf(
    Array.from({ length: count }, (_, i) => {
      const z = 1 - (2 * (i + 0.5)) / count;
      const r = Math.sqrt(1 - z * z);
      const phi = i * Math.PI * (3 - Math.sqrt(5));
      return {
        direction: [r * Math.cos(phi), r * Math.sin(phi), z] as const,
        vMag: 9.4 + (i % 101) / 100,
        cameraBandMag: terms[i % terms.length] ?? 0,
      };
    }),
  );
}

/** EV100 from −14 to 42, `MAN`'s span, by tenths. */
const EV100_SPAN = Array.from({ length: 561 }, (_, i) => -14 + i / 10);

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

  it.each([
    { sky: "a black sky", backgroundCdM2: 0 },
    { sky: "μ 24", backgroundCdM2: MU_24_CD_M2 },
    { sky: "μ 21.26", backgroundCdM2: skyOf(21.26) },
  ])("gives a limit no exposure in MAN's span reaches deeper, over $sky", ({ backgroundCdM2 }) => {
    const deepest = deepestLimitV(60, backgroundCdM2);
    const deeper = EV100_SPAN.filter(
      (ev100) =>
        cameraLimitV(DEFAULT_VIEW_CAMERA, programTriple(VIEW_CAMERA, ev100), 60, backgroundCdM2) >
        deepest + 1e-12,
    );
    expect(deeper).toEqual([]);
  });
});

describe("viewSkyRequest", () => {
  it("asks the eye's limits for an eye view, and no camera limit", () => {
    const request = viewSkyRequest({ ...INPUT, role: "eye" });
    expect(request.eye).toEqual({ field_factor: 1.4, age_years: 25, pigmentation: 0.5 });
    expect(request.camera_limit_v).toBeNull();
    expect(request.exclude_system).toBe("0200080020000000");
  });

  it("asks a camera's black-sky limit at every step of FOV_STEPS_DEG, clamped at V 11.0", () => {
    const asked = FOV_STEPS_DEG.map(
      (fovDeg) => viewSkyRequest({ ...INPUT, role: "camera", fovDeg }).camera_limit_v,
    );
    expect([
      asked.filter(
        (limitV, at) =>
          limitV !== Math.min(MAX_CUT_V, deepestLimitV(FOV_STEPS_DEG[at] ?? Number.NaN, 0)),
      ),
      asked.map((limitV) => Math.round((limitV ?? Number.NaN) * 100) / 100),
    ]).toEqual([[], [11, 11, 11, 10.87, 10.15, 8.96, 7.77]]);
  });
});

describe("cameraTexelLimitsV", () => {
  it("reads each texel's own background at the deepest triple: V 9.55 at μ 21.26, 10.12 at μ 25.25", () => {
    const sky = cameraSky(nearSunBand);
    const limits = cameraTexelLimitsV(sky, 60);
    const own = Array.from(sky.band.luminanceCdM2, (luminance) => deepestLimitV(60, luminance));
    expect([
      [...limits].filter((limitV, texel) => limitV !== own[texel]).length,
      Math.round((limits[texelOf(BRIGHT_DIRECTION)] ?? Number.NaN) * 100) / 100,
      Math.round((limits[texelOf(DARK_DIRECTION)] ?? Number.NaN) * 100) / 100,
    ]).toEqual([0, 9.55, 10.12]);
  });

  it("is never deeper than the reply's cut", () => {
    const sky = cameraSky(() => skyOf(25.25), { cutV: 9.5 });
    expect(new Set(cameraTexelLimitsV(sky, 60))).toEqual(new Set([9.5]));
  });

  it("reads a texel that is not finite or is below 0 as a black sky, in the cull and the label", () => {
    // Texels the server never sends, beside μ 21.6, at 90° on a reply cut at 11, so that the black
    // sky's limit, V 8.96, is neither the cut nor any lit texel's.
    const unsent = [Number.NaN, Number.POSITIVE_INFINITY, -1e-4];
    const sky = cameraSky((texel) => unsent[texel] ?? skyOf(21.6), { fovDeg: 90, cutV: MAX_CUT_V });
    const blackV = deepestLimitV(90, 0);
    const shownBlackV = cameraLimitV(DEFAULT_VIEW_CAMERA, shownTriple(DEFAULT_EXPOSURE), 90, 0);
    expect([
      [...cameraTexelLimitsV(sky, 90).subarray(0, 4)],
      viewSkyLabelV(sky, "camera", DEFAULT_EXPOSURE, 90),
    ]).toEqual([
      [blackV, blackV, blackV, deepestLimitV(90, Math.fround(skyOf(21.6)))],
      shownBlackV,
    ]);
  });

  it("over bands of μ 15 to 30, holds no texel deeper than the request's cut at any field", () => {
    // Every texel a step of μ from 15 to 30, and one black, on a reply cut at V 11, the deepest any
    // reply holds, so that only the request's own cut can bound the limits.
    const texels = 6 * FACE * FACE;
    const luminanceOf = (texel: number): number =>
      texel === texels - 1 ? 0 : skyOf(15 + (15 * texel) / (texels - 2));
    const deeper = FOV_STEPS_DEG.filter((fovDeg) => {
      const sky = cameraSky(luminanceOf, { fovDeg, cutV: MAX_CUT_V });
      const askedV = sky.request.camera_limit_v ?? Number.NaN;
      return Math.max(...cameraTexelLimitsV(sky, fovDeg)) > askedV;
    });
    expect(deeper).toEqual([]);
  });
});

describe("viewSkyLimit", () => {
  it("culls an eye view by its band texels' limits", () => {
    expect(viewSkyLimit(heldSky(), "eye", 60).kind).toBe("eye");
  });

  it("reads a direction past a short band's texels as a black sky", () => {
    // The +X face's texels alone, though the reply states six faces; at 90° on a reply cut at 11.
    const sky = cameraSky(nearSunBand, { fovDeg: 90, cutV: MAX_CUT_V });
    const perFace = FACE * FACE;
    const short = {
      ...sky,
      band: {
        ...sky.band,
        count: perFace,
        luminanceCdM2: sky.band.luminanceCdM2.slice(0, perFace),
      },
    };
    const limit = viewSkyLimit(short, "camera", 90);
    expect([limit.limitAt(...BRIGHT_DIRECTION), limit.limitAt(...DARK_DIRECTION)]).toEqual([
      deepestLimitV(90, sky.band.luminanceCdM2[texelOf(BRIGHT_DIRECTION)] ?? Number.NaN),
      deepestLimitV(90, 0),
    ]);
  });

  it("keeps a camera's star brighter than its own texel's limit, by V plus its band term, in a bright texel and a dark one", () => {
    const sky = cameraSky(nearSunBand);
    const limits = cameraTexelLimitsV(sky, 60);
    const brightV = limits[texelOf(BRIGHT_DIRECTION)] ?? Number.NaN;
    const darkV = limits[texelOf(DARK_DIRECTION)] ?? Number.NaN;
    const stars = starsOf([
      { direction: BRIGHT_DIRECTION, vMag: brightV - 0.02 - 0.14, cameraBandMag: 0.14 },
      { direction: BRIGHT_DIRECTION, vMag: brightV + 0.02 + 0.3, cameraBandMag: -0.3 },
      { direction: DARK_DIRECTION, vMag: brightV + 0.02 + 0.3, cameraBandMag: -0.3 },
      { direction: DARK_DIRECTION, vMag: darkV - 0.02 + 0.3, cameraBandMag: -0.3 },
      { direction: DARK_DIRECTION, vMag: darkV + 0.02 - 0.14, cameraBandMag: 0.14 },
    ]);
    const culled = cullSky(stars, viewSkyLimit(sky, "camera", 60), FACE);
    expect([...culled.kept]).toEqual([0, 2, 3]);
  });

  it("over a uniform μ 24 band, keeps what the uniform cull at V 10.06 kept", () => {
    const stars = latticeStars();
    const uniform = { kind: "camera" as const, limitAt: () => deepestLimitV(60, MU_24_CD_M2) };
    const byTexel = cullSky(
      stars,
      viewSkyLimit(
        cameraSky(() => MU_24_CD_M2),
        "camera",
        60,
      ),
      FACE,
    );
    const before = cullSky(stars, uniform, FACE);
    expect([
      [...byTexel.kept],
      byTexel.kept.length > 0 && byTexel.culledCount > 0,
      Math.round(deepestLimitV(60, MU_24_CD_M2) * 100) / 100,
    ]).toEqual([[...before.kept], true, 10.06]);
  });

  it("hands the dropped stars' light to the band layer, to 10⁻⁶", () => {
    const stars = latticeStars();
    const sky = cameraSky(nearSunBand);
    const culled = cullSky(stars, viewSkyLimit(sky, "camera", 60), FACE);
    const kept = new Set(culled.kept);
    let expected = 0;
    for (let index = 0; index < stars.count; index += 1) {
      if (!kept.has(index)) {
        expected += starIlluminanceRgbLx(stars.vMag[index] ?? 0, 1 / 3, 1 / 3)[1];
      }
    }
    const empty = { ...sky.band, luminanceCdM2: new Float32Array(sky.band.count) };
    const texels = bandTexels(empty, FACE, culled.bandIlluminanceLx);
    const omegas = texelSolidAnglesSr(FACE);
    let flux = 0;
    for (let texel = 0; texel < sky.band.count; texel += 1) {
      flux += (texels[texel * 4 + 1] ?? 0) * (omegas[texel % (FACE * FACE)] ?? 0);
    }
    expect(culled.culledCount).toBeGreaterThan(0);
    expect(Math.abs(flux / expected - 1)).toBeLessThan(1e-6);
  });
});

describe("viewSkyLabelV", () => {
  it("states the eye's deepest limit over the sky", () => {
    expect(viewSkyLabelV(heldSky(), "eye", DEFAULT_EXPOSURE, 60)).toBeCloseTo(7.9, 5);
  });

  it("states the darkest texel's limit over all six faces at the exposure shown", () => {
    // The darkest texel, μ 25.25, alone on the −Z face's last row; the rest at μ 21.26 to 24.
    const texels = 6 * FACE * FACE;
    const sky = cameraSky((texel) =>
      texel === texels - 2 ? skyOf(25.25) : skyOf(21.26 + (2.74 * texel) / texels),
    );
    const exposures = [DEFAULT_EXPOSURE, autoAt(0), autoAt(5), autoAt(10), autoAt(15), manualAt(8)];
    const deepestOverTexels = (exposure: ExposureControl): number =>
      Math.min(
        sky.response.cut_v,
        Math.max(
          ...Array.from(sky.band.luminanceCdM2, (luminance) =>
            cameraLimitV(DEFAULT_VIEW_CAMERA, shownTriple(exposure), 60, luminance),
          ),
        ),
      );
    expect(
      exposures.map(
        (exposure) => viewSkyLabelV(sky, "camera", exposure, 60) - deepestOverTexels(exposure),
      ),
    ).toEqual(exposures.map(() => 0));
  });

  it("states a camera's limit at the default MAN exposure, V 10.06 at 60° under μ 24", () => {
    const label = viewSkyLabelV(
      cameraSky(() => MU_24_CD_M2),
      "camera",
      DEFAULT_EXPOSURE,
      60,
    );
    expect(Math.abs(label - 10.06)).toBeLessThan(0.01);
  });

  it("states a camera's limit at AUTO's exposure, V 2.56 at EV100 15", () => {
    const label = viewSkyLabelV(
      cameraSky(() => MU_24_CD_M2),
      "camera",
      autoAt(15),
      60,
    );
    expect(Math.abs(label - 2.56)).toBeLessThan(0.01);
  });

  it.each([
    { reply: "a final reply", overflow: 0 },
    { reply: "a reply that overflows", overflow: 140_000 },
  ])("states at most the cut, V 11.0 at 30° near the Sun, on $reply", ({ overflow }) => {
    // At 30° the camera's limit is V 11.2–11.8 over T17's band, and a camera's sky is cut at 11.0.
    const sky = cameraSky(nearSunBand, { fovDeg: 30, overflow });
    expect([
      sky.response.cut_v,
      Math.round(deepestLimitV(30, skyOf(25.25)) * 100) / 100,
      viewSkyLabelV(sky, "camera", DEFAULT_EXPOSURE, 30),
    ]).toEqual([MAX_CUT_V, 11.79, MAX_CUT_V]);
  });

  it("states an instrument's camera on the eye's sky at the eye's cut, V 9.5 at 60°", () => {
    expect(viewSkyLabelV(heldSky(), "camera", DEFAULT_EXPOSURE, 60)).toBe(9.5);
  });

  it("states one limit for one EV100 at every level: one camera", () => {
    const sky = cameraSky(nearSunBand);
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

/**
 * Texel backgrounds over which the deeper cull is checked: μ 17, T17's two ends near the Sun, the
 * old uniform μ 24, and a black sky; each with the EV100 from which its cull and its shown limit
 * part by 0.05 mag or more.
 */
const BACKGROUNDS = [
  { sky: "μ 17", backgroundCdM2: skyOf(17), onsetEv100: 6 },
  { sky: "μ 21.26", backgroundCdM2: skyOf(21.26), onsetEv100: 4.3 },
  { sky: "μ 24", backgroundCdM2: MU_24_CD_M2, onsetEv100: 3.5 },
  { sky: "μ 25.25", backgroundCdM2: skyOf(25.25), onsetEv100: 3.4 },
  { sky: "a black sky", backgroundCdM2: 0, onsetEv100: 3.3 },
] as const;

/**
 * The least margin, EV, below AgX's floor at which the module's remark puts a centred star at its
 * texel's shown limit over {@link BACKGROUNDS}, at 60°.
 */
const FLOOR_MARGIN_EV = { "1080p": 2.8, "4K": 0.8 } as const;

/**
 * `MAN`'s EV100s at which a sky's label, over a uniform band, is 0.05 mag or more shallower than
 * its cull: where the cull keeps stars a sensor at the exposure shown would not detect.
 */
function shallowerExposures(sky: SkyModel): ReadonlyArray<number> {
  const cullV = viewSkyLimit(sky, "camera", 60).limitAt(0, 0, -1);
  return EV100_SPAN.filter(
    (ev100) => cullV - viewSkyLabelV(sky, "camera", autoAt(ev100), 60) >= 0.05,
  );
}

describe("the deeper cull", () => {
  it.each(BACKGROUNDS)(
    "parts from the label's limit by 0.05 mag or more from EV100 $onsetEv100 up, over $sky",
    ({ backgroundCdM2, onsetEv100 }) => {
      const shallower = shallowerExposures(cameraSky(() => backgroundCdM2));
      expect([shallower[0], shallower.length]).toEqual([
        expect.closeTo(onsetEv100, 9),
        EV100_SPAN.filter((ev100) => ev100 >= onsetEv100 - 0.05).length,
      ]);
    },
  );

  it.each(
    BACKGROUNDS.flatMap(
      (background) =>
        [
          { name: "1080p", widthPx: 1_920, heightPx: 1_080, ...background },
          { name: "4K", widthPx: 3_840, heightPx: 2_160, ...background },
        ] as const,
    ),
  )(
    "draws a star at its texel's shown limit, centred, below AgX's floor by its margin and as black at $name and 60°, over $sky",
    ({ name, widthPx, heightPx, backgroundCdM2 }) => {
      const centreSr = pixelSolidAngle(
        vec3(0, 0, -1),
        { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
        { widthPx, heightPx },
      );
      const sky = cameraSky(() => backgroundCdM2);
      const exposed = shallowerExposures(sky).map(
        (ev100) =>
          pixelLuminance(
            illuminanceLx(viewSkyLabelV(sky, "camera", autoAt(ev100), 60)),
            CENTRED_PEAK_WEIGHT,
            centreSr,
          ) * exposureScale(ev100),
      );
      expect([
        exposed.length > 0,
        exposed.filter((value) => !(Math.log2(value) - AGX_MIN_EV <= -FLOOR_MARGIN_EV[name])),
        exposed.filter((value) => {
          const out = toneCurve([value, value, value]);
          return out.some((channel, at) => channel !== TONE_CURVE_BLACK[at]);
        }),
      ]).toEqual([true, [], []]);
    },
  );

  it.each(BACKGROUNDS)(
    "moves no 8-bit code at a 4K corner for the bluest star at its texel's shown limit, over $sky",
    ({ backgroundCdM2 }) => {
      const tan = Math.tan(Math.PI / 6);
      const cornerSr = pixelSolidAngle(
        vec3(tan, (tan * 9) / 16, -1),
        { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
        { widthPx: 3_840, heightPx: 2_160 },
      );
      // The bluest colour the fit gives with the largest camera band term, A0V's +0.14 (Pickles
      // 1998 through the default sensor, decision-camera-eta), which lets the star in at
      // V = limit − 0.14: a bound no one star reaches, some 0.3 EV above AgX's floor at EV100 3.5
      // under μ 24.
      const [r, g, b] = starColour(PLANCKIAN_FIT_RANGE_K[1]);
      const sky = cameraSky(() => backgroundCdM2);
      const codes = shallowerExposures(sky).flatMap((ev100) => {
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
          255 *
            Math.abs(srgbEncode(pass[at] ?? Number.NaN) - srgbEncode(TONE_CURVE_BLACK[at] ?? 0)),
        ]);
      });
      expect([codes.length > 0, codes.filter((code) => !(code < 0.01))]).toEqual([true, []]);
    },
  );
});
