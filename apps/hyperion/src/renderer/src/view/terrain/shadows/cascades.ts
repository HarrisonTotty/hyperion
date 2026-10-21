/**
 * The high setting's cascaded shadow maps as geometry and data (plan R10, R10.T9.a; Design note 11;
 * decision-r10-t9-split): the four cascades' splits, bounding spheres, texel snapping and light
 * matrices, the caster reach, the cascades' weight against the horizon map, the depth bias and
 * normal offset, and the layers' texture spec.
 *
 * @remarks
 * It draws nothing and creates nothing on a GPU, and nothing calls it yet: R10.T9.b wires it into
 * the terrain pass (the layers' creation under {@link CASCADE_MEMORY_CATEGORY}, a caster pass for
 * each cascade, and the receiver beside R11's `terrain_shadow_factor` in `terrainLit.wgsl`). T9.b
 * calls {@link fitCascades} once a frame for each photorealistic view that hands a body over on
 * the high setting; the low setting has no cascades.
 *
 * - **The combination** (Design note 11, decision-r10-t8b §2; T9.b's): the horizon map shadows the
 *   direct sun everywhere, and where a cascade exists its lit fraction c enters as
 *   min(1, c′ ÷ m), 1 where m = 0, m the map's visible share and c′ = 1 − w (1 − c) with w the
 *   {@link cascadeWeight}. Each cascade blends into the next over its last
 *   {@link CASCADE_FADE_FRACTION}, and the last fades to lit, so there is no seam at its end.
 * - **The splits** are the practical split scheme of parallel-split shadow maps in Zhang, Sun and
 *   Nyman 2007's form (GPU Gems 3, ch. 10, §10.2.1), λ weighting the logarithmic split, over the
 *   shadowed range {@link shadowedRangeM}, continuous in the height above the ground. Zhang, Sun,
 *   Xu and Lun 2006 (VRCIA, §3.3, eq. 6–7) take the even average of the two splits plus a bias.
 * - **The fit** is a bounding sphere of each slice of the view's frustum ({@link sliceSphere}),
 *   whose radius does not change as the camera turns, its projection snapped to whole texels
 *   (Valient, "Deferred Rendering in Killzone 2", Develop 2007, slide 37: "Make bounding sphere of
 *   cascade frustum … round light matrix to nearest shadow pixel corner"; his ShaderX6 chapter of
 *   2008 was not reached). Microsoft's "Common Techniques to Improve Shadow Depth Maps" snaps the
 *   same way ("Moving the Light in Texel-Sized Increments"), but fits to the slice's diagonal, not
 *   a sphere.
 * - **The fitted sun** is the top edge of the star's disc, not its centre
 *   (decision-r10-t9b-penumbra): the pass gives {@link fitCascades} s★ raised by the star's
 *   angular radius ρ towards the zenith ({@link discTopDirection}). So the cascades are fitted to
 *   the disc's top edge, their shadow begins where the horizon map's share m reaches 0, and
 *   min(m, c′) is m across the penumbra; non-terrain casters' shadows are short by about half a
 *   penumbra, ρL (4.7 cm at 10 m). The receiver's N·L and normal offset keep the true star.
 * - **The light's axes** ({@link lightBasis}) are built from the fitted body-fixed sun (the split
 *   ruling's S4), and each sphere's centre is snapped in them to whole texels, body-fixed in `f64`;
 *   everything is then expressed camera-relative.
 * - **The grid is carried** (decision-r10-t9-split, ruling 7 amended, replacing S3; issue #52).
 *   Each cascade's grid is carried from its own snapped centre P of the frame before: with c its
 *   unsnapped centre now, a = (c − P) · x_L ÷ texel, b = (c − P) · y_L ÷ texel and
 *   d = (c − P) · z_L, the snapped centre is P + texel (round(a) x_L + round(b) y_L) + d z_L, in
 *   this frame's axes and texel. The step is whole texels in the light's plane and free along its
 *   depth, so a carried grid never re-grids, however the camera moves. The sun's turn rotates it
 *   about P, inside the cascade: a receiver slides by at most about δ r a frame, δ the sun's
 *   turn a frame in body-fixed axes, about 1,024 δ texel for every cascade (0.074 at
 *   `TERRAIN TEST`'s 60 s/s and 60 Hz, 0.0012 on an Earth at 1 s/s); a texel's change scales the
 *   grid about P. With no frame before, all four are snapped from one body-fixed lattice point O,
 *   the camera's place rounded to a lattice of {@link gridLatticeSpacingM}: stateless, it re-grids
 *   once at each lattice face the camera crosses, and at each step of L. A grid measured from the
 *   body's centre, the brief's first S3, would slide by up to R δ a frame (126 m, 1,400 of cascade
 *   1's texels, on `TERRAIN TEST`), since a light basis built from the body-fixed sun and pole
 *   turns with the body about its pole and is the non-rotating basis seen body-fixed.
 * - **Precision.** Every light matrix is formed camera-relative in `f64` and narrowed to `f32`
 *   once, since a snapped grid at planetary coordinates in `f32` would jitter: an `f32`
 *   body-centred position places a point only to tenths of a metre on an Earth, while cascade 1's
 *   texel at 2 m up is 8.8 cm.
 * - **The receiver** takes the camera-relative position in the body's non-rotating axes, as the
 *   terrain's vertices are drawn (`terrainPatch.wgsl`: `draw.offsetFromCameraM + inst.originM +
 *   bodyRotation × q`), through each cascade's {@link Cascade.lightFromCamera}; the caster pass
 *   takes {@link Cascade.viewRotation} and {@link Cascade.projection} as its frame's.
 * - **Filtering** is T9.b's 2 × 2 hardware PCF through `textureSampleCompare` with a few rotated
 *   taps. WebGPU leaves a comparison sampler's filtering to the implementation (§7.1.1: "the
 *   sampling results will be implementation-dependent"), so the hardware's 2 × 2 is not
 *   guaranteed. The Sun's penumbra needs no more (no PCSS) for casters near the receiver: the Sun
 *   subtends 2 arcsin(R☉ ÷ au) = 2 arcsin(6.957 × 10⁸ ÷ 1.495 978 707 × 10¹¹) = 0.533° (IAU 2015
 *   B3's nominal radius, IAU 2012 B2's au), so 10 m behind an occluder its penumbra is
 *   10 m × 9.30 × 10⁻³ rad = 9.3 cm, about one of cascade 1's texels at 2 m up. At the same
 *   insolation a host's disc scales as (T☉ ÷ T★)², T☉ = 5,772 K (IAU 2015 B3), so a 3,000 K star
 *   subtends about 2.0° and its penumbra there is about 34 cm, four texels. The filter does not
 *   follow ρ (decision-r10-t9b-penumbra): beyond some 38 m the penumbra is the horizon map's, which
 *   the top-edge fit leaves whole.
 */

import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../../geometry/vec3";
import type { Quaternion } from "../../camera/pose";
import { NEAR_PLANE_M } from "../../camera/projection";
import { rotationRows } from "../../camera/quaternion";
import { rotateToBody, type Rotation3 } from "../../coords/rotation";
import { TEXTURE_USAGE } from "../../engine/gpuFlags";
import type { TextureSpec } from "../../engine/memory";

/**
 * The number of cascades, N: 4 (Design note 11; the brainstorm's four layers). GPU Gems 3's tests
 * used four splits (§10.3.2) and Valient's Killzone 2 at most four cascades.
 */
export const CASCADE_COUNT = 4;

/** Each cascade's side, px: 2,048 (Design note 11; the brainstorm's 64 MB of four layers). */
export const CASCADE_SIZE_PX = 2048;

/**
 * The split weight, λ: 0.75 on the logarithmic split and 0.25 on the uniform, the split ruling's
 * value (S2; Design note 11's "λ ≈ 0.75").
 *
 * @remarks
 * The form, with λ on the logarithmic term, is GPU Gems 3's (§10.2.1, 0 < λ < 1). The value is
 * not a source's: GPU Gems 3's default is ½ ("λ = 0.5 is the default setting in our current
 * implementation"), and Zhang et al. 2006 take the even average. Near a 0.1 m near plane the
 * logarithmic split alone puts C₁ at 1.19 m of a 2 km range, ½ at 250.6 m and 0.75 at 125.9 m.
 */
export const CASCADE_SPLIT_LAMBDA = 0.75;

/** The least shadowed range, m: 2 km, at any height up to 100 m (Design note 11). */
export const MIN_SHADOWED_RANGE_M = 2000;

/** The shadowed range per metre of height above the ground: 20 (Design note 11). */
export const SHADOWED_RANGE_PER_HEIGHT = 20;

/**
 * The share of each cascade's split range over which it blends into the next, and the last into
 * lit: its last 10% (Design note 11).
 */
export const CASCADE_FADE_FRACTION = 0.1;

/**
 * The normal offset, in texels of each cascade's world size: 1.5, tuned by hand in R10.T9.b within
 * Design note 11's 1–1.5 (the split ruling's S7), its upper end.
 *
 * @remarks
 * Holbert's normal offset ("Saying 'Goodbye' to Shadow Acne", GDC 2011 poster): the receiver's
 * world position moves along its geometric normal (not a normal-mapped one) before it is
 * projected, by the offset × saturate(1 − N·L), with "a small, constant depth bias" beside it; his
 * demo's offset is 1.0 texel. T9.b applies the angular factor per fragment.
 *
 * The tuning (R10.T9.b, the RTX 3080, hidden, `TERRAIN TEST` at 232 s, looking north): the share
 * of the terrain's pixels the cascades darken by more than 2% of full scale fell from 1.4% at 1
 * texel to 0.6% at 1.5 (and to 0.1% with the fit to the disc's top edge), the acne's moiré and a
 * step at a level boundary that 1 texel showed gone at 1.5. A slope scale of 2 changed the share
 * by under 0.2 points, so {@link CASCADE_DEPTH_BIAS} stays { 1, 1, 0 }.
 */
export const NORMAL_OFFSET_TEXELS = 1.5;

/**
 * The ratio of cascade 1's texel to the horizon map's at the nadir at and below which the cascades
 * are wholly on: ½ (the split ruling's S5).
 */
export const CASCADE_WEIGHT_FULL = 0.5;

/**
 * The ratio of cascade 1's texel to the horizon map's at and above which the cascades are off: 1,
 * where cascade 1 is coarser than the map, as in orbit (Design note 11; the split ruling's S5).
 */
export const CASCADE_WEIGHT_OFF = 1;

/**
 * The angle from either pole within which the body-fixed +x stands in for the pole in
 * {@link lightBasis}, rad: 1° (the split ruling's S4).
 */
export const POLE_FALLBACK_RAD = Math.PI / 180;

/**
 * The four cascades' layers, without their category (the split ruling's S1): one
 * `texture_depth_2d_array` of 2,048² `depth32float` layers, drawn as depth attachments and sampled
 * by comparison.
 *
 * @remarks
 * T9.b adds `shadows` to R01's `MemoryCategory` and creates them with
 * `{ ...CASCADE_TEXTURE, category: CASCADE_MEMORY_CATEGORY }`; a path under `view/engine/` would
 * send this task's gate to the Electron smoke run.
 */
export const CASCADE_TEXTURE: Omit<TextureSpec, "category"> = {
  name: "terrain shadow cascades",
  size: { width: CASCADE_SIZE_PX, height: CASCADE_SIZE_PX, depthOrArrayLayers: CASCADE_COUNT },
  dimension: "2d",
  format: "depth32float",
  mips: 1,
  usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
};

/**
 * The `MemoryCategory` the layers are filed under: `shadows`, R12's name (plan 12's `MemoryItem`),
 * which T9.b adds to R01's union.
 */
export const CASCADE_MEMORY_CATEGORY = "shadows" as const;

/**
 * The layers' bytes: 2,048 × 2,048 × 4 layers × 4 B (`depth32float`, WebGPU's texel-format
 * table) = 67,108,864 B, 64 MiB, the brainstorm's figure.
 */
export const CASCADE_BYTES = 67_108_864;

/**
 * A depth bias, positive away from the light, in `depthBiasAway`'s sign (R01's
 * `WgslMaterialSpec.depthBiasAway`, "Positive is away from the camera, whatever the depth
 * direction").
 */
export interface CascadeDepthBias {
  /** WebGPU's `depthBias`, an integer count of the format's minimum resolvable difference. */
  readonly constant: number;
  /** WebGPU's `depthBiasSlopeScale`, a factor on the primitive's largest depth slope. */
  readonly slopeScale: number;
  /** WebGPU's `depthBiasClamp`, the largest bias, in depth units (the map's [0, 1]); 0 for none. */
  readonly clamp: number;
}

/**
 * The casters' slope-scaled depth bias, as data (Design note 11; the split ruling's S7), kept by
 * R10.T9.b's tuning ({@link NORMAL_OFFSET_TEXELS}).
 *
 * @remarks
 * The values: `constant` 1, the least non-zero integer, for Holbert's "small, constant depth bias"
 * beside the normal offset ({@link NORMAL_OFFSET_TEXELS}); `slopeScale` 1 and `clamp` 0 (none), the
 * rasterizer state of Microsoft's CascadedShadowMaps11 sample, whose articles give no figures.
 *
 * WebGPU's rasterizer biases a triangle's fragments (§10.3.6, "biased fragment depth") by
 * bias = `depthBias` × 2^(e − 23) + `depthBiasSlopeScale` × m for a float format, m the
 * triangle's largest depth slope and e the exponent of its largest depth; where `depthBiasClamp`
 * is positive the bias is at most it, and where negative at least it. `depthBias` is an integer
 * (`GPUDepthBias`, a `long`). The specification's float wording reuses its unorm r; the 23, the
 * mantissa bits of a 32-bit float, is Direct3D 11's ("Depth Bias"), which it follows.
 *
 * So one constant unit is 2^(e − 23) of the depth range D, at a caster's depth d in [2^e, 2^(e+1)):
 * 2⁻²⁴ of D for d in [½, 1), the half nearer the sun, and half that for each halving below. As d D
 * is the caster's distance from the far plane, the unit's world size is 2⁻²⁴ to 2⁻²³ of that
 * distance, whatever D is: the far plane stands at r from the centre on both capability paths
 * (D = 2r with `depth-clip-control`, 2r plus the caster reach without it,
 * {@link Cascade.depthNearM}), so for a caster in the sphere one constant is the same to within a
 * factor of 2 on both. The slope term's world size does not depend on D either. T9.b maps the
 * values as `depthBiasOf` maps `depthBiasAway`, each negated for the reversed depth, the clamp too.
 */
export const CASCADE_DEPTH_BIAS: CascadeDepthBias = {
  constant: 1,
  slopeScale: 1,
  clamp: 0,
};

/** Throws a {@link RangeError} naming `name` unless `value` is finite. */
function requireFinite(name: string, value: number): void {
  if (!Number.isFinite(value)) {
    throw new RangeError(`cascades need a finite ${name}, not ${String(value)}`);
  }
}

/** Throws a {@link RangeError} naming `name` unless every component of `v` is finite. */
function requireFiniteVec(name: string, v: Vec3): void {
  requireFinite(`${name}.x`, v.x);
  requireFinite(`${name}.y`, v.y);
  requireFinite(`${name}.z`, v.z);
}

/**
 * The shadowed range, f, m: min(far, max(2 km, 20 h)), continuous in h (Design note 11; the split
 * ruling's S2).
 *
 * @param heightAboveGroundM - h, the camera's height above the ground at its nadir, m; below 0
 * (under the drawn ground, by morphing) it is taken as 0.
 * @param farM - The view's far plane, m; ∞ by default, since the view's projection is
 * infinite-far (`perspectiveReversedInfinite`).
 * @throws RangeError for a height that is not finite or a far that is not positive.
 */
export function shadowedRangeM(heightAboveGroundM: number, farM = Infinity): number {
  requireFinite("height above the ground", heightAboveGroundM);
  if (!(farM > 0)) {
    throw new RangeError(`cascades need a positive far plane, not ${String(farM)}`);
  }
  const heightM = Math.max(heightAboveGroundM, 0);
  return Math.min(farM, Math.max(MIN_SHADOWED_RANGE_M, SHADOWED_RANGE_PER_HEIGHT * heightM));
}

/**
 * The split distances along the view axis, m: C₀ … C₄, cascade i covering [Cᵢ, Cᵢ₊₁], by the
 * practical split scheme in GPU Gems 3's form (Zhang, Sun and Nyman 2007, §10.2.1; Design note 11),
 * Cᵢ = λ n (f ÷ n)^(i ÷ N) + (1 − λ)(n + (f − n) i ÷ N), λ the split ruling's 0.75 (S2), with
 * C₀ = n and C₄ = f exactly.
 *
 * @param nearM - n, the view's near plane, m, positive.
 * @param rangeM - f, the shadowed range, m, finite and beyond n.
 * @throws RangeError for a near plane that is not positive and finite, or a range not beyond it.
 */
export function cascadeSplitsM(nearM: number, rangeM: number): Float64Array {
  if (!(Number.isFinite(nearM) && nearM > 0 && Number.isFinite(rangeM) && rangeM > nearM)) {
    const given = `${String(nearM)} and ${String(rangeM)}`;
    throw new RangeError(
      `cascades need a positive near plane and a finite range beyond it: ${given}`,
    );
  }
  const splits = new Float64Array(CASCADE_COUNT + 1);
  splits[0] = nearM;
  for (let i = 1; i < CASCADE_COUNT; i += 1) {
    const share = i / CASCADE_COUNT;
    const logarithmicM = nearM * (rangeM / nearM) ** share;
    const uniformM = nearM + (rangeM - nearM) * share;
    splits[i] = CASCADE_SPLIT_LAMBDA * logarithmicM + (1 - CASCADE_SPLIT_LAMBDA) * uniformM;
  }
  splits[CASCADE_COUNT] = rangeM;
  return splits;
}

/**
 * t², the squared tangent of the view's half-diagonal: tan²(φx ÷ 2)(1 + 1 ÷ a²), a frustum corner
 * standing t d off the axis at a distance d along it.
 *
 * @param fovXRad - φx, the horizontal field of view, rad, in (0, π).
 * @param aspect - a, width ÷ height, positive.
 * @throws RangeError for a field of view outside (0, π) or an aspect that is not positive.
 */
export function halfDiagonalTanSquared(fovXRad: number, aspect: number): number {
  if (!(fovXRad > 0 && fovXRad < Math.PI)) {
    throw new RangeError(`cascades need a field of view in (0, π), not ${String(fovXRad)}`);
  }
  if (!(aspect > 0 && Number.isFinite(aspect))) {
    throw new RangeError(`cascades need a positive aspect, not ${String(aspect)}`);
  }
  const tanHalfX = Math.tan(fovXRad / 2);
  return tanHalfX * tanHalfX * (1 + 1 / (aspect * aspect));
}

/** A frustum slice's bounding sphere: its centre on the view axis and its radius. */
export interface SliceSphere {
  /** The centre's distance from the camera along the view axis, m. */
  readonly distanceM: number;
  /** The radius, m. */
  readonly radiusM: number;
}

/**
 * The smallest sphere holding a slice of the view's frustum, centred on its axis (Design note 11):
 * z = (n′ + f′)(1 + t²) ÷ 2 and r = √((f′ − z)² + f′² t²), equidistant from the slice's eight
 * corners; or, where z > f′, the far cap's sphere, z = f′ and r = f′ t, which then holds the near
 * corners too.
 *
 * @remarks
 * It depends only on the slice's ends and the view's shape, so it does not change as the camera
 * turns (Valient's bounding sphere of the cascade's frustum, Develop 2007). It is the least such
 * sphere on the axis, tighter than one about the corners' centroid.
 *
 * @param nearM - n′, the slice's near end along the view axis, m, 0 or more.
 * @param farM - f′, its far end, m, beyond n′ and finite.
 * @param tanHalfDiagonalSquared - t², from {@link halfDiagonalTanSquared}, positive.
 * @throws RangeError for ends out of order or not finite, or a t² that is not positive.
 */
export function sliceSphere(
  nearM: number,
  farM: number,
  tanHalfDiagonalSquared: number,
): SliceSphere {
  if (!(nearM >= 0 && farM > nearM && Number.isFinite(farM))) {
    throw new RangeError(`a slice needs 0 ≤ n′ < f′ < ∞, not ${String(nearM)} and ${String(farM)}`);
  }
  if (!(tanHalfDiagonalSquared > 0 && Number.isFinite(tanHalfDiagonalSquared))) {
    throw new RangeError(`a slice needs a positive t², not ${String(tanHalfDiagonalSquared)}`);
  }
  const distanceM = ((nearM + farM) * (1 + tanHalfDiagonalSquared)) / 2;
  if (distanceM > farM) {
    return { distanceM: farM, radiusM: farM * Math.sqrt(tanHalfDiagonalSquared) };
  }
  const behindM = farM - distanceM;
  return {
    distanceM,
    radiusM: Math.sqrt(behindM * behindM + farM * farM * tanHalfDiagonalSquared),
  };
}

/** The light's axes, unit and right-handed (x × y = z), in the body-fixed axes. */
export interface LightBasis {
  /** x_L: the body's pole projected normal to the sun, or the body-fixed +x near a pole. */
  readonly x: Vec3;
  /** y_L = z_L × x_L. */
  readonly y: Vec3;
  /** z_L, the fitted sun ({@link discTopDirection}); the light looks down −z_L. */
  readonly z: Vec3;
}

/** The body's pole, body-fixed +z. */
const BODY_POLE: Vec3 = { x: 0, y: 0, z: 1 };

/** The body-fixed +x, the pole's stand-in within {@link POLE_FALLBACK_RAD} of either pole. */
const BODY_X: Vec3 = { x: 1, y: 0, z: 0 };

/**
 * The light's axes for a body-fixed sun (the split ruling's S4): z_L = s★; x_L the body's pole
 * projected normal to s★ and normalised; y_L = z_L × x_L.
 *
 * @remarks
 * Where s★ is within {@link POLE_FALLBACK_RAD} of either pole, the body-fixed +x stands in for the
 * pole. It is stateless: crossing that angle re-grids the cascades for one frame, which changes the
 * shadows by sub-texel amounts, not their shapes.
 *
 * @param sunBodyFixed - The direction to the sun, body-fixed; any length.
 * @throws RangeError for a sun of zero length or with a component that is not finite.
 */
export function lightBasis(sunBodyFixed: Vec3): LightBasis {
  requireFiniteVec("sun direction", sunBodyFixed);
  const z = normalise(sunBodyFixed);
  const fromPoleRad = Math.atan2(Math.hypot(z.x, z.y), Math.abs(z.z));
  const reference = fromPoleRad < POLE_FALLBACK_RAD ? BODY_X : BODY_POLE;
  const x = normalise(sub(reference, scale(z, dot(reference, z))));
  return { x, y: cross(z, x), z };
}

/**
 * e★, the sun's elevation above the datum at the nadir, rad: π ÷ 2 − angle(s★, ν), in
 * [−π ÷ 2, π ÷ 2], by atan2 of the dot and cross products so that it keeps its precision near the
 * horizon.
 *
 * @param sunBodyFixed - s★, the direction to the sun, body-fixed; any length.
 * @param datumNormalBodyFixed - ν, the datum's normal at the camera's nadir, body-fixed; any
 * length.
 * @throws RangeError for a direction of zero length or with a component that is not finite.
 */
export function sunElevationRad(sunBodyFixed: Vec3, datumNormalBodyFixed: Vec3): number {
  requireFiniteVec("sun direction", sunBodyFixed);
  requireFiniteVec("datum normal", datumNormalBodyFixed);
  const sun = normalise(sunBodyFixed);
  const normal = normalise(datumNormalBodyFixed);
  return Math.atan2(dot(sun, normal), norm(cross(sun, normal)));
}

/**
 * Below this |u|, the sun's distance from the datum's normal in {@link discTopDirection}, the sun
 * stands at the zenith or the nadir, where its vertical plane is undefined.
 */
const DISC_TOP_DEGENERATE = 1e-9;

/**
 * The direction the cascades are fitted to: the top edge of the star's disc, s★ raised by its
 * angular radius ρ towards the zenith in the sun's vertical plane (decision-r10-t9b-penumbra).
 *
 * @remarks
 * With s the unit sun and ν the datum's unit normal at the camera's nadir, u = ν − (ν·s) s and
 * s′ = cos ρ s + sin ρ u ÷ |u|, a unit vector at angle ρ from s on ν's side of it, in the plane of
 * the two; s′ = s where |u| < 10⁻⁹, the sun at the zenith or the nadir. Its elevation is e★ + ρ in
 * s★'s azimuth.
 *
 * Why: the horizon map cuts the disc from below, and its visible share m reaches 0 where the
 * skyline passes the disc's top edge (R07's cut, decision-r10-t8b §2). Fitted to that edge, the
 * cascades' shadow begins where m reaches 0, so that the direct sun's min(m, c′) is m across the
 * whole penumbra, the cascades darkening only where the horizon map's m has reached 0, for every
 * caster the map resolves. Fitted to the centre, they cut at m = ½, a step across the penumbra.
 * Non-terrain casters' shadows are short by about half a penumbra, ρL (4.7 cm at 10 m). The
 * receiver's N·L, its normal offset and the shading keep the true star, s★.
 *
 * @param sunBodyFixed - s★, the direction to the star's centre, body-fixed; any length.
 * @param datumNormalBodyFixed - ν, the datum's normal at the camera's nadir, body-fixed; any
 * length.
 * @param angularRadiusRad - ρ, the star's angular radius, rad, in [0, π ÷ 2]:
 * asin(min(1, radius ÷ distance)), R07's; 0 gives s★.
 * @throws RangeError for a direction of zero length or with a component that is not finite, or a
 * ρ outside [0, π ÷ 2].
 */
export function discTopDirection(
  sunBodyFixed: Vec3,
  datumNormalBodyFixed: Vec3,
  angularRadiusRad: number,
): Vec3 {
  requireFiniteVec("sun direction", sunBodyFixed);
  requireFiniteVec("datum normal", datumNormalBodyFixed);
  if (!(angularRadiusRad >= 0 && angularRadiusRad <= Math.PI / 2)) {
    throw new RangeError(
      `a star's angular radius lies in [0, π/2], not ${String(angularRadiusRad)}`,
    );
  }
  const sun = normalise(sunBodyFixed);
  const normal = normalise(datumNormalBodyFixed);
  const up = sub(normal, scale(sun, dot(normal, sun)));
  const upLength = norm(up);
  if (!(upLength >= DISC_TOP_DEGENERATE)) {
    return sun;
  }
  return add(
    scale(sun, Math.cos(angularRadiusRad)),
    scale(up, Math.sin(angularRadiusRad) / upLength),
  );
}

/**
 * The caster reach, m: how far towards the sun a caster can stand and still shadow the cascades,
 * min(Δh ÷ sin e★, √(2 R Δh)) (the split ruling's S6).
 *
 * @remarks
 * The cap is Design note 10's march limit: an obstacle Δh high at a distance d along the ground
 * stands d² ÷ 2R below the receiver's tangent plane by curvature, so beyond √(2 R Δh) it is below
 * the horizon (on `TERRAIN TEST`, R 1,737.4 km and Δh 14.9 km, √(2 × 1,737.4 km × 14.9 km) =
 * 227.5 km). At e★ ≤ 0, or where Δh ÷ sin e★ exceeds it, the reach is the cap. T9.b culls casters
 * by it on both capability paths; without `depth-clip-control` it is also how far the near plane
 * stands beyond the sphere ({@link Cascade.depthNearM}).
 *
 * @param elevationRad - e★, the sun's elevation at the nadir, rad, in [−π ÷ 2, π ÷ 2].
 * @param reliefRangeM - Δh, m, 0 or more: the body's realised relief range, or a caster's height
 * above the ground where that is larger.
 * @param bodyRadiusM - R, the body's mean radius, m, positive.
 * @throws RangeError for an elevation outside [−π ÷ 2, π ÷ 2], a negative or non-finite relief, or
 * a radius that is not positive and finite.
 */
export function casterReachM(
  elevationRad: number,
  reliefRangeM: number,
  bodyRadiusM: number,
): number {
  if (!(Math.abs(elevationRad) <= Math.PI / 2)) {
    throw new RangeError(`a sun's elevation lies in [−π/2, π/2], not ${String(elevationRad)}`);
  }
  if (!(reliefRangeM >= 0 && Number.isFinite(reliefRangeM))) {
    throw new RangeError(`cascades need a relief range of 0 or more, not ${String(reliefRangeM)}`);
  }
  if (!(bodyRadiusM > 0 && Number.isFinite(bodyRadiusM))) {
    throw new RangeError(`cascades need a positive body radius, not ${String(bodyRadiusM)}`);
  }
  const capM = Math.sqrt(2 * bodyRadiusM * reliefRangeM);
  const sine = Math.sin(elevationRad);
  return sine > 0 ? Math.min(reliefRangeM / sine, capM) : capM;
}

/**
 * The cascades' weight, w: 1 where cascade 1's texel is at most half the horizon map's at the
 * nadir, 0 where it is at least the map's, linear between (the split ruling's S5).
 *
 * @remarks
 * So the cascades are off where cascade 1 is coarser than the map, as in orbit (Design note 11),
 * and fade out with no pop of a craft's shadow. T9.b applies c′ = 1 − w (1 − c) and skips the
 * caster passes at w = 0.
 *
 * @param cascade1TexelM - Cascade 1's texel, m, positive.
 * @param horizonTexelM - The horizon map's texel at the nadir, m, positive: the nadir patch's
 * vertex spacing × 2^k, k its morph factor, continuous through a split.
 * @throws RangeError for a texel that is not positive and finite.
 */
export function cascadeWeight(cascade1TexelM: number, horizonTexelM: number): number {
  if (!(cascade1TexelM > 0 && Number.isFinite(cascade1TexelM))) {
    throw new RangeError(`cascades need a positive texel, not ${String(cascade1TexelM)}`);
  }
  if (!(horizonTexelM > 0 && Number.isFinite(horizonTexelM))) {
    throw new RangeError(`cascades need a positive horizon texel, not ${String(horizonTexelM)}`);
  }
  const ratio = cascade1TexelM / horizonTexelM;
  const weight = (CASCADE_WEIGHT_OFF - ratio) / (CASCADE_WEIGHT_OFF - CASCADE_WEIGHT_FULL);
  return Math.min(1, Math.max(0, weight));
}

/**
 * The spacing of the body-fixed lattice the cascades are snapped from with no frame before, m:
 * L = 2^⌈log₂(2 r₁)⌉, the power of two in [D₁, 2D₁), D₁ cascade 1's diameter
 * (decision-r10-t9-split, ruling 7 amended).
 *
 * @remarks
 * 256 m at h ≤ 100 m for a 60°, 16:9 view; piecewise constant in h, so it does not jump every
 * frame in a descent. The exponent is corrected after `Math.log2`, so that L is exactly the
 * power of two in [D₁, 2D₁) whatever the rounding of the logarithm.
 *
 * @param cascade1RadiusM - r₁, cascade 1's sphere's radius, m, positive.
 * @throws RangeError for a radius that is not positive and finite.
 */
export function gridLatticeSpacingM(cascade1RadiusM: number): number {
  if (!(cascade1RadiusM > 0 && Number.isFinite(cascade1RadiusM))) {
    throw new RangeError(
      `a lattice needs a positive cascade radius, not ${String(cascade1RadiusM)}`,
    );
  }
  const diameterM = 2 * cascade1RadiusM;
  let exponent = Math.ceil(Math.log2(diameterM));
  if (2 ** exponent < diameterM) {
    exponent += 1;
  }
  if (2 ** (exponent - 1) >= diameterM) {
    exponent -= 1;
  }
  return 2 ** exponent;
}

/** The lattice point O nearest `positionM`: L ⌊x ÷ L + ½⌋ in each body-fixed component, m. */
function latticePoint(positionM: Vec3, spacingM: number): Vec3 {
  const round = (componentM: number): number => spacingM * Math.floor(componentM / spacingM + 0.5);
  return { x: round(positionM.x), y: round(positionM.y), z: round(positionM.z) };
}

/** A cascade's light matrices in `f64`, before narrowing: 4 × 4, column-major. */
export interface CascadeMatricesF64 {
  /** The body's non-rotating axes to the light's, translation zero, w 1. */
  readonly viewRotation: Float64Array;
  /** The reversed orthographic projection about the snapped centre. */
  readonly projection: Float64Array;
  /** `projection` × `viewRotation`, formed here, for the receiver. */
  readonly lightFromCamera: Float64Array;
}

/** The product a × b of two column-major 4 × 4 matrices, in `f64`. */
function multiply4(a: Float64Array, b: Float64Array): Float64Array {
  const product = new Float64Array(16);
  for (let column = 0; column < 4; column += 1) {
    for (let row = 0; row < 4; row += 1) {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) {
        sum += (a[k * 4 + row] ?? 0) * (b[column * 4 + k] ?? 0);
      }
      product[column * 4 + row] = sum;
    }
  }
  return product;
}

/**
 * A cascade's light matrices, in `f64` (Design note 11; decision-r10-t9-split).
 *
 * @remarks
 * `viewRotation`'s rows are the light's axes in the body's non-rotating axes, R x_L, R y_L and
 * R z_L: the basis composed with the transpose of the body's rotation, so that a camera-relative
 * position in the non-rotating axes, as the terrain draws its vertices, comes out in the light's
 * axes. `projection` is orthographic about the snapped centre, whose offset from the camera, in the
 * light's axes, is its translation: clip x and y are (x_L − o_x) ÷ r and (y_L − o_y) ÷ r, ±1 at
 * the sphere's sides, and depth is (z_L − o_z + d_far) ÷ (d_near + d_far), 1 at the near plane
 * (towards the sun) and 0 at the far.
 *
 * _The depth convention is kept for uniformity only._ An orthographic projection's depth is linear
 * in distance, so reversed-Z, which spreads a float's precision against perspective's 1 ÷ z, buys
 * nothing where the sphere fills the range: the maps keep the project's convention (1 near, cleared
 * to 0, compared `greater-equal`) only so that every depth target reads alike. `depth32float`'s
 * step is at worst 2⁻²⁴ of the depth range, its spacing in [½, 1): 3.0 mm over 50 km. With
 * `depth-clip-control` the range is the sphere's 2r and that is the receivers' worst step. Without
 * it the sphere sits in [0, 2r ÷ (2r + reach)], where the floats are densest, so its receivers step
 * by at most 2⁻²³ × 2r. The range's worst step, 2⁻²⁴ (2r + reach), falls on far casters alone:
 * 1.36 cm for cascade 1 at `TERRAIN TEST`'s cap of 227.5 km, 2.0 cm for cascade 4 at 4 km up. So
 * the convention happens to help there, by placing the receivers near 0; it was not chosen for it,
 * and the bias should not be tuned against it.
 *
 * @param basis - The light's axes, body-fixed ({@link lightBasis}).
 * @param rotation - The body's rotation, body-fixed to non-rotating.
 * @param centreFromCameraLightM - o, the snapped centre less the camera, in the light's axes, m.
 * @param radiusM - r, the sphere's radius, m, positive.
 * @param depthNearM - d_near, the near plane's distance towards the sun from the centre, m,
 * positive.
 * @param depthFarM - d_far, the far plane's distance away from the sun from the centre, m,
 * positive.
 * @throws RangeError for a radius or depth distance that is not positive and finite.
 */
export function cascadeMatricesF64(
  basis: LightBasis,
  rotation: Rotation3,
  centreFromCameraLightM: Vec3,
  radiusM: number,
  depthNearM: number,
  depthFarM: number,
): CascadeMatricesF64 {
  for (const [name, value] of [
    ["radius", radiusM],
    ["near depth", depthNearM],
    ["far depth", depthFarM],
  ] as const) {
    if (!(value > 0 && Number.isFinite(value))) {
      throw new RangeError(`a cascade needs a positive ${name}, not ${String(value)}`);
    }
  }
  requireFiniteVec("centre offset", centreFromCameraLightM);
  const x = rotateToBody(rotation, basis.x);
  const y = rotateToBody(rotation, basis.y);
  const z = rotateToBody(rotation, basis.z);
  const viewRotation = new Float64Array([
    x.x,
    y.x,
    z.x,
    0,
    x.y,
    y.y,
    z.y,
    0,
    x.z,
    y.z,
    z.z,
    0,
    0,
    0,
    0,
    1,
  ]);
  const o = centreFromCameraLightM;
  const spanM = depthNearM + depthFarM;
  const projection = new Float64Array([
    1 / radiusM,
    0,
    0,
    0,
    0,
    1 / radiusM,
    0,
    0,
    0,
    0,
    1 / spanM,
    0,
    -o.x / radiusM,
    -o.y / radiusM,
    (depthFarM - o.z) / spanM,
    1,
  ]);
  return { viewRotation, projection, lightFromCamera: multiply4(projection, viewRotation) };
}

/** What {@link fitCascades} needs of a view and its body, one frame. */
export interface CascadeInput {
  /** The camera less the body's centre, body-fixed, m (`f64`; the pass's `cameraBodyFixedM`). */
  readonly cameraBodyFixedM: Vec3;
  /** The camera's orientation in the body-fixed axes (the pass's `orientationBodyFixed`). */
  readonly orientationBodyFixed: Quaternion;
  /** The body's rotation, body-fixed axes into its non-rotating axes. */
  readonly rotation: Rotation3;
  /** φx, the view's horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
  /** a, the view's width ÷ height, positive. */
  readonly aspect: number;
  /** n, the view's near plane, m, positive; `NEAR_PLANE_M` by default. */
  readonly nearM?: number;
  /** The view's far plane, m; ∞ by default, as the view's projection is built. */
  readonly farM?: number;
  /** h, the camera's height above the ground at its nadir, m; below 0 it is taken as 0. */
  readonly heightAboveGroundM: number;
  /**
   * The direction the cascades are fitted to, body-fixed; any length. The pass gives the top edge
   * of star 0's disc ({@link discTopDirection}; decision-r10-t9b-penumbra), so that the light's
   * axes, the elevation, the caster reach and the near plane all take it.
   */
  readonly sunDirectionBodyFixed: Vec3;
  /** ν, the datum's normal at the camera's nadir, body-fixed; any length. */
  readonly datumNormalBodyFixed: Vec3;
  /** R, the body's mean radius, m, positive. */
  readonly bodyRadiusM: number;
  /** Δh, the body's realised relief range, m, 0 or more. */
  readonly reliefRangeM: number;
  /** A caster's height above the ground, m, 0 or more, taken for Δh where larger; 0 by default. */
  readonly casterHeightM?: number;
  /** The horizon map's texel at the nadir, m, positive ({@link cascadeWeight}). */
  readonly horizonTexelM: number;
  /** Whether the adapter offers `depth-clip-control` (R01's `GpuCapabilities.depthClipControl`). */
  readonly depthClipControl: boolean;
  /**
   * The four cascades' snapped centres of the frame before, body-fixed, m: each the
   * {@link Cascade.centreBodyFixedM} the last call returned, from which each grid is carried by
   * whole texels (decision-r10-t9-split, ruling 7 amended). Absent, all four are snapped from the
   * body-fixed lattice point nearest the camera ({@link gridLatticeSpacingM}).
   *
   * @remarks
   * T9.b keeps them for each view and hands them back each frame, and drops them when the view is
   * new, its handed-over body changes, or the pass is remade after a device loss. They move only
   * the grid's sub-texel phase, which snapping exists to hide.
   */
  readonly previousCentresBodyFixedM?: readonly [Vec3, Vec3, Vec3, Vec3];
}

/** One cascade: its slice, its sphere, its texel and its light matrices. */
export interface Cascade {
  /** Its split range's near end along the view axis, m. */
  readonly nearM: number;
  /** Its split range's far end along the view axis, m. */
  readonly farM: number;
  /** Where it starts to blend into the next, or the last into lit, m: 90% of the way through. */
  readonly fadeStartM: number;
  /** The sphere's centre less the body's centre, body-fixed, m (`f64`), snapped to whole texels. */
  readonly centreBodyFixedM: Vec3;
  /** The snapped centre less the camera, in the light's axes, m (`f64`): the projection's shift. */
  readonly centreFromCameraLightM: Vec3;
  /** r, the sphere's radius, m: the projection's half-width. */
  readonly radiusM: number;
  /** The texel's world size, 2r ÷ 2,048, m. */
  readonly texelM: number;
  /** The receiver's offset along its normal, m: {@link NORMAL_OFFSET_TEXELS} texels. */
  readonly normalOffsetM: number;
  /**
   * The near plane's distance towards the sun from the centre, m: r with `depth-clip-control`,
   * casters nearer being pancaked onto it by `unclippedDepth`; r plus the caster reach without it.
   */
  readonly depthNearM: number;
  /** The far plane's distance away from the sun from the centre, m: r. */
  readonly depthFarM: number;
  /** The caster pass's view rotation: non-rotating axes to the light's, translation zero. */
  readonly viewRotation: Float32Array;
  /** The caster pass's projection: reversed orthographic, its translation the centre's offset. */
  readonly projection: Float32Array;
  /**
   * `projection` × `viewRotation`, formed in `f64` and narrowed once: a camera-relative position
   * in the body's non-rotating axes to the cascade's clip x, y and depth, for the receiver.
   */
  readonly lightFromCamera: Float32Array;
}

/** The four cascades of one view and frame, and what T9.b needs beside them. */
export interface CascadeSet {
  /** f, the shadowed range, m ({@link shadowedRangeM}). */
  readonly rangeM: number;
  /** C₀ … C₄, m ({@link cascadeSplitsM}). */
  readonly splitsM: Float64Array;
  /** w, in [0, 1] ({@link cascadeWeight}); the caster passes are skipped at 0. */
  readonly weight: number;
  /** Whether the caster pipelines take `primitive.unclippedDepth`: `depth-clip-control`'s offer. */
  readonly unclippedDepth: boolean;
  /** The caster reach, m ({@link casterReachM}), for culling. */
  readonly casterReachM: number;
  /**
   * The fitted sun's elevation at the nadir, rad ({@link sunElevationRad}): the disc top's, e★ + ρ,
   * as the pass fits it ({@link discTopDirection}).
   */
  readonly sunElevationRad: number;
  /** The light's axes, body-fixed ({@link lightBasis}). */
  readonly basis: LightBasis;
  /** The cascades, nearest first: layer i of {@link CASCADE_TEXTURE} is cascade i. */
  readonly cascades: readonly [Cascade, Cascade, Cascade, Cascade];
}

/** Refuses a {@link CascadeInput} that {@link fitCascades} cannot fit. */
function checkInput(input: CascadeInput): void {
  requireFiniteVec("camera position", input.cameraBodyFixedM);
  const q = input.orientationBodyFixed;
  for (const [name, value] of [
    ["orientation.w", q.w],
    ["orientation.x", q.x],
    ["orientation.y", q.y],
    ["orientation.z", q.z],
    ["field of view", input.fovXRad],
    ["aspect", input.aspect],
    ["near plane", input.nearM ?? NEAR_PLANE_M],
    ["height above the ground", input.heightAboveGroundM],
    ["body radius", input.bodyRadiusM],
    ["relief range", input.reliefRangeM],
    ["caster height", input.casterHeightM ?? 0],
    ["horizon texel", input.horizonTexelM],
  ] as const) {
    requireFinite(name, value);
  }
  if (input.farM !== undefined && !(input.farM > 0)) {
    throw new RangeError(`cascades need a positive far plane, not ${String(input.farM)}`);
  }
  for (const [name, value] of [
    ["near plane", input.nearM ?? NEAR_PLANE_M],
    ["body radius", input.bodyRadiusM],
    ["horizon texel", input.horizonTexelM],
  ] as const) {
    if (!(value > 0)) {
      throw new RangeError(`cascades need a positive ${name}, not ${String(value)}`);
    }
  }
  for (const [name, value] of [
    ["relief range", input.reliefRangeM],
    ["caster height", input.casterHeightM ?? 0],
  ] as const) {
    if (!(value >= 0)) {
      throw new RangeError(`cascades need a ${name} of 0 or more, not ${String(value)}`);
    }
  }
  const previous: ReadonlyArray<Vec3> | undefined = input.previousCentresBodyFixedM;
  if (previous !== undefined) {
    if (previous.length !== CASCADE_COUNT) {
      throw new RangeError(
        `cascades carry ${String(CASCADE_COUNT)} previous centres, not ${String(previous.length)}`,
      );
    }
    for (const [i, centre] of previous.entries()) {
      requireFiniteVec(`previous centre ${String(i + 1)}`, centre);
    }
  }
}

/** The view's axis, the camera's −z, in the frame its orientation is given in. */
function viewAxis(orientation: Quaternion): Vec3 {
  const [r0, r1, r2] = rotationRows(orientation);
  // The third column of the rotation is the camera's backward axis.
  return normalise({ x: -r0.z, y: -r1.z, z: -r2.z });
}

/** A cascade's index, nearest first. */
type CascadeIndex = 0 | 1 | 2 | 3;

/**
 * The origins of a first frame: the body-fixed lattice point nearest the camera, for all four
 * cascades, on a lattice of cascade 1's diameter rounded up to a power of two.
 */
function latticeOrigins(
  cameraBodyFixedM: Vec3,
  splitsM: Float64Array,
  t2: number,
): readonly [Vec3, Vec3, Vec3, Vec3] {
  const first = sliceSphere(splitAt(splitsM, 0), splitAt(splitsM, 1), t2);
  const origin = latticePoint(cameraBodyFixedM, gridLatticeSpacingM(first.radiusM));
  return [origin, origin, origin, origin];
}

/** A split, by index, from {@link cascadeSplitsM}'s five. */
function splitAt(splitsM: Float64Array, index: number): number {
  const split = splitsM[index];
  if (split === undefined) {
    throw new RangeError(`cascade splits hold ${String(splitsM.length)} entries, not ${index + 1}`);
  }
  return split;
}

/**
 * Fits the four cascades of one view to its frustum and the sun, one frame (Design note 11;
 * decision-r10-t9-split).
 *
 * @remarks
 * Each cascade's sphere ({@link sliceSphere}) is centred on the view axis, and its centre is
 * snapped in the light's body-fixed axes to whole texels from its origin P, keeping its offset
 * along z_L: P is its snapped centre of the frame before where the caller gives them
 * ({@link CascadeInput.previousCentresBodyFixedM}), and otherwise the body-fixed lattice point
 * nearest the camera, one for all four (decision-r10-t9-split, ruling 7 amended). The snapped
 * centre's offset from the camera is formed from P's and the steps in `f64`, and every matrix from
 * it in `f64`, narrowed once ({@link cascadeMatricesF64}). Nothing body-fixed depends on the body's
 * rotation, which enters only the view rotation. Δh is the larger of the relief range and the
 * caster height. Every use of the sun here (the light's axes, the elevation, the caster reach and
 * the near plane) takes {@link CascadeInput.sunDirectionBodyFixed} as given, which the pass raises
 * to the disc's top edge ({@link discTopDirection}). The outputs depend on the inputs alone,
 * whatever the order of calls.
 *
 * @throws RangeError for a non-finite input other than an infinite far plane; a sun or datum
 * normal of zero length; a field of view outside (0, π); an aspect, near plane, body radius or
 * horizon texel that is not positive; a negative relief range or caster height; a far plane not
 * beyond the near; or previous centres that are not four or not finite.
 */
export function fitCascades(input: CascadeInput): CascadeSet {
  checkInput(input);
  const nearPlaneM = input.nearM ?? NEAR_PLANE_M;
  const rangeM = shadowedRangeM(input.heightAboveGroundM, input.farM ?? Infinity);
  const splitsM = cascadeSplitsM(nearPlaneM, rangeM);
  const t2 = halfDiagonalTanSquared(input.fovXRad, input.aspect);
  const basis = lightBasis(input.sunDirectionBodyFixed);
  const elevationRad = sunElevationRad(input.sunDirectionBodyFixed, input.datumNormalBodyFixed);
  const reliefM = Math.max(input.reliefRangeM, input.casterHeightM ?? 0);
  const reachM = casterReachM(elevationRad, reliefM, input.bodyRadiusM);
  const camera = input.cameraBodyFixedM;
  const forward = viewAxis(input.orientationBodyFixed);
  const origins = input.previousCentresBodyFixedM ?? latticeOrigins(camera, splitsM, t2);

  const fit = (index: CascadeIndex): Cascade => {
    const nearM = splitAt(splitsM, index);
    const farM = splitAt(splitsM, index + 1);
    const { distanceM, radiusM } = sliceSphere(nearM, farM, t2);
    const texelM = (2 * radiusM) / CASCADE_SIZE_PX;
    const origin = origins[index];
    // c − P: the unsnapped centre from its origin, body-fixed, small where P is the last frame's.
    const fromOrigin = sub(add(camera, scale(forward, distanceM)), origin);
    const stepXM = Math.round(dot(fromOrigin, basis.x) / texelM) * texelM;
    const stepYM = Math.round(dot(fromOrigin, basis.y) / texelM) * texelM;
    const alongZM = dot(fromOrigin, basis.z);
    const originFromCamera = sub(origin, camera);
    const centreFromCameraLightM = {
      x: dot(originFromCamera, basis.x) + stepXM,
      y: dot(originFromCamera, basis.y) + stepYM,
      z: dot(originFromCamera, basis.z) + alongZM,
    };
    const centreBodyFixedM = add(
      origin,
      add(add(scale(basis.x, stepXM), scale(basis.y, stepYM)), scale(basis.z, alongZM)),
    );
    const depthNearM = input.depthClipControl ? radiusM : radiusM + reachM;
    const matrices = cascadeMatricesF64(
      basis,
      input.rotation,
      centreFromCameraLightM,
      radiusM,
      depthNearM,
      radiusM,
    );
    return {
      nearM,
      farM,
      fadeStartM: nearM + (1 - CASCADE_FADE_FRACTION) * (farM - nearM),
      centreBodyFixedM,
      centreFromCameraLightM,
      radiusM,
      texelM,
      normalOffsetM: NORMAL_OFFSET_TEXELS * texelM,
      depthNearM,
      depthFarM: radiusM,
      viewRotation: Float32Array.from(matrices.viewRotation),
      projection: Float32Array.from(matrices.projection),
      lightFromCamera: Float32Array.from(matrices.lightFromCamera),
    };
  };

  const cascades = [fit(0), fit(1), fit(2), fit(3)] as const;
  const first = cascades[0];
  return {
    rangeM,
    splitsM,
    weight: cascadeWeight(first.texelM, input.horizonTexelM),
    unclippedDepth: input.depthClipControl,
    casterReachM: reachM,
    sunElevationRad: elevationRad,
    basis,
    cascades,
  };
}
