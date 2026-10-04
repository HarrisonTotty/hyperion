/**
 * The disc regime's per-body record and the TypeScript twin of `shaders/bodyDisc.wgsl` (plan R07,
 * T8.a; Design notes 2, 6, 10 and 19).
 *
 * @remarks
 * A disc is one screen rectangle whose fragments intersect their rays with the body's spheroid
 * (the polar axis scaled by a ÷ c makes it a sphere, the normal taken by the inverse transpose)
 * and shade each hit with the body's law: I/F = A f(α) [L · 2h ÷ (μ₀ + μ) + (1 − L) h], where h,
 * the horizon term (`sphereIrradianceFactor`), stands for the law's μ₀ and equals it wherever the
 * whole star is up; each star's light is cut by the eclipse term of up to two occluders and summed
 * over up to two stars. Every length is divided before it reaches `f32`: the centre is a unit
 * direction with a ÷ D, and every light and occluder is relative to the body's centre over a.
 * A pixel's n × n cells are {@link SMALL_DISC_SAMPLES} per axis on a disc under
 * {@link SMALL_DISC_PX}, so that its summed flux meets the point's at the 3 px switch; on a larger
 * disc one inside and {@link LIMB_SAMPLES} per axis on the limb. A pixel whose four corners all
 * meet the body is drawn opaque with its meter class; one on the limb premultiplied by its
 * coverage, keeping the class beneath (decision-r07-t8a, item 3). Near the limb a cell's light is
 * integrated across its profile in √δ, the depth inside the limb (see {@link rasteriseDisc}).
 * {@link rasteriseDisc} runs the shader's arithmetic in `f64`.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  phaseFactorFromTable,
  type PhaseFactorTable,
  type PhotometricLaw,
} from "../appearance/law";
import { type ProjectionCamera, toViewAxes, type Viewport } from "../camera/projection";
import type { AnnulusSet } from "../lighting/annuli";
import { annulusVisibleFraction } from "../lighting/annuli";
import { sphereIrradianceFactor } from "../lighting/sphereIrradiance";
import type { Rgb } from "../photometry/toneCurve";
import { HALF_FLOAT_MAX } from "../photometry/toneCurve";
import { METER_CLASS, type MeterClass } from "../post/meter";
import type { ScreenRect } from "../wireframe/submit";

/**
 * A sample is lit directly where a star's horizon and eclipse terms exceed this share of the
 * face-on irradiance, so that `f32` and `f64` agree at the terminator's edge.
 */
export const LIT_IRRADIANCE = 1e-5;

/** `vec4f` rows per disc in the shader's `discs` buffer. */
export const DISC_ROWS = 24;

/** The stars that light one disc at most: the brightest two (Design note 4's multiple systems). */
export const MAX_DISC_LIGHTS = 2;

/** The occluders one disc's light is cut by at most: the two largest seen from it. */
export const MAX_DISC_OCCLUDERS = 2;

/** A disc under this diameter, px, is sampled {@link SMALL_DISC_SAMPLES}² times in every pixel. */
export const SMALL_DISC_PX = 32;

/** Samples per axis in every pixel of a small disc. */
export const SMALL_DISC_SAMPLES = 8;

/** Samples per axis in a larger disc's limb pixels. */
export const LIMB_SAMPLES = 4;

/** The display channels' indices, r, g, b. */
const CHANNELS = [0, 1, 2] as const;

const FIRST_LIGHT_ROW = 6;
const LIGHT_ROWS = 8;
const FIRST_OCCLUDER_ROW = 22;

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

/** One body that may eclipse a disc's stars, relative to the body's centre ÷ a. */
export interface DiscOccluder {
  readonly centre: Vec3;
  readonly radius: number;
}

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
  readonly law: PhotometricLaw;
  /**
   * The factor on the law's A that makes an oblate disc reach p equator-on at zero phase
   * (`oblateAlbedoScale`); 1 for a sphere.
   */
  readonly albedoScale: number;
  /** The law's row of the frame's phase table. */
  readonly tableRow: number;
  /** The exposure scale over π, so that each pixel holds pre-exposed luminance. */
  readonly exposureOverPi: number;
  readonly lights: ReadonlyArray<DiscLight>;
  readonly occluders: ReadonlyArray<DiscOccluder>;
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

/** Packs the frame's discs as the shader's `array<vec4f>`, {@link DISC_ROWS} rows each. */
export function packDiscRecords(records: ReadonlyArray<DiscRecord>): Float32Array {
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
    const k = record.albedoScale;
    const [ar, ag, ab] = record.law.a;
    put(4, [ar * k, ag * k, ab * k, record.law.lommelSeeligerShare]);
    put(5, [record.tableRow, annulusCount(record), record.exposureOverPi, 0]);
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
  });
  return out;
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
}

function viewBodyOf(record: DiscRecord, camera: ProjectionCamera): ViewBody {
  const toView = (v: Vec3): Vec3 => toViewAxes(v, camera.orientation);
  const centre = toView(record.direction);
  const pole = normalise(toView(record.pole));
  const stretch = 1 / record.polarOverEquatorial;
  return {
    centre,
    pole,
    stretch,
    radius: record.radiusOverDistance,
    scaledCentre: stretchAlong(pole, stretch, centre),
    lights: record.lights
      .slice(0, MAX_DISC_LIGHTS)
      .map((light) => ({ place: scale(toView(light.direction), light.distance), light })),
    occluders: record.occluders
      .slice(0, MAX_DISC_OCCLUDERS)
      .map((occluder) => ({ centre: toView(occluder.centre), radius: occluder.radius })),
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
  const aspect = viewport.widthPx / viewport.heightPx;
  const ndcX = (xPx / viewport.widthPx) * 2 - 1;
  const ndcY = 1 - (yPx / viewport.heightPx) * 2;
  return normalise(vec3(ndcX / s, ndcY / (s * aspect), -1));
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

function shade(
  body: ViewBody,
  record: DiscRecord,
  table: PhaseFactorTable,
  q: Vec3,
  ray: Vec3,
): Shaded {
  const stretch2 = body.stretch * body.stretch;
  const normal = normalise(stretchAlong(body.pole, stretch2, q));
  const mu = -dot(normal, ray);
  if (mu <= 0) {
    return { radiance: [0, 0, 0], lit: false };
  }
  const { law } = record;
  const radiance: [number, number, number] = [0, 0, 0];
  let lit = false;
  for (const { place, light } of body.lights) {
    const toStar = sub(place, q);
    const distance = norm(toStar);
    const towards = scale(toStar, 1 / distance);
    const mu0 = dot(normal, towards);
    const horizon = sphereIrradianceFactor(
      distance / light.radius,
      Math.acos(Math.min(1, Math.max(-1, mu0))),
    );
    if (horizon <= 0) {
      continue;
    }
    const alpha = Math.acos(Math.min(1, Math.max(-1, -dot(towards, ray))));
    const share = law.lommelSeeligerShare;
    const starRadius = Math.asin(Math.min(1, light.radius / distance));
    // The Lommel–Seeliger term's μ₀ + μ, floored at the star's angular radius: past the geometric
    // terminator, in the soft band, an extended star's term stays bounded at the limb.
    const lsDenominator = Math.max(Math.max(mu0, 0) + mu, starRadius);
    const discTerm = (share * 2 * horizon) / lsDenominator + (1 - share) * horizon;
    const f = phaseFactorFromTable(table, alpha);
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
    for (const c of CHANNELS) {
      const v = Math.max(visible[c], 0);
      radiance[c] +=
        light.illuminance[c] *
        record.exposureOverPi *
        record.albedoScale *
        law.a[c] *
        f[c] *
        discTerm *
        v;
      lit ||= horizon > LIT_IRRADIANCE && v > LIT_IRRADIANCE;
    }
  }
  return { radiance, lit };
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
 * @param table - The law's phase table (`phaseFactorTableOf(record.law)`).
 */
export function rasteriseDisc(
  record: DiscRecord,
  table: PhaseFactorTable,
  camera: ProjectionCamera,
  viewport: Viewport,
): DiscPixel[] {
  const body = viewBodyOf(record, camera);
  const pixels: DiscPixel[] = [];
  const { rect } = record;
  const angleAt = (x: number, y: number): number =>
    limbAngle(body, viewRay(x, y, camera, viewport));
  const shadeAt = (x: number, y: number): Shaded | null => {
    const ray = viewRay(x, y, camera, viewport);
    const q = hitSpheroid(body, ray);
    return q === null ? null : shade(body, record, table, q, ray);
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
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      if (dot(body.centre, viewRay(x + 0.5, y + 0.5, camera, viewport)) <= 0) {
        continue;
      }
      const corners = [angleAt(x, y), angleAt(x + 1, y), angleAt(x, y + 1), angleAt(x + 1, y + 1)];
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
