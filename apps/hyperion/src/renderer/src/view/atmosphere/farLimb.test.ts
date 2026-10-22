import { beforeAll, describe, expect, it } from "vitest";

import { cross } from "../../geometry/vec3";
import { CONVERGED, REFERENCE_STEPS, sunAt } from "../../test/atmosphereGate";
import {
  SATURN_SPHEROID,
  saturnClassMedium,
  saturnScaleHeightM,
} from "../../test/atmosphereFixtures";
import { DEFAULT_FOV_DEG, NEAR_PLANE_M } from "../camera/projection";
import { quaternionFromRows } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  RAYLEIGH_SCALE_HEIGHT_M,
} from "./earth";
import { type AtmosphereCamera, type SpheroidFigure, type SunState, TABLE_SIZES } from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import type { AbsorberCurveSlots } from "./tables";
import {
  planetTwin,
  type PlanetTwin,
  type RayMarchRay,
  rayMarchRayTwin,
  rayMarchTexelTwin,
  twinView,
  type TwinView,
} from "./tablesCpu";

/**
 * R08.T8.b's far limb's fidelity under Design note 10's 5% (plan R08, R08.T8): per distance, the
 * ray march's limb pixels at the output resolution against the twin's mean over each pixel's
 * footprint, 8 × 8 sub-rays, on Design note 10's limb set (0.1, 0.3, 1, 3 and 10 scale heights),
 * from 10⁷, 10⁸ and 10⁹ m from the body's centre. The march and the mean are the same twin
 * (`rayMarchTexelTwin`) at the setting's own count, so a pixel's e is what one ray (through the
 * pixel's centre on high, the picked texel's on low) misses of the light across the pixel, not the
 * quadrature's error; that is e_q, recorded beside it per body against a converged march. The test
 * records the figures; it does not hold them to the 5%. From about one scale height a texel the
 * pixels over it are a known blocker owned by R08.T8.c, which integrates the march across each
 * texel's footprint (`decision-r08-t8-leans.md` §6.1) and replaces this test's 8 × 8 reference
 * with a footprint mean converged across the limb: from 10⁹ m the 8 × 8's rows lie 8.9 H apart,
 * and on the ruling's probe it is itself off that converged mean by up to 87%.
 *
 * @remarks
 * Each image is one distance, one sun and one setting: five frames from one camera position, each
 * pitched so that one pixel's centre lies on one ray of the limb set, at a 60° field across
 * (`DEFAULT_FOV_DEG`) on high's 1920 × 1080 (the 1080p target) and low's 1280 × 720 (its 720
 * rows). Its sun is set at its zenith angle and azimuth at the tangent point of the ray from the
 * camera that grazes the ground, as the gate sets a limb ray's (`test/atmosphereGate.ts`'s
 * `sunAt`): 30° at 0°, 90° and 180°, and 90° and 100° at 0° (beyond the limb) and 180° (behind the
 * camera). At 90°/0° the Sun's centre is on the limb, within 0.7° of every ray of the image from
 * 10⁷ m, which Design note 10's 3° keeps out of its set; it is recorded as the gate records its
 * limb at 90°/0°. The metric is Design note 10's: e = max over channels |L − M| ÷ max(M, 6 × 10⁻³
 * L_max), M the footprint's mean and L_max the brightest of the image's five means. The radiance is
 * the twin's, the bare ground it lights where a sub-ray meets it included, and also recorded of the
 * air alone, the ground's light left out.
 *
 * Low's march is at half resolution, and where the four candidate texels' depths tie, as every sky
 * pixel's do, the composite draws the one whose centre is nearest the pixel's (`composite.wgsl`'s
 * `rayMarched`, since R08.T8.b; `decision-r08-t8-leans.md` §6.2): texel ⌊x ÷ 2⌋ in each axis, its
 * centre 0.5 px below and right of an even pixel's centre and 0.5 px above and left of an odd
 * one's. Both parities are recorded: `low even` at pixel (640, 360), `low odd` at (641, 361). Up
 * the frame is up the limb, so the drawn value is the sky 0.5 px lower than an even pixel and
 * 0.5 px higher than an odd one.
 *
 * A pixel's span in tangent height, its angular height times the distance to the tangent point
 * (measured on the 1 H ray at the frame's middle, {@link pixelTangentM}):
 *
 * | Body | Distance | High (1920 × 1080) | Low (1280 × 720) |
 * | --- | --- | --- | --- |
 * | Earth, H 8.4345 km | 10⁷ m | 4.63 km, 0.549 H | 6.94 km, 0.823 H |
 * | Earth | 10⁸ m | 60.0 km, 7.12 H | 90.0 km, 10.7 H |
 * | Earth | 10⁹ m | 601 km, 71.2 H | 900 km, 107 H |
 * | Saturn-class, H 49.7 km | 10⁸ m | 48.0 km, 0.965 H | 72.0 km, 1.45 H |
 * | Saturn-class | 10⁹ m | 600 km, 12.1 H | 900 km, 18.1 H |
 *
 * Earth's media sit on WGS 84's equatorial radius as a sphere (6,378,137 m, NIMA TR8350.2), H the
 * Rayleigh term's 8,434.5 m (`earth.ts`); the Saturn-class medium (`test/atmosphereFixtures.ts`)
 * on Saturn's equatorial radius at 1 bar as a sphere (60,268 km, Archinal et al. 2018), H its own
 * isothermal 49.7 km, and only from 10⁸ and 10⁹ m, 10⁷ m being inside it.
 */

const RAD = Math.PI / 180;
const CHANNELS = [0, 1, 2] as const;

/** Design note 10's floor, 20F: 6 × 10⁻³ of the image's brightest. */
const FLOOR = 6e-3;

/** R05.T12.e's floor, at which the references' convergence is judged: 10⁻³ of the brightest. */
const CONVERGENCE_FLOOR = 1e-3;

/** Design note 10's bound on a pixel, which the figures count against and do not assert. */
const BOUND = 0.05;

/** The sub-rays along each side of a pixel's footprint, 8 × 8 in all. */
const SUB_RAYS = 8;

/** The most placed steps a reference takes, confirmed against twice as many (the gate's rule). */
const MAX_REFERENCE_STEPS = 8_192;

/** The output at each setting: high's 1080p target (R05 Design note 21), low's rows at 16:9. */
const OUTPUT: Readonly<Record<QualitySetting, ViewSize>> = {
  high: { widthPx: 1_920, heightPx: 1_080 },
  low: {
    widthPx: ((SETTINGS.low.terrain.renderHeightPx ?? Number.NaN) * 16) / 9,
    heightPx: SETTINGS.low.terrain.renderHeightPx ?? Number.NaN,
  },
};

/** The field across, rad. */
const FOV_X_RAD = DEFAULT_FOV_DEG * RAD;

/**
 * The pixel each limb-set ray is aimed through, by how it is drawn: high's at the frame's middle,
 * and low's of either parity, whose picked texel stands 0.5 px below and right of an even pixel's
 * centre and 0.5 px above and left of an odd one's.
 */
const VARIANTS = {
  high: { setting: "high", xPx: 960, yPx: 540 },
  "low even": { setting: "low", xPx: 640, yPx: 360 },
  "low odd": { setting: "low", xPx: 641, yPx: 361 },
} as const;
type Variant = keyof typeof VARIANTS;
const VARIANT_NAMES: readonly Variant[] = ["high", "low even", "low odd"];

/** Design note 10's limb set, in scale heights. */
const LIMB_SET = [0.1, 0.3, 1, 3, 10] as const;

/** The images' suns, zenith angle and azimuth at the grazing ray's tangent point, degrees. */
const SUNS = [
  [30, 0],
  [30, 90],
  [30, 180],
  [90, 0],
  [90, 180],
  [100, 0],
  [100, 180],
] as const;

/** A body the far limb is measured on, as a sphere. */
interface FarBody {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots | undefined;
  readonly radiusM: number;
  readonly scaleHeightM: number;
  readonly distancesM: readonly number[];
}

/** WGS 84's equatorial radius a, m (NIMA TR8350.2). */
const EARTH_M = 6_378_137;

const BODIES: readonly FarBody[] = [
  {
    name: "R05's Earth",
    medium: R05_EARTH_REFERENCE,
    curves: undefined,
    radiusM: EARTH_M,
    scaleHeightM: RAYLEIGH_SCALE_HEIGHT_M,
    distancesM: [1e7, 1e8, 1e9],
  },
  {
    name: "the measured Earth",
    medium: EARTH_REFERENCE,
    curves: EARTH_REFERENCE_CURVES,
    radiusM: EARTH_M,
    scaleHeightM: RAYLEIGH_SCALE_HEIGHT_M,
    distancesM: [1e7, 1e8, 1e9],
  },
  {
    name: "Saturn-class",
    medium: saturnClassMedium(),
    curves: undefined,
    radiusM: SATURN_SPHEROID.equatorialRadiusM,
    scaleHeightM: saturnScaleHeightM(),
    distancesM: [1e8, 1e9],
  },
];

const DISTANCE_NAMES: Readonly<Record<number, string>> = {
  1e7: "10⁷ m",
  1e8: "10⁸ m",
  1e9: "10⁹ m",
};
const distanceName = (distanceM: number): string => DISTANCE_NAMES[distanceM] ?? `${distanceM} m`;

const sphereOf = (radiusM: number): SpheroidFigure => ({
  equatorialRadiusM: radiusM,
  polarRadiusM: radiusM,
});

/** A pixel's ray's offsets on the image plane, at the field's tangents. */
interface PlaneOffsets {
  /** Along the camera's right. */
  readonly a: number;
  /** Along the camera's up. */
  readonly b: number;
}

/** A pixel's ray's offsets on the image plane, right and up, at the field's tangents. */
function planeOffsets(viewport: ViewSize, xPx: number, yPx: number): PlaneOffsets {
  const tanX = Math.tan(FOV_X_RAD / 2);
  const tanY = tanX / (viewport.widthPx / viewport.heightPx);
  return {
    a: ((2 * xPx) / viewport.widthPx - 1) * tanX,
    b: (1 - (2 * yPx) / viewport.heightPx) * tanY,
  };
}

/**
 * The pitch ψ of a {@link limbCamera} whose pixel (`xPx`, `yPx`) has its centre on the ray of
 * tangent height `tangentM`, rad.
 */
function limbPitchRad(
  distanceM: number,
  radiusM: number,
  tangentM: number,
  viewport: ViewSize,
  xPx: number,
  yPx: number,
): number {
  const { a, b } = planeOffsets(viewport, xPx + 0.5, yPx + 0.5);
  // The pixel's ray is F + aR + bU. With R = (0, 0, −1) and F and U in the plane z = 0, pitched by
  // ψ, its part in the plane is √(1 + b²)(−cos φ, sin φ, 0) at φ = ψ + atan b, so its distance from
  // the centre is D √((1 + b²) sin²φ + a²) ÷ √(1 + a² + b²).
  const k = (radiusM + tangentM) / distanceM;
  const sinPhi = Math.sqrt((k * k * (1 + a * a + b * b) - a * a) / (1 + b * b));
  return Math.asin(sinPhi) - Math.atan(b);
}

/**
 * The tangent height over the sphere of the ray through the point (`xPx`, `yPx`) of a
 * {@link limbCamera}'s frame pitched by `pitchRad`, pixel centres at half-integers, m.
 */
function pixelTangentM(
  distanceM: number,
  radiusM: number,
  pitchRad: number,
  viewport: ViewSize,
  xPx: number,
  yPx: number,
): number {
  const { a, b } = planeOffsets(viewport, xPx, yPx);
  const phi = pitchRad + Math.atan(b);
  return (
    (distanceM * Math.sqrt((1 + b * b) * Math.sin(phi) ** 2 + a * a)) /
      Math.sqrt(1 + a * a + b * b) -
    radiusM
  );
}

/**
 * A camera `distanceM` from the centre on +x, looking past the limb in the plane z = 0 at a 60°
 * field across, the limb level across the frame and the tangent height rising up it (as the
 * smoke's `limbCamera` looks), pitched by `pitchRad` ({@link limbPitchRad}).
 */
function limbCamera(distanceM: number, pitchRad: number, viewport: ViewSize): AtmosphereCamera {
  const forward = { x: -Math.cos(pitchRad), y: Math.sin(pitchRad), z: 0 };
  const up = { x: Math.sin(pitchRad), y: Math.cos(pitchRad), z: 0 };
  const right = { x: 0, y: 0, z: -1 };
  return {
    positionM: { x: distanceM, y: 0, z: 0 },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: FOV_X_RAD,
    viewport,
  };
}

/**
 * An image's sun, unit light: at its zenith angle and azimuth at the tangent point of the ray from
 * a camera `distanceM` from the centre on +x that grazes the sphere in the plane z = 0, as the gate
 * sets a ray's (`test/atmosphereGate.ts`'s `sunAt`).
 */
function imageSun(
  distanceM: number,
  radiusM: number,
  zenithDeg: number,
  azimuthDeg: number,
): SunState {
  const sin = radiusM / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  const d = { x: -cos, y: sin, z: 0 };
  const q = { x: radiusM * sin, y: radiusM * cos, z: 0 };
  return {
    directionBodyFixed: sunAt(q, d, zenithDeg, azimuthDeg),
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.004_65,
  };
}

const rgbOf = (texel: readonly [number, number, number, number]): Rgb => [
  texel[0],
  texel[1],
  texel[2],
];

/**
 * The ray-march texel the composite draws at an output pixel along one axis with every depth 0
 * (`composite.wgsl`'s `rayMarched`): the pixel itself at full resolution, and at half, of its four
 * candidates, whose depths tie, the one whose centre is nearest the pixel's, the texel holding the
 * pixel's centre, ⌊(x + ½) ÷ 2⌋ = ⌊x ÷ 2⌋, within the target.
 */
function pickedTexel(pixel: number, outputPx: number, rayMarchScale: number): number {
  if (rayMarchScale >= 1) {
    return pixel;
  }
  const size = Math.ceil(outputPx * rayMarchScale);
  return Math.min(Math.max(Math.floor((pixel + 0.5) * rayMarchScale), 0), size - 1);
}

/** The march's drawn value at an output pixel: the picked texel's, along its centre's ray. */
function drawnAt(
  planet: PlanetTwin,
  view: TwinView,
  setting: QualitySetting,
  xPx: number,
  yPx: number,
): Rgb {
  const sizes = TABLE_SIZES[setting];
  const { widthPx: w, heightPx: h } = view.viewport;
  const s = sizes.rayMarchScale;
  const u = (pickedTexel(xPx, w, s) + 0.5) / Math.ceil(w * s);
  const v = (pickedTexel(yPx, h, s) + 0.5) / Math.ceil(h * s);
  const marched = rayMarchRayTwin(planet.medium, view, u, v, 0, sizes);
  return rgbOf(rayMarchTexelTwin(planet, view, sizes, marched));
}

/**
 * A footprint's means: with the bare ground's light where a sub-ray meets it, and of the air
 * alone.
 */
interface FootprintMean {
  readonly mean: Rgb;
  readonly air: Rgb;
}

/**
 * The twin's mean over the footprint of output pixel (`xPx`, `yPx`), 8 × 8 sub-rays at
 * u = (x + (i + ½) ÷ 8) ÷ W, v = (y + (j + ½) ÷ 8) ÷ H, at the setting's count; `air` the same
 * planet with a black ground.
 */
function footprintMean(
  planet: PlanetTwin,
  air: PlanetTwin,
  view: TwinView,
  setting: QualitySetting,
  xPx: number,
  yPx: number,
): FootprintMean {
  const sizes = TABLE_SIZES[setting];
  const { widthPx: w, heightPx: h } = view.viewport;
  const mean: [number, number, number] = [0, 0, 0];
  const alone: [number, number, number] = [0, 0, 0];
  const share = 1 / SUB_RAYS ** 2;
  for (let j = 0; j < SUB_RAYS; j += 1) {
    for (let i = 0; i < SUB_RAYS; i += 1) {
      const u = (xPx + (i + 0.5) / SUB_RAYS) / w;
      const v = (yPx + (j + 0.5) / SUB_RAYS) / h;
      const marched = rayMarchRayTwin(planet.medium, view, u, v, 0, sizes);
      const texel = rayMarchTexelTwin(planet, view, sizes, marched);
      // Only a sub-ray that meets the bare ground draws anything the black ground changes.
      const airTexel =
        marched !== null && marched.bareGroundM !== null
          ? rayMarchTexelTwin(air, view, sizes, marched)
          : texel;
      for (const c of CHANNELS) {
        mean[c] += texel[c] * share;
        alone[c] += airTexel[c] * share;
      }
    }
  }
  return { mean, air: alone };
}

/** e = max over channels of |L − M| ÷ max(M, floor · L_max) (Design note 10's metric). */
function errorOf(l: Rgb, m: Rgb, lMax: Rgb, floor: number): number {
  let e = 0;
  for (const c of CHANNELS) {
    e = Math.max(e, Math.abs(l[c] - m[c]) / Math.max(m[c], floor * lMax[c]));
  }
  return e;
}

const brightestOf = (values: readonly Rgb[]): Rgb => [
  Math.max(...values.map((v) => v[0])),
  Math.max(...values.map((v) => v[1])),
  Math.max(...values.map((v) => v[2])),
];

const medianOf = (values: readonly number[]): number => {
  const sorted = values.toSorted((x, y) => x - y);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? Number.NaN)
    : ((sorted[mid - 1] ?? Number.NaN) + (sorted[mid] ?? Number.NaN)) / 2;
};

/** A set of e's figures: the worst, the median, and how many stand over the 5%. */
interface Figures {
  readonly worst: number;
  readonly median: number;
  readonly over: number;
}

const figuresOfErrors = (es: readonly number[]): Figures => ({
  worst: Math.max(...es),
  median: medianOf(es),
  over: es.filter((e) => !(e <= BOUND)).length,
});

/** Figures as recorded: each within 5 × 10⁻⁵ of its value. */
const asRecorded = (figures: Readonly<Record<string, number>>): unknown =>
  Object.fromEntries(
    Object.entries(figures).map(([key, figure]) => [key, expect.closeTo(figure, 4)]),
  );

/** A set of figures as recorded: the worst and the median within 5 × 10⁻⁵, the count exactly. */
const recordedFigure = (f: Figures): Record<keyof Figures, unknown> => ({
  worst: expect.closeTo(f.worst, 4),
  median: expect.closeTo(f.median, 4),
  over: f.over,
});

/** Each place's set of figures as recorded ({@link recordedFigure}). */
const recordedFigures = (figures: Readonly<Record<string, Figures>>): unknown =>
  Object.fromEntries(Object.entries(figures).map(([key, f]) => [key, recordedFigure(f)]));

/** One pixel's e, with the bare ground and of the air alone, by its sun and tangent height. */
interface PixelError {
  readonly name: string;
  readonly e: number;
  readonly eAir: number;
}

/** The pixels' e of every image at one body, distance and variant. */
function pixelErrors(
  entry: BuiltBody,
  body: FarBody,
  distanceM: number,
  variant: Variant,
): PixelError[] {
  const { setting, xPx, yPx } = VARIANTS[variant];
  const viewport = OUTPUT[setting];
  const figure = sphereOf(body.radiusM);
  const cameras = LIMB_SET.map((scaleHeights) =>
    limbCamera(
      distanceM,
      limbPitchRad(distanceM, body.radiusM, scaleHeights * body.scaleHeightM, viewport, xPx, yPx),
      viewport,
    ),
  );
  return SUNS.flatMap(([zenithDeg, azimuthDeg]) => {
    const sun = imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg);
    const pixels = cameras.map((camera) => {
      const view = twinView(camera, sun, figure, NEAR_PLANE_M);
      const { mean, air } = footprintMean(entry.planet, entry.air, view, setting, xPx, yPx);
      return { drawn: drawnAt(entry.planet, view, setting, xPx, yPx), mean, air };
    });
    const lMax = brightestOf(pixels.map((p) => p.mean));
    const lMaxAir = brightestOf(pixels.map((p) => p.air));
    return pixels.map((p, k) => ({
      name: `sun ${zenithDeg}°/${azimuthDeg}°, ${LIMB_SET[k] ?? Number.NaN} H`,
      e: errorOf(p.drawn, p.mean, lMax, FLOOR),
      eAir: errorOf(p.drawn, p.air, lMaxAir, FLOOR),
    }));
  });
}

/** A body's twin, and the same with a black ground. */
interface BuiltBody {
  readonly planet: PlanetTwin;
  readonly air: PlanetTwin;
}

/** Each body's twins, which the file's `beforeAll` builds. */
const built = new Map<string, BuiltBody>();

function builtOf(body: FarBody): BuiltBody {
  const entry = built.get(body.name);
  if (entry === undefined) {
    throw new Error(`${body.name}'s twin is built before the tests read it`);
  }
  return entry;
}

// The three bodies' tables took 13 s at four workers on a loaded machine (2026-10-21): the limit is
// over ten times that, as the config's rule for CPU-bound work asks.
beforeAll(() => {
  for (const body of BODIES) {
    const planet = planetTwin(body.medium, body.radiusM, body.curves);
    built.set(body.name, {
      planet,
      air: { ...planet, medium: { ...planet.medium, groundAlbedo: [0, 0, 0] } },
    });
  }
}, 300_000);

/** Every body at every distance it is measured from. */
const PLACES = BODIES.flatMap((body) => body.distancesM.map((distanceM) => ({ body, distanceM })));

describe("the far limb's frames (R08.T8.b)", () => {
  it("aims each limb-set ray through its pixel's centre, to 1 mm of its tangent height", () => {
    const off: string[] = [];
    for (const { body, distanceM } of PLACES) {
      for (const variant of VARIANT_NAMES) {
        const { setting, xPx, yPx } = VARIANTS[variant];
        const viewport = OUTPUT[setting];
        for (const scaleHeights of LIMB_SET) {
          const tangentM = scaleHeights * body.scaleHeightM;
          const pitch = limbPitchRad(distanceM, body.radiusM, tangentM, viewport, xPx, yPx);
          const view = twinView(
            limbCamera(distanceM, pitch, viewport),
            imageSun(distanceM, body.radiusM, 30, 0),
            sphereOf(body.radiusM),
            NEAR_PLANE_M,
          );
          const u = (xPx + 0.5) / viewport.widthPx;
          const v = (yPx + 0.5) / viewport.heightPx;
          const marched = rayMarchRayTwin(body.medium, view, u, v, 0, TABLE_SIZES[setting]);
          const c = marched === null ? null : cross(marched.ray.originM, marched.ray.direction);
          const twinTangentM = c === null ? Number.NaN : Math.hypot(c.x, c.y, c.z) - body.radiusM;
          if (!(Math.abs(twinTangentM - tangentM) < 1e-3)) {
            off.push(`${body.name}, ${distanceName(distanceM)}, ${variant}, ${scaleHeights} H`);
          }
        }
      }
    }
    expect(off).toEqual([]);
  });

  // The pixel's centre less the texel's, in output pixels: negative where the texel lies below and
  // right of it. The test's own emulation of the pick, which its figures read; the smoke's
  // `checkLowPick` holds the shader's.
  it("emulates low's pick: the texel 0.5 px from the pixel's centre, of either parity", () => {
    const offsetPx = (pixel: number): number =>
      pixel + 0.5 - 2 * (pickedTexel(pixel, 1_280, 0.5) + 0.5);
    expect([640, 641, 360, 361].map(offsetPx)).toEqual([-0.5, 0.5, -0.5, 0.5]);
    expect([0, 1, 2, 3, 1_279].map((x) => pickedTexel(x, 1_280, 0.5))).toEqual([0, 0, 1, 1, 639]);
    expect(pickedTexel(960, 1_920, 1)).toBe(960);
  });

  // The table in the module's remarks, in scale heights: each body's own, the two Earths' alike.
  it("records a pixel's span in tangent height at each distance", () => {
    const spans = Object.fromEntries(
      PLACES.filter(({ body }) => body.name !== "the measured Earth").flatMap(
        ({ body, distanceM }) =>
          (["high", "low"] as const).map((setting) => {
            const viewport = OUTPUT[setting];
            const x = viewport.widthPx / 2;
            const y = viewport.heightPx / 2;
            const pitch = limbPitchRad(distanceM, body.radiusM, body.scaleHeightM, viewport, x, y);
            const spanM =
              pixelTangentM(distanceM, body.radiusM, pitch, viewport, x + 0.5, y) -
              pixelTangentM(distanceM, body.radiusM, pitch, viewport, x + 0.5, y + 1);
            const key = `${body.name}, ${distanceName(distanceM)}, ${setting}`;
            return [key, spanM / body.scaleHeightM];
          }),
      ),
    );
    expect(spans).toEqual(
      asRecorded({
        "R05's Earth, 10⁷ m, high": 0.54867,
        "R05's Earth, 10⁷ m, low": 0.82301,
        "R05's Earth, 10⁸ m, high": 7.11568,
        "R05's Earth, 10⁸ m, low": 10.67337,
        "R05's Earth, 10⁹ m, high": 71.22245,
        "R05's Earth, 10⁹ m, low": 106.68413,
        "Saturn-class, 10⁸ m, high": 0.96472,
        "Saturn-class, 10⁸ m, low": 1.44708,
        "Saturn-class, 10⁹ m, high": 12.07303,
        "Saturn-class, 10⁹ m, low": 18.10927,
      }),
    );
  });
});

describe("the far limb's fidelity under Design note 10's 5% (R08.T8.b)", () => {
  /** Each body, distance and variant's pixels' e. */
  const results = new Map<string, PixelError[]>();
  const keyOf = (body: FarBody, distanceM: number, variant: Variant): string =>
    `${body.name}, ${distanceName(distanceM)}, ${variant}`;

  // 4.7 s at four workers on a loaded machine (2026-10-21); the limit is over ten times that.
  beforeAll(() => {
    for (const { body, distanceM } of PLACES) {
      for (const variant of VARIANT_NAMES) {
        results.set(
          keyOf(body, distanceM, variant),
          pixelErrors(builtOf(body), body, distanceM, variant),
        );
      }
    }
  }, 60_000);

  /** Each place's figures of e, with the bare ground or of the air alone. */
  const figuresOf = (of: (p: PixelError) => number): Record<string, Figures> =>
    Object.fromEntries(
      [...results.entries()].map(([key, es]) => [key, figuresOfErrors(es.map(of))]),
    );

  // Measured 2026-10-21. From 10⁷ m on high a pixel spans 0.55 H, and 29 of an Earth's 35 pixels
  // hold the 5%, the median 0.74–0.78%; the six over are the Sun beyond the limb (90° and 100° at
  // 0°) at 0.1–1 H, where the lit air lies beyond the tangent point and the footprint's lower part
  // meets the ground on the night side or falls in its shadow (82–96% at 0.1 H). The Saturn-class
  // limb from 10⁸ m, 0.96 H a pixel, is the same picture: 28 of 35, median 1.06%. From 10⁸ m on
  // Earth (7.1 H a pixel) and 10⁹ m on either body (71 H and 12 H) no pixel holds it: one ray
  // cannot stand for a footprint that tall, which from 10⁹ m takes in the ground's edge, the whole
  // limb and space (from 10⁹ m the 8 × 8 reference is not converged either, R08.T8.c's to replace;
  // `decision-r08-t8-leans.md` §6.1). On low the picked texel's 0.5 px down (even) or up (odd) the
  // limb, 0.41 H from 10⁷ m, and the half-resolution footprint put 23–30 of 35 pixels over from the
  // nearest distance, median 21–25% (Earth) and 32–43% (Saturn-class). Re-measured 2026-10-21
  // after §6.2's tie-break, which moved the even pixel's texel from 1.5 px up the limb to 0.5 px
  // down it (it was 53–65%, worst up to 196 times, and drew nothing from 10⁸ m on Earth); the odd
  // pixel's texel did not move.
  it("records the worst and median e and the count over 5% per body, distance and setting", () => {
    expect(figuresOf((p) => p.e)).toEqual(
      recordedFigures({
        "R05's Earth, 10⁷ m, high": { worst: 0.94785, median: 0.00778, over: 6 },
        "R05's Earth, 10⁷ m, low even": { worst: 0.9966, median: 0.22251, over: 30 },
        "R05's Earth, 10⁷ m, low odd": { worst: 6.77876, median: 0.21324, over: 30 },
        "R05's Earth, 10⁸ m, high": { worst: 1.7569, median: 0.53906, over: 35 },
        "R05's Earth, 10⁸ m, low even": { worst: 9.97337, median: 1, over: 35 },
        "R05's Earth, 10⁸ m, low odd": { worst: 1.69997, median: 0.97757, over: 35 },
        "R05's Earth, 10⁹ m, high": { worst: 190.70351, median: 1.45251, over: 35 },
        "R05's Earth, 10⁹ m, low even": { worst: 2.60116, median: 1, over: 35 },
        "R05's Earth, 10⁹ m, low odd": { worst: 1, median: 1, over: 35 },
        "the measured Earth, 10⁷ m, high": { worst: 0.956, median: 0.00744, over: 6 },
        "the measured Earth, 10⁷ m, low even": { worst: 0.99707, median: 0.2219, over: 23 },
        "the measured Earth, 10⁷ m, low odd": { worst: 7.50661, median: 0.2543, over: 24 },
        "the measured Earth, 10⁸ m, high": { worst: 1.85941, median: 0.46814, over: 35 },
        "the measured Earth, 10⁸ m, low even": { worst: 13.49675, median: 1, over: 35 },
        "the measured Earth, 10⁸ m, low odd": { worst: 1, median: 0.9871, over: 35 },
        "the measured Earth, 10⁹ m, high": { worst: 499.54336, median: 0.99995, over: 35 },
        "the measured Earth, 10⁹ m, low even": { worst: 2.7045, median: 1, over: 35 },
        "the measured Earth, 10⁹ m, low odd": { worst: 1, median: 1, over: 35 },
        "Saturn-class, 10⁸ m, high": { worst: 0.56581, median: 0.01061, over: 7 },
        "Saturn-class, 10⁸ m, low even": { worst: 0.9919, median: 0.42526, over: 24 },
        "Saturn-class, 10⁸ m, low odd": { worst: 6.10261, median: 0.31826, over: 26 },
        "Saturn-class, 10⁹ m, high": { worst: 3.2301, median: 0.51813, over: 35 },
        "Saturn-class, 10⁹ m, low even": { worst: 8.03638, median: 0.99922, over: 35 },
        "Saturn-class, 10⁹ m, low odd": { worst: 1, median: 0.99797, over: 35 },
      }),
    );
  });

  // Measured 2026-10-21, as above. The ground's light moves the figures from 10⁸ and 10⁹ m, where
  // the footprints take in the lit ground: on high only the medians; on low the medians and low
  // even's worst as well (9.973 → 9.989 from 10⁸ m and 2.601 → 2.636 from 10⁹ m on R05's Earth,
  // 2.705 → 2.815 from 10⁹ m on the measured Earth). The figures from 10⁷ m and Saturn's from
  // 10⁸ m are the same, every pixel's worst channel lying in the air.
  it("records the same of the air alone, the bare ground's light left out", () => {
    expect(figuresOf((p) => p.eAir)).toEqual(
      recordedFigures({
        "R05's Earth, 10⁷ m, high": { worst: 0.94785, median: 0.00778, over: 6 },
        "R05's Earth, 10⁷ m, low even": { worst: 0.9966, median: 0.22251, over: 30 },
        "R05's Earth, 10⁷ m, low odd": { worst: 6.77876, median: 0.21324, over: 30 },
        "R05's Earth, 10⁸ m, high": { worst: 1.7569, median: 0.54805, over: 35 },
        "R05's Earth, 10⁸ m, low even": { worst: 9.9891, median: 1, over: 35 },
        "R05's Earth, 10⁸ m, low odd": { worst: 1.69997, median: 0.97757, over: 35 },
        "R05's Earth, 10⁹ m, high": { worst: 190.70351, median: 1.90175, over: 35 },
        "R05's Earth, 10⁹ m, low even": { worst: 2.63642, median: 1.09788, over: 35 },
        "R05's Earth, 10⁹ m, low odd": { worst: 1, median: 1, over: 35 },
        "the measured Earth, 10⁷ m, high": { worst: 0.956, median: 0.00744, over: 6 },
        "the measured Earth, 10⁷ m, low even": { worst: 0.99707, median: 0.2219, over: 23 },
        "the measured Earth, 10⁷ m, low odd": { worst: 7.50661, median: 0.2543, over: 24 },
        "the measured Earth, 10⁸ m, high": { worst: 1.85941, median: 0.49577, over: 35 },
        "the measured Earth, 10⁸ m, low even": { worst: 13.49675, median: 1, over: 35 },
        "the measured Earth, 10⁸ m, low odd": { worst: 1, median: 0.98701, over: 35 },
        "the measured Earth, 10⁹ m, high": { worst: 499.54336, median: 0.99995, over: 35 },
        "the measured Earth, 10⁹ m, low even": { worst: 2.81454, median: 1.13, over: 35 },
        "the measured Earth, 10⁹ m, low odd": { worst: 1, median: 1, over: 35 },
        "Saturn-class, 10⁸ m, high": { worst: 0.56581, median: 0.01061, over: 7 },
        "Saturn-class, 10⁸ m, low even": { worst: 0.9919, median: 0.42526, over: 24 },
        "Saturn-class, 10⁸ m, low odd": { worst: 6.10261, median: 0.31826, over: 26 },
        "Saturn-class, 10⁹ m, high": { worst: 3.2301, median: 0.60136, over: 35 },
        "Saturn-class, 10⁹ m, low even": { worst: 8.03638, median: 0.99948, over: 35 },
        "Saturn-class, 10⁹ m, low odd": { worst: 1, median: 0.99775, over: 35 },
      }),
    );
  });

  // Measured 2026-10-21: every one the Sun beyond the limb, at 0.1–1 H (and 3 H on Saturn).
  it("names the pixels over 5% on high from each body's nearest distance", () => {
    const named = Object.fromEntries(
      BODIES.flatMap((body) => {
        const distanceM = body.distancesM[0] ?? Number.NaN;
        const es = results.get(keyOf(body, distanceM, "high")) ?? [];
        return es
          .filter((p) => !(p.e <= BOUND))
          .map((p) => [`${body.name}, ${distanceName(distanceM)}, ${p.name}`, p.e]);
      }),
    );
    expect(named).toEqual(
      asRecorded({
        "R05's Earth, 10⁷ m, sun 90°/0°, 0.1 H": 0.85341,
        "R05's Earth, 10⁷ m, sun 90°/0°, 0.3 H": 0.33325,
        "R05's Earth, 10⁷ m, sun 90°/0°, 1 H": 0.09934,
        "R05's Earth, 10⁷ m, sun 100°/0°, 0.1 H": 0.94785,
        "R05's Earth, 10⁷ m, sun 100°/0°, 0.3 H": 0.69493,
        "R05's Earth, 10⁷ m, sun 100°/0°, 1 H": 0.22861,
        "the measured Earth, 10⁷ m, sun 90°/0°, 0.1 H": 0.81949,
        "the measured Earth, 10⁷ m, sun 90°/0°, 0.3 H": 0.34241,
        "the measured Earth, 10⁷ m, sun 90°/0°, 1 H": 0.13669,
        "the measured Earth, 10⁷ m, sun 100°/0°, 0.1 H": 0.956,
        "the measured Earth, 10⁷ m, sun 100°/0°, 0.3 H": 0.72891,
        "the measured Earth, 10⁷ m, sun 100°/0°, 1 H": 0.31771,
        "Saturn-class, 10⁸ m, sun 90°/0°, 0.1 H": 0.12257,
        "Saturn-class, 10⁸ m, sun 90°/0°, 0.3 H": 0.23224,
        "Saturn-class, 10⁸ m, sun 90°/0°, 1 H": 0.20766,
        "Saturn-class, 10⁸ m, sun 100°/0°, 0.1 H": 0.12407,
        "Saturn-class, 10⁸ m, sun 100°/0°, 0.3 H": 0.42288,
        "Saturn-class, 10⁸ m, sun 100°/0°, 1 H": 0.56581,
        "Saturn-class, 10⁸ m, sun 100°/0°, 3 H": 0.12182,
      }),
    );
  });
});

describe("the kernel's own quadrature on the limb set, e_q (R08.T8.b)", () => {
  /** One limb-set ray's e_q, and its reference's steps and convergence. */
  interface QuadratureError {
    readonly name: string;
    readonly e: number;
    readonly steps: number;
    readonly converged: number;
  }
  const results = new Map<string, QuadratureError[]>();

  /**
   * The converged march of a ray: R05.T12.e's placed steps, as the gate's references take them
   * (the camera side placed about the lowest point from outside the air, no warp).
   */
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

  // A limb chord seen from outside the air is the same at every distance, so e_q is measured once
  // a body, from its nearest distance, where its pixels come nearest the 5%. The image's sun is set
  // at the grazing ray's tangent point, up to 0.7° from where it stands at a ray's own (10 H from
  // 10⁷ m). 3.0 s at four workers on a loaded machine (2026-10-21); the limit is over ten times
  // that.
  beforeAll(() => {
    for (const body of BODIES) {
      const { planet } = builtOf(body);
      const distanceM = body.distancesM[0] ?? Number.NaN;
      const figure = sphereOf(body.radiusM);
      for (const setting of ["high", "low"] as const) {
        const sizes = TABLE_SIZES[setting];
        const viewport = OUTPUT[setting];
        const x = viewport.widthPx / 2;
        const y = viewport.heightPx / 2;
        const items = SUNS.flatMap(([zenithDeg, azimuthDeg]) => {
          const sun = imageSun(distanceM, body.radiusM, zenithDeg, azimuthDeg);
          const rays = LIMB_SET.map((scaleHeights) => {
            const pitch = limbPitchRad(
              distanceM,
              body.radiusM,
              scaleHeights * body.scaleHeightM,
              viewport,
              x,
              y,
            );
            const view = twinView(
              limbCamera(distanceM, pitch, viewport),
              sun,
              figure,
              NEAR_PLANE_M,
            );
            const u = (x + 0.5) / viewport.widthPx;
            const v = (y + 0.5) / viewport.heightPx;
            const marched = rayMarchRayTwin(planet.medium, view, u, v, 0, sizes);
            if (marched === null) {
              throw new Error(`the ${scaleHeights} H ray from ${distanceM} m misses the air`);
            }
            return {
              scaleHeights,
              view,
              marched,
              kernel: rgbOf(rayMarchTexelTwin(planet, view, sizes, marched)),
              reference: placedMarch(planet, view, marched, REFERENCE_STEPS),
              confirm: placedMarch(planet, view, marched, 2 * REFERENCE_STEPS),
            };
          });
          const lMax = brightestOf(rays.map((r) => r.reference));
          return rays.map((r) => {
            let steps = REFERENCE_STEPS;
            let { reference, confirm } = r;
            while (
              steps < MAX_REFERENCE_STEPS &&
              !(errorOf(reference, confirm, lMax, CONVERGENCE_FLOOR) < CONVERGED)
            ) {
              steps *= 2;
              reference = confirm;
              confirm = placedMarch(planet, r.view, r.marched, 2 * steps);
            }
            return {
              name: `sun ${zenithDeg}°/${azimuthDeg}°, ${r.scaleHeights} H`,
              e: errorOf(r.kernel, reference, lMax, FLOOR),
              steps,
              converged: errorOf(reference, confirm, lMax, CONVERGENCE_FLOOR),
            };
          });
        });
        results.set(`${body.name}, ${setting}`, items);
      }
    }
  }, 60_000);

  it("has a reference of 2,048 placed steps within 0.05% of 4,096 on every limb-set ray", () => {
    const unconverged = [...results.entries()].flatMap(([key, es]) =>
      es
        .filter((p) => !(p.steps === REFERENCE_STEPS && p.converged < CONVERGED))
        .map((p) => `${key}, ${p.name}`),
    );
    expect(unconverged).toEqual([]);
  });

  // Measured 2026-10-21. High's 36 steps hold every limb-set ray within 5% on all three bodies,
  // the worst at 0.1 H with the Sun 90–100° behind the camera (the gate's limb from 400 km stands
  // at 4.35% and its night limb at 4.69% on R05's Earth). Low's 16 steps put nine rays a body
  // over, the Saturn-class limb's worst at 1 H, sun 100°/0° (31.86%), over the 24.0% that
  // `LOW_TWIN_WORST.rayMarch` measured on Earth's media.
  it("records e_q per body and setting: the worst, where it is, the median and the count over 5%", () => {
    const record = Object.fromEntries(
      [...results.entries()].map(([key, es]) => {
        const worst = es.reduce((w, p) => (p.e > w.e ? p : w));
        return [key, { ...figuresOfErrors(es.map((p) => p.e)), at: worst.name }];
      }),
    );
    const at = (where: string, f: Figures): unknown => ({ ...recordedFigure(f), at: where });
    expect(record).toEqual({
      "R05's Earth, high": at("sun 100°/180°, 0.1 H", { worst: 0.04645, median: 0.0033, over: 0 }),
      "R05's Earth, low": at("sun 90°/0°, 0.1 H", { worst: 0.24039, median: 0.0107, over: 9 }),
      "the measured Earth, high": at("sun 100°/180°, 0.1 H", {
        worst: 0.04418,
        median: 0.00082,
        over: 0,
      }),
      "the measured Earth, low": at("sun 100°/180°, 0.1 H", {
        worst: 0.22169,
        median: 0.01938,
        over: 9,
      }),
      "Saturn-class, high": at("sun 90°/180°, 0.1 H", { worst: 0.01303, median: 0.00178, over: 0 }),
      "Saturn-class, low": at("sun 100°/0°, 1 H", { worst: 0.31862, median: 0.01014, over: 9 }),
    });
  });
});
