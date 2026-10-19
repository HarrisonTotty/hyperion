/**
 * The disc regime's per-body record and the TypeScript twin of `shaders/bodyDisc.wgsl` (plan R07,
 * T8.a, T8.b and T11; Design notes 2, 6, 7, 10, 19 and 24).
 *
 * @remarks
 * A disc is one screen rectangle whose fragments intersect their rays with the body's spheroid
 * (the polar axis scaled by a ÷ c makes it a sphere, the normal taken by the inverse transpose)
 * and shade each hit with the body's law: I/F = A f(α) [L(α) · 2h ÷ (μ₀ + μ) + (1 − L(α)) h], f
 * and L(α) from its phase row, where h, the horizon term (`sphereIrradianceFactor`), stands for the
 * law's μ₀ and equals it wherever the whole star is up; each star's light is cut by the eclipse term of up to two occluders and summed
 * over up to two stars, and up to two lit neighbours add their planetshine through the same law,
 * each a uniform sphere through `planetshineIrradiance` (T11, Design note 7), never eclipsed. The
 * law is its `DiscSurface`'s: one law, or under a class map each class's law weighted by the
 * class's share at the hit and the uniform law by what is left (`discSurface.ts`). Every length is divided before it reaches `f32`: the centre is a unit
 * direction with a ÷ D, and every light and occluder is relative to the body's centre over a.
 * A pixel's n × n cells are set by the disc's size ({@link discSamples}): {@link FINE_DISC_SAMPLES}
 * per axis on a disc under {@link FINE_DISC_PX}, so that its summed flux meets the point's at the
 * 3 px switch; {@link SMALL_DISC_SAMPLES} per axis from there to under {@link SMALL_DISC_PX}; on a
 * larger disc one inside and {@link LIMB_SAMPLES} per axis on the limb. A pixel whose four corners all
 * meet the body is drawn opaque with its meter class; one on the limb premultiplied by its
 * coverage, keeping the class beneath (decision-r07-t8a, item 3). Near the limb a cell's light is
 * integrated across its profile in √δ, the depth inside the limb (see {@link rasteriseDisc}).
 * Under {@link SMALL_DISC_PX} a pixel's cells are shaded at once by the `disc cells` pass, one
 * invocation a cell, and summed there in the draws' order ({@link discCellJobs}, R07.T8.d); the
 * draws read the pixel's sums through row 51. The arithmetic is the draws' own.
 * {@link rasteriseDisc} runs the shader's arithmetic in `f64`.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { ClassMapDiscSurface, UniformDiscSurface } from "../appearance/bodyAppearance";
import {
  AIRLESS_LIGHTING,
  type SurfaceLightingReader,
  type SurfaceLightingTables,
} from "../atmosphere/surfaceLighting";
import {
  limbShareFromTable,
  phaseFactorFromTable,
  type PhaseFactorTable,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "../appearance/law";
import { type ProjectionCamera, toViewAxes, type Viewport } from "../camera/projection";
import type { AnnulusSet } from "../lighting/annuli";
import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import { annulusVisibleFraction } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH, planetshineIrradiance } from "../lighting/planetshine";
import { sphereIrradianceFactor } from "../lighting/sphereIrradiance";
import type { Rgb } from "../photometry/toneCurve";
import { HALF_FLOAT_MAX } from "../photometry/toneCurve";
import { METER_CLASS, type MeterClass } from "../post/meter";
import { type Rotation3, rotateToBody } from "../coords/rotation";
import type { ScreenRect } from "../wireframe/submit";
import {
  classMapSharesAt,
  type ClassMapTexels,
  discSurfaceLaws,
  MAX_DISC_CLASSES,
} from "./discSurface";
import { oblateAlbedoScale } from "./oblate";

/**
 * A sample is lit directly where a star's horizon and eclipse terms exceed this share of the
 * face-on irradiance, so that `f32` and `f64` agree at the terminator's edge.
 */
export const LIT_IRRADIANCE = 1e-5;

/** `vec4f` rows per disc in the shader's `discs` buffer. */
export const DISC_ROWS = 52;

/** The stars that light one disc at most: the brightest two, `MAX_BODY_LIGHTS`. */
export const MAX_DISC_LIGHTS = MAX_BODY_LIGHTS;

/** The occluders one disc's light is cut by at most: the two largest seen from it. */
export const MAX_DISC_OCCLUDERS = 2;

/** The lit neighbours that light one disc by planetshine at most: the high setting's count. */
export const MAX_DISC_SECONDARIES = PLANETSHINE_SOURCES_HIGH;

/**
 * A disc under this diameter, px, is sampled {@link FINE_DISC_SAMPLES}² times in every pixel: the
 * discs that can meet their point at the 3 px switch, 0.7 px above its band's top, 3.3 px
 * (decision-r07-small-disc-cost).
 */
export const FINE_DISC_PX = 4;

/**
 * Samples per axis in every pixel of a disc under {@link FINE_DISC_PX}: of the counts measured (4,
 * 5, 6 and 8 per axis), the only one that meets the point there to 1% (6 × 6 errs by 1.15%, 4 × 4
 * by 1.38%).
 */
export const FINE_DISC_SAMPLES = 8;

/**
 * A disc under this diameter, px, is sampled in every pixel, {@link SMALL_DISC_SAMPLES}² times from
 * {@link FINE_DISC_PX} up.
 */
export const SMALL_DISC_PX = 32;

/**
 * Samples per axis in every pixel of a disc from {@link FINE_DISC_PX} to under
 * {@link SMALL_DISC_PX}: within T8.a's 1% of the exact flux there, as 8 × 8 is (worst 0.52%
 * against 0.49%), for a quarter of its serial shades.
 */
export const SMALL_DISC_SAMPLES = 4;

/** Samples per axis in a larger disc's limb pixels. */
export const LIMB_SAMPLES = 4;

/** Samples per axis in a disc's wholly covered pixels and in its limb pixels. */
export interface DiscSamples {
  readonly interior: number;
  readonly limb: number;
}

/**
 * The cells per axis a disc `diameterPx` across takes in its pixels (R07.T8.c,
 * decision-r07-small-disc-cost).
 *
 * @remarks
 * Two levels below {@link SMALL_DISC_PX}: {@link FINE_DISC_SAMPLES} in every pixel under
 * {@link FINE_DISC_PX}, {@link SMALL_DISC_SAMPLES} in every pixel from there; one inside and
 * {@link LIMB_SAMPLES} on the limb above. A pure function of the frame, with no hysteresis: a disc
 * crossing 4 px changes its flux by up to 0.41%, one crossing 32 px by up to 0.29%, under half an
 * 8-bit step at white. A diameter that is not a number takes the large disc's counts, as an
 * unbounded one does.
 *
 * @param diameterPx - The disc's angular diameter at the centre pixel's scale
 *   (`angularDiameterPx`), px.
 */
export function discSamples(diameterPx: number): DiscSamples {
  if (diameterPx < FINE_DISC_PX) {
    return { interior: FINE_DISC_SAMPLES, limb: FINE_DISC_SAMPLES };
  }
  if (diameterPx < SMALL_DISC_PX) {
    return { interior: SMALL_DISC_SAMPLES, limb: SMALL_DISC_SAMPLES };
  }
  return { interior: 1, limb: LIMB_SAMPLES };
}

/** The display channels' indices, r, g, b. */
const CHANNELS = [0, 1, 2] as const;

const FIRST_LIGHT_ROW = 6;
const LIGHT_ROWS = 8;
const FIRST_OCCLUDER_ROW = 22;
const AXES_ROW = 24;
const FIRST_CLASS_ROW = 26;
const FIRST_CLASS_TABLE_ROW = FIRST_CLASS_ROW + MAX_DISC_CLASSES;
const SECONDARY_COUNT_ROW = 46;
const FIRST_SECONDARY_ROW = 47;
const SECONDARY_ROWS = 2;
const CELL_SUMS_ROW = 51;

/**
 * Whether a record's pixels can take their sums from the `disc cells` pass (R07.T8.d): every pixel
 * of it takes the same cells, at most {@link FINE_DISC_SAMPLES} per axis, as {@link discSamples}
 * gives every disc under {@link SMALL_DISC_PX}, its surface is one law, and its body has no
 * atmosphere.
 *
 * @remarks
 * A class map's disc sums its own cells: the pass binds one class map for all its records, and none
 * is live before R10. So does a disc with an atmosphere (plan R08, R08.T9.b): the pass binds the
 * one-texel defaults of a body without one, and no body's atmosphere is drawn before R08.T10.b.
 */
export function drawnThroughCells(record: DiscRecord): boolean {
  const n = record.interiorSamples;
  return (
    n === record.limbSamples &&
    Number.isInteger(n) &&
    n >= 1 &&
    n <= FINE_DISC_SAMPLES &&
    record.surface.kind === "uniform" &&
    record.atmosphere === undefined
  );
}

/** Where the `disc cells` pass leaves one record's pixels' sums. */
export interface DiscCellRange {
  /** The index in `cell_sums` of its first pixel's sums, two `vec4f` a pixel. */
  readonly first: number;
  /** The rectangle summed, ⌊left⌋ and ⌊top⌋, px. */
  readonly leftPx: number;
  readonly topPx: number;
  /** Its width and height, ⌈right⌉ − ⌊left⌋ and ⌈bottom⌉ − ⌊top⌋, px. */
  readonly widthPx: number;
  readonly heightPx: number;
}

/** A frame's jobs for the `disc cells` pass: one a pixel of each small disc's rectangle. */
export interface DiscCellJobs {
  /**
   * Four `i32` a job, the shader's `vec4i`: the record's index, the pixel's x and y, px, and 0; the
   * records in their order, each rectangle's pixels in row order.
   */
  readonly jobs: Int32Array;
  /** The jobs J, the pixels whose sums the pass leaves, in the jobs' order. */
  readonly count: number;
  /** Each record's sums, by index; `null` for a record whose draws sum their own cells. */
  readonly ranges: ReadonlyArray<DiscCellRange | null>;
}

/** The jobs a frame takes at most: 2²⁴, past which row 51's `f32` would lose a first index. */
export const MAX_CELL_JOBS = 2 ** 24;

/**
 * The `disc cells` pass's jobs for a frame's records (R07.T8.d, decision-r07-small-disc-cost §1.2).
 *
 * @remarks
 * Each record {@link drawnThroughCells} takes one job a pixel of [⌊left⌋, ⌈right⌉) ×
 * [⌊top⌋, ⌈bottom⌉), the rectangle {@link rasteriseDisc} walks: a superset of the pixels either of
 * its draws shades. Job j's sums are the j-th in `cell_sums`, so a record's pixel (x, y) finds its
 * own at first + (y − top) w + (x − left), which row 51 carries. A record whose pixels would take
 * the jobs past `maxJobs` takes none, and its draws sum their own cells, as before the pass.
 *
 * @param maxJobs - The jobs at most: {@link MAX_CELL_JOBS}, or fewer where the device's buffers
 *   hold fewer pixels' sums.
 */
export function discCellJobs(
  records: ReadonlyArray<DiscRecord>,
  maxJobs: number = MAX_CELL_JOBS,
): DiscCellJobs {
  const most = Math.min(maxJobs, MAX_CELL_JOBS);
  let count = 0;
  const ranges = records.map((record): DiscCellRange | null => {
    if (!drawnThroughCells(record)) {
      return null;
    }
    const { rect } = record;
    const leftPx = Math.floor(rect.leftPx);
    const topPx = Math.floor(rect.topPx);
    const widthPx = Math.max(0, Math.ceil(rect.rightPx) - leftPx);
    const heightPx = Math.max(0, Math.ceil(rect.bottomPx) - topPx);
    if (count + widthPx * heightPx > most) {
      return null;
    }
    const range = { first: count, leftPx, topPx, widthPx, heightPx };
    count += widthPx * heightPx;
    return range;
  });
  const jobs = new Int32Array(count * 4);
  ranges.forEach((range, index) => {
    if (range === null) {
      return;
    }
    for (let row = 0; row < range.heightPx; row += 1) {
      for (let column = 0; column < range.widthPx; column += 1) {
        jobs.set(
          [index, range.leftPx + column, range.topPx + row, 0],
          (range.first + row * range.widthPx + column) * 4,
        );
      }
    }
  });
  return { jobs, count, ranges };
}

/** One star lighting a disc, relative to the body's centre. */
export interface DiscLight {
  /** The star's unit direction from the body's centre, along the galactic axes. */
  readonly direction: Vec3;
  /** Its distance from the body's centre ÷ a. */
  readonly distance: number;
  /** Its radius ÷ a. */
  readonly radius: number;
  /** Its illuminance face-on at the body, lx, per display channel (r, g, b). */
  readonly illuminance: Rgb;
  /** Its annuli per display channel (r, g, b), each of the frame's K. */
  readonly annuli: readonly [AnnulusSet, AnnulusSet, AnnulusSet];
}

/** One lit neighbour lighting a disc by planetshine, relative to the body's centre (Design note 7). */
export interface DiscSecondary {
  /** The neighbour's unit direction from the body's centre, along the galactic axes. */
  readonly direction: Vec3;
  /** Its distance from the body's centre ÷ a. */
  readonly distance: number;
  /** Its radius ÷ a. */
  readonly radius: number;
  /** Its illuminance face-on at the body's centre, lx, per display channel (r, g, b). */
  readonly illuminance: Rgb;
}

/** One body that may eclipse a disc's stars, relative to the body's centre ÷ a. */
export interface DiscOccluder {
  readonly centre: Vec3;
  readonly radius: number;
}

/** A class map oriented by its body's rotation, as a disc draws it. */
export interface OrientedClassMap extends ClassMapDiscSurface {
  /** The rotation from the body-fixed axes to the galactic axes. */
  readonly rotation: Rotation3;
}

/** A disc's surface as it is drawn: one law, or a class map oriented on the body. */
export type DrawnDiscSurface = UniformDiscSurface | OrientedClassMap;

/** Everything one disc draw reads, before it is packed into `f32`. */
export interface DiscRecord {
  /** The body drawn. */
  readonly body: BodyIdHex;
  readonly rect: ScreenRect;
  /** The centre's unit direction from the camera, along the galactic axes. */
  readonly direction: Vec3;
  /** a ÷ D. */
  readonly radiusOverDistance: number;
  /** The pole's unit direction, along the galactic axes. */
  readonly pole: Vec3;
  /** c ÷ a, 1 for a sphere. */
  readonly polarOverEquatorial: number;
  /** Samples per axis in a wholly covered pixel and in a limb pixel. */
  readonly interiorSamples: number;
  readonly limbSamples: number;
  /**
   * What the disc shades with: one law, or a class map with a law per class (Design note 24).
   * Each law's A takes the factor that makes an oblate disc reach p equator-on at zero phase
   * (`oblateAlbedoScale` of its L), 1 for a sphere.
   */
  readonly surface: DrawnDiscSurface;
  /** The rows of the frame's phase table holding `discSurfaceLaws(surface)`, in its order. */
  readonly tableRows: ReadonlyArray<number>;
  /** The exposure scale over π, so that each pixel holds pre-exposed luminance. */
  readonly exposureOverPi: number;
  readonly lights: ReadonlyArray<DiscLight>;
  readonly occluders: ReadonlyArray<DiscOccluder>;
  /** Its planetshine sources (`planetshineSources`), the brightest first. */
  readonly secondaries: ReadonlyArray<DiscSecondary>;
  /**
   * Its body's atmosphere tables, which its draws bind (plan R08, R08.T9.b); absent for a body
   * without one, whose draws bind the defaults. The twin reads a body's lighting through a
   * {@link SurfaceLightingReader} given beside the record, as it reads a class map's texels.
   */
  readonly atmosphere?: SurfaceLightingTables;
}

/** The pole's stretch M x = x + (a ÷ c − 1)(x · p) p. */
function stretchAlong(pole: Vec3, stretch: number, x: Vec3): Vec3 {
  const along = (stretch - 1) * dot(x, pole);
  return vec3(x.x + along * pole.x, x.y + along * pole.y, x.z + along * pole.z);
}

/** The annuli in use, shared by every light of a frame: the first light's count. */
function annulusCount(record: DiscRecord): number {
  return record.lights[0]?.annuli[0].flux.length ?? 0;
}

/**
 * One of a record's laws as the shader holds it: A scaled per channel, L(0) and its phase table,
 * whose alpha holds its L(α).
 */
interface ScaledLaw {
  readonly a: Rgb;
  /** L(0). */
  readonly share: number;
  readonly table: PhaseFactorTable;
  readonly tableRow: number;
}

/**
 * A record's laws in `discSurfaceLaws`' order, each A scaled for the figure.
 *
 * @throws Error if the surface has more than `MAX_DISC_CLASSES` classes, or a table row is missing.
 */
function scaledLaws(record: DiscRecord): ScaledLaw[] {
  const laws = discSurfaceLaws(record.surface);
  if (laws.length > MAX_DISC_CLASSES + 1) {
    throw new Error(
      `a disc shades with at most ${MAX_DISC_CLASSES} classes, got ${laws.length - 1}`,
    );
  }
  return laws.map((law, m) => {
    const tableRow = record.tableRows[m];
    if (tableRow === undefined) {
      throw new Error(`law ${m} of disc ${record.body} has no row of the phase table`);
    }
    return scaledLawOf(law, record.polarOverEquatorial, tableRow);
  });
}

/** A law as the shader holds it, its A scaled for a figure of c ÷ a `polarOverEquatorial`. */
function scaledLawOf(
  law: PhotometricLaw,
  polarOverEquatorial: number,
  tableRow: number,
): ScaledLaw {
  const k = oblateAlbedoScale(law.lommelSeeligerShare, polarOverEquatorial);
  return {
    a: [law.a[0] * k, law.a[1] * k, law.a[2] * k],
    share: law.lommelSeeligerShare,
    table: phaseFactorTableOf(law),
    tableRow,
  };
}

/** The body-fixed x and y axes along the galactic axes: the rotation's first two columns. */
function bodyAxesOf(rotation: Rotation3): readonly [Vec3, Vec3] {
  return [rotateToBody(rotation, vec3(1, 0, 0)), rotateToBody(rotation, vec3(0, 1, 0))];
}

/**
 * Packs the frame's discs as the shader's `array<vec4f>`, {@link DISC_ROWS} rows each.
 *
 * @param cells - The `disc cells` pass's jobs for these records, whose ranges row 51 carries; `null`
 *   where no pass runs, when every record's row 51 says its draws sum their own cells.
 * @throws Error if `cells` was made for another number of records.
 */
export function packDiscRecords(
  records: ReadonlyArray<DiscRecord>,
  cells: DiscCellJobs | null = null,
): Float32Array {
  if (cells !== null && cells.ranges.length !== records.length) {
    throw new Error(
      `the cell pass's jobs are for ${cells.ranges.length} records, not ${records.length}`,
    );
  }
  const out = new Float32Array(records.length * DISC_ROWS * 4);
  records.forEach((record, index) => {
    const base = index * DISC_ROWS * 4;
    const put = (rowIndex: number, values: ReadonlyArray<number>): void => {
      out.set(values, base + rowIndex * 4);
    };
    const { rect, direction: u, pole } = record;
    put(0, [rect.leftPx, rect.topPx, rect.rightPx, rect.bottomPx]);
    put(1, [u.x, u.y, u.z, record.radiusOverDistance]);
    put(2, [pole.x, pole.y, pole.z, record.polarOverEquatorial]);
    put(3, [
      record.interiorSamples,
      record.limbSamples,
      Math.min(record.lights.length, MAX_DISC_LIGHTS),
      Math.min(record.occluders.length, MAX_DISC_OCCLUDERS),
    ]);
    const [uniform, ...classes] = scaledLaws(record);
    if (uniform !== undefined) {
      put(4, [...uniform.a, uniform.share]);
    }
    put(5, [uniform?.tableRow ?? 0, annulusCount(record), record.exposureOverPi, classes.length]);
    if (record.surface.kind === "class-map") {
      const [x, y] = bodyAxesOf(record.surface.rotation);
      put(AXES_ROW, [x.x, x.y, x.z, 0]);
      put(AXES_ROW + 1, [y.x, y.y, y.z, 0]);
    }
    classes.forEach((law, k) => {
      put(FIRST_CLASS_ROW + k, [...law.a, law.share]);
      out[base + FIRST_CLASS_TABLE_ROW * 4 + k] = law.tableRow;
    });
    record.lights.slice(0, MAX_DISC_LIGHTS).forEach((light, j) => {
      const first = FIRST_LIGHT_ROW + j * LIGHT_ROWS;
      put(first, [light.direction.x, light.direction.y, light.direction.z, light.distance]);
      put(first + 1, [...light.illuminance, light.radius]);
      light.annuli.forEach((set, c) => {
        // DiscAnnuli: the outer edge of each annulus and its flux, at most four.
        put(
          first + 2 + 2 * c,
          [1, 2, 3, 4].map((edge) => set.edges[edge] ?? 1),
        );
        put(
          first + 3 + 2 * c,
          [0, 1, 2, 3].map((annulus) => set.flux[annulus] ?? 0),
        );
      });
    });
    record.occluders.slice(0, MAX_DISC_OCCLUDERS).forEach((occluder, slot) => {
      const c = occluder.centre;
      put(FIRST_OCCLUDER_ROW + slot, [c.x, c.y, c.z, occluder.radius]);
    });
    const secondaries = record.secondaries.slice(0, MAX_DISC_SECONDARIES);
    put(SECONDARY_COUNT_ROW, [secondaries.length, 0, 0, 0]);
    secondaries.forEach((source, j) => {
      const first = FIRST_SECONDARY_ROW + j * SECONDARY_ROWS;
      const d = source.direction;
      put(first, [d.x, d.y, d.z, source.distance]);
      put(first + 1, [...source.illuminance, source.radius]);
    });
    const range = cells?.ranges[index] ?? null;
    put(
      CELL_SUMS_ROW,
      range === null ? [-1, 0, 0, 0] : [range.first, range.leftPx, range.topPx, range.widthPx],
    );
  });
  return out;
}

/** A class map as one camera sees it: its texels and the body-fixed axes in the view's axes. */
interface ViewClassMap {
  readonly texels: ClassMapTexels;
  readonly axes: readonly [Vec3, Vec3, Vec3];
}

/** The record seen from one camera: everything in the view's axes. */
interface ViewBody {
  readonly centre: Vec3;
  readonly pole: Vec3;
  readonly stretch: number;
  readonly radius: number;
  readonly scaledCentre: Vec3;
  readonly lights: ReadonlyArray<{ readonly place: Vec3; readonly light: DiscLight }>;
  readonly occluders: ReadonlyArray<DiscOccluder>;
  readonly secondaries: ReadonlyArray<{ readonly place: Vec3; readonly source: DiscSecondary }>;
  readonly laws: ReadonlyArray<ScaledLaw>;
  /** `null` for a uniform surface. */
  readonly classMap: ViewClassMap | null;
  /** Its atmosphere's lookups (R08.T9.b), or a body's without one. */
  readonly lighting: SurfaceLightingReader;
}

function viewBodyOf(
  record: DiscRecord,
  camera: ProjectionCamera,
  texels: ClassMapTexels | null,
  lighting: SurfaceLightingReader,
): ViewBody {
  const toView = (v: Vec3): Vec3 => toViewAxes(v, camera.orientation);
  const centre = toView(record.direction);
  const pole = normalise(toView(record.pole));
  const stretch = 1 / record.polarOverEquatorial;
  const laws = scaledLaws(record);
  let classMap: ViewClassMap | null = null;
  if (record.surface.kind === "class-map") {
    if (texels === null || texels.classes !== record.surface.laws.length) {
      throw new Error(`the twin of class-map disc ${record.body} needs its map's texels`);
    }
    const [bodyX, bodyY] = bodyAxesOf(record.surface.rotation);
    const x = toView(bodyX);
    const y = toView(bodyY);
    classMap = { texels, axes: [x, y, cross(x, y)] };
  }
  return {
    centre,
    pole,
    stretch,
    radius: record.radiusOverDistance,
    scaledCentre: stretchAlong(pole, stretch, centre),
    laws,
    classMap,
    lighting,
    lights: record.lights
      .slice(0, MAX_DISC_LIGHTS)
      .map((light) => ({ place: scale(toView(light.direction), light.distance), light })),
    occluders: record.occluders
      .slice(0, MAX_DISC_OCCLUDERS)
      .map((occluder) => ({ centre: toView(occluder.centre), radius: occluder.radius })),
    secondaries: record.secondaries
      .slice(0, MAX_DISC_SECONDARIES)
      .map((source) => ({ place: scale(toView(source.direction), source.distance), source })),
  };
}

/** The ray through a point of the view, px, in view space, as `view_ray` builds it. */
export function viewRay(
  xPx: number,
  yPx: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): Vec3 {
  const s = 1 / Math.tan(camera.fovXRad / 2);
  return rayThrough(xPx, yPx, viewport, s, s * (viewport.widthPx / viewport.heightPx));
}

/**
 * {@link viewRay} for one camera and viewport, its projection's scales taken once: the rasterisers
 * cast millions of rays through one view, and the tangent is the same for each.
 */
function viewRays(
  camera: ProjectionCamera,
  viewport: Viewport,
): (xPx: number, yPx: number) => Vec3 {
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const sAspect = s * (viewport.widthPx / viewport.heightPx);
  return (xPx, yPx) => rayThrough(xPx, yPx, viewport, s, sAspect);
}

/** The ray through a point of the view, px, from the projection's scales s and s × aspect. */
function rayThrough(
  xPx: number,
  yPx: number,
  viewport: Viewport,
  s: number,
  sAspect: number,
): Vec3 {
  const ndcX = (xPx / viewport.widthPx) * 2 - 1;
  const ndcY = 1 - (yPx / viewport.heightPx) * 2;
  return normalise(vec3(ndcX / s, ndcY / sAspect, -1));
}

/**
 * Where a ray meets the spheroid, from the body's centre ÷ a, or `null`: by cross products in the
 * scaled space, as `hit_spheroid` does to keep a small body's limb in `f32`.
 */
function hitSpheroid(body: ViewBody, ray: Vec3): Vec3 | null {
  const scaled = normalise(stretchAlong(body.pole, body.stretch, ray));
  if (dot(scaled, body.scaledCentre) <= 0) {
    return null;
  }
  const across = cross(scaled, body.scaledCentre);
  const h2 = body.radius * body.radius - dot(across, across);
  if (h2 < 0) {
    return null;
  }
  const scaledHit = sub(scale(cross(across, scaled), -1), scale(scaled, Math.sqrt(h2)));
  const hit = stretchAlong(body.pole, 1 / body.stretch, scaledHit);
  return scale(hit, 1 / body.radius);
}

/** One sample's light, pre-exposed, and whether a star lights it directly. */
interface Shaded {
  readonly radiance: Rgb;
  readonly lit: boolean;
}

/**
 * The share of each of the body's laws at the point `q` (from the centre ÷ a): the uniform law's
 * alone, or under a class map the shares the survey-masked bilinear reconstruction gives at the
 * point's cube-sphere direction (`classMapSharesAt`, R10.T10.d). R05's spheroid point of the unit
 * direction d is M d (`spheroidPoint`), so d is q stretched along the pole by a ÷ c, taken along
 * the body-fixed axes.
 */
function sharesAt(body: ViewBody, q: Vec3): number[] {
  if (body.classMap === null) {
    return [1];
  }
  const d = normalise(stretchAlong(body.pole, body.stretch, q));
  const [x, y, z] = body.classMap.axes;
  return classMapSharesAt(body.classMap.texels, [dot(d, x), dot(d, y), dot(d, z)]);
}

/**
 * The reflectance I/F of the body's laws by their shares at a point lit by one source:
 * A f(α) [L(α) · 2h ÷ (μ₀ + μ) + (1 − L(α)) h], f and L(α) from each law's row, with h the source's
 * irradiance factor in place of μ₀
 * (it equals μ₀ wherever the whole source is up, and lights the soft band past the terminator) and
 * the Lommel–Seeliger term's μ₀ + μ floored at the source's angular radius, so that it stays
 * bounded at the limb in that band; `surface_reflectance` and `lit_disc_term` in the shader.
 */
function surfaceReflectance(
  laws: ReadonlyArray<ScaledLaw>,
  shares: ReadonlyArray<number>,
  h: number,
  mu0: number,
  mu: number,
  alpha: number,
  sourceRadius: number,
): Rgb {
  const lsDenominator = Math.max(Math.max(mu0, 0) + mu, sourceRadius);
  const reflectance: [number, number, number] = [0, 0, 0];
  laws.forEach((law, m) => {
    const weight = shares[m] ?? 0;
    if (!(weight > 0)) {
      return;
    }
    const share = limbShareFromTable(law.table, alpha);
    const discTerm = (share * 2 * h) / lsDenominator + (1 - share) * h;
    const f = phaseFactorFromTable(law.table, alpha);
    for (const c of CHANNELS) {
      reflectance[c] += weight * law.a[c] * f[c] * discTerm;
    }
  });
  return reflectance;
}

/**
 * The I/F of a disc's laws by their shares at a point lit by one point source wholly above its
 * horizon, by the disc's own arithmetic (`surface_reflectance` with h = μ₀ and no source radius),
 * each law's A scaled for the figure as a record's laws are: the twin R10's hand-over holds the
 * terrain's to (plan R10, R10.T10.e). Nothing where μ₀ or μ is not positive.
 *
 * @param laws - The surface's laws in their order (`discSurfaceLaws`).
 * @param polarOverEquatorial - The figure's c ÷ a, 1 for a sphere.
 * @param shares - Each law's share at the point.
 * @param alphaRad - The phase angle, rad.
 */
export function discReflectance(
  laws: ReadonlyArray<PhotometricLaw>,
  polarOverEquatorial: number,
  shares: ReadonlyArray<number>,
  mu0: number,
  mu: number,
  alphaRad: number,
): Rgb {
  if (!(mu0 > 0 && mu > 0)) {
    return [0, 0, 0];
  }
  const scaled = laws.map((law, m) => scaledLawOf(law, polarOverEquatorial, m));
  return surfaceReflectance(scaled, shares, mu0, mu0, mu, alphaRad, 0);
}

/**
 * One of a disc's laws at a phase, per channel, split by the two terms of its disc factor: a law's
 * I/F at a point lit by one point source wholly above its horizon is
 * `lommelSeeliger` · 2μ₀ ÷ (μ₀ + μ) + `lambert` · μ₀.
 */
export interface DiscLawTerms {
  /** A′ f(α) L(α), A′ the law's A scaled for the figure. */
  readonly lommelSeeliger: Rgb;
  /** A′ f(α) (1 − L(α)). */
  readonly lambert: Rgb;
}

/**
 * Each of a disc's laws at phase α split by its two terms, by the disc's own arithmetic
 * ({@link discReflectance}'s: each A scaled for the figure, f and L(α) read from the law's row as
 * `surface_reflectance` reads them), so that Σ_m share_m [`lommelSeeliger`_m · 2μ₀ ÷ (μ₀ + μ) +
 * `lambert`_m · μ₀] is {@link discReflectance} at any point: the point regime's integral under a
 * class map (plan R10, R10.T10.f) sums each law's geometry over the surface and multiplies by these
 * once.
 *
 * @param laws - The surface's laws in their order (`discSurfaceLaws`).
 * @param polarOverEquatorial - The figure's c ÷ a, 1 for a sphere.
 * @param alphaRad - The phase angle, rad.
 */
export function discLawTerms(
  laws: ReadonlyArray<PhotometricLaw>,
  polarOverEquatorial: number,
  alphaRad: number,
): DiscLawTerms[] {
  return laws.map((law, m) => {
    const scaled = scaledLawOf(law, polarOverEquatorial, m);
    const share = limbShareFromTable(scaled.table, alphaRad);
    const f = phaseFactorFromTable(scaled.table, alphaRad);
    const term = (weight: number): Rgb => [
      scaled.a[0] * f[0] * weight,
      scaled.a[1] * f[1] * weight,
      scaled.a[2] * f[2] * weight,
    ];
    return { lommelSeeliger: term(share), lambert: term(1 - share) };
  });
}

/**
 * A lit point's frame for R08's lookups (plan R08, R08.T9.b; `litBody.wgsl`'s `local_frame`): its
 * geodetic latitude from its spheroid normal against the pole, and its north and east, in the
 * view's axes; at a pole any horizontal east.
 */
interface LocalFrame {
  readonly latitudeRad: number;
  readonly north: Vec3;
  readonly east: Vec3;
}

/** Below this cosine of its latitude a point is taken as at a pole (`litBody.wgsl`'s `AT_POLE`). */
const AT_POLE = 1e-6;

function localFrameOf(normal: Vec3, pole: Vec3): LocalFrame {
  const sinLatitude = Math.min(Math.max(dot(normal, pole), -1), 1);
  const towardsEast = cross(pole, normal);
  const cosLatitude = norm(towardsEast);
  const east =
    cosLatitude > AT_POLE
      ? scale(towardsEast, 1 / cosLatitude)
      : normalise(cross(Math.abs(normal.x) > 0.9 ? vec3(0, 1, 0) : vec3(1, 0, 0), normal));
  return { latitudeRad: Math.atan2(sinLatitude, cosLatitude), north: cross(normal, east), east };
}

/** A unit direction's azimuth from north towards east, rad; 0 at the zenith or the nadir. */
function localAzimuthOf(frame: LocalFrame, d: Vec3): number {
  const towardsNorth = dot(d, frame.north);
  const towardsEast = dot(d, frame.east);
  return towardsNorth === 0 && towardsEast === 0 ? 0 : Math.atan2(towardsEast, towardsNorth);
}

function shade(body: ViewBody, record: DiscRecord, q: Vec3, ray: Vec3): Shaded {
  const stretch2 = body.stretch * body.stretch;
  const normal = normalise(stretchAlong(body.pole, stretch2, q));
  const mu = -dot(normal, ray);
  if (mu <= 0) {
    return { radiance: [0, 0, 0], lit: false };
  }
  const shares = sharesAt(body, q);
  const radiance: [number, number, number] = [0, 0, 0];
  let lit = false;
  // The surface's mean A, which R08's sky light reflects on the Lambert share's flat term, and the
  // point's frame for R08's lookups, its geodetic height 0 on the figure (R08.T9.b).
  const meanA: [number, number, number] = [0, 0, 0];
  body.laws.forEach((law, m) => {
    const weight = shares[m] ?? 0;
    if (weight > 0) {
      for (const c of CHANNELS) {
        meanA[c] += weight * law.a[c];
      }
    }
  });
  const local = localFrameOf(normal, body.pole);
  const lighting = body.lighting;
  for (const [j, { place, light }] of body.lights.entries()) {
    const toStar = sub(place, q);
    const distance = norm(toStar);
    const towards = scale(toStar, 1 / distance);
    const mu0 = dot(normal, towards);
    // R08's sky light at the star's own μ₀ and light slot, before the horizon's `continue`.
    const sky = lighting.skyIrradiance(0, mu0, local.latitudeRad, j);
    const horizon = sphereIrradianceFactor(
      distance / light.radius,
      Math.acos(Math.min(1, Math.max(-1, mu0))),
    );
    if (horizon <= 0 && !sky.some((value) => value > 0)) {
      continue;
    }
    const starRadius = Math.asin(Math.min(1, light.radius / distance));
    let visible: [number, number, number] = [1, 1, 1];
    for (const occluder of body.occluders) {
      const toOccluder = sub(occluder.centre, q);
      const occluderDistance = norm(toOccluder);
      if (occluderDistance >= distance) {
        continue;
      }
      if (occluderDistance <= occluder.radius) {
        visible = [0, 0, 0];
        break;
      }
      const occluderRadius = Math.asin(occluder.radius / occluderDistance);
      const along = scale(toOccluder, 1 / occluderDistance);
      const separation = Math.atan2(norm(cross(along, towards)), dot(along, towards));
      for (const c of CHANNELS) {
        visible[c] -=
          1 -
          annulusVisibleFraction(
            light.annuli[c],
            occluderRadius / starRadius,
            separation / starRadius,
          );
      }
    }
    visible = [Math.max(visible[0], 0), Math.max(visible[1], 0), Math.max(visible[2], 0)];
    // The sky light shares the point's eclipse: the air above the point lies in the shadow with it.
    for (const c of CHANNELS) {
      radiance[c] += light.illuminance[c] * record.exposureOverPi * meanA[c] * sky[c] * visible[c];
    }
    if (horizon <= 0) {
      continue;
    }
    const alpha = Math.acos(Math.min(1, Math.max(-1, -dot(towards, ray))));
    const reflectance = surfaceReflectance(body.laws, shares, horizon, mu0, mu, alpha, starRadius);
    // R08's transmittance to the star's centre, its light slot's.
    const transmitted = lighting.sunTransmittance(
      0,
      mu0,
      local.latitudeRad,
      localAzimuthOf(local, towards),
      j,
    );
    for (const c of CHANNELS) {
      const v = visible[c];
      radiance[c] +=
        light.illuminance[c] * record.exposureOverPi * reflectance[c] * v * transmitted[c];
      lit ||= horizon > LIT_IRRADIANCE && v > LIT_IRRADIANCE;
    }
  }
  // Planetshine (Design note 7): it lights the night side, which stays unlit for the meter. Through
  // the air (R08.T9.b): its sky term at its illuminance at the point and its own μ₀, before the
  // horizon's `continue`, and its direct term by the three-node rule, both at light slot 0.
  for (const { place, source } of body.secondaries) {
    const toSource = sub(place, q);
    const distance = norm(toSource);
    const towards = scale(toSource, 1 / distance);
    const mu0 = dot(normal, towards);
    const near = source.distance / distance;
    const sky = lighting.skyIrradiance(0, mu0, local.latitudeRad, 0);
    for (const c of CHANNELS) {
      radiance[c] +=
        source.illuminance[c] * near * near * record.exposureOverPi * meanA[c] * sky[c];
    }
    const irradiance = planetshineIrradiance(toSource, source.radius, source.distance, normal);
    if (irradiance <= 0) {
      continue;
    }
    const alpha = Math.acos(Math.min(1, Math.max(-1, -dot(towards, ray))));
    const sourceRadius = Math.asin(Math.min(1, source.radius / distance));
    const reflectance = surfaceReflectance(
      body.laws,
      shares,
      irradiance,
      mu0,
      mu,
      alpha,
      sourceRadius,
    );
    const transmitted = lighting.sourceTransmittance(
      0,
      mu0,
      local.latitudeRad,
      localAzimuthOf(local, towards),
      sourceRadius,
      0,
    );
    for (const c of CHANNELS) {
      radiance[c] +=
        source.illuminance[c] * record.exposureOverPi * reflectance[c] * transmitted[c];
    }
  }
  return { radiance, lit };
}

/**
 * The light the twin shades one point of a record with, and whether a star lights it directly:
 * `bodyDisc.wgsl`'s `shade` in `f64` at the point `q`, from the body's centre ÷ a in the view's
 * axes, seen along the unit view-space `ray` (R08.T9.b's tests of the lighting through the air).
 *
 * @param lighting - The body's atmosphere's lookups ({@link AIRLESS_LIGHTING} for a body without
 *   one, R07's).
 * @param classMap - The texels of the record's class map; `null` for a uniform surface.
 * @throws Error as {@link rasteriseDisc}.
 */
export function shadeDiscPoint(
  record: DiscRecord,
  camera: ProjectionCamera,
  q: Vec3,
  ray: Vec3,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
  classMap: ClassMapTexels | null = null,
): { readonly radiance: Rgb; readonly lit: boolean } {
  return shade(viewBodyOf(record, camera, classMap, lighting), record, q, ray);
}

/** One pixel of a rasterised disc: its stored colour and alpha as the two draws leave them. */
export interface DiscPixel {
  readonly xPx: number;
  readonly yPx: number;
  /** Pre-exposed luminance per channel, premultiplied by the coverage on the limb. */
  readonly rgb: Rgb;
  /** The covered share of the pixel. */
  readonly coverage: number;
  /** Which of the two draws wrote it: the opaque interior, or the premultiplied limb. */
  readonly draw: "interior" | "limb";
  /** The meter class the interior writes; `null` on the limb, which keeps the one beneath. */
  readonly meterClass: MeterClass | null;
}

/** A sample's offset from its pixel's centre, px: k-th of an n × n grid, row by row. */
export function sampleOffset(k: number, n: number): readonly [number, number] {
  return [(Math.floor(k / n) + 0.5) / n - 0.5, ((k % n) + 0.5) / n - 0.5];
}

/**
 * The signed angle of a ray from the limb in the scaled space, rad: positive outside the body.
 *
 * @remarks
 * In the space where the spheroid is a sphere of radius a ÷ D about M u, the silhouette is the cone
 * of half-angle asin((a ÷ D) ÷ |M u|) about M u; the ray's angle from M u less that half-angle.
 */
function limbAngle(body: ViewBody, ray: Vec3): number {
  const scaled = stretchAlong(body.pole, body.stretch, ray);
  const along = dot(scaled, body.scaledCentre);
  const across = norm(cross(scaled, body.scaledCentre));
  return Math.atan2(across, along) - Math.asin(body.radius / norm(body.scaledCentre));
}

/** A limb pixel's corners within this many pixels inside the limb still count as on it. */
export const LIMB_OVERLAP_PX = 1e-3;

/** The step of the limb angle's gradient at a sample, px. */
const GRADIENT_STEP_PX = 1 / 64;

/** A pixel whose centre is further than this outside the limb, px, holds none of the body. */
const OUTSIDE_PX = 0.75;

/** A cell whose inner side is deeper than this many cell widths inside the limb is shaded once. */
const NEAR_LIMB_CELLS = 2;

/** Three-point Gauss–Legendre nodes on [0, 1] and their weights, exact to degree 5. */
const GAUSS_NODES = [0.5 - 0.5 * Math.sqrt(0.6), 0.5, 0.5 + 0.5 * Math.sqrt(0.6)] as const;
const GAUSS_WEIGHTS = [5 / 18, 8 / 18, 5 / 18] as const;

/**
 * A square cell's profile across a line of normal (n_x, n_y): the length of its chord at offset t
 * from its centre along the normal, in pieces linear in t, for a cell of side h. It is the
 * convolution of boxes of widths h|n_x| and h|n_y|: rising, flat, falling; its integral is h².
 */
export function cellProfile(
  normalX: number,
  normalY: number,
  h: number,
): ReadonlyArray<readonly [number, number, number, number]> {
  const a = h * Math.max(Math.abs(normalX), Math.abs(normalY));
  const b = h * Math.min(Math.abs(normalX), Math.abs(normalY));
  const top = (h * h) / a;
  const outer = (a + b) / 2;
  const inner = (a - b) / 2;
  // [t₀, t₁, p(t₀), p(t₁)] for each piece of non-zero width.
  const pieces: Array<readonly [number, number, number, number]> = [
    [-outer, -inner, 0, top],
    [-inner, inner, top, top],
    [inner, outer, top, 0],
  ];
  return pieces.filter(([t0, t1]) => t1 > t0);
}

/** A value held to what the rgba16float target can store. */
function clampHdr(v: number): number {
  return Math.min(v, HALF_FLOAT_MAX);
}

/** One cell's light, its covered share and whether its shaded points were lit. */
interface CellSum {
  readonly radiance: Rgb;
  readonly coverage: number;
  readonly lit: number;
  readonly shaded: number;
}

/**
 * The disc's pixels as the shader's two draws leave them, in `f64`: the CPU rasteriser the flux and
 * geometry tests read.
 *
 * @remarks
 * A pixel is the interior draw's where its four corner rays all meet the body, which then covers it
 * wholly, the silhouette being convex; the limb draw's where any corner lies outside or within
 * {@link LIMB_OVERLAP_PX} inside the limb, so that a pixel at the threshold is drawn by both and
 * never by neither. Each of a pixel's n × n samples stands for its cell of side h = 1 ÷ n. Near the
 * limb a cell's profile across the limb's local normal n̂ (the gradient of the limb angle at the
 * sample) is a trapezoid, the convolution of boxes of widths h|n̂_x| and h|n̂_y|; its covered share
 * is the profile's integral inside the limb over h², and its light the profile-weighted integral
 * over its depth δ inside the limb by three Gauss points in u = √δ per linear piece, exact where
 * the light goes as A + B√δ + Cδ: μ, and μ₀ on a crescent, go as √δ at the limb, which a point
 * sample of a thin crescent cannot follow. Deeper cells are shaded at their centre. The interior's
 * class is pure: `litBody` where every shaded point is lit directly (above
 * {@link LIT_IRRADIANCE}), `unlitBody` where none is, `other` where they are mixed (at the
 * terminator) or no star lights the body.
 *
 * @param classMap - The texels of the record's class map, which its surface holds only as a
 *   texture; `null` for a uniform surface.
 * @param lighting - The body's atmosphere's lookups (plan R08, R08.T9.b), which its record's
 *   tables hold only as textures: `tablesCpu.ts`'s `surfaceLightingTwin`, or
 *   {@link AIRLESS_LIGHTING} for a body without one.
 * @throws Error for a class-map record without its texels, or with another map's class count.
 */
export function rasteriseDisc(
  record: DiscRecord,
  camera: ProjectionCamera,
  viewport: Viewport,
  classMap: ClassMapTexels | null = null,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
): DiscPixel[] {
  const body = viewBodyOf(record, camera, classMap, lighting);
  const pixels: DiscPixel[] = [];
  const { rect } = record;
  const rayAt = viewRays(camera, viewport);
  const angleAt = (x: number, y: number): number => limbAngle(body, rayAt(x, y));
  const shadeAt = (x: number, y: number): Shaded | null => {
    const ray = rayAt(x, y);
    const q = hitSpheroid(body, ray);
    return q === null ? null : shade(body, record, q, ray);
  };
  const cell = (px: number, py: number, n: number): CellSum => {
    const angle = angleAt(px, py);
    const sx = (angleAt(px + GRADIENT_STEP_PX, py) - angle) / GRADIENT_STEP_PX;
    const sy = (angleAt(px, py + GRADIENT_STEP_PX) - angle) / GRADIENT_STEP_PX;
    const slope = Math.hypot(sx, sy);
    const normalX = slope > 0 ? sx / slope : 0;
    const normalY = slope > 0 ? sy / slope : 0;
    const h = 1 / n;
    // The cell's half-extent along the normal.
    const half = (h * (Math.abs(normalX) + Math.abs(normalY))) / 2;
    const depth = slope > 0 ? -angle / slope : Number.POSITIVE_INFINITY;
    const radiance: [number, number, number] = [0, 0, 0];
    let lit = 0;
    let shaded = 0;
    const add = (sample: Shaded | null, weight: number): void => {
      if (sample === null) {
        return;
      }
      for (const c of CHANNELS) {
        radiance[c] += weight * sample.radiance[c];
      }
      lit += sample.lit ? 1 : 0;
      shaded += 1;
    };
    if (depth - half > NEAR_LIMB_CELLS * h) {
      add(shadeAt(px, py), 1);
      return { radiance, coverage: 1, lit, shaded };
    }
    // Over each piece of the cell's profile inside the limb (t ≤ depth, t outward from the centre),
    // ∫ g(δ) p(depth − δ) dδ ÷ h² with δ = u², by three Gauss points in u.
    let covered = 0;
    for (const [t0, t1, p0, p1] of cellProfile(normalX, normalY, h)) {
      if (t0 >= depth) {
        continue;
      }
      const end = Math.min(t1, depth);
      const profile = (t: number): number => p0 + ((p1 - p0) * (t - t0)) / (t1 - t0);
      covered += ((end - t0) * (profile(t0) + profile(end))) / 2;
      const ua = Math.sqrt(depth - end);
      const ub = Math.sqrt(depth - t0);
      GAUSS_NODES.forEach((node, i) => {
        const u = ua + node * (ub - ua);
        const t = depth - u * u;
        const weight = ((GAUSS_WEIGHTS[i] ?? 0) * (ub - ua) * 2 * u * profile(t)) / (h * h);
        add(shadeAt(px + t * normalX, py + t * normalY), weight);
      });
    }
    return { radiance, coverage: Math.min(1, covered / (h * h)), lit, shaded };
  };
  const pixelSum = (x: number, y: number, n: number): CellSum => {
    const radiance: [number, number, number] = [0, 0, 0];
    let coverage = 0;
    let lit = 0;
    let shaded = 0;
    for (let k = 0; k < n * n; k += 1) {
      const [dx, dy] = sampleOffset(k, n);
      const one = cell(x + 0.5 + dx, y + 0.5 + dy, n);
      for (const c of CHANNELS) {
        radiance[c] += one.radiance[c];
      }
      coverage += one.coverage;
      lit += one.lit;
      shaded += one.shaded;
    }
    const cells = n * n;
    return {
      radiance: [radiance[0] / cells, radiance[1] / cells, radiance[2] / cells],
      coverage: coverage / cells,
      lit,
      shaded,
    };
  };
  // Each pixel corner's limb angle, taken once: four pixels share a corner.
  const left = Math.floor(rect.leftPx);
  const top = Math.floor(rect.topPx);
  const cornersAcross = Math.ceil(rect.rightPx) - left + 1;
  const cornersDown = Math.ceil(rect.bottomPx) - top + 1;
  const cornerAngles = new Float64Array(Math.max(cornersAcross * cornersDown, 0));
  const cornerKnown = new Uint8Array(cornerAngles.length);
  const cornerAt = (x: number, y: number): number => {
    const index = (y - top) * cornersAcross + (x - left);
    if (cornerKnown[index] === 1) {
      return cornerAngles[index] ?? Number.NaN;
    }
    const angle = angleAt(x, y);
    cornerAngles[index] = angle;
    cornerKnown[index] = 1;
    return angle;
  };
  for (let y = top; y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = left; x < Math.ceil(rect.rightPx); x += 1) {
      if (dot(body.centre, rayAt(x + 0.5, y + 0.5)) <= 0) {
        continue;
      }
      const corners = [
        cornerAt(x, y),
        cornerAt(x + 1, y),
        cornerAt(x, y + 1),
        cornerAt(x + 1, y + 1),
      ];
      const [c00 = 0, c10 = 0, c01 = 0, c11 = 0] = corners;
      // The angle's gradient across the pixel, rad per px, from its corners.
      const gradient = Math.hypot((c10 + c11 - c00 - c01) / 2, (c01 + c11 - c00 - c10) / 2);
      const centre = (c00 + c10 + c01 + c11) / 4;
      if (!(gradient > 0) || centre / gradient > OUTSIDE_PX) {
        continue;
      }
      const interior = corners.every((angle) => angle < 0);
      const limb = !corners.every((angle) => angle / gradient < -LIMB_OVERLAP_PX);
      if (interior) {
        const sum = pixelSum(x, y, record.interiorSamples);
        // A wholly covered pixel's light is its mean over its cells.
        const mean = sum.coverage > 0 ? 1 / sum.coverage : 0;
        const meterClass =
          record.lights.length === 0 || sum.shaded === 0 || (sum.lit > 0 && sum.lit < sum.shaded)
            ? METER_CLASS.other
            : sum.lit === sum.shaded
              ? METER_CLASS.litBody
              : METER_CLASS.unlitBody;
        pixels.push({
          xPx: x,
          yPx: y,
          rgb: [
            clampHdr(sum.radiance[0] * mean),
            clampHdr(sum.radiance[1] * mean),
            clampHdr(sum.radiance[2] * mean),
          ],
          coverage: 1,
          draw: "interior",
          meterClass,
        });
      }
      if (!limb) {
        continue;
      }
      const sum = pixelSum(x, y, record.limbSamples);
      if (sum.coverage <= 0) {
        continue;
      }
      pixels.push({
        xPx: x,
        yPx: y,
        rgb: [clampHdr(sum.radiance[0]), clampHdr(sum.radiance[1]), clampHdr(sum.radiance[2])],
        coverage: Math.min(1, sum.coverage),
        draw: "limb",
        meterClass: null,
      });
    }
  }
  return pixels;
}

/** One pixel as both draws leave it over a black background. */
export interface CompositePixel {
  readonly xPx: number;
  readonly yPx: number;
  readonly rgb: Rgb;
  readonly coverage: number;
  /** The alpha left: the interior's class, or `null` where only the limb drew (the class beneath). */
  readonly meterClass: MeterClass | null;
}

/**
 * The rasterised draws composited in their order over black: the interior's opaque write, then
 * the limb's premultiplied colour over it.
 */
export function compositeDiscPixels(pixels: ReadonlyArray<DiscPixel>): CompositePixel[] {
  const byPixel = new Map<string, CompositePixel>();
  for (const pixel of pixels) {
    const key = `${String(pixel.xPx)},${String(pixel.yPx)}`;
    const beneath = byPixel.get(key);
    if (pixel.draw === "interior" || beneath === undefined) {
      byPixel.set(key, { ...pixel });
      continue;
    }
    const keep = 1 - pixel.coverage;
    byPixel.set(key, {
      xPx: pixel.xPx,
      yPx: pixel.yPx,
      rgb: [
        pixel.rgb[0] + keep * beneath.rgb[0],
        pixel.rgb[1] + keep * beneath.rgb[1],
        pixel.rgb[2] + keep * beneath.rgb[2],
      ],
      coverage: Math.min(1, pixel.coverage + keep * beneath.coverage),
      meterClass: beneath.meterClass,
    });
  }
  return [...byPixel.values()];
}

/** One pixel's mean light over its point samples, and the share of them that meet the body. */
export interface SampledPixel {
  readonly xPx: number;
  readonly yPx: number;
  readonly rgb: Rgb;
  readonly coverage: number;
}

/**
 * The record's pixels point-sampled m × m times each with the disc's own light, in `f64`: the
 * brute force that the twin's cells are held to (R07.T8.c, decision-r07-small-disc-cost, G5′).
 *
 * @remarks
 * Each sample is the ray through its point of the pixel, on the cells' grid of centres
 * ({@link sampleOffset}), shaded where it meets the spheroid and black where it misses: no cell
 * profile and no Gauss points. Every pixel of the record's rectangle that a sample meets is
 * returned, in row order. The cost grows as m², so a test keeps the disc to a few pixels.
 *
 * @param samplesPerAxis - m, a positive integer.
 * @param classMap - The texels of the record's class map; `null` for a uniform surface.
 * @param lighting - The body's atmosphere's lookups, as {@link rasteriseDisc}'s.
 * @throws Error if `samplesPerAxis` is not a positive integer; for a class-map record without its
 *   texels, or with another map's class count.
 */
export function pointSampleDisc(
  record: DiscRecord,
  camera: ProjectionCamera,
  viewport: Viewport,
  samplesPerAxis: number,
  classMap: ClassMapTexels | null = null,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
): SampledPixel[] {
  if (!Number.isInteger(samplesPerAxis) || samplesPerAxis < 1) {
    throw new Error(
      `a pixel takes a positive whole number of samples per axis, got ${samplesPerAxis}`,
    );
  }
  const body = viewBodyOf(record, camera, classMap, lighting);
  const m = samplesPerAxis;
  const share = 1 / (m * m);
  const pixels: SampledPixel[] = [];
  const { rect } = record;
  const rayAt = viewRays(camera, viewport);
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      const sum: [number, number, number] = [0, 0, 0];
      let hits = 0;
      for (let i = 0; i < m; i += 1) {
        for (let j = 0; j < m; j += 1) {
          const ray = rayAt(x + (i + 0.5) / m, y + (j + 0.5) / m);
          const q = hitSpheroid(body, ray);
          if (q === null) {
            continue;
          }
          hits += 1;
          const { radiance } = shade(body, record, q, ray);
          for (const c of CHANNELS) {
            sum[c] += radiance[c];
          }
        }
      }
      if (hits > 0) {
        pixels.push({
          xPx: x,
          yPx: y,
          rgb: [sum[0] * share, sum[1] * share, sum[2] * share],
          coverage: hits * share,
        });
      }
    }
  }
  return pixels;
}
