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
 *   limb and the terminator cancels in the ratio. Without a normal map the ratio is bounded by the
 *   map's laws' extremes; with one it is not, since the slopes move the light as well as the laws
 *   (one slope of 12° along every face's s moves it by +62% facing a pole at 120°, R10.T10.g).
 * - **The cache.** The ratio, the light's own and uneclipsed, is held per surface and light by the
 *   body-fixed directions of the light and the observer, and taken again while both stay within
 *   δ of where it was found ({@link classMapPointCacheAngleRad}), and past δ while a fresh one is
 *   found (below); the uniform point is always fresh.
 *   The entries are held in a `WeakMap` keyed by the surface, at most eight a surface, so they go
 *   with their surface when its owner drops it (a new map, a new `elsewhere`, the body released);
 *   the elements' shares likewise with the map's texels.
 * - **The normals** (plan R10, R10.T10.g). Where the surface carries the field's normal map, each
 *   element shades on that map's normals (`normalMapNormalAt`; the datum's where it holds none), as
 *   the disc's pixels do (`discShading.ts`), found once a map and held with its texels: k × k an
 *   element at its sub-elements' centres, two a normal texel's side
 *   ({@link classMapPointNormalSamples}: one, the element's centre, at levels 5 and 6, 2 × 2 at 7,
 *   4 × 4 at 8), each element's terms their mean. The I/F is on the shading normal n′, μ floored at
 *   half the datum's, the projected area the datum's, so that the sums are
 *   Σ share · 2μ₀′μ_ν ÷ (μ₀′ + μ′) dS and Σ share · μ₀′μ_ν dS over the normals lit on both (μ₀′ > 0
 *   and μ₀_ν > 0, the point-source form of the disc's cut at the datum's horizon) and visible
 *   (μ_ν > 0). The normalisation's sums, `elsewhere`'s alone, stay on the datum's normals, so that
 *   the ratio carries the slopes. Without a normal map every value is as before.
 * - **The maturity** (plan R10, R10.T1.c's client half; decision-r10-t1c-client-scope §2.3). Where
 *   the map carries its texels' maturity, each element's share of a class drawn from its nodes is
 *   multiplied per channel by T_c(m_e) ÷ T_c(m_body) as the shares are made: the class's albedo at
 *   the element, as the disc draws it (`classMapAlbedoAt`), over its law's A, its node K − 1 (the
 *   store's `classMapLawsOf`), so that the class's sums carry its albedo across the body. The I/F
 *   is linear in A for a law of fixed row, so the fold is exact, and the frame's path is as it was.
 *   The point meets the disc only while each drawn class's A is its node K − 1, so a surface whose
 *   A is another is refused (`assertLawPerClass`), as one whose laws are not one per class is.
 *   The factors, three `f32` an entry, are made in the same steps as the shares, the sliced and the
 *   synchronous builds alike. Without a maturity every value is as before, bit for bit.
 * - **The build, off the frame** (plan R10, R10.T14.f). The elements' shares and normals are made
 *   in slices of at most {@link CLASS_MAP_POINT_SLICE_MS} a frame ({@link stepClassMapPoints},
 *   the terrain session's step), from the moment the store takes a map
 *   ({@link queueClassMapPoint}), the oldest map's first; the frame's path
 *   ({@link classMapPointReflection}) never makes them. Made at once on the render thread they
 *   stopped it for about 0.3 s at field level 6, 0.6 s at 7 and 1.4 s at 8 (T10.f's and T10.g's
 *   records). Until a map's are made, its point is its previous map's, the one it replaced, where
 *   that one's were made, and otherwise the uniform point of `elsewhere`: what was drawn before the
 *   map came. A newer map cancels the older's unfinished build. Each slice takes the elements in
 *   the order the synchronous path does ({@link classMapReflectionRatio}, for tests and harnesses),
 *   by the same code, so that the ratio is the same bit for bit however the build is cut.
 * - **The misses, off the frame too** (R10.T14.f). A light's evaluation at level 8 costs about 6 ms
 *   (k² = 16 normals an element), and under time acceleration a fast rotator misses the cache every
 *   frame. So past δ a ratio held within {@link classMapPointStaleAngleRad} stands in, and the fresh
 *   one is found in the same slices, its sums in the synchronous order, and held. As a map's
 *   elements are made, what its point was drawn with (its stand-in's ratios, or the uniform
 *   point's) stands in the same way until its own are found, so that no light is evaluated on the
 *   frame they are made. A light's first evaluation with none held so near, the body's first frame
 *   as a point under a map made before, is found at once, so that the point meets the disc it
 *   switches with from that frame.
 */
import { dot, type Vec3, vec3 } from "../../geometry/vec3";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { type Rotation3, rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import type { Rgb } from "../photometry/toneCurve";
import { faceUvToXyz, stToUv } from "../terrain/cube";
import { MATURITY_NODES } from "../terrain/maturityNodes";
import type { BodyFigure } from "../terrain/planet";
import type { Face } from "../terrain/patchKey";
import { type NormalMapTexels, normalMapNormalAt } from "./discNormals";
import { discLawTerms } from "./discShading";
import {
  classMapAlbedoAt,
  type ClassMapMaturity,
  classMapSharesAt,
  type ClassMapTexels,
  discSurfaceLaws,
} from "./discSurface";
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
 * The shading normals an element takes along each side under a normal map of `normalTexels` a
 * face's side (R10.T10.g), its elements `elementSide` a face's side: two a normal texel's side, at
 * its quarter points, where the reconstruction is bilinear between the four nearest texel centres;
 * so 1 at levels 5 and 6, 2 at level 7 and 4 at level 8 under R10's 128 elements a face's side.
 *
 * @remarks
 * One normal at each element's centre reads a level-8 map at the corner four of its texels share,
 * so that slopes changing from cell to cell are seen at their mean: the point then missed the disc
 * at the 3 px switch by up to 2.4% under ridges of level-8 cells four a period and 1.4% under a
 * ±12° checkerboard of them, against R07's 1%; one normal a level-8 texel, at its centre, sees each
 * cell's whole tilt and missed an `f64` brute force by up to 12% (the checkerboard). At two a
 * texel's side the point is within 6.3 × 10⁻³ of the brute force at levels 7 and 8 (4 × 10⁶
 * directions), as at level 6, and within 0.74% of the disc at the switch (2026-10-21). The cost at
 * level 8: the normals 1.1 s once a map, 19 MB (`f32`), and an evaluation 2 to 7 ms a light,
 * against 62 ms, 1.2 MB and 1.2 ms at level 6 (vitest's V8, provisional).
 */
export function classMapPointNormalSamples(elementSide: number, normalTexels: number): number {
  return Math.max(1, Math.ceil((2 * normalTexels) / elementSide));
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

/**
 * The longest a frame's slice of the class-mapped points' work runs, ms (R10.T14.f;
 * decision-next-lane-2026-10-21b §2): an eighth of a 60 Hz frame, against R05 Design note 21's
 * hitch row (none over 50 ms past 3 T) and the views' gap limit of 250 ms
 * (decision-r10-t14d-stall §3).
 *
 * @remarks
 * A slice ends at the first step past it, a step making a row of 128 elements' geometry, about 16
 * elements' shares or 16 normals, copying a block of 65,536 shares, or taking 256 elements of an
 * evaluation: some tens of microseconds each.
 */
export const CLASS_MAP_POINT_SLICE_MS = 2;

/** The normals, or the elements' shares, a step of a build makes before it yields. */
const STEP_WORK = 16;

/** Runs a build's steps to their end at once: the synchronous path. */
function finished<T>(steps: Generator<void, T>): T {
  for (;;) {
    const step = steps.next();
    if (step.done === true) {
      return step.value;
    }
  }
}

/** Each fineness's elements, made on first use: one per fineness in use. */
const GEOMETRIES = new Map<number, ElementGeometry>();

/**
 * The sphere's elements at `side` along a face's side, face by face, then row by row in t, a row a
 * step; the one held where another build made them meanwhile.
 */
function* geometrySteps(side: number): Generator<void, ElementGeometry> {
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
      yield;
    }
  }
  const geometry = GEOMETRIES.get(side) ?? { count, directions, solidAngles };
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
  /**
   * Under a map's maturity (R10.T1.c's client half), each entry's factor on its law's A per display
   * channel, three an entry (r, g, b): T_c(m_e) ÷ T_c(m_body), the class's albedo at the element
   * (`classMapAlbedoAt`) over its law's A, its node K − 1; 1 for `elsewhere` and a class not drawn
   * from its nodes. `null` for a map without a maturity.
   */
  readonly factor: Float32Array | null;
}

/** Each map's element shares, made once and held while the map is. */
const SHARES = new WeakMap<ClassMapTexels, ElementShares>();

/** The entries a block holds while a map's shares are made, a block's copy a step (R10.T14.f). */
const SHARE_BLOCK = 2 ** 16;

/**
 * A block of a map's share entries while they are made: each entry's law and share, and its
 * factors under a maturity.
 */
interface ShareBlock {
  readonly law: Uint8Array;
  readonly share: Float64Array;
  readonly factor: Float32Array | null;
}

/** The display channels' indices, r, g, b. */
const CHANNELS = [0, 1, 2] as const;

/**
 * An entry's factors on its law's A under a map's maturity (R10.T1.c's client half), into `out` at
 * `at`: law m's class's albedo at the element over its node K − 1 per channel, 1 for `elsewhere`, a
 * class drawn by its law's A there (`albedo` `null`), or a channel whose node K − 1 is not above 0.
 */
function putFactors(
  out: Float32Array,
  at: number,
  maturity: ClassMapMaturity,
  albedo: ReadonlyArray<Rgb | null> | null,
  m: number,
): void {
  const drawn = m === 0 ? null : (albedo?.[m - 1] ?? null);
  const background = 3 * (MATURITY_NODES * (m - 1) + MATURITY_NODES - 1);
  for (const c of CHANNELS) {
    const own = maturity.nodes[background + c] ?? 0;
    out[at + c] = drawn === null || !(own > 0) ? 1 : drawn[c] / own;
  }
}

/** The elements along a face's side for a class map's point. */
function elementSideOf(texels: ClassMapTexels): number {
  return texels.faceTexels * classMapPointSubdivision(texels.faceTexels);
}

/**
 * The shares of a map's laws at each of its elements, by the survey-masked reconstruction, after
 * the elements' geometry where it is not made: {@link STEP_WORK} elements a step, and the stores'
 * copy one; those held where they were made meanwhile. Under a map's maturity (R10.T1.c's client
 * half) each entry's factors on its law's A are found with its share, from the classes' albedos
 * at the element (`classMapAlbedoAt`), in the same steps: the one loop of the sliced and the
 * synchronous builds.
 */
function* sharesSteps(texels: ClassMapTexels): Generator<void, ElementShares> {
  const held = SHARES.get(texels);
  if (held !== undefined) {
    return held;
  }
  const geometry = yield* geometrySteps(elementSideOf(texels));
  const offsets = new Uint32Array(geometry.count + 1);
  const { maturity } = texels;
  // The entries in blocks while they are made, then copied into their stores a block a step, so
  // that no step copies a whole map's (614,400 entries on an eight-class map; R10.T14.f).
  const newBlock = (): ShareBlock => ({
    law: new Uint8Array(SHARE_BLOCK),
    share: new Float64Array(SHARE_BLOCK),
    factor: maturity === undefined ? null : new Float32Array(3 * SHARE_BLOCK),
  });
  let block = newBlock();
  const blocks = [block];
  let used = 0;
  let entries = 0;
  const { directions } = geometry;
  for (let e = 0; e < geometry.count; e += 1) {
    const direction = [
      directions[3 * e] ?? 0,
      directions[3 * e + 1] ?? 0,
      directions[3 * e + 2] ?? 0,
    ] as const;
    const shares = classMapSharesAt(texels, direction);
    const albedo = maturity === undefined ? null : classMapAlbedoAt(texels, direction, shares);
    for (const [m, value] of shares.entries()) {
      if (value > 0) {
        if (used === SHARE_BLOCK) {
          block = newBlock();
          blocks.push(block);
          used = 0;
        }
        block.law[used] = m;
        block.share[used] = value;
        if (maturity !== undefined && block.factor !== null) {
          putFactors(block.factor, 3 * used, maturity, albedo, m);
        }
        used += 1;
        entries += 1;
      }
    }
    offsets[e + 1] = entries;
    if ((e + 1) % STEP_WORK === 0) {
      yield;
    }
  }
  const law = new Uint8Array(entries);
  const share = new Float64Array(entries);
  const factor = maturity === undefined ? null : new Float32Array(3 * entries);
  let copied = 0;
  for (const full of blocks) {
    yield;
    const length = Math.min(SHARE_BLOCK, entries - copied);
    law.set(full.law.subarray(0, length), copied);
    share.set(full.share.subarray(0, length), copied);
    if (factor !== null && full.factor !== null) {
      factor.set(full.factor.subarray(0, 3 * length), 3 * copied);
    }
    copied += length;
  }
  const made = SHARES.get(texels) ?? {
    geometry,
    laws: texels.classes + 1,
    offsets,
    law,
    share,
    factor,
  };
  SHARES.set(texels, made);
  return made;
}

/** A normal map's shading normals at a fineness of elements, k × k an element. */
interface ElementNormals {
  /** k, the normals along an element's side ({@link classMapPointNormalSamples}). */
  readonly samples: number;
  /**
   * Element e's k² normals from entry 3 k² e, row by row in t, each three: body-fixed, `f32`
   * (ample for the map's half floats), NaN in the first where the map holds none (the datum's).
   */
  readonly normals: Float32Array;
}

/** Each normal map's normals by the elements' fineness, made once, held while it is. */
const NORMALS = new WeakMap<NormalMapTexels, Map<number, ElementNormals>>();

/** The normals of `normals` made at `side` elements a face's side, if they are. */
function madeNormals(normals: NormalMapTexels, side: number): ElementNormals | undefined {
  return NORMALS.get(normals)?.get(side);
}

/**
 * The shading normals of `normals` at each element of `geometry`, `side` a face's side, k × k an
 * element at its sub-elements' centres in (s, t): at k = 1 the element's own centre. A step makes
 * the elements of about {@link STEP_WORK} normals; those held where they were made meanwhile.
 */
function* normalsSteps(
  normals: NormalMapTexels,
  geometry: ElementGeometry,
  side: number,
): Generator<void, ElementNormals> {
  const found = madeNormals(normals, side);
  if (found !== undefined) {
    return found;
  }
  const k = classMapPointNormalSamples(side, normals.faceTexels);
  const perStep = Math.max(1, Math.floor(STEP_WORK / (k * k)));
  const made = new Float32Array(3 * k * k * geometry.count);
  const put = (at: number, normal: Vec3 | null): void => {
    made[at] = normal?.x ?? Number.NaN;
    made[at + 1] = normal?.y ?? Number.NaN;
    made[at + 2] = normal?.z ?? Number.NaN;
  };
  const { directions } = geometry;
  let e = 0;
  for (const face of FACES) {
    for (let j = 0; j < side; j += 1) {
      for (let i = 0; i < side; i += 1) {
        if (k === 1) {
          put(
            3 * e,
            normalMapNormalAt(normals, [
              directions[3 * e] ?? 0,
              directions[3 * e + 1] ?? 0,
              directions[3 * e + 2] ?? 0,
            ]),
          );
        } else {
          for (let b = 0; b < k; b += 1) {
            const v = stToUv((j + (b + 0.5) / k) / side);
            for (let a = 0; a < k; a += 1) {
              const u = stToUv((i + (a + 0.5) / k) / side);
              put(3 * (k * k * e + k * b + a), normalMapNormalAt(normals, faceUvToXyz(face, u, v)));
            }
          }
        }
        e += 1;
        if (e % perStep === 0) {
          yield;
        }
      }
    }
  }
  const held = NORMALS.get(normals) ?? new Map<number, ElementNormals>();
  const elementNormals = held.get(side) ?? { samples: k, normals: made };
  held.set(side, elementNormals);
  NORMALS.set(normals, held);
  return elementNormals;
}

/** A map's elements as the integral takes them: their shares, and their normals under a normal map. */
interface PointElements {
  readonly shares: ElementShares;
  readonly normals: ElementNormals | null;
}

/** A map's elements, the shares first, then the normals under `normals`, step by step. */
function* elementsSteps(
  texels: ClassMapTexels,
  normals: NormalMapTexels | null,
): Generator<void, PointElements> {
  const shares = yield* sharesSteps(texels);
  return {
    shares,
    normals:
      normals === null
        ? null
        : yield* normalsSteps(normals, shares.geometry, elementSideOf(texels)),
  };
}

/** A map's elements where both are made, `null` while either is not. */
function madeElements(
  texels: ClassMapTexels,
  normals: NormalMapTexels | null,
): PointElements | null {
  const shares = SHARES.get(texels);
  if (shares === undefined) {
    return null;
  }
  if (normals === null) {
    return { shares, normals: null };
  }
  const made = madeNormals(normals, elementSideOf(texels));
  return made === undefined ? null : { shares, normals: made };
}

/** A surface's elements where they are made, `null` while they are not. */
function surfaceElements(surface: ClassMapDiscSurface): PointElements | null {
  return madeElements(surface.texels, surface.normals?.texels ?? null);
}

/** A map's build, queued: its steps, run a slice at a time. */
interface QueuedBuild {
  readonly texels: ClassMapTexels;
  readonly normals: NormalMapTexels | null;
  readonly steps: Generator<void, PointElements>;
}

/** The builds queued, the oldest first: the store's maps, and any a frame's point found unmade. */
const QUEUE: QueuedBuild[] = [];

/**
 * Each unfinished map's stand-in, the surface its point is drawn by until its elements are made:
 * the map it replaced where that one's were made, or that one's own stand-in. Forgotten as its
 * elements are made, so that the map it stands for is let go, its held ratios kept as the map's
 * provisional ones ({@link PROVISIONAL}).
 */
const STAND_INS = new WeakMap<ClassMapTexels, ClassMapDiscSurface>();

/**
 * Each map's provisional ratios (R10.T14.f): what its point was drawn with before its elements were
 * made, its stand-in's held ratios or the uniform point's ratio of 1 where it was drawn so, which
 * stand in past δ, as a held ratio does, until the map's own are found in the frames' slices; so
 * that the frame on which the elements are made evaluates no light. Never taken within δ as the
 * map's own.
 */
const PROVISIONAL = new WeakMap<ClassMapTexels, ReadonlyArray<HeldRatio>>();

/** The maps whose build threw: their points stay their stand-ins', and they are not queued again. */
const FAILED = new WeakSet<ClassMapTexels>();

/** Queues `texels`' elements under `normals` unless they are made, queued or failed. */
function queueBuild(texels: ClassMapTexels, normals: NormalMapTexels | null): void {
  if (
    FAILED.has(texels) ||
    madeElements(texels, normals) !== null ||
    QUEUE.some((build) => build.texels === texels && build.normals === normals)
  ) {
    return;
  }
  QUEUE.push({ texels, normals, steps: elementsSteps(texels, normals) });
}

/** The map a newly taken one replaces (R10.T14.f): what its point was drawn by. */
export interface ClassMapPointPrevious {
  readonly texels: ClassMapTexels;
  /** The surface last handed out over it, `null` where none was. */
  readonly surface: ClassMapDiscSurface | null;
}

/**
 * Queues the making of a map's elements for the frames' slices ({@link stepClassMapPoints}), as
 * its owner takes it in place of `previous` (the class-map store, R10.T14.f).
 *
 * @remarks
 * `previous` is the body's map it replaces. That one's unfinished build is cancelled, and its
 * point, where its elements were made and a surface was handed out over it (or else its own
 * stand-in), stands in for this map's until this map's elements are made, with the provisional
 * ratios it was drawn with meanwhile. With no `previous`, or none that stands, the point is the
 * uniform point of `elsewhere` meanwhile.
 *
 * @param normals - The field's normal map the surface carries with `texels`, or `null`.
 */
export function queueClassMapPoint(
  texels: ClassMapTexels,
  normals: NormalMapTexels | null,
  previous: ClassMapPointPrevious | null,
): void {
  if (previous !== null && previous.texels !== texels) {
    const standIn =
      previous.surface !== null && surfaceElements(previous.surface) !== null
        ? previous.surface
        : (STAND_INS.get(previous.texels) ?? null);
    const provisional = PROVISIONAL.get(previous.texels);
    cancelClassMapPoint(previous.texels);
    if (standIn !== null) {
      STAND_INS.set(texels, standIn);
    }
    if (provisional !== undefined) {
      PROVISIONAL.set(texels, provisional);
    }
  }
  queueBuild(texels, normals);
}

/**
 * Cancels a map's unfinished build, if it has one, and the refreshes queued for its point, its
 * stand-in's among them, and forgets its stand-in and provisional ratios.
 */
export function cancelClassMapPoint(texels: ClassMapTexels): void {
  STAND_INS.delete(texels);
  PROVISIONAL.delete(texels);
  for (let q = QUEUE.length - 1; q >= 0; q -= 1) {
    if (QUEUE[q]?.texels === texels) {
      QUEUE.splice(q, 1);
    }
  }
  dropRefreshes((refresh) => refresh.forTexels === texels || refresh.surface.texels === texels);
}

/** Drops the queued refreshes `dropped` picks. */
function dropRefreshes(dropped: (refresh: QueuedRefresh) => boolean): void {
  for (let q = REFRESHES.length - 1; q >= 0; q -= 1) {
    const refresh = REFRESHES[q];
    if (refresh !== undefined && dropped(refresh)) {
      REFRESHES.splice(q, 1);
    }
  }
}

/** Where a surface's point stands (R10.T14.f): its elements made, queued, or neither. */
export type ClassMapPointState = "made" | "queued" | "unmade";

/** Where `surface`'s point stands: `unmade` for one never queued, cancelled or failed. */
export function classMapPointState(surface: ClassMapDiscSurface): ClassMapPointState {
  if (surfaceElements(surface) !== null) {
    return "made";
  }
  const normals = surface.normals?.texels ?? null;
  return QUEUE.some((build) => build.texels === surface.texels && build.normals === normals)
    ? "queued"
    : "unmade";
}

/** The work the frames' slices hold (R10.T14.f). */
export interface ClassMapPointWork {
  /** The maps' builds queued and not finished. */
  readonly builds: number;
  /** The ratios queued to be found again. */
  readonly refreshes: number;
}

/** The work the frames' slices hold now. */
export function classMapPointWork(): ClassMapPointWork {
  return { builds: QUEUE.length, refreshes: REFRESHES.length };
}

/**
 * Each law's geometric sums over the lit and visible elements, the shares weighting them: the
 * Lommel–Seeliger term's Σ share · 2μ₀μ ÷ (μ₀ + μ) dS and the Lambert term's Σ share · μ₀μ dS, and
 * the same sums over every element unweighted, `elsewhere`'s alone.
 *
 * @remarks
 * With `normals` (R10.T10.g), each law's sums take each element's shading normals n′, the mean of
 * its k² terms: Σ share · 2μ₀′μ ÷ (μ₀′ + μ′) dS and Σ share · μ₀′μ dS, μ the datum's (the
 * projected area), μ′ = max(n′ · v, μ ÷ 2), a term 0 where n′ is unlit, the datum's where the map
 * holds none, over the elements lit and visible on the datum; the unweighted sums stay the datum's.
 *
 * Under a map's maturity (R10.T1.c's client half) each law's sums are per display channel too,
 * each entry's share times its factor on the law's A (`ElementShares.factor`): Σ f_c share · …,
 * law m's channel c at 3m + c. The I/F is linear in A for a law of fixed row, so these times the
 * law's terms are the map's light with each class's A its albedo at each element.
 */
interface LawSums {
  readonly lommelSeeliger: Float64Array;
  readonly lambert: Float64Array;
  /** Under a maturity, each law's factor-weighted sums per channel, 3 a law; `null` without. */
  readonly lommelSeeligerRgb: Float64Array | null;
  readonly lambertRgb: Float64Array | null;
  totalLommelSeeliger: number;
  totalLambert: number;
}

/** The elements a step of an evaluation takes before it yields (R10.T14.f): some tens of µs. */
const EVALUATION_STEP = 256;

/** The sums before any element, each law's and the unweighted, 0. */
function emptyLawSums(shares: ElementShares): LawSums {
  const rgb = shares.factor !== null;
  return {
    lommelSeeliger: new Float64Array(shares.laws),
    lambert: new Float64Array(shares.laws),
    lommelSeeligerRgb: rgb ? new Float64Array(3 * shares.laws) : null,
    lambertRgb: rgb ? new Float64Array(3 * shares.laws) : null,
    totalLommelSeeliger: 0,
    totalLambert: 0,
  };
}

/**
 * Adds elements `from` to `to` (not included) to `sums`, in order: called once over every element
 * at once, or a step at a time over consecutive ranges, it sums the same terms in the same order.
 */
function addLawSums(
  sums: LawSums,
  shares: ElementShares,
  cOverA: number,
  towardsLight: Vec3,
  towardsObserver: Vec3,
  normals: ElementNormals | null,
  from: number,
  to: number,
): void {
  const { geometry, offsets, law, share, factor } = shares;
  const { directions, solidAngles } = geometry;
  const { lommelSeeliger, lambert, lommelSeeligerRgb, lambertRgb } = sums;
  let { totalLommelSeeliger, totalLambert } = sums;
  const { x: sx, y: sy, z: sz } = towardsLight;
  const { x: vx, y: vy, z: vz } = towardsObserver;
  const tilted = normals === null ? null : normals.normals;
  const perElement = normals === null ? 0 : normals.samples * normals.samples;
  const stride = 3 * perElement;
  for (let e = from; e < to; e += 1) {
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
    // The element's own terms: the datum's, or the mean over its shading normals n′ (R10.T10.g).
    let shadedFlat = flat;
    let shadedLs = ls;
    if (tilted !== null) {
      let sumFlat = 0;
      let sumLs = 0;
      const first = stride * e;
      const last = first + stride;
      for (let q = first; q < last; q += 3) {
        const tx = tilted[q] ?? Number.NaN;
        if (Number.isNaN(tx)) {
          sumFlat += flat;
          sumLs += ls;
          continue;
        }
        const ty = tilted[q + 1] ?? 0;
        const tz = tilted[q + 2] ?? 0;
        const mu0Tilted = tx * sx + ty * sy + tz * sz;
        if (!(mu0Tilted > 0)) {
          continue;
        }
        const muTilted = Math.max(tx * vx + ty * vy + tz * vz, 0.5 * mu);
        sumFlat += mu0Tilted * mu * area;
        sumLs += ((2 * mu0Tilted * mu) / (mu0Tilted + muTilted)) * area;
      }
      shadedFlat = sumFlat / perElement;
      shadedLs = sumLs / perElement;
    }
    const end = offsets[e + 1] ?? 0;
    if (factor === null || lommelSeeligerRgb === null || lambertRgb === null) {
      for (let k = offsets[e] ?? 0; k < end; k += 1) {
        const m = law[k] ?? 0;
        const w = share[k] ?? 0;
        lommelSeeliger[m] = (lommelSeeliger[m] ?? 0) + w * shadedLs;
        lambert[m] = (lambert[m] ?? 0) + w * shadedFlat;
      }
    } else {
      // Under a maturity (R10.T1.c): each channel's sum by the entry's factor on its law's A.
      for (let k = offsets[e] ?? 0; k < end; k += 1) {
        const at = 3 * (law[k] ?? 0);
        const w = share[k] ?? 0;
        const wLs = w * shadedLs;
        const wFlat = w * shadedFlat;
        const fr = factor[3 * k] ?? 1;
        const fg = factor[3 * k + 1] ?? 1;
        const fb = factor[3 * k + 2] ?? 1;
        lommelSeeligerRgb[at] = (lommelSeeligerRgb[at] ?? 0) + fr * wLs;
        lommelSeeligerRgb[at + 1] = (lommelSeeligerRgb[at + 1] ?? 0) + fg * wLs;
        lommelSeeligerRgb[at + 2] = (lommelSeeligerRgb[at + 2] ?? 0) + fb * wLs;
        lambertRgb[at] = (lambertRgb[at] ?? 0) + fr * wFlat;
        lambertRgb[at + 1] = (lambertRgb[at + 1] ?? 0) + fg * wFlat;
        lambertRgb[at + 2] = (lambertRgb[at + 2] ?? 0) + fb * wFlat;
      }
    }
  }
  sums.totalLommelSeeliger = totalLommelSeeliger;
  sums.totalLambert = totalLambert;
}

/**
 * The ratio, per channel, of the light a class map reflects towards a distant observer to the
 * light its `elsewhere` law alone would, both by the same elements: what the point regime
 * multiplies the photometry's point by under the map. 1 in a channel where `elsewhere` alone
 * reflects nothing by the elements: where none is lit and visible, at the extreme of phase. Where
 * the surface carries the field's normal map, the map's sums take its slopes and `elsewhere`'s stay
 * the datum's (R10.T10.g), so that the ratio carries them.
 *
 * @remarks
 * The synchronous path, for tests, harnesses and benches: where the surface's elements are not
 * made it makes them at once, on the calling thread, as the frame's path never does
 * ({@link classMapPointReflection}).
 *
 * @param figure - The body's figure, drawn about the body-fixed z axis.
 * @param towardsLight - The unit direction from the body to the light, body-fixed.
 * @param towardsObserver - The unit direction from the body to the observer, body-fixed.
 * @throws Error if the surface's laws are not one per class of its map, or, under its map's
 *   maturity, a class drawn from its nodes has an A other than its node K − 1.
 */
export function classMapReflectionRatio(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Rgb {
  assertLawPerClass(surface);
  return ratioOf(figure, surface, elementsNow(surface), towardsLight, towardsObserver);
}

/** A surface's elements, made at once where they are not. */
function elementsNow(surface: ClassMapDiscSurface): PointElements {
  return (
    surfaceElements(surface) ??
    finished(elementsSteps(surface.texels, surface.normals?.texels ?? null))
  );
}

/**
 * Makes a surface's elements at once on the calling thread where they are not made: the
 * synchronous path's build, for tests, harnesses and benches.
 *
 * @remarks
 * The frame's path then draws the surface's own map. A queued build of the same elements is left
 * to finish on what this made.
 */
export function makeClassMapPointNow(surface: ClassMapDiscSurface): void {
  elementsNow(surface);
}

/**
 * Checks the invariants the point's sums take of a surface's laws: one law per class of its map,
 * and, under the map's maturity, each class drawn from its nodes with its node K − 1 as its A.
 *
 * @remarks
 * A drawn class's factors are T_c(m_e) ÷ T_c(m_body) on its law's A (`putFactors`), so the
 * point draws the disc's T_c(m_e) only where that A is T_c(m_body), its node K − 1 in the display
 * channels, as the store's `classMapLawsOf` sets it. The test is exact equality in `f32`, the
 * nodes' own precision: each channel of the A rounded to `f32` (`Math.fround`) must equal the
 * node's value as its `Float32Array` holds it. The store's A, read from those very values, passes
 * as it is; an `f64` A within the rounding of the node passes too, its factor's `f32` error being as
 * large. A class not drawn from its nodes, and a map without a maturity, take any A.
 *
 * @throws Error if the surface's laws are not one per class of its map, or if a class drawn from
 *   its nodes has an A that is not its node K − 1.
 */
function assertLawPerClass(surface: ClassMapDiscSurface): void {
  if (surface.laws.length !== surface.texels.classes) {
    throw new Error(
      `a class map of ${String(surface.texels.classes)} classes needs as many laws, got ${String(surface.laws.length)}`,
    );
  }
  const { maturity } = surface.texels;
  if (maturity === undefined) {
    return;
  }
  surface.laws.forEach((law, k) => {
    if (maturity.drawn[k] !== true) {
      return;
    }
    const background = 3 * (MATURITY_NODES * k + MATURITY_NODES - 1);
    const node = CHANNELS.map((c) => maturity.nodes[background + c] ?? Number.NaN);
    if (CHANNELS.some((c) => Math.fround(law.a[c]) !== node[c])) {
      throw new Error(
        `class ${String(k)} of a matured class map is drawn from its nodes, so its law's A must be its node K − 1 (${node.join(", ")}), not (${law.a.join(", ")}): the point would not meet the disc`,
      );
    }
  });
}

/** {@link classMapReflectionRatio} by the surface's made elements, at once, over every element. */
function ratioOf(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  elements: PointElements,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Rgb {
  const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
  const sums = emptyLawSums(elements.shares);
  addLawSums(
    sums,
    elements.shares,
    cOverA,
    towardsLight,
    towardsObserver,
    elements.normals,
    0,
    elements.shares.geometry.count,
  );
  return ratioOfSums(surface, cOverA, towardsLight, towardsObserver, sums);
}

/**
 * {@link classMapReflectionRatio} by the surface's made elements, {@link EVALUATION_STEP} elements
 * a step: the same sums in the same order as {@link ratioOf}'s.
 */
function* ratioSteps(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  elements: PointElements,
  towardsLight: Vec3,
  towardsObserver: Vec3,
): Generator<void, Rgb> {
  const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
  const sums = emptyLawSums(elements.shares);
  const { count } = elements.shares.geometry;
  for (let from = 0; from < count; from += EVALUATION_STEP) {
    if (from > 0) {
      yield;
    }
    addLawSums(
      sums,
      elements.shares,
      cOverA,
      towardsLight,
      towardsObserver,
      elements.normals,
      from,
      Math.min(count, from + EVALUATION_STEP),
    );
  }
  return ratioOfSums(surface, cOverA, towardsLight, towardsObserver, sums);
}

/** The ratio of the map's light to `elsewhere`'s alone, from the elements' sums. */
function ratioOfSums(
  surface: ClassMapDiscSurface,
  cOverA: number,
  towardsLight: Vec3,
  towardsObserver: Vec3,
  sums: LawSums,
): Rgb {
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towardsLight, towardsObserver))));
  const terms = discLawTerms(discSurfaceLaws(surface), cOverA, alpha);
  const ratio: [number, number, number] = [1, 1, 1];
  const { lommelSeeligerRgb, lambertRgb } = sums;
  for (const c of [0, 1, 2] as const) {
    let mapped = 0;
    if (lommelSeeligerRgb === null || lambertRgb === null) {
      terms.forEach((term, m) => {
        mapped +=
          term.lommelSeeliger[c] * (sums.lommelSeeliger[m] ?? 0) +
          term.lambert[c] * (sums.lambert[m] ?? 0);
      });
    } else {
      // Under a maturity (R10.T1.c), each law's channel sums by its entries' factors.
      terms.forEach((term, m) => {
        mapped +=
          term.lommelSeeliger[c] * (lommelSeeligerRgb[3 * m + c] ?? 0) +
          term.lambert[c] * (lambertRgb[3 * m + c] ?? 0);
      });
    }
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
  /** The light it was found for ({@link classMapPointReflection}'s `lightKey`). */
  readonly lightKey: string;
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
  return chordSquared(a, b) <= angleRad * angleRad;
}

/** Holds a ratio found for `surface`, its oldest let go once it holds the most. */
function hold(surface: ClassMapDiscSurface, found: HeldRatio): void {
  HELD.set(surface, [...(HELD.get(surface) ?? []).slice(-(HELD_PER_SURFACE - 1)), found]);
}

/**
 * Holds a refreshed ratio for `surface` in place of its light's held one nearest it within
 * {@link classMapPointStaleAngleRad}, the one that stood in, and otherwise as {@link hold} does: so
 * that under time acceleration each light keeps its one entry and no light's refresh lets another's
 * only entry go (R10.T14.f).
 */
function holdRefreshed(surface: ClassMapDiscSurface, found: HeldRatio): void {
  const held = HELD.get(surface) ?? [];
  const alpha = Math.acos(
    Math.min(1, Math.max(-1, dot(found.towardsLight, found.towardsObserver))),
  );
  const replaced = nearestHeld(held, found, classMapPointStaleAngleRad(alpha));
  if (replaced === undefined) {
    hold(surface, found);
    return;
  }
  HELD.set(surface, [...held.filter((entry) => entry !== replaced), found]);
}

/**
 * The angle at phase 0 within which a held ratio stands in for a fresh one while the frames'
 * slices find it (R10.T14.f), rad, narrowing with the lit and visible lune as the cache's does.
 *
 * @remarks
 * A refresh at level 8 takes about three frames' slices a light, so four lights each lag their
 * geometry by about twelve frames under time acceleration: within this angle a spin of up to about
 * 0.02 rad a frame (1.2 rad/s on the screen) at phase 0 is followed by held ratios and the slices
 * alone. At the narrower 0.05 rad, misses came back to the frame at 0.002 rad a frame past 100° of
 * phase, for up to 18 ms a frame over four lights (2026-10-21, provisional). By the cache's
 * sensitivity, S (π − α) ≤ 5.2 on the synthetic Iapetus, a held ratio within it can be up to about
 * 5.2 × 0.25 ÷ π ≈ 40% from a fresh one at that body's contrast: the point's light curve then lags
 * the turn by those frames, about 0.2 s. Past it the miss is found at once, as before.
 */
export const CLASS_MAP_POINT_STALE_RAD = 0.25;

/** The angle within which a held ratio stands in at phase α while a fresh one is found, rad. */
export function classMapPointStaleAngleRad(alphaRad: number): number {
  return (CLASS_MAP_POINT_STALE_RAD * Math.max(0, Math.PI - alphaRad)) / Math.PI;
}

/** A light at a geometry: the key and directions a held ratio, a refresh and a miss share. */
interface LightGeometry {
  readonly lightKey: string;
  readonly towardsLight: Vec3;
  readonly towardsObserver: Vec3;
  readonly cOverA: number;
}

/** A ratio being found again in the frames' slices, at the geometry it was missed at. */
interface QueuedRefresh extends LightGeometry {
  /** The surface whose ratio it finds: the map's own, or its stand-in's. */
  readonly surface: ClassMapDiscSurface;
  /** The map whose point it is found for, with which it is cancelled. */
  readonly forTexels: ClassMapTexels;
  readonly steps: Generator<void, Rgb>;
}

/** The refreshes queued, the oldest first. */
const REFRESHES: QueuedRefresh[] = [];

/** Whether `entry` is of `at`'s light and within `angleRad` of it on both directions. */
function nearAs(entry: LightGeometry, at: LightGeometry, angleRad: number): boolean {
  return (
    entry.lightKey === at.lightKey &&
    entry.cOverA === at.cOverA &&
    within(entry.towardsLight, at.towardsLight, angleRad) &&
    within(entry.towardsObserver, at.towardsObserver, angleRad)
  );
}

/** The held entry of `at`'s light nearest it of those within `angleRad`, if any. */
function nearestHeld(
  held: ReadonlyArray<HeldRatio>,
  at: LightGeometry,
  angleRad: number,
): HeldRatio | undefined {
  let nearest: HeldRatio | undefined;
  let nearestChord = Number.POSITIVE_INFINITY;
  for (const entry of held) {
    if (!nearAs(entry, at, angleRad)) {
      continue;
    }
    const chord = Math.max(
      chordSquared(entry.towardsLight, at.towardsLight),
      chordSquared(entry.towardsObserver, at.towardsObserver),
    );
    if (chord < nearestChord) {
      nearest = entry;
      nearestChord = chord;
    }
  }
  return nearest;
}

/** The squared chord between two unit directions. */
function chordSquared(a: Vec3, b: Vec3): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return dx * dx + dy * dy + dz * dz;
}

/** The surface a point is drawn by, with its made elements, for the map `forTexels`. */
interface DrawnPoint {
  readonly surface: ClassMapDiscSurface;
  readonly elements: PointElements;
  readonly forTexels: ClassMapTexels;
}

/**
 * The ratio at a geometry by a surface's made elements (R10.T14.f).
 *
 * @remarks
 * One held within the cache's angle, of any light; past it, the light's own nearest held within
 * {@link classMapPointStaleAngleRad}, the surface's or else the provisional ones of the map it is
 * drawn for ({@link PROVISIONAL}), while the frames' slices find the fresh one
 * ({@link stepClassMapPoints}), queued once a light; and where none is held so near, the light's
 * first, found at once and held.
 */
function heldRatio(
  figure: BodyFigure,
  { surface, elements, forTexels }: DrawnPoint,
  at: LightGeometry,
): Rgb {
  const { towardsLight, towardsObserver, cOverA } = at;
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
  const staleAngle = classMapPointStaleAngleRad(alpha);
  const near =
    nearestHeld(held, at, staleAngle) ??
    nearestHeld(PROVISIONAL.get(forTexels) ?? [], at, staleAngle);
  if (near !== undefined) {
    if (
      !REFRESHES.some((refresh) => refresh.surface === surface && nearAs(refresh, at, staleAngle))
    ) {
      REFRESHES.push({
        ...at,
        surface,
        forTexels,
        steps: ratioSteps(figure, surface, elements, towardsLight, towardsObserver),
      });
    }
    return near.ratio;
  }
  const ratio = ratioOf(figure, surface, elements, towardsLight, towardsObserver);
  hold(surface, { ...at, ratio });
  return ratio;
}

/** The uniform point's ratio. */
const UNIFORM_RATIO: Rgb = [1, 1, 1];

/**
 * Notes that a map's point was drawn as the uniform point for a light at a geometry (R10.T14.f),
 * as a provisional ratio of 1, unless one is noted within δ of it already.
 */
function noteUniform(texels: ClassMapTexels, at: LightGeometry): void {
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(at.towardsLight, at.towardsObserver))));
  const noted = PROVISIONAL.get(texels) ?? [];
  if (noted.some((entry) => nearAs(entry, at, classMapPointCacheAngleRad(alpha)))) {
    return;
  }
  PROVISIONAL.set(texels, [
    ...noted.slice(-(HELD_PER_SURFACE - 1)),
    { ...at, ratio: UNIFORM_RATIO },
  ]);
}

/**
 * What a frame's point under `surface` is drawn by (R10.T14.f): the surface itself once its
 * elements are made; until then its stand-in where it has one whose are, its build queued if it is
 * not; `null` for the uniform point, noted as the map's provisional ratio.
 */
function drawnPointOf(surface: ClassMapDiscSurface, at: LightGeometry): DrawnPoint | null {
  const forTexels = surface.texels;
  const elements = surfaceElements(surface);
  if (elements !== null) {
    return { surface, elements, forTexels };
  }
  queueBuild(forTexels, surface.normals?.texels ?? null);
  const standIn = STAND_INS.get(forTexels);
  const standInElements = standIn === undefined ? null : surfaceElements(standIn);
  if (standIn === undefined || standInElements === null) {
    noteUniform(forTexels, at);
    return null;
  }
  return { surface: standIn, elements: standInElements, forTexels };
}

/**
 * The light a body under a class map reflects towards a distant observer, per channel, over the
 * illuminance on it and (a ÷ Δ)², as `bodyReflection` gives a uniform body's: the photometry's
 * point of `elsewhere` on the figure drawn about the rotation's pole, times the map's ratio to it
 * ({@link classMapReflectionRatio}), cached by the body-fixed directions within
 * {@link classMapPointCacheAngleRad}. Uneclipsed: the caller cuts each star's light.
 *
 * @remarks
 * The frame's path: it never makes the map's elements (R10.T14.f). Until they are made, by the
 * frames' slices ({@link stepClassMapPoints}), to which it queues them if the map's owner did
 * not, the ratio is its stand-in's, the map it replaced ({@link queueClassMapPoint}), and where it
 * has none the point is the uniform point of `elsewhere`, bit for bit what the body drew before
 * its first map. Once they are made, what it was drawn with stands in, past δ, until its own ratio
 * is found in the slices.
 *
 * @param rotation - The body's rotation, body-fixed to galactic axes.
 * @param towardsLight - The unit direction from the body to the light, along the galactic axes.
 * @param towardsObserver - The unit direction from the body to the observer, along the galactic
 *   axes.
 * @param lightKey - The light's identity among the body's lights, a star's or a planetshine
 *   source's (`pointFlux`'s): past δ only its own held ratios stand in for it. One key for every
 *   light, the default, treats them as one.
 * @throws Error if the surface's laws are not one per class of its map, or, under its map's
 *   maturity, a class drawn from its nodes has an A other than its node K − 1.
 */
export function classMapPointReflection(
  figure: BodyFigure,
  surface: ClassMapDiscSurface,
  rotation: Rotation3,
  towardsLight: Vec3,
  towardsObserver: Vec3,
  lightKey = "",
): Rgb {
  assertLawPerClass(surface);
  // The uniform point in the galactic axes, about the rotation's pole, as `pointFlux` takes a
  // uniform body's about its figure's: the same call, bit for bit, where the two poles agree.
  const uniform = bodyReflection(
    { ...figure, pole: rotateToBody(rotation, BODY_FIXED_POLE) },
    surface.elsewhere,
    towardsLight,
    towardsObserver,
  );
  const at: LightGeometry = {
    lightKey,
    towardsLight: rotateToBodyFixed(rotation, towardsLight),
    towardsObserver: rotateToBodyFixed(rotation, towardsObserver),
    cOverA: figure.polarRadiusM / figure.equatorialRadiusM,
  };
  const drawn = drawnPointOf(surface, at);
  if (drawn === null) {
    return uniform;
  }
  const ratio = heldRatio(figure, drawn, at);
  return [uniform[0] * ratio[0], uniform[1] * ratio[1], uniform[2] * ratio[2]];
}

/** Whether a refresh failed already, said once. */
const SAID = { refresh: false };

/**
 * The maps whose elements the last slice made: their provisional ratios are let go at the next
 * slice unless a frame between drew their points (R10.T14.f), so that a map made while its body was
 * a disc meets the disc at its first frame as a point, and not what it was drawn with before.
 */
let JUST_MADE: ClassMapTexels[] = [];

/**
 * One step of the oldest build, or its end: its elements held, its stand-in let go (its held
 * ratios kept as the map's provisional ones) and the stand-in's refreshes dropped; or its drop as
 * it throws, said once a map, its point left to its stand-in.
 */
function stepBuild(build: QueuedBuild): void {
  let done: boolean;
  try {
    done = build.steps.next().done === true;
  } catch (error: unknown) {
    QUEUE.shift();
    FAILED.add(build.texels);
    console.error("the elements of a class-mapped point could not be made:", error);
    return;
  }
  if (!done) {
    return;
  }
  QUEUE.shift();
  const standIn = STAND_INS.get(build.texels);
  if (standIn !== undefined) {
    STAND_INS.delete(build.texels);
    const kept = [...(PROVISIONAL.get(build.texels) ?? []), ...(HELD.get(standIn) ?? [])];
    if (kept.length > 0) {
      PROVISIONAL.set(build.texels, kept.slice(-HELD_PER_SURFACE));
    }
  }
  dropRefreshes(
    (refresh) => refresh.forTexels === build.texels && refresh.surface.texels !== build.texels,
  );
  JUST_MADE.push(build.texels);
}

/** Lets a map's provisional ratios go once none of its own refreshes remains queued. */
function settleProvisional(texels: ClassMapTexels): void {
  if (!REFRESHES.some((refresh) => refresh.forTexels === texels)) {
    PROVISIONAL.delete(texels);
  }
}

/**
 * One step of the oldest refresh: its ratio held as it ends, and its map's provisional ratios let go
 * once none of its own refreshes remains queued; or its drop as it throws, said once.
 */
function stepRefresh(refresh: QueuedRefresh): void {
  let step: IteratorResult<void, Rgb>;
  try {
    step = refresh.steps.next();
  } catch (error: unknown) {
    REFRESHES.shift();
    settleProvisional(refresh.forTexels);
    if (!SAID.refresh) {
      SAID.refresh = true;
      console.error("the light of a class-mapped point could not be found again:", error);
    }
    return;
  }
  if (step.done === true) {
    REFRESHES.shift();
    const { surface, forTexels, lightKey, towardsLight, towardsObserver, cOverA } = refresh;
    holdRefreshed(surface, {
      lightKey,
      towardsLight,
      towardsObserver,
      cOverA,
      ratio: step.value,
    });
    if (surface.texels === forTexels) {
      settleProvisional(forTexels);
    }
  }
}

/**
 * A frame's slice of the class-mapped points' work (R10.T14.f), once a frame from the terrain
 * session's step: at most `budgetMs`, and a step past it.
 *
 * @remarks
 * The oldest build's and the oldest refresh's steps in turn (the one that has work where the other
 * has none), until `budgetMs` has passed since the slice began, read on `now` before each step. A
 * finished build's elements are held for its map and its stand-in let go; a finished refresh's
 * ratio is held for its surface. A step that throws drops its work. First, the provisional ratios
 * of the maps the last slice made are let go where no frame since drew their points.
 *
 * @param budgetMs - The slice's length, ms.
 * @param now - A monotonic clock, ms.
 * @returns Whether work remains queued.
 */
export function stepClassMapPoints(
  budgetMs: number = CLASS_MAP_POINT_SLICE_MS,
  now: () => number = () => globalThis.performance.now(),
): boolean {
  for (const texels of JUST_MADE) {
    settleProvisional(texels);
  }
  JUST_MADE = [];
  if (QUEUE.length === 0 && REFRESHES.length === 0) {
    return false;
  }
  const startMs = now();
  let refreshTurn = true;
  for (;;) {
    const build = QUEUE[0];
    const refresh = REFRESHES[0];
    if (build === undefined && refresh === undefined) {
      return false;
    }
    if (!(now() - startMs < budgetMs)) {
      return true;
    }
    if (refresh !== undefined && (refreshTurn || build === undefined)) {
      stepRefresh(refresh);
    } else if (build !== undefined) {
      stepBuild(build);
    }
    refreshTurn = !refreshTurn;
  }
}
