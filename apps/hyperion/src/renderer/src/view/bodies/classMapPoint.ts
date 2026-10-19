/**
 * The point regime's light under a class map (plan R10, R10.T10.f; R07 Design note 5,
 * decision-r07-t8b): the body's class map integrated over its figure, so that a point's flux stays
 * the disc's it switches with at 3 px (R07.T8.a's 1%) wherever the disc shades a pattern.
 *
 * @remarks
 * - **The integral.** The disc's own arithmetic in `f64`, summed over the lit and visible surface
 *   elements in the body-fixed axes (the rotation takes the light and the observer into them): each
 *   element's shares as R10.T10.d reconstructs them (`classMapSharesAt`) times each law's I/F
 *   (`discLawTerms`, `discReflectance`'s split by its two terms: A scaled for the figure,
 *   f and L(α) from its row) times μ and the element's area on the spheroid of the figure, whose
 *   pole is the body-fixed z axis, as the class-map disc draws it.
 * - **The elements** are the class map's texels, each cut into n × n by its face coordinates
 *   (s, t), n = ⌈{@link CLASS_MAP_POINT_FACE_ELEMENTS} ÷ N⌉ (2 at R10's 64 texels a face): 6 × 128²
 *   = 98,304 elements on the sphere. Each is taken at its centre in (s, t), a texel's quarter
 *   points at n = 2, within whose quarters the reconstruction is bilinear between the four nearest
 *   texel centres, with its exact solid angle on the unit sphere (the face rectangle's, through the
 *   quadratic warp) times the spheroid's area factor c ÷ a · |(x, y, z a ÷ c)| there. Each
 *   element's shares are found once a map; an evaluation is then one pass over the elements, two
 *   dot products each and a multiply-add per share of the lit and visible ones.
 * - **The normalisation.** The sum is divided by the same sum under `elsewhere` alone and
 *   multiplied by `bodyReflection` of `elsewhere`, the photometry's point, so that a map whose
 *   every law is the uniform one gives the uniform point exactly and the elements' error in the
 *   limb and the terminator cancels in the ratio, which is bounded by the map's laws' extremes.
 * - **The cache.** The ratio, the light's own and uneclipsed, is held per surface and light by the
 *   body-fixed directions of the light and the observer, and taken again while both stay within
 *   δ of where it was found ({@link classMapPointCacheAngleRad}); the uniform point is always fresh.
 *   The entries are held in a `WeakMap` keyed by the surface, at most eight a surface, so they go
 *   with their surface when its owner drops it (a new map, a new `elsewhere`, the body released);
 *   the elements' shares likewise with the map's texels.
 */
import { dot, type Vec3, vec3 } from "../../geometry/vec3";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { type Rotation3, rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import type { Rgb } from "../photometry/toneCurve";
import { faceUvToXyz, stToUv } from "../terrain/cube";
import type { BodyFigure } from "../terrain/planet";
import type { Face } from "../terrain/patchKey";
import { discLawTerms } from "./discShading";
import { type ClassMapTexels, classMapSharesAt, discSurfaceLaws } from "./discSurface";
import { bodyReflection } from "./oblate";

/**
 * The elements along a face's side the integral takes at the least: two a side of each of R10's
 * level-6 texels (R10.T10.d's `CLASS_MAP_FACE_TEXELS`, 64).
 *
 * @remarks
 * Against an `f64` brute force over 2 × 10⁶ directions, on R10.T10.f's synthetic Iapetus (a 70°
 * cap at A 0.04 about the leading apex, the rest at 0.55) seen from the leading and trailing sides
 * and a pole at 0° to 150° of phase, the integral errs by at most 3.8 × 10⁻⁴ at two elements a
 * texel side, against 1.33 × 10⁻³ at one, where the texels' centres stand for the reconstruction
 * between them; on a half-surveyed checkerboard of texels and a half-surveyed Iapetus by 2.0 and
 * 1.1 × 10⁻⁴ (2.6 and 3.8 × 10⁻⁴ at one). Two keep a margin of eight under the task's 0.3% for
 * contrasts beyond the Iapetus's 14 to 1 (measured 2026-10-16).
 */
export const CLASS_MAP_POINT_FACE_ELEMENTS = 128;

/** The elements along a texel's side for a map of N texels a face's side. */
export function classMapPointSubdivision(faceTexels: number): number {
  return Math.max(1, Math.ceil(CLASS_MAP_POINT_FACE_ELEMENTS / faceTexels));
}

/**
 * The cache's angle at phase 0, rad: a cached ratio is taken again while the light's and the
 * observer's body-fixed directions both stay within {@link classMapPointCacheAngleRad} of where it
 * was found.
 *
 * @remarks
 * The ratio's sensitivity S, |Δ ln R| per radian with both directions moved together, measured on
 * the synthetic Iapetus over its three views at 0° to 165° of phase, keeps S (π − α) at or below
 * about 5.2 (its worst at 60° to 90°, falling to 1.8 at 150° and 165°): a pattern's share of the
 * light moves by the angle over the lit and visible lune's width π − α. So within
 * δ = δ₀ (π − α) ÷ π a cached ratio is within about 5.2 δ₀ ÷ π of a fresh one, 1.7 × 10⁻³ at this
 * δ₀, about half the task's 0.3%, the rest left for contrasts beyond the Iapetus's; measured, at
 * most 1.60 × 10⁻³ over 64 pairs of moved directions at each view and phase from 0° to 150°
 * (2026-10-19).
 */
export const CLASS_MAP_POINT_CACHE_RAD = 1e-3;

/**
 * The angle within which a cached ratio is taken again at phase α, rad: {@link
 * CLASS_MAP_POINT_CACHE_RAD} × (π − α) ÷ π, narrowing as the lit and visible lune does, across
 * which a pattern's share of the light moves the faster.
 */
export function classMapPointCacheAngleRad(alphaRad: number): number {
  return (CLASS_MAP_POINT_CACHE_RAD * Math.max(0, Math.PI - alphaRad)) / Math.PI;
}

/** The body-fixed pole, about which the class-map disc draws the figure. */
const BODY_FIXED_POLE = vec3(0, 0, 1);

/** The cube sphere's faces, in order. */
const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The elements of the sphere at one fineness, in the body-fixed axes. */
interface ElementGeometry {
  readonly count: number;
  /** Each element's centre's unit direction, x, y and z. */
  readonly directions: Float64Array;
  /** Each element's solid angle on the unit sphere, sr. */
  readonly solidAngles: Float64Array;
}

/** F(u, v) = atan(u v ÷ √(1 + u² + v²)), the solid angle of the face rectangle [0, u] × [0, v]. */
function cornerSolidAngle(u: number, v: number): number {
  return Math.atan2(u * v, Math.sqrt(1 + u * u + v * v));
}

/**
 * The solid angle of the face rectangle [u₀, u₁] × [v₀, v₁] seen from the cube's centre, sr:
 * F(u₁, v₁) − F(u₀, v₁) − F(u₁, v₀) + F(u₀, v₀) ({@link cornerSolidAngle}); a face's is 4π ÷ 6.
 */
function rectangleSolidAngle(u0: number, u1: number, v0: number, v1: number): number {
  return (
    cornerSolidAngle(u1, v1) -
    cornerSolidAngle(u0, v1) -
    cornerSolidAngle(u1, v0) +
    cornerSolidAngle(u0, v0)
  );
}

/** Each fineness's elements, made on first use: one per fineness in use. */
const GEOMETRIES = new Map<number, ElementGeometry>();

/** The sphere's elements at `side` along a face's side, face by face, then row by row in t. */
function elementGeometry(side: number): ElementGeometry {
  const held = GEOMETRIES.get(side);
  if (held !== undefined) {
    return held;
  }
  const count = FACES.length * side * side;
  const directions = new Float64Array(3 * count);
  const solidAngles = new Float64Array(count);
  const edges = Float64Array.from({ length: side + 1 }, (_, k) => stToUv(k / side));
  let e = 0;
  for (const face of FACES) {
    for (let j = 0; j < side; j += 1) {
      const v = stToUv((j + 0.5) / side);
      for (let i = 0; i < side; i += 1) {
        const [x, y, z] = faceUvToXyz(face, stToUv((i + 0.5) / side), v);
        const length = Math.sqrt(x * x + y * y + z * z);
        directions[3 * e] = x / length;
        directions[3 * e + 1] = y / length;
        directions[3 * e + 2] = z / length;
        solidAngles[e] = rectangleSolidAngle(
          edges[i] ?? 0,
          edges[i + 1] ?? 0,
          edges[j] ?? 0,
          edges[j + 1] ?? 0,
        );
        e += 1;
      }
    }
  }
  const geometry = { count, directions, solidAngles };
  GEOMETRIES.set(side, geometry);
  return geometry;
}

/** A map's elements with each element's shares of its laws, those above 0 alone. */
interface ElementShares {
  readonly geometry: ElementGeometry;
  /** The laws, `elsewhere` first (`discSurfaceLaws`' order). */
  readonly laws: number;
  /** Element e's shares are entries `offsets[e]` to `offsets[e + 1]`. */
  readonly offsets: Uint32Array;
  /** Each entry's law. */
  readonly law: Uint8Array;
  /** Each entry's share. */
  readonly share: Float64Array;
}

/** Each map's element shares, made on first use and held while the map is. */
const SHARES = new WeakMap<ClassMapTexels, ElementShares>();

/** The shares of a map's laws at each of its elements, by the survey-masked reconstruction. */
function elementSharesOf(texels: ClassMapTexels): ElementShares {
  const held = SHARES.get(texels);
  if (held !== undefined) {
    return held;
  }
  const geometry = elementGeometry(texels.faceTexels * classMapPointSubdivision(texels.faceTexels));
  const offsets = new Uint32Array(geometry.count + 1);
  const law: number[] = [];
  const share: number[] = [];
  const { directions } = geometry;
  for (let e = 0; e < geometry.count; e += 1) {
    const shares = classMapSharesAt(texels, [
      directions[3 * e] ?? 0,
      directions[3 * e + 1] ?? 0,
      directions[3 * e + 2] ?? 0,
    ]);
    shares.forEach((value, m) => {
      if (value > 0) {
        law.push(m);
        share.push(value);
      }
    });
    offsets[e + 1] = law.length;
  }
  const made = {
    geometry,
    laws: texels.classes + 1,
    offsets,
    law: Uint8Array.from(law),
    share: Float64Array.from(share),
  };
  SHARES.set(texels, made);
  return made;
}

/**
 * Each law's geometric sums over the lit and visible elements, the shares weighting them: the
 * Lommel–Seeliger term's Σ share · 2μ₀μ ÷ (μ₀ + μ) dS and the Lambert term's Σ share · μ₀μ dS, and
 * the same sums over every element unweighted, `elsewhere`'s alone.
 */
interface LawSums {
  readonly lommelSeeliger: Float64Array;
  readonly lambert: Float64Array;
  readonly totalLommelSeeliger: number;
  readonly totalLambert: number;
}

function lawSums(
  shares: ElementShares,
  cOverA: number,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): LawSums {
  const { geometry, offsets, law, share } = shares;
  const { directions, solidAngles } = geometry;
  const lommelSeeliger = new Float64Array(shares.laws);
  const lambert = new Float64Array(shares.laws);
  let totalLommelSeeliger = 0;
  let totalLambert = 0;
  const { x: sx, y: sy, z: sz } = towardsLight;
  const { x: vx, y: vy, z: vz } = towardsObserver;
  for (let e = 0; e < geometry.count; e += 1) {
    const dx = directions[3 * e] ?? 0;
    const dy = directions[3 * e + 1] ?? 0;
    // R05's spheroid point of the unit direction d is (dx, dy, dz c ÷ a), its outward normal along
    // (dx, dy, dz a ÷ c), and its area c ÷ a · |(dx, dy, dz a ÷ c)| dΩ.
    const nz = (directions[3 * e + 2] ?? 0) / cOverA;
    const length = Math.sqrt(dx * dx + dy * dy + nz * nz);
    const mu = (dx * vx + dy * vy + nz * vz) / length;
    if (!(mu > 0)) {
      continue;
    }
    const mu0 = (dx * sx + dy * sy + nz * sz) / length;
    if (!(mu0 > 0)) {
      continue;
    }
    const area = cOverA * length * (solidAngles[e] ?? 0);
    const flat = mu0 * mu * area;
    const ls = ((2 * mu0 * mu) / (mu0 + mu)) * area;
    totalLommelSeeliger += ls;
    totalLambert += flat;
    const end = offsets[e + 1] ?? 0;
    for (let k = offsets[e] ?? 0; k < end; k += 1) {
      const m = law[k] ?? 0;
      const w = share[k] ?? 0;
      lommelSeeliger[m] = (lommelSeeliger[m] ?? 0) + w * ls;
      lambert[m] = (lambert[m] ?? 0) + w * flat;
    }
  }
  return { lommelSeeliger, lambert, totalLommelSeeliger, totalLambert };
}

/**
 * The ratio, per channel, of the light a class map reflects towards a distant observer to the
 * light its `elsewhere` law alone would, both by the same elements: what the point regime
 * multiplies the photometry's point by under the map. 1 in a channel where `elsewhere` alone
 * reflects nothing by the elements: where none is lit and visible, at the extreme of phase.
 *
 * @param figure - The body's figure, drawn about the body-fixed z axis.
 * @param towardsLight - The unit direction from the body to the light, body-fixed.
 * @param towardsObserver - The unit direction from the body to the observer, body-fixed.
 * @throws Error if the surface's laws are not one per class of its map.
 */
export function classMapReflectionRatio(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Rgb {
  if (surface.laws.length !== surface.texels.classes) {
    throw new Error(
      `a class map of ${String(surface.texels.classes)} classes needs as many laws, got ${String(surface.laws.length)}`,
    );
  }
  const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
  const sums = lawSums(elementSharesOf(surface.texels), cOverA, towardsLight, towardsObserver);
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towardsLight, towardsObserver))));
  const terms = discLawTerms(discSurfaceLaws(surface), cOverA, alpha);
  const ratio: [number, number, number] = [1, 1, 1];
  for (const c of [0, 1, 2] as const) {
    let mapped = 0;
    terms.forEach((term, m) => {
      mapped +=
        term.lommelSeeliger[c] * (sums.lommelSeeliger[m] ?? 0) +
        term.lambert[c] * (sums.lambert[m] ?? 0);
    });
    const uniform = terms[0];
    const alone =
      uniform === undefined
        ? 0
        : uniform.lommelSeeliger[c] * sums.totalLommelSeeliger +
          uniform.lambert[c] * sums.totalLambert;
    if (alone > 0) {
      ratio[c] = mapped / alone;
    }
  }
  return ratio;
}

/** A ratio found at one geometry. */
interface HeldRatio {
  readonly towardsLight: Vec3;
  readonly towardsObserver: Vec3;
  readonly cOverA: number;
  readonly ratio: Rgb;
}

/** The ratios a surface holds at most: two stars and two planetshine sources, from two views. */
const HELD_PER_SURFACE = 8;

/** Each surface's ratios, the latest found last, held while the surface is. */
const HELD = new WeakMap<ClassMapDiscSurface, HeldRatio[]>();

/** Whether two unit directions are within `angleRad` (by their chord, 2 sin(θ ÷ 2) ≤ θ). */
function within(a: Vec3, b: Vec3, angleRad: number): boolean {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return dx * dx + dy * dy + dz * dz <= angleRad * angleRad;
}

/** The ratio at a geometry: one held within the cache's angle, or found and held. */
function heldRatio(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Rgb {
  const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towardsLight, towardsObserver))));
  const angle = classMapPointCacheAngleRad(alpha);
  const held = HELD.get(surface) ?? [];
  const found = held.find(
    (entry) =>
      entry.cOverA === cOverA &&
      within(entry.towardsLight, towardsLight, angle) &&
      within(entry.towardsObserver, towardsObserver, angle),
  );
  if (found !== undefined) {
    return found.ratio;
  }
  const ratio = classMapReflectionRatio(figure, surface, towardsLight, towardsObserver);
  HELD.set(surface, [
    ...held.slice(-(HELD_PER_SURFACE - 1)),
    { towardsLight, towardsObserver, cOverA, ratio },
  ]);
  return ratio;
}

/**
 * The light a body under a class map reflects towards a distant observer, per channel, over the
 * illuminance on it and (a ÷ Δ)², as `bodyReflection` gives a uniform body's: the photometry's
 * point of `elsewhere` on the figure drawn about the rotation's pole, times the map's ratio to it
 * ({@link classMapReflectionRatio}), cached by the body-fixed directions within
 * {@link classMapPointCacheAngleRad}. Uneclipsed: the caller cuts each star's light.
 *
 * @param rotation - The body's rotation, body-fixed to galactic axes.
 * @param towardsLight - The unit direction from the body to the light, along the galactic axes.
 * @param towardsObserver - The unit direction from the body to the observer, along the galactic
 *   axes.
 * @throws Error if the surface's laws are not one per class of its map.
 */
export function classMapPointReflection(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  rotation: Rotation3,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Rgb {
  // The uniform point in the galactic axes, about the rotation's pole, as `pointFlux` takes a
  // uniform body's about its figure's: the same call, bit for bit, where the two poles agree.
  const uniform = bodyReflection(
    { ...figure, pole: rotateToBody(rotation, BODY_FIXED_POLE) },
    surface.elsewhere,
    towardsLight,
    towardsObserver,
  );
  const ratio = heldRatio(
    figure,
    surface,
    rotateToBodyFixed(rotation, towardsLight),
    rotateToBodyFixed(rotation, towardsObserver),
  );
  return [uniform[0] * ratio[0], uniform[1] * ratio[1], uniform[2] * ratio[2]];
}
