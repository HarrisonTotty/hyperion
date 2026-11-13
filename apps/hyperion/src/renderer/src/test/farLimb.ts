/**
 * The far limb's frames and references for the CPU tests of R08.T8.b and R08.T8.c (plan R08,
 * R08.T8; `decision-r08-t8-leans.md` §6.1): a camera at a distance from a body's centre, looking
 * past the limb at a 60° field across, each frame pitched so that one pixel's centre lies on one
 * ray of Design note 10's limb set, the image's sun set at the grazing ray's tangent point; the
 * march's drawn value at a pixel; and the footprint mean converged across the limb, the reference
 * T8.c's figures are taken against (`farLimb.test.ts`, `farLimbLow.test.ts`,
 * `farLimbFlicker.test.ts`).
 *
 * @remarks
 * Earth's media sit on WGS 84's equatorial radius as a sphere (6,378,137 m, NIMA TR8350.2), H the
 * Rayleigh term's 8,434.5 m (`earth.ts`); the Saturn-class medium (`test/atmosphereFixtures.ts`)
 * on Saturn's equatorial radius at 1 bar as a sphere (60,268 km, Archinal et al. 2018), H its own
 * isothermal 49.7 km, and only from 10⁸ and 10⁹ m, 10⁷ m being inside it.
 */

import { expect } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3 } from "../geometry/vec3";
import { quaternionFromRows } from "../view/camera/quaternion";
import { DEFAULT_FOV_DEG, NEAR_PLANE_M } from "../view/camera/projection";
import type { ViewSize } from "../view/engine/types";
import type { Rgb } from "../view/photometry/toneCurve";
import { type QualitySetting, SETTINGS } from "../view/quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  RAYLEIGH_SCALE_HEIGHT_M,
} from "../view/atmosphere/earth";
import {
  type AtmosphereCamera,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
} from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { AbsorberCurveSlots } from "../view/atmosphere/tables";
import {
  type PlanetTwin,
  rayMarchRayTwin,
  rayMarchTargetTexelTwin,
  rayMarchTexelTwin,
  type TwinView,
  twinView,
} from "../view/atmosphere/tablesCpu";
import { SATURN_SPHEROID, saturnClassMedium, saturnScaleHeightM } from "./atmosphereFixtures";
import { CONVERGED } from "./atmosphereGate";

const RAD = Math.PI / 180;
/** The colour channels, red, green and blue. */
export const CHANNELS = [0, 1, 2] as const;

/** Design note 10's floor, 20F: 6 × 10⁻³ of the image's brightest. */
export const FLOOR = 6e-3;

/** R05.T12.e's floor, at which the references' convergence is judged: 10⁻³ of the brightest. */
export const CONVERGENCE_FLOOR = 1e-3;

/** Design note 10's bound on a pixel. */
export const BOUND = 0.05;

/** The output at each setting: high's 1080p target (R05 Design note 21), low's rows at 16:9. */
export const OUTPUT: Readonly<Record<QualitySetting, ViewSize>> = {
  high: { widthPx: 1_920, heightPx: 1_080 },
  low: {
    widthPx: ((SETTINGS.low.terrain.renderHeightPx ?? Number.NaN) * 16) / 9,
    heightPx: SETTINGS.low.terrain.renderHeightPx ?? Number.NaN,
  },
};

/** The field across, rad. */
export const FOV_X_RAD = DEFAULT_FOV_DEG * RAD;

/**
 * The pixel each limb-set ray is aimed through, by how it is drawn: high's at the frame's middle,
 * and low's of either parity, whose picked texel stands 0.5 px below and right of an even pixel's
 * centre and 0.5 px above and left of an odd one's (`composite.wgsl`'s `rayMarched`).
 */
export const VARIANTS = {
  high: { setting: "high", xPx: 960, yPx: 540 },
  "low even": { setting: "low", xPx: 640, yPx: 360 },
  "low odd": { setting: "low", xPx: 641, yPx: 361 },
} as const;
/** How a limb-set pixel is drawn: on high, or on low at either parity. */
export type Variant = keyof typeof VARIANTS;
/** The variants in the records' order. */
export const VARIANT_NAMES: readonly Variant[] = ["high", "low even", "low odd"];

/** Design note 10's limb set, in scale heights. */
export const LIMB_SET = [0.1, 0.3, 1, 3, 10] as const;

/** The images' suns, zenith angle and azimuth at the grazing ray's tangent point, degrees. */
export const SUNS = [
  [30, 0],
  [30, 90],
  [30, 180],
  [90, 0],
  [90, 180],
  [100, 0],
  [100, 180],
] as const;

/** The sun whose centre lies on the limb, inside Design note 10's 3°: recorded, not held. */
export const ON_THE_LIMB = "sun 90°/0°";

/** A body the far limb is measured on, as a sphere. */
export interface FarBody {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots | undefined;
  readonly radiusM: number;
  readonly scaleHeightM: number;
  readonly distancesM: readonly number[];
}

/** WGS 84's equatorial radius a, m (NIMA TR8350.2). */
export const EARTH_M = 6_378_137;

/** The bodies the far limb is measured on: R05's Earth, the measured Earth and the Saturn-class figure. */
export const BODIES: readonly FarBody[] = [
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

/** A distance's name in the records. */
export const distanceName = (distanceM: number): string =>
  DISTANCE_NAMES[distanceM] ?? `${distanceM.toPrecision(4)} m`;

/** A body's datum as a sphere. */
export const sphereOf = (radiusM: number): SpheroidFigure => ({
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

/** A point's ray's offsets on the image plane, right and up, at the field's tangents. */
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
export function limbPitchRad(
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
export function pixelTangentM(
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
export function limbCamera(
  distanceM: number,
  pitchRad: number,
  viewport: ViewSize,
): AtmosphereCamera {
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
 * The unit direction to a sun at zenith angle `zenithDeg` and azimuth `azimuthDeg` (from the
 * horizontal of `d`, toward `q × d`'s side) at the point `q` of a ray along `d`, as the gate sets
 * a limb ray's (`test/atmosphereGate.ts`'s `sunAt`).
 */
function sunAtPoint(q: Vec3, d: Vec3, zenithDeg: number, azimuthDeg: number): Vec3 {
  const n = normalise(q);
  const horizontal = (v: Vec3): Vec3 => add(v, scale(n, -dot(v, n)));
  const towards = horizontal(d);
  const f = normalise(dot(towards, towards) > 1e-12 ? towards : horizontal({ x: 1, y: 0, z: 0 }));
  const y = cross(n, f);
  const h = add(scale(f, Math.cos(azimuthDeg * RAD)), scale(y, Math.sin(azimuthDeg * RAD)));
  return normalise(add(scale(n, Math.cos(zenithDeg * RAD)), scale(h, Math.sin(zenithDeg * RAD))));
}

/**
 * An image's sun, unit light: at its zenith angle and azimuth at the tangent point of the ray from
 * a camera `distanceM` from the centre on +x that grazes the sphere in the plane z = 0.
 */
export function imageSun(
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
    directionBodyFixed: sunAtPoint(q, d, zenithDeg, azimuthDeg),
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.004_65,
  };
}

/** A frame's view: the camera pitched by `pitchRad` at `distanceM`, under `sun`, on the sphere. */
export function limbView(
  body: FarBody,
  distanceM: number,
  pitchRad: number,
  viewport: ViewSize,
  sun: SunState,
): TwinView {
  return twinView(
    limbCamera(distanceM, pitchRad, viewport),
    sun,
    sphereOf(body.radiusM),
    NEAR_PLANE_M,
  );
}

/** A texel's RGB. */
export const rgbOf = (texel: readonly [number, number, number, number]): Rgb => [
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
export function pickedTexel(pixel: number, outputPx: number, rayMarchScale: number): number {
  if (rayMarchScale >= 1) {
    return pixel;
  }
  const size = Math.ceil(outputPx * rayMarchScale);
  return Math.min(Math.max(Math.floor((pixel + 0.5) * rayMarchScale), 0), size - 1);
}

/**
 * The march's drawn value at an output pixel since R08.T8.c, RGBA: the picked texel's, as the
 * kernel draws it ({@link rayMarchTargetTexelTwin}: its centre ray, and its footprint's mean
 * blended in by w_L from above the top), nothing drawn.
 */
export function drawnAt(
  planet: PlanetTwin,
  view: TwinView,
  setting: QualitySetting,
  xPx: number,
  yPx: number,
): readonly [number, number, number, number] {
  const sizes = TABLE_SIZES[setting];
  const { widthPx: w, heightPx: h } = view.viewport;
  const s = sizes.rayMarchScale;
  return rayMarchTargetTexelTwin(
    planet,
    view,
    sizes,
    pickedTexel(xPx, w, s),
    pickedTexel(yPx, h, s),
    0,
  );
}

/**
 * The picked texel's centre ray alone, RGBA: what the march drew at an output pixel before
 * R08.T8.c.
 */
export function centreAt(
  planet: PlanetTwin,
  view: TwinView,
  setting: QualitySetting,
  xPx: number,
  yPx: number,
): readonly [number, number, number, number] {
  const sizes = TABLE_SIZES[setting];
  const { widthPx: w, heightPx: h } = view.viewport;
  const s = sizes.rayMarchScale;
  const u = (pickedTexel(xPx, w, s) + 0.5) / Math.ceil(w * s);
  const v = (pickedTexel(yPx, h, s) + 0.5) / Math.ceil(h * s);
  return rayMarchTexelTwin(
    planet,
    view,
    sizes,
    rayMarchRayTwin(planet.medium, view, u, v, 0, sizes),
  );
}

/** Where a pixel's rows meet the top and the datum's edge, each a fraction of the pixel down. */
export interface PixelCuts {
  readonly topY: number;
  readonly groundY: number;
}

/**
 * Where the rows of output pixel (`xPx`, `yPx`) of a {@link limbCamera}'s frame meet the top and
 * the datum's edge, each as its fraction of the pixel down from its top edge in [0, 1]: the
 * tangent height is linear down the pixel's middle column (the limb level across the frame, its
 * second derivative a few metres across a pixel).
 */
export function pixelCuts(
  body: FarBody,
  distanceM: number,
  pitchRad: number,
  viewport: ViewSize,
  xPx: number,
  yPx: number,
): PixelCuts {
  const above = pixelTangentM(distanceM, body.radiusM, pitchRad, viewport, xPx + 0.5, yPx);
  const below = pixelTangentM(distanceM, body.radiusM, pitchRad, viewport, xPx + 0.5, yPx + 1);
  const yOf = (heightM: number): number =>
    Math.min(Math.max((heightM - above) / (below - above), 0), 1);
  return { topY: yOf(body.medium.topHeightM), groundY: yOf(0) };
}

/** The columns of a pixel the reference's rows sample, its fractions across. */
const REFERENCE_COLUMNS = [0.25, 0.75] as const;

/** The ground piece's rows for a shell piece of n: n ÷ 8, at least 8. */
const groundRowsOf = (rows: number): number => Math.max(rows / 8, 8);

/**
 * The twin's mean over output pixel (`xPx`, `yPx`)'s footprint, RGBA, across the limb: the pixel
 * cut at the top and the datum's edge, its shell piece taken in `rows` midpoint rows and its ground
 * piece in {@link groundRowsOf} rows, each at two columns (a quarter and three quarters across),
 * each piece weighted by its share; above the top nothing scatters and the light passes, (0, 0, 0,
 * 1), exactly. Each sub-ray is the march's own ray at the setting's count
 * ({@link rayMarchTexelTwin}).
 */
export function footprintMeanOf(
  planet: PlanetTwin,
  view: TwinView,
  setting: QualitySetting,
  xPx: number,
  yPx: number,
  cuts: PixelCuts,
  rows: number,
): readonly [number, number, number, number] {
  const sizes = TABLE_SIZES[setting];
  const { widthPx: w, heightPx: h } = view.viewport;
  const mean = [0, 0, 0, cuts.topY];
  const piece = (from: number, to: number, n: number): void => {
    if (!(to > from)) {
      return;
    }
    const share = (to - from) / (n * REFERENCE_COLUMNS.length);
    for (let j = 0; j < n; j += 1) {
      const v = (yPx + from + ((j + 0.5) / n) * (to - from)) / h;
      for (const column of REFERENCE_COLUMNS) {
        const u = (xPx + column) / w;
        const texel = rayMarchTexelTwin(
          planet,
          view,
          sizes,
          rayMarchRayTwin(planet.medium, view, u, v, 0, sizes),
        );
        for (let c = 0; c < 4; c += 1) {
          mean[c] = (mean[c] ?? Number.NaN) + share * (texel[c] ?? Number.NaN);
        }
      }
    }
  };
  piece(cuts.topY, cuts.groundY, rows);
  piece(cuts.groundY, 1, groundRowsOf(rows));
  return [
    mean[0] ?? Number.NaN,
    mean[1] ?? Number.NaN,
    mean[2] ?? Number.NaN,
    mean[3] ?? Number.NaN,
  ];
}

/** The reference's shell rows at first, doubled until it converges. */
export const REFERENCE_ROWS = 128;

/** The most shell rows a reference takes, confirmed against twice as many. */
export const MAX_REFERENCE_ROWS = 2_048;

/** A pixel's converged footprint mean: its RGBA, its rows, and its e against half as many. */
export interface ConvergedFootprint {
  readonly mean: readonly [number, number, number, number];
  readonly rows: number;
  readonly converged: number;
}

/**
 * The footprint means of an image's pixels converged across the limb, as the gate's references
 * are (`test/atmosphereGate.ts`): each pixel's shell rows doubled from {@link REFERENCE_ROWS} until
 * its mean is within `CONVERGED` (0.05%) of the mean at half as many rows, at
 * {@link CONVERGENCE_FLOOR} of the image's brightest, or {@link MAX_REFERENCE_ROWS} is reached; the
 * mean returned is the finer of the two.
 */
export function convergedFootprints(
  meanAt: ReadonlyArray<(rows: number) => readonly [number, number, number, number]>,
): ConvergedFootprint[] {
  let coarse = meanAt.map((at) => at(REFERENCE_ROWS));
  let fine = meanAt.map((at) => at(2 * REFERENCE_ROWS));
  const rows = meanAt.map(() => 2 * REFERENCE_ROWS);
  const lMax = brightestOf(fine.map(rgbOf));
  const convergedOf = (i: number): number =>
    errorOf(
      rgbOf(coarse[i] ?? [0, 0, 0, 0]),
      rgbOf(fine[i] ?? [0, 0, 0, 0]),
      lMax,
      CONVERGENCE_FLOOR,
    );
  for (let i = 0; i < meanAt.length; i += 1) {
    while (!(convergedOf(i) < CONVERGED) && (rows[i] ?? 0) < MAX_REFERENCE_ROWS) {
      const next = 2 * (rows[i] ?? Number.NaN);
      coarse = coarse.with(i, fine[i] ?? [0, 0, 0, 0]);
      fine = fine.with(i, meanAt[i]?.(next) ?? [0, 0, 0, 0]);
      rows[i] = next;
    }
  }
  return fine.map((mean, i) => ({ mean, rows: rows[i] ?? Number.NaN, converged: convergedOf(i) }));
}

/** e = max over channels of |L − M| ÷ max(M, floor · L_max) (Design note 10's metric). */
export function errorOf(l: Rgb, m: Rgb, lMax: Rgb, floor: number): number {
  let e = 0;
  for (const c of CHANNELS) {
    e = Math.max(e, Math.abs(l[c] - m[c]) / Math.max(m[c], floor * lMax[c]));
  }
  return e;
}

/** Each channel's largest over `values`. */
export const brightestOf = (values: readonly Rgb[]): Rgb => [
  Math.max(...values.map((v) => v[0])),
  Math.max(...values.map((v) => v[1])),
  Math.max(...values.map((v) => v[2])),
];

/** The median of `values`. */
export const medianOf = (values: readonly number[]): number => {
  const sorted = values.toSorted((x, y) => x - y);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? Number.NaN)
    : ((sorted[mid - 1] ?? Number.NaN) + (sorted[mid] ?? Number.NaN)) / 2;
};

/** A set of e's figures: the worst, the median, and how many stand over the 5%. */
export interface Figures {
  readonly worst: number;
  readonly median: number;
  readonly over: number;
}

/** The worst, the median and the count over {@link BOUND} of a set of e. */
export const figuresOfErrors = (es: readonly number[]): Figures => ({
  worst: Math.max(...es),
  median: medianOf(es),
  over: es.filter((e) => !(e <= BOUND)).length,
});

/** Figures as recorded: each within 5 × 10⁻⁵ of its value. */
export const asRecorded = (figures: Readonly<Record<string, number>>): unknown =>
  Object.fromEntries(
    Object.entries(figures).map(([key, figure]) => [key, expect.closeTo(figure, 4)]),
  );

/** A set of figures as recorded: the worst and the median within 5 × 10⁻⁵, the count exactly. */
export const recordedFigure = (f: Figures): Record<keyof Figures, unknown> => ({
  worst: expect.closeTo(f.worst, 4),
  median: expect.closeTo(f.median, 4),
  over: f.over,
});

/** Each place's set of figures as recorded ({@link recordedFigure}). */
export const recordedFigures = (figures: Readonly<Record<string, Figures>>): unknown =>
  Object.fromEntries(Object.entries(figures).map(([key, f]) => [key, recordedFigure(f)]));

/** Every body at every distance it is measured from. */
export const PLACES = BODIES.flatMap((body) =>
  body.distancesM.map((distanceM) => ({ body, distanceM })),
);
