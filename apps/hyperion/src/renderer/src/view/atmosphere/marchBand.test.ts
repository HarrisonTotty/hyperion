import { beforeAll, describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { CONVERGED } from "../../test/atmosphereGate";
import { quaternionFromRows } from "../camera/quaternion";
import { ev100FromAverageLuminance, exposureScale } from "../photometry/exposure";
import type { Rgb } from "../photometry/toneCurve";
import { tonemapTexel } from "../post/tonemap";
import { QUALITY_SETTINGS, type QualitySetting } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  RAYLEIGH_SCALE_HEIGHT_M,
} from "./earth";
import {
  type AtmosphereCamera,
  type MarchBand,
  marchWeightOf,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
} from "./hillaire";
import {
  type PlanetTwin,
  planetTwin,
  type RayMarchRay,
  rayMarchRayAlongTwin,
  rayMarchTexelTwin,
  skyViewRayTwin,
  skyViewReadTwin,
  skyViewTexelsTwin,
  twinView,
  type TwinView,
} from "./tablesCpu";

/** R05's Earth's top, m: the band's edges are shares of it in h*. */
const TOP_M = R05_EARTH_REFERENCE.topHeightM;

/** A setting's band edges in h* under R05's Earth, m. */
function edgesM(band: MarchBand): { readonly lowerM: number; readonly upperM: number } {
  return { lowerM: band.lowerShareOfTop * TOP_M, upperM: band.upperShareOfTop * TOP_M };
}

/** WGS 84's equatorial radius a (NIMA TR8350.2) as a sphere: the twins' planet, s ≡ 1, h* = h. */
const A_M = 6_378_137;
const SPHERE: SpheroidFigure = { equatorialRadiusM: A_M, polarRadiusM: A_M };
const RAD = Math.PI / 180;

/** The frames' one column of pixels, top to bottom. */
const ROWS = 90;

/** The steps of the converged march, R05.T12.e's placed steps, and of its confirmation. */
const REFERENCE_STEPS = 1_024;

/** Design note 10's floor, 20F: 6 × 10⁻³ of the frame's brightest. */
const FRAME_FLOOR = 6e-3;

/** R05.T12.e's floor: 10⁻³ of the brightest. */
const GATE_FLOOR = 1e-3;

/**
 * A camera `heightM` over the equator on the sphere's +x axis, looking along +y, its one column
 * spanning elevations `fromDeg` to `toDeg`.
 */
function columnCamera(heightM: number, fromDeg: number, toDeg: number): AtmosphereCamera {
  const mid = ((fromDeg + toDeg) / 2) * RAD;
  const half = ((toDeg - fromDeg) / 2) * RAD;
  const forward = { x: Math.sin(mid), y: Math.cos(mid), z: 0 };
  const up = { x: Math.cos(mid), y: -Math.sin(mid), z: 0 };
  const right = { x: 0, y: 0, z: -1 };
  return {
    positionM: { x: A_M + heightM, y: 0, z: 0 },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(Math.tan(half) / ROWS),
    viewport: { widthPx: 1, heightPx: ROWS },
  };
}

/** A sun at an elevation over the camera's horizon and an azimuth from the view's, unit light. */
function sunAtCamera(elevationDeg: number, azimuthDeg: number): SunState {
  const e = elevationDeg * RAD;
  const a = azimuthDeg * RAD;
  return {
    directionBodyFixed: {
      x: Math.sin(e),
      y: Math.cos(e) * Math.cos(a),
      z: -Math.cos(e) * Math.sin(a),
    },
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.004_65,
  };
}

/** The frames' suns, elevation and azimuth: high and ahead, high across, low behind. */
const FRAME_SUNS = [
  [30, 0],
  [30, 90],
  [5, 180],
] as const;

/** The visible horizon's dip from a camera `heightM` up, degrees. */
const dipDeg = (heightM: number): number => Math.acos(A_M / (A_M + heightM)) / RAD;

/** The direction of a column's row, as `tablesCpu.ts`'s texel rays take it at u = ½. */
function rowDirection(view: TwinView, row: number): Vec3 {
  const ny = 1 - 2 * ((row + 0.5) / ROWS);
  return normalise(add(view.forward, scale(view.upRay, ny)));
}

/** The march's RGB of a texel's twin. */
const rgbOf = (texel: readonly [number, number, number, number]): Rgb => [
  texel[0],
  texel[1],
  texel[2],
];

/** The march of a sky pixel at `samples` of R05.T12.e's placed steps: the reference's. */
function placedMarch(
  planet: PlanetTwin,
  view: TwinView,
  marched: RayMarchRay,
  samples: number,
): Rgb {
  const ray = { ...marched.ray, fromCamera: marched.ray.tStartM <= 0, skySegment: false };
  const settings = { ...TABLE_SIZES.high, rayMarchSamples: samples };
  return rgbOf(rayMarchTexelTwin(planet, view, settings, { ...marched, ray }));
}

/** One pixel of a frame: the sky view's read, the march at the setting's steps, and the references. */
interface FramePixel {
  readonly read: Rgb;
  readonly march: Rgb;
  readonly reference: Rgb;
  readonly confirm: Rgb;
}

/**
 * The frame through the visible horizon from `heightM` up (its dip − 0.2° to dip + 4.5°), its rows
 * that miss the ground: the sky view's read of the setting's table, the ray march at the setting's
 * steps under the kernel's rule (`rayMarchRayAlongTwin`, every two-sided sky segment in even steps),
 * and a march of {@link REFERENCE_STEPS} of R05.T12.e's placed steps, confirmed against twice as many
 * where `confirmed` (otherwise its confirmation is itself).
 */
function frameOf(
  planet: PlanetTwin,
  setting: QualitySetting,
  heightM: number,
  sun: SunState,
  confirmed: boolean,
): FramePixel[] {
  const sizes = TABLE_SIZES[setting];
  const dip = dipDeg(heightM);
  const view = twinView(columnCamera(heightM, -dip - 0.2, -dip + 4.5), sun, SPHERE, 0.1);
  const texelAt = skyViewTexelsTwin(planet, view, sizes.skyView, sizes.skyViewSamples);
  const pixels: FramePixel[] = [];
  for (let row = 0; row < ROWS; row += 1) {
    const d = rowDirection(view, row);
    const marched = rayMarchRayAlongTwin(
      planet.medium,
      view,
      view.cameraM,
      d,
      Number.POSITIVE_INFINITY,
    );
    if (marched === null || marched.bareGroundM !== null) {
      continue;
    }
    const reference = placedMarch(planet, view, marched, REFERENCE_STEPS);
    pixels.push({
      read: skyViewReadTwin(texelAt, sizes.skyView, planet.medium, view, d),
      march: rgbOf(rayMarchTexelTwin(planet, view, sizes, marched)),
      reference,
      confirm: confirmed ? placedMarch(planet, view, marched, 2 * REFERENCE_STEPS) : reference,
    });
  }
  return pixels;
}

/**
 * e = max over pixels and channels of |L − L_ref| ÷ max(L_ref, floor · L_max), L_max the brightest
 * of `against` per channel (Design note 10's metric).
 */
function worstOver(
  pixels: readonly FramePixel[],
  of: (p: FramePixel) => Rgb,
  against: (p: FramePixel) => Rgb,
  floor: number,
): number {
  const lMax = [0, 1, 2].map((c) => Math.max(...pixels.map((p) => against(p)[c] ?? 0)));
  let e = 0;
  for (const p of pixels) {
    const l = of(p);
    const r = against(p);
    for (const c of [0, 1, 2] as const) {
      e = Math.max(e, Math.abs(l[c] - r[c]) / Math.max(r[c], floor * (lMax[c] ?? 0)));
    }
  }
  return e;
}

/** The composite's sky in the band: (1 − w) times the sky view's read plus w times the march. */
function blendOf(p: FramePixel, w: number): Rgb {
  return [
    (1 - w) * p.read[0] + w * p.march[0],
    (1 - w) * p.read[1] + w * p.march[1],
    (1 - w) * p.read[2] + w * p.march[2],
  ];
}

/**
 * The worst e over the three suns of the sky view's texels from a camera `heightM` up against the
 * march along each texel's direction at the setting's steps (R05.T12.e's floor, of the marches'
 * brightest), over columns 0, ¼ and ½ of the table's width and every row whose ray misses the
 * ground.
 */
function texelsAgainstMarch(planet: PlanetTwin, setting: QualitySetting, heightM: number): number {
  const sizes = TABLE_SIZES[setting];
  return Math.max(
    ...FRAME_SUNS.map(([elevationDeg, azimuthDeg]) => {
      const view = twinView(
        columnCamera(heightM, -1, 1),
        sunAtCamera(elevationDeg, azimuthDeg),
        SPHERE,
        0.1,
      );
      const texelAt = skyViewTexelsTwin(planet, view, sizes.skyView, sizes.skyViewSamples);
      const n = view.normal;
      const x = view.skyAxis;
      const y = cross(n, x);
      const width = sizes.skyView.widthTexels;
      const pairs: FramePixel[] = [];
      for (const column of [0, Math.floor(width / 4), Math.floor(width / 2)]) {
        for (let row = 0; row < sizes.skyView.heightTexels; row += 1) {
          const local = skyViewRayTwin(planet.medium, view, column, row, sizes.skyView).direction;
          const d = add(add(scale(x, local.x), scale(y, local.y)), scale(n, local.z));
          const marched = rayMarchRayAlongTwin(
            planet.medium,
            view,
            view.cameraM,
            d,
            Number.POSITIVE_INFINITY,
          );
          if (marched === null || marched.bareGroundM !== null) {
            continue;
          }
          const march = rgbOf(rayMarchTexelTwin(planet, view, sizes, marched));
          pairs.push({ read: texelAt(column, row), march, reference: march, confirm: march });
        }
      }
      return worstOver(
        pairs,
        (p) => p.read,
        (p) => p.march,
        GATE_FLOOR,
      );
    }),
  );
}

/** Each figure as recorded: within 5 × 10⁻⁵ (0.005 percentage points) of its value. */
const asRecorded = (figures: readonly number[]): unknown[] =>
  figures.map((figure) => expect.closeTo(figure, 4));

describe("marchWeightOf (R08.T8)", () => {
  it.each(QUALITY_SETTINGS)(
    "leaves the frame to the tables below %s's band, up to its lower edge",
    (setting) => {
      const band = TABLE_SIZES[setting].marchBand;
      const { lowerM } = edgesM(band);
      // Below the datum (the Dead Sea's shore), at it, 2 m up, and up to the lower edge.
      for (const heightStarM of [-430, 0, 2, 10_000, lowerM - 1, lowerM]) {
        expect(marchWeightOf(heightStarM, TOP_M, band)).toBe(0);
      }
      expect(marchWeightOf(Number.NEGATIVE_INFINITY, TOP_M, band)).toBe(0);
    },
  );

  it.each(QUALITY_SETTINGS)(
    "gives %s's frame to the march from its band's upper edge up, the top and above it included",
    (setting) => {
      const band = TABLE_SIZES[setting].marchBand;
      const { upperM } = edgesM(band);
      for (const heightStarM of [upperM, upperM + 1, TOP_M - 1, TOP_M, TOP_M + 1, 400_000, 1e9]) {
        expect(marchWeightOf(heightStarM, TOP_M, band)).toBe(1);
      }
      expect(marchWeightOf(Number.POSITIVE_INFINITY, TOP_M, band)).toBe(1);
    },
  );

  it.each(QUALITY_SETTINGS)(
    "blends the two linearly in h* across %s's band, half and half at its middle",
    (setting) => {
      const band = TABLE_SIZES[setting].marchBand;
      const { lowerM, upperM } = edgesM(band);
      for (const share of [0.01, 0.25, 0.5, 0.75, 0.99]) {
        expect(marchWeightOf(lowerM + share * (upperM - lowerM), TOP_M, band)).toBeCloseTo(
          share,
          12,
        );
      }
    },
  );

  it("takes the band as a share of each medium's own top", () => {
    const band = TABLE_SIZES.high.marchBand;
    // A 200 km top puts high's band at 60–100 km, where R05's Earth's 100 km top puts it at 30–50.
    expect(marchWeightOf(60_000, 200_000, band)).toBe(0);
    expect(marchWeightOf(80_000, 200_000, band)).toBeCloseTo(0.5, 12);
    expect(marchWeightOf(100_000, 200_000, band)).toBe(1);
    expect(marchWeightOf(40_000, TOP_M, band)).toBeCloseTo(0.5, 12);
  });

  it("refuses a band whose edges are not 0 ≤ lower < upper ≤ 1", () => {
    const bands: ReadonlyArray<MarchBand> = [
      { lowerShareOfTop: 0.5, upperShareOfTop: 0.5 },
      { lowerShareOfTop: 0.6, upperShareOfTop: 0.5 },
      { lowerShareOfTop: -0.1, upperShareOfTop: 0.5 },
      { lowerShareOfTop: 0.3, upperShareOfTop: 1.1 },
      { lowerShareOfTop: Number.NaN, upperShareOfTop: 0.5 },
      { lowerShareOfTop: 0.3, upperShareOfTop: Number.NaN },
    ];
    for (const band of bands) {
      expect(() => marchWeightOf(40_000, TOP_M, band)).toThrow(RangeError);
    }
  });

  it("refuses a top that is not finite and above 0", () => {
    const band = TABLE_SIZES.high.marchBand;
    for (const topHeightM of [0, -100_000, Number.POSITIVE_INFINITY, Number.NaN]) {
      expect(() => marchWeightOf(40_000, topHeightM, band)).toThrow(RangeError);
    }
  });

  it("refuses a camera height that is not a number", () => {
    expect(() => marchWeightOf(Number.NaN, TOP_M, TABLE_SIZES.high.marchBand)).toThrow(RangeError);
  });
});

/** R05's Earth's twin on the sphere, built once for the twins' tests below. */
let planet: PlanetTwin | undefined;

/** The planet's twin, which the file's `beforeAll` builds. */
function earthTwin(): PlanetTwin {
  if (planet === undefined) {
    throw new Error("the planet's twin is built before the tests read it");
  }
  return planet;
}

beforeAll(() => {
  planet = planetTwin(R05_EARTH_REFERENCE, A_M);
}, 60_000);

/** The cameras' heights of the frames, m: R08.T8's 20–99.999 km, and every band's edges and middles. */
const FRAME_HEIGHTS_M: Readonly<Record<QualitySetting, readonly number[]>> = {
  high: [20_000, 30_000, 35_000, 40_000, 45_000, 50_000, 60_000, 80_000, TOP_M - 1],
  low: [20_000, 25_000, 30_000, 40_000, 50_000, 60_000, 80_000, TOP_M - 1],
};

/** The cameras of R08.T8's interpolation test: 20, 30, 40, 50, 60, 80 and 99.999 km. */
const LIMB_CAMERAS_M = [20_000, 30_000, 40_000, 50_000, 60_000, 80_000, TOP_M - 1] as const;

describe("the sky view against the ray march, from 20 km to the top (R08.T8)", () => {
  /** Each frame, by setting and camera height, one per sun. */
  const frames = new Map<string, FramePixel[][]>();
  const framesAt = (setting: QualitySetting, heightM: number): FramePixel[][] => {
    const found = frames.get(`${setting} ${heightM}`);
    if (found === undefined) {
      throw new Error(`no frame from ${heightM} m on ${setting}`);
    }
    return found;
  };
  /** The worst e over a camera's three suns. */
  const worstAt = (
    setting: QualitySetting,
    heightM: number,
    of: (p: FramePixel) => Rgb,
    against: (p: FramePixel) => Rgb,
    floor: number,
  ): number => Math.max(...framesAt(setting, heightM).map((p) => worstOver(p, of, against, floor)));

  beforeAll(() => {
    for (const setting of QUALITY_SETTINGS) {
      for (const heightM of FRAME_HEIGHTS_M[setting]) {
        frames.set(
          `${setting} ${heightM}`,
          FRAME_SUNS.map(([e, a]) =>
            frameOf(earthTwin(), setting, heightM, sunAtCamera(e, a), true),
          ),
        );
      }
    }
  }, 300_000);

  it("has a converged reference: 1,024 placed steps within 0.05% of 2,048, on every frame", () => {
    const unconverged = QUALITY_SETTINGS.flatMap((setting) =>
      FRAME_HEIGHTS_M[setting].filter(
        (heightM) =>
          !(
            worstAt(
              setting,
              heightM,
              (p) => p.reference,
              (p) => p.confirm,
              FRAME_FLOOR,
            ) < CONVERGED
          ),
      ),
    );
    expect(unconverged).toEqual([]);
  });

  // `decision-r08-t8-leans.md` §3: T8's "1% at the band's edges" is a recorded figure, not a cap.
  // Measured 2026-10-21 under the march's sun-aware warp (§5.4) and recorded in plan R08's Risks
  // ("Deviations in T8.a, as built"), the worst over the three suns: on high at 30 and 50 km
  // 0.334% and 0.430% (0.367% in §4's even steps); on low at 20 and 30 km 3.486% and 3.573% (low's
  // 16 sky-view steps).
  it("records the sky view's texels against the march along each one's direction at the band's edges", () => {
    const figures = QUALITY_SETTINGS.flatMap((setting) => {
      const { lowerM, upperM } = edgesM(TABLE_SIZES[setting].marchBand);
      return [lowerM, upperM].map((heightM) => texelsAgainstMarch(earthTwin(), setting, heightM));
    });
    expect(figures).toEqual(asRecorded([0.00334, 0.0043, 0.03486, 0.03573]));
  });

  // The same frames' read against the march at its pixels, at Design note 10's floor: on high at 30
  // and 50 km 2.352% and 4.285%, on low at 20 and 30 km 4.114% and 5.805% (measured 2026-10-21
  // under the sun-aware warp; 4.279% and 5.664% in §4's even steps).
  it("records the sky view's read against the march at the same pixels at the band's edges", () => {
    const figures = QUALITY_SETTINGS.flatMap((setting) => {
      const { lowerM, upperM } = edgesM(TABLE_SIZES[setting].marchBand);
      return [lowerM, upperM].map((heightM) =>
        worstAt(
          setting,
          heightM,
          (p) => p.read,
          (p) => p.march,
          FRAME_FLOOR,
        ),
      );
    });
    expect(figures).toEqual(asRecorded([0.02352, 0.04285, 0.04114, 0.05805]));
  });

  // R08.T8's interpolation across the limb, recorded with the band's altitudes (measured 2026-10-21,
  // plan R08's Risks): from 20, 30, 40, 50, 60, 80 and 99.999 km, the read and the march, each
  // against the converged march at Design note 10's floor. The read errs more the higher the camera,
  // past the 5% from 50 km on high and 30 km on low, where the band hands the frame to the march.
  // Under the sun-aware warp the march's worst is its frame with the Sun 5° up behind the camera,
  // whose rays toward the visible horizon take R05's placement from the start (a = 1) along a
  // daylit chord: on high from 80 km 1.78% and from 99.999 km 2.40% (0.76% and 1.55% in §4's even
  // steps), on low from 80 km 11.55% (3.76%).
  it("records the read and the march against the converged march from 20 km to 1 m under the top, on high", () => {
    const read = LIMB_CAMERAS_M.map((h) =>
      worstAt(
        "high",
        h,
        (p) => p.read,
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    const march = LIMB_CAMERAS_M.map((h) =>
      worstAt(
        "high",
        h,
        (p) => p.march,
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect({ read, march }).toEqual({
      read: asRecorded([0.02499, 0.02308, 0.02946, 0.04299, 0.06206, 0.10624, 0.15796]),
      march: asRecorded([0.00447, 0.00521, 0.00521, 0.00594, 0.00653, 0.0178, 0.02395]),
    });
  });

  it("records the same on low, at its 16 and 16 steps", () => {
    const read = LIMB_CAMERAS_M.map((h) =>
      worstAt(
        "low",
        h,
        (p) => p.read,
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    const march = LIMB_CAMERAS_M.map((h) =>
      worstAt(
        "low",
        h,
        (p) => p.march,
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect({ read, march }).toEqual({
      read: asRecorded([0.04262, 0.05729, 0.08866, 0.10888, 0.18167, 0.33024, 0.51684]),
      march: asRecorded([0.02116, 0.03033, 0.0222, 0.03511, 0.06046, 0.11554, 0.10831]),
    });
  });

  // The band's blocker check: the composite's sky, (1 − w) · read + w · march with w = marchWeightOf,
  // through high's band.
  it("holds the composite's blend through high's band within Design note 10's 5% of the converged march", () => {
    const band = TABLE_SIZES.high.marchBand;
    const over = [30_000, 35_000, 40_000, 45_000, 50_000].filter(
      (h) =>
        !(
          worstAt(
            "high",
            h,
            (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
            (p) => p.reference,
            FRAME_FLOOR,
          ) <= 0.05
        ),
    );
    expect(over).toEqual([]);
  });

  // Measured 2026-10-21 under the sun-aware warp at 30, 35, 40, 45 and 50 km; in §4's even steps
  // 2.308, 2.160, 1.473, 0.960 and 0.403%.
  it("records the composite's blend through high's band", () => {
    const band = TABLE_SIZES.high.marchBand;
    const figures = [30_000, 35_000, 40_000, 45_000, 50_000].map((h) =>
      worstAt(
        "high",
        h,
        (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect(figures).toEqual(asRecorded([0.02308, 0.02158, 0.01462, 0.00947, 0.00594]));
  });

  // Low's bound is R05's low rule, not 5%: recorded, at 20, 25 and 30 km (measured 2026-10-21 under
  // the sun-aware warp; 4.262, 2.465 and 3.487% in §4's even steps).
  it("records the composite's blend through low's band", () => {
    const band = TABLE_SIZES.low.marchBand;
    const figures = [20_000, 25_000, 30_000].map((h) =>
      worstAt(
        "low",
        h,
        (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect(figures).toEqual(asRecorded([0.04262, 0.02453, 0.03033]));
  });
});

describe("the sky view against the ray march on the drawn Earth (R08.T8)", () => {
  /** Each frame on `earth.ts`'s `EARTH_REFERENCE`, by setting and camera height, one per sun. */
  const frames = new Map<string, FramePixel[][]>();
  const worstAt = (
    setting: QualitySetting,
    heightM: number,
    of: (p: FramePixel) => Rgb,
    against: (p: FramePixel) => Rgb,
    floor: number,
  ): number => {
    const found = frames.get(`${setting} ${heightM}`);
    if (found === undefined) {
      throw new Error(`no frame from ${heightM} m on ${setting}`);
    }
    return Math.max(...found.map((p) => worstOver(p, of, against, floor)));
  };

  beforeAll(() => {
    const drawn = planetTwin(EARTH_REFERENCE, A_M, EARTH_REFERENCE_CURVES);
    for (const setting of QUALITY_SETTINGS) {
      for (const heightM of FRAME_HEIGHTS_M[setting]) {
        frames.set(
          `${setting} ${heightM}`,
          FRAME_SUNS.map(([e, a]) => frameOf(drawn, setting, heightM, sunAtCamera(e, a), false)),
        );
      }
    }
  }, 300_000);

  // The drawn Earth (its column's top, 104.6 km, cut at 100 km; its scale height above 12.6 km about
  // 6.3 km, 6.31 km at 13–20 km and 6.39 km at 40–80 km), measured 2026-10-21 and recorded in plan
  // R08's Risks ("Deviations in T8.a, as built"),
  // as R05's Earth's above. Its references' 1,024 steps stood within 2.9 × 10⁻⁵ of 2,048 over every
  // frame, which the test does not repeat. The read errs more than on R05's Earth: past 5% from
  // 40 km on high and from 20 km on low. Under the sun-aware warp the march from 80 km errs most,
  // with the Sun 5° up behind the camera: 1.31% on high and 9.95% on low (0.71% and 4.76% in §4's
  // even steps).
  it("records the read and the march against the converged march from 20 km to 1 m under the top, on high and low", () => {
    const of = (setting: QualitySetting, pick: (p: FramePixel) => Rgb): number[] =>
      LIMB_CAMERAS_M.map((h) => worstAt(setting, h, pick, (p) => p.reference, FRAME_FLOOR));
    expect({
      highRead: of("high", (p) => p.read),
      highMarch: of("high", (p) => p.march),
      lowRead: of("low", (p) => p.read),
      lowMarch: of("low", (p) => p.march),
    }).toEqual({
      highRead: asRecorded([0.02029, 0.0371, 0.05009, 0.07767, 0.11153, 0.18793, 0.25972]),
      highMarch: asRecorded([0.0035, 0.00354, 0.00473, 0.00491, 0.0062, 0.01312, 0.01034]),
      lowRead: asRecorded([0.05076, 0.09497, 0.13995, 0.23507, 0.34518, 0.61041, 0.88936]),
      lowMarch: asRecorded([0.02301, 0.02121, 0.01845, 0.02701, 0.03799, 0.09946, 0.09222]),
    });
  });

  it("holds the composite's blend through high's band within Design note 10's 5% of the converged march", () => {
    const band = TABLE_SIZES.high.marchBand;
    const over = [30_000, 35_000, 40_000, 45_000, 50_000].filter(
      (h) =>
        !(
          worstAt(
            "high",
            h,
            (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
            (p) => p.reference,
            FRAME_FLOOR,
          ) <= 0.05
        ),
    );
    expect(over).toEqual([]);
  });

  // Measured 2026-10-21 under the sun-aware warp at 30, 35, 40, 45 and 50 km; in §4's even steps
  // 3.710, 2.812, 2.479, 1.556 and 0.531%.
  it("records the composite's blend through high's band", () => {
    const band = TABLE_SIZES.high.marchBand;
    const figures = [30_000, 35_000, 40_000, 45_000, 50_000].map((h) =>
      worstAt(
        "high",
        h,
        (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect(figures).toEqual(asRecorded([0.0371, 0.02805, 0.02491, 0.01553, 0.00491]));
  });

  // In §4's even steps 5.076, 4.421 and 2.267%.
  it("records the composite's blend through low's band", () => {
    const band = TABLE_SIZES.low.marchBand;
    const figures = [20_000, 25_000, 30_000].map((h) =>
      worstAt(
        "low",
        h,
        (p) => blendOf(p, marchWeightOf(h, TOP_M, band)),
        (p) => p.reference,
        FRAME_FLOOR,
      ),
    );
    expect(figures).toEqual(asRecorded([0.05076, 0.04485, 0.02121]));
  });
});

/** The top's frame's suns: R08.T8's three, a low sun ahead, one just set, and two set behind. */
const TOP_SUNS = [...FRAME_SUNS, [5, 0], [-3, 0], [0, 180], [-5, 180]] as const;

/**
 * Each row of the frame through the horizon of the top, from `heightM` up, the ray march at the
 * setting's steps under the kernel's rule, every row (the bare ground's included).
 */
function topFrameOf(setting: QualitySetting, heightM: number, sun: SunState): Rgb[] {
  const dip = dipDeg(TOP_M);
  const view = twinView(columnCamera(heightM, -dip - 0.2, -dip + 4.5), sun, SPHERE, 0.1);
  return Array.from({ length: ROWS }, (_, row) =>
    rgbOf(
      rayMarchTexelTwin(
        earthTwin(),
        view,
        TABLE_SIZES[setting],
        rayMarchRayAlongTwin(
          R05_EARTH_REFERENCE,
          view,
          view.cameraM,
          rowDirection(view, row),
          Number.POSITIVE_INFINITY,
        ),
      ),
    ),
  );
}

/** The limb's tangent heights from 10 km up, and R08's limb set below it, m. */
const LIMB_TANGENTS_M = [
  ...[0.1, 0.3, 1, 3, 10].map((k) => k * RAYLEIGH_SCALE_HEIGHT_M),
  ...Array.from({ length: 18 }, (_, i) => 10_000 + 5_000 * i),
  TOP_M - 1_000,
  TOP_M - 1,
].toSorted((a, b) => a - b);

/** A camera's ray to a tangent point `tangentM` up, in the plane of the camera's +x and +y. */
function rayToTangent(cameraM: Vec3, tangentM: number): Vec3 {
  const alpha = Math.asin(
    Math.min((A_M + tangentM) / Math.hypot(cameraM.x, cameraM.y, cameraM.z), 1),
  );
  return { x: -Math.cos(alpha), y: Math.sin(alpha), z: 0 };
}

/**
 * The view from a camera `heightM` over the equator with the sun 30° from the zenith at its 0.1 H
 * tangent point, across the chord: the limb lit by day.
 */
function limbView(heightM: number): TwinView {
  const camera = columnCamera(heightM, -10, 10);
  const o = camera.positionM;
  const d = rayToTangent(o, 0.1 * RAYLEIGH_SCALE_HEIGHT_M);
  const up = normalise(add(o, scale(d, -dot(o, d))));
  const across = cross(up, d);
  const sun: SunState = {
    directionBodyFixed: add(scale(up, Math.cos(30 * RAD)), scale(across, Math.sin(30 * RAD))),
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.004_65,
  };
  return twinView(camera, sun, SPHERE, 0.1);
}

/** The limb cameras' heights: R05.T12.e's orbit, and 1 km over the top. */
const ORBIT_CAMERAS_M = [400_000, TOP_M + 1_000] as const;

/** The limb's texels from a camera `cameraM` up at {@link LIMB_TANGENTS_M}, at the setting's steps. */
function limbTexels(setting: QualitySetting, cameraM: number): Rgb[] {
  const view = limbView(cameraM);
  return LIMB_TANGENTS_M.map((tangentM) =>
    rgbOf(
      rayMarchTexelTwin(
        earthTwin(),
        view,
        TABLE_SIZES[setting],
        rayMarchRayAlongTwin(
          R05_EARTH_REFERENCE,
          view,
          view.cameraM,
          rayToTangent(view.cameraM, tangentM),
          Number.POSITIVE_INFINITY,
        ),
      ),
    ),
  );
}

/** The step across the top on a setting for each of {@link TOP_SUNS}: the march 1 m under it against 1 m over it. */
function topSteps(setting: QualitySetting): number[] {
  return TOP_SUNS.map(([elevationDeg, azimuthDeg]) => {
    const sun = sunAtCamera(elevationDeg, azimuthDeg);
    const under = topFrameOf(setting, TOP_M - 1, sun);
    const over = topFrameOf(setting, TOP_M + 1, sun);
    const pixels = under.map((read, i) => {
      const march = over[i] ?? read;
      return { read, march, reference: march, confirm: march };
    });
    return worstOver(
      pixels,
      (p) => p.read,
      (p) => p.march,
      FRAME_FLOOR,
    );
  });
}

describe("the ray march across the top, and the limb outside it (R08.T8)", () => {
  /** Each setting's steps across the top, by sun. */
  const steps = new Map<QualitySetting, number[]>();
  const stepsOn = (setting: QualitySetting): number[] => {
    const found = steps.get(setting);
    if (found === undefined) {
      throw new Error(`no steps across the top on ${setting}`);
    }
    return found;
  };

  beforeAll(() => {
    for (const setting of QUALITY_SETTINGS) {
      steps.set(setting, topSteps(setting));
    }
  }, 120_000);

  // With one rule on both sides of the top, every two-sided sky segment under the sun-aware warp
  // (`decision-r08-t8-leans.md` §4.1 and §5.4).
  it.each(QUALITY_SETTINGS)(
    "steps by under 0.2% across the top on %s: the march 1 m under it against 1 m over it",
    (setting) => {
      expect(stepsOn(setting).filter((step) => !(step < 0.002))).toEqual([]);
    },
  );

  // Measured 2026-10-21 under the sun-aware warp, for the suns in order, the last two set behind
  // the camera, each within 5 × 10⁻⁷; under §4's even steps they were 0.0244, 0.0247, 0.0238,
  // 0.0529, 0.0559, 0.0207 and 0.0260% on high, and 0.0250, 0.0248, 0.0238, 0.0543, 0.0571, 0.0415
  // and 0.0547% on low.
  it("records the step across the top by sun, on both settings", () => {
    expect({ high: stepsOn("high"), low: stepsOn("low") }).toEqual({
      high: [2.44e-4, 2.48e-4, 2.384e-4, 5.288e-4, 5.588e-4, 2.056e-4, 2.603e-4].map((step) =>
        expect.closeTo(step, 6),
      ),
      low: [2.499e-4, 2.476e-4, 2.383e-4, 5.428e-4, 5.715e-4, 4.152e-4, 5.474e-4].map((step) =>
        expect.closeTo(step, 6),
      ),
    });
  });

  it.each(QUALITY_SETTINGS)(
    "dims the limb in every channel as its tangent point climbs from 10 km to 1 m under the top, on %s",
    (setting) => {
      for (const cameraM of ORBIT_CAMERAS_M) {
        const texels = limbTexels(setting, cameraM);
        const rising = LIMB_TANGENTS_M.flatMap((tangentM, i) => {
          const above = texels[i + 1];
          const here = texels[i];
          if (tangentM < 10_000 || above === undefined || here === undefined) {
            return [];
          }
          return [0, 1, 2].some((c) => !((above[c] ?? 0) < (here[c] ?? 0))) ? [tangentM] : [];
        });
        expect(rising).toEqual([]);
      }
    },
  );

  it("draws nothing along a ray whose closest approach is above the top: no ray, no light, transmittance 1", () => {
    for (const cameraM of ORBIT_CAMERAS_M) {
      const view = limbView(cameraM);
      for (const tangentM of [TOP_M + 1, TOP_M + 1_000]) {
        const d = rayToTangent(view.cameraM, tangentM);
        const marched = rayMarchRayAlongTwin(
          R05_EARTH_REFERENCE,
          view,
          view.cameraM,
          d,
          Number.POSITIVE_INFINITY,
        );
        expect(marched).toBeNull();
        expect(rayMarchTexelTwin(earthTwin(), view, TABLE_SIZES.high, marched)).toEqual([
          0, 0, 0, 1,
        ]);
      }
    }
  });

  // Recorded, not capped (measured 2026-10-21): 1 m under the top the chord crosses 7.2 km of air at
  // e^(−11.86) of the ground's Rayleigh density, R05's Earth's own, where a 4,096-step march gives
  // the same; the brief's 10⁻⁶ held red and green but not blue. From 400 km and from 101 km, red,
  // green and blue over each channel's brightest on the limb.
  it("records the limb's light 1 m under the top at about 10⁻⁶ of its brightest", () => {
    const ratios = QUALITY_SETTINGS.flatMap((setting) =>
      ORBIT_CAMERAS_M.flatMap((cameraM) => {
        const texels = limbTexels(setting, cameraM);
        const last = texels.at(-1) ?? [0, 0, 0];
        return [0, 1, 2].map(
          (c) => (last[c] ?? 0) / Math.max(...texels.map((texel) => texel[c] ?? 0)),
        );
      }),
    );
    const recorded = [
      3.081e-7, 7.537e-7, 1.213e-6, 3.12e-7, 7.63e-7, 1.228e-6, 3.084e-7, 7.504e-7, 1.214e-6,
      3.122e-7, 7.597e-7, 1.228e-6,
    ];
    expect(ratios.map((ratio, i) => ratio / (recorded[i] ?? Number.NaN))).toEqual(
      recorded.map(() => expect.closeTo(1, 2)),
    );
  });

  // The medium ends at a hard top (`decision-r08-t8-leans.md` §7, plan R08's Risks, "The medium
  // ends at a hard top"): the horizontal ray from Δ under it crosses only the air between the
  // camera and the top, a chord of √(2 (R + h) Δ) each way, so its light shrinks as √(top − h) to
  // none at the top, and from over the top the ray misses the shell. The ascent's suns
  // (`smoke/atmosphere.ts`'s `captureAscent`): by day 20° up at 90° from the view, and at the
  // camera's last light 0.5° over its own visible horizon at 150°.
  const glowSunsAt = (heightM: number): readonly SunState[] => [
    sunAtCamera(20, 90),
    sunAtCamera(0.5 - dipDeg(heightM), 150),
  ];
  /** The horizontal ray's light from `heightM` on high, unit light; `null` where it misses. */
  const glowAt = (heightM: number, sun: SunState): Rgb | null => {
    const view = twinView(columnCamera(heightM, -1, 1), sun, SPHERE, 0.1);
    const marched = rayMarchRayAlongTwin(
      R05_EARTH_REFERENCE,
      view,
      view.cameraM,
      atElevation(0),
      Number.POSITIVE_INFINITY,
    );
    return marched === null
      ? null
      : rgbOf(rayMarchTexelTwin(earthTwin(), view, TABLE_SIZES.high, marched));
  };
  const noGlow: Rgb = [Number.NaN, Number.NaN, Number.NaN];

  it("misses the shell from just over the top", () => {
    expect(glowSunsAt(TOP_M + 1).map((sun) => glowAt(TOP_M + 1, sun))).toEqual([null, null]);
  });

  // Recorded, not capped (measured 2026-10-22), on high, unit light, per channel.
  it("records the glow from just under the top", () => {
    // Day then last light at 1, 10 and 100 m under the top, red, green and blue.
    const values = [1, 10, 100].flatMap((underM) =>
      glowSunsAt(TOP_M - underM).flatMap((sun) => glowAt(TOP_M - underM, sun) ?? noGlow),
    );
    const recorded = [
      8.301e-9, 2e-8, 5.323e-8, 3.711e-9, 1.399e-9, 3.952e-10, 2.627e-8, 6.328e-8, 1.684e-7,
      1.106e-8, 3.889e-9, 9.758e-10, 8.367e-8, 2.016e-7, 5.365e-7, 2.715e-8, 7.723e-9, 1.862e-9,
    ];
    expect(values.map((value, i) => value / (recorded[i] ?? Number.NaN))).toEqual(
      recorded.map(() => expect.closeTo(1, 2)),
    );
  });

  // By day the light from 100 m under the top should be about √100 = 10 times that from 1 m
  // under the top. Measured 2026-10-22: 10.08 in every channel, the √(top − h) law to 1%. At the
  // last light the Sun sets lower the higher the camera, and the ratio falls short (7.3, 5.5 and
  // 4.7).
  it("falls as √(top − h) toward the top by day", () => {
    const day = (underM: number): Rgb => glowAt(TOP_M - underM, sunAtCamera(20, 90)) ?? noGlow;
    const ratios = [0, 1, 2].map((c) => (day(100)[c] ?? Number.NaN) / (day(1)[c] ?? Number.NaN));
    expect(ratios).toEqual([10.08, 10.08, 10.08].map((ratio) => expect.closeTo(ratio, 1)));
  });
});

/** The horizontal's cameras, m: in and above each setting's band. */
const HORIZON_CAMERAS_M: Readonly<Record<QualitySetting, readonly number[]>> = {
  high: [30_000, 40_000, 50_000, TOP_M - 1],
  low: [20_000, 25_000, 30_000],
};

/** The direction at an elevation over the horizontal of a {@link columnCamera}'s view, along +y. */
function atElevation(elevationRad: number): Vec3 {
  return { x: Math.sin(elevationRad), y: Math.cos(elevationRad), z: 0 };
}

/** The march's step across the horizontal, and the quadrature's own part of it. */
interface HorizonStep {
  /** |L(+δ) − L(−δ)| over the floor's denominator: what a viewer sees, the sky's own change in it. */
  readonly march: number;
  /** |(L − L_ref)(+δ) − (L − L_ref)(−δ)| over the same: the step the rule's change makes. */
  readonly quadrature: number;
}

/**
 * The march's step across the horizontal from a camera `heightM` up: the rays at elevations
 * +`deltaRad` (one-sided, R05.T12.e's placement toward the camera) and −`deltaRad` (two-sided, the
 * sun-aware warp) at the setting's steps, each against the converged march along it, over the converged march
 * at −`deltaRad` or Design note 10's floor of the brightest march of the frame from the visible
 * horizon's dip − 0.2° to 5° above the horizontal.
 */
function horizonStep(
  setting: QualitySetting,
  heightM: number,
  sun: SunState,
  deltaRad: number,
): HorizonStep {
  const sizes = TABLE_SIZES[setting];
  const view = twinView(columnCamera(heightM, -dipDeg(heightM) - 0.2, 5), sun, SPHERE, 0.1);
  const along = (d: Vec3): RayMarchRay => {
    const marched = rayMarchRayAlongTwin(
      R05_EARTH_REFERENCE,
      view,
      view.cameraM,
      d,
      Number.POSITIVE_INFINITY,
    );
    if (marched === null) {
      throw new Error("a ray from inside the air meets the air");
    }
    return marched;
  };
  const lMax: [number, number, number] = [0, 0, 0];
  for (let row = 0; row < ROWS; row += 1) {
    const marched = along(rowDirection(view, row));
    if (marched.bareGroundM === null) {
      const l = rgbOf(rayMarchTexelTwin(earthTwin(), view, sizes, marched));
      for (const c of [0, 1, 2] as const) {
        lMax[c] = Math.max(lMax[c], l[c]);
      }
    }
  }
  const above = along(atElevation(deltaRad));
  const below = along(atElevation(-deltaRad));
  const lAbove = rgbOf(rayMarchTexelTwin(earthTwin(), view, sizes, above));
  const lBelow = rgbOf(rayMarchTexelTwin(earthTwin(), view, sizes, below));
  const reference = placedMarch(earthTwin(), view, below, REFERENCE_STEPS);
  const referenceAbove = placedMarch(earthTwin(), view, above, REFERENCE_STEPS);
  let march = 0;
  let quadrature = 0;
  for (const c of [0, 1, 2] as const) {
    const denominator = Math.max(reference[c], FRAME_FLOOR * lMax[c]);
    march = Math.max(march, Math.abs(lAbove[c] - lBelow[c]) / denominator);
    quadrature = Math.max(
      quadrature,
      Math.abs(lAbove[c] - referenceAbove[c] - (lBelow[c] - reference[c])) / denominator,
    );
  }
  return { march, quadrature };
}

/** A horizontal's step as recorded: each part within 5 × 10⁻⁵ (0.005 percentage points). */
function recordedStep(march: number, quadrature: number): unknown {
  return { march: expect.closeTo(march, 4), quadrature: expect.closeTo(quadrature, 4) };
}

describe("the ray march across the horizontal from inside the band (R08.T8)", () => {
  // From inside the air the rule changes at view zenith 90°: just under the horizontal a sky
  // segment is two-sided and takes the sun-aware warp, just over it one-sided and keeps R05.T12.e's
  // placement toward the camera (`decision-r08-t8-leans.md` §4 and §5.4). Measured 2026-10-21 and
  // recorded in plan R08's Risks ("Deviations in T8.a, as built"), the worst over the top's seven
  // suns, at ±10⁻⁴ and ±10⁻³ rad. The quadrature's own step is under 0.08% on high and 0.29% on low.
  // With §4's even steps it was 6.67% on high from 30 km (the band's lower edge, w = 0, not drawn),
  // 1.98% from 40 km (w = ½) and 0.21% from 50 km, and on low 1.25% from 20 km, 1.17% from 25 km
  // and 44.6% from 30 km (w = 1), with the Sun 5° set behind the camera (−5°/180°), where the
  // even-stepped ray under the horizontal erred by 47.5% and the one over it by 3.2%; the warp puts
  // that ray's steps where the light is, at the chord's start, as the ray over it. From 1 m under
  // the top the march is its reference's, its step the sky's own (11.2% at ±10⁻⁴, 93.3% at ±10⁻³).
  it("records the march's step across the horizontal, and the quadrature's own part of it", () => {
    const figures = Object.fromEntries(
      QUALITY_SETTINGS.flatMap((setting) =>
        HORIZON_CAMERAS_M[setting].flatMap((heightM) =>
          [1e-4, 1e-3].map((deltaRad) => {
            const steps = TOP_SUNS.map(([e, a]) =>
              horizonStep(setting, heightM, sunAtCamera(e, a), deltaRad),
            );
            return [
              `${setting}, ${heightM} m, ±${deltaRad} rad`,
              {
                march: Math.max(...steps.map((s) => s.march)),
                quadrature: Math.max(...steps.map((s) => s.quadrature)),
              },
            ];
          }),
        ),
      ),
    );
    const recorded = recordedStep;
    expect(figures).toEqual({
      "high, 30000 m, ±0.0001 rad": recorded(0.00444, 0.00056),
      "high, 30000 m, ±0.001 rad": recorded(0.03984, 0.0008),
      "high, 40000 m, ±0.0001 rad": recorded(0.00486, 0.00054),
      "high, 40000 m, ±0.001 rad": recorded(0.04345, 0.00055),
      "high, 50000 m, ±0.0001 rad": recorded(0.00477, 0.00037),
      "high, 50000 m, ±0.001 rad": recorded(0.04359, 0.00037),
      "high, 99999 m, ±0.0001 rad": recorded(0.11231, 0),
      "high, 99999 m, ±0.001 rad": recorded(0.93284, 0),
      "low, 20000 m, ±0.0001 rad": recorded(0.00692, 0.00286),
      "low, 20000 m, ±0.001 rad": recorded(0.04273, 0.00288),
      "low, 25000 m, ±0.0001 rad": recorded(0.00596, 0.00223),
      "low, 25000 m, ±0.001 rad": recorded(0.04026, 0.00221),
      "low, 30000 m, ±0.0001 rad": recorded(0.00653, 0.00271),
      "low, 30000 m, ±0.001 rad": recorded(0.04147, 0.00272),
    });
  }, 120_000);
});

/**
 * The window's suns, the gate's `last light` suns (`decision-r08-t8-leans.md` §5.1 and §5.5): 0.5°
 * under to 1.5° over the camera's own visible horizon in 0.5° steps, degrees over it, at 120°,
 * 150° and 180° from the view's azimuth.
 */
const WINDOW_SUNS_OVER_HORIZON_DEG = [-0.5, 0, 0.5, 1, 1.5] as const;
const WINDOW_AZIMUTHS_DEG = [120, 150, 180] as const;

/** The window's cameras, m: in each setting's band and above it, to 1 m under the top. */
const WINDOW_CAMERAS_M: Readonly<Record<QualitySetting, readonly number[]>> = {
  high: [30_000, 40_000, 50_000, 60_000, 80_000, 95_000, TOP_M - 1],
  low: [25_000, 30_000, 40_000, 50_000, 60_000, 80_000, 95_000, TOP_M - 1],
};

/** The steps of the converged march the frame's meter reads: R05.T12.e's placed steps. */
const METER_STEPS = 128;

/** ITU-R BT.709's luminance row, the luminance AUTO meters the frame's mean of. */
const LUMINANCE_709 = [0.2126, 0.7152, 0.0722] as const;

/** A pixel's 8-bit sRGB output codes at an exposure: `tonemapTexel`'s AgX and encoding, undithered. */
function codesOf(light: Rgb, exposure: number): Rgb {
  const [r, g, b] = tonemapTexel(light, exposure, null);
  return [r * 255, g * 255, b * 255];
}

/** The worst channel's difference between two pixels' codes. */
const codeStep = (a: Rgb, b: Rgb): number =>
  Math.max(...([0, 1, 2] as const).map((c) => Math.abs(a[c] - b[c])));

/** One frame's figures in the window. */
interface WindowFigures {
  /**
   * The quadrature's own step across the horizontal, codes: |(c − c_ref)(+δ) − (c − c_ref)(−δ)| in
   * the worst channel, over δ = 10⁻⁴ and 10⁻³ rad, about a 1080p row at 36°.
   */
  readonly seamCodes: number;
  /** The worst code error of the sky under the horizontal, down to 0.05° over the visible horizon. */
  readonly codes: number;
  /** The worst relative error there, Design note 10's metric at its floor of the rows' brightest. */
  readonly relative: number;
  /** The worst relative error of those rows' references against twice their steps, the same metric. */
  readonly unconverged: number;
}

/**
 * The window's frame from a camera `heightM` up on a setting, the Sun `overDeg` over its visible
 * horizon at `azimuthDeg` from the view's: the march at the setting's steps against the converged
 * march, in display codes (`decision-r08-t8-leans.md` §5, "How a figure is put on the display"):
 * `tonemapTexel` at AUTO's calibration, exposure 1 ÷ (9.6 L̄) (`exposureScale` of
 * `ev100FromAverageLuminance`), L̄ the mean luminance of the converged march at {@link METER_STEPS}
 * over a 36° column centred on the horizontal, bare ground included. The march is taken alone, as
 * the composite draws it from the band's upper edge up; below it the blend takes it by w.
 */
function windowFrame(
  twin: PlanetTwin,
  setting: QualitySetting,
  heightM: number,
  overDeg: number,
  azimuthDeg: number,
): WindowFigures {
  const sizes = TABLE_SIZES[setting];
  const dip = dipDeg(heightM);
  const sun = sunAtCamera(overDeg - dip, azimuthDeg);
  const view = twinView(columnCamera(heightM, -18, 18), sun, SPHERE, 0.1);
  const along = (d: Vec3): RayMarchRay => {
    const marched = rayMarchRayAlongTwin(
      twin.medium,
      view,
      view.cameraM,
      d,
      Number.POSITIVE_INFINITY,
    );
    if (marched === null) {
      throw new Error("a ray from inside the air meets the air");
    }
    return marched;
  };
  let sumY = 0;
  for (let row = 0; row < ROWS; row += 1) {
    const l = placedMarch(twin, view, along(rowDirection(view, row)), METER_STEPS);
    sumY += LUMINANCE_709[0] * l[0] + LUMINANCE_709[1] * l[1] + LUMINANCE_709[2] * l[2];
  }
  const exposure = exposureScale(ev100FromAverageLuminance(sumY / ROWS));
  const march = (d: Vec3): Rgb => rgbOf(rayMarchTexelTwin(twin, view, sizes, along(d)));
  const reference = (d: Vec3, samples: number): Rgb => placedMarch(twin, view, along(d), samples);
  const elevationsDeg: number[] = [];
  for (let el = -0.06; el > -dip + 0.05; el -= 0.5) {
    elevationsDeg.push(el);
  }
  elevationsDeg.push(-dip + 0.05);
  const rows = elevationsDeg.map((el) => {
    const d = atElevation(el * RAD);
    return {
      march: march(d),
      reference: reference(d, REFERENCE_STEPS),
      confirm: reference(d, 2 * REFERENCE_STEPS),
    };
  });
  const lMax = ([0, 1, 2] as const).map((c) => Math.max(...rows.map((p) => p.reference[c])));
  let codes = 0;
  let relative = 0;
  let unconverged = 0;
  for (const p of rows) {
    codes = Math.max(codes, codeStep(codesOf(p.march, exposure), codesOf(p.reference, exposure)));
    for (const c of [0, 1, 2] as const) {
      const denominator = Math.max(p.reference[c], FRAME_FLOOR * (lMax[c] ?? 0));
      relative = Math.max(relative, Math.abs(p.march[c] - p.reference[c]) / denominator);
      unconverged = Math.max(unconverged, Math.abs(p.reference[c] - p.confirm[c]) / denominator);
    }
  }
  let seamCodes = 0;
  for (const deltaRad of [1e-4, 1e-3]) {
    const up = atElevation(deltaRad);
    const down = atElevation(-deltaRad);
    const marchUp = codesOf(march(up), exposure);
    const marchDown = codesOf(march(down), exposure);
    const referenceUp = codesOf(reference(up, REFERENCE_STEPS), exposure);
    const referenceDown = codesOf(reference(down, REFERENCE_STEPS), exposure);
    for (const c of [0, 1, 2] as const) {
      seamCodes = Math.max(
        seamCodes,
        Math.abs(marchUp[c] - referenceUp[c] - (marchDown[c] - referenceDown[c])),
      );
    }
  }
  return { seamCodes, codes, relative, unconverged };
}

/** A camera's worst figures in the window as recorded: codes to 0.005, relative errors to 5 × 10⁻⁵. */
function windowRecorded(seamCodes: number, codes: number, relative: number): unknown {
  return {
    seamCodes: expect.closeTo(seamCodes, 2),
    codes: expect.closeTo(codes, 2),
    relative: expect.closeTo(relative, 4),
  };
}

/**
 * The window's media, and each one's worst figures by setting and camera over the window's suns,
 * measured 2026-10-21: the quadrature's own step across the horizontal in codes, and the worst code
 * and relative error of the sky under it.
 */
const WINDOW_MEDIA: ReadonlyArray<{
  readonly name: string;
  readonly twinOf: () => PlanetTwin;
  readonly record: Readonly<Record<string, unknown>>;
}> = [
  {
    name: "R05's Earth",
    twinOf: earthTwin,
    // The ruling's probe (§5.4) had the warp's worst at 1.7 codes on high and 12.2 on low over a
    // finer sweep of suns from 1.5° under to 2.5° over the visible horizon; §4's even steps were off
    // by 45–124 codes (63–97%) here, under a seam of 2–36 codes.
    record: {
      "high, 30000 m": windowRecorded(0.11961, 0.88884, 0.02034),
      "high, 40000 m": windowRecorded(0.06393, 1.09378, 0.02443),
      "high, 50000 m": windowRecorded(0.11714, 1.22619, 0.02659),
      "high, 60000 m": windowRecorded(0.10593, 1.203, 0.02515),
      "high, 80000 m": windowRecorded(0.13953, 1.21127, 0.02479),
      "high, 95000 m": windowRecorded(0.21545, 1.67771, 0.03425),
      "high, 99999 m": windowRecorded(0.00112, 1.5817, 0.03453),
      "low, 25000 m": windowRecorded(0.4684, 5.56011, 0.12738),
      "low, 30000 m": windowRecorded(0.26533, 7.34617, 0.1599),
      "low, 40000 m": windowRecorded(0.39236, 9.28477, 0.19375),
      "low, 50000 m": windowRecorded(0.23828, 10.81423, 0.21693),
      "low, 60000 m": windowRecorded(0.2463, 10.04573, 0.19534),
      "low, 80000 m": windowRecorded(0.21869, 8.86331, 0.16815),
      "low, 95000 m": windowRecorded(0.14519, 8.78179, 0.16811),
      "low, 99999 m": windowRecorded(0.00459, 10.01637, 0.19482),
    },
  },
  {
    name: "the drawn Earth",
    twinOf: () => planetTwin(EARTH_REFERENCE, A_M, EARTH_REFERENCE_CURVES),
    // The probe had the warp at 0.9–1.3 codes on high and 5.4–10.2 on low here, §4's even steps
    // off by 26–91 codes (§5.3).
    record: {
      "high, 30000 m": windowRecorded(0.09985, 0.8803, 0.02294),
      "high, 40000 m": windowRecorded(0.0882, 1.004, 0.0263),
      "high, 50000 m": windowRecorded(0.09524, 1.04099, 0.02761),
      "high, 60000 m": windowRecorded(0.07771, 1.0115, 0.02492),
      "high, 80000 m": windowRecorded(0.09977, 1.18896, 0.03086),
      "high, 95000 m": windowRecorded(0.1267, 1.32125, 0.04389),
      "high, 99999 m": windowRecorded(0.0004, 1.47518, 0.04648),
      "low, 25000 m": windowRecorded(0.10405, 4.32373, 0.12171),
      "low, 30000 m": windowRecorded(0.10137, 6.43979, 0.16593),
      "low, 40000 m": windowRecorded(0.18394, 8.13342, 0.20269),
      "low, 50000 m": windowRecorded(0.28161, 9.13403, 0.22495),
      "low, 60000 m": windowRecorded(0.18759, 7.76829, 0.18278),
      "low, 80000 m": windowRecorded(0.17437, 7.48558, 0.20042),
      "low, 95000 m": windowRecorded(0.06645, 7.06602, 0.24464),
      "low, 99999 m": windowRecorded(0.00342, 8.51993, 0.2681),
    },
  },
];

describe.each(WINDOW_MEDIA)(
  "the camera's last light in display codes on $name (R08.T8, `decision-r08-t8-leans.md` §5.5)",
  ({ twinOf, record }) => {
    /** Each camera's worst figures over the window's suns, by setting. */
    const worst = new Map<string, WindowFigures>();
    const worstAt = (setting: QualitySetting, heightM: number): WindowFigures => {
      const found = worst.get(`${setting} ${heightM}`);
      if (found === undefined) {
        throw new Error(`no window from ${heightM} m on ${setting}`);
      }
      return found;
    };

    beforeAll(() => {
      const twin = twinOf();
      for (const setting of QUALITY_SETTINGS) {
        for (const heightM of WINDOW_CAMERAS_M[setting]) {
          const frames = WINDOW_SUNS_OVER_HORIZON_DEG.flatMap((overDeg) =>
            WINDOW_AZIMUTHS_DEG.map((az) => windowFrame(twin, setting, heightM, overDeg, az)),
          );
          worst.set(`${setting} ${heightM}`, {
            seamCodes: Math.max(...frames.map((f) => f.seamCodes)),
            codes: Math.max(...frames.map((f) => f.codes)),
            relative: Math.max(...frames.map((f) => f.relative)),
            unconverged: Math.max(...frames.map((f) => f.unconverged)),
          });
        }
      }
    }, 600_000);

    it("has a converged reference under the horizontal: 1,024 placed steps within 0.05% of 2,048", () => {
      const unconverged = QUALITY_SETTINGS.flatMap((setting) =>
        WINDOW_CAMERAS_M[setting].filter(
          (heightM) => !(worstAt(setting, heightM).unconverged < CONVERGED),
        ),
      );
      expect(unconverged).toEqual([]);
    });

    // The blocker line (§5): a seam is visible when a channel steps by 2 codes or more across a row.
    it.each(QUALITY_SETTINGS)(
      "keeps the quadrature's own step across the horizontal under 2 codes in every channel on %s",
      (setting) => {
        const seamed = WINDOW_CAMERAS_M[setting].filter(
          (heightM) => !(worstAt(setting, heightM).seamCodes < 2),
        );
        expect(seamed).toEqual([]);
      },
    );

    // Measured 2026-10-21 and recorded in plan R08's Risks ("Deviations in T8.a, as built"): on R05's
    // Earth the sky under the horizontal within 1.68 codes (3.45%) on high and 10.81 codes (21.7%)
    // on low, the quadrature's step across the horizontal within 0.22 and 0.47 codes; on the drawn
    // Earth within 1.48 codes (4.65%) and 9.13 codes (26.8%), its step within 0.13 and 0.28 codes.
    it("records the window's worst figures by setting and camera", () => {
      const figures = Object.fromEntries(
        QUALITY_SETTINGS.flatMap((setting) =>
          WINDOW_CAMERAS_M[setting].map((heightM) => {
            const { seamCodes, codes, relative } = worstAt(setting, heightM);
            return [`${setting}, ${heightM} m`, { seamCodes, codes, relative }];
          }),
        ),
      );
      expect(figures).toEqual(record);
    });
  },
);
