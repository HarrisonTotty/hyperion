/**
 * The high setting's cascaded shadows as the lit terrain receives them (plan R10, R10.T9.b; Design
 * note 11; decision-r10-t9b-design D5): the TypeScript twin of `cascadeShadow.wgsl` and of
 * `terrainLit.wgsl`'s `terrain_cascade_term`, and the packing of R10.T9.a's {@link CascadeSet}
 * into the lit terrain's `Draw`.
 *
 * @remarks
 * - **The cascade** is the first whose far end the receiver's distance d along the view axis does
 *   not pass ({@link cascadeAt}); beyond C₄ the receiver is lit. Over its last
 *   `CASCADE_FADE_FRACTION`, [fadeStart, far], it blends linearly into the next, and the last into
 *   lit ({@link cascadeBlend}), so nothing steps at a split or at C₄.
 * - **The overhang**: snapping moves a map by up to half a texel off its slice (R10.T9.a's finding),
 *   so where the receiver with the filter's footprint ({@link CASCADE_FOOTPRINT_TEXELS}) falls
 *   outside a map ({@link coversWithFootprint}), the next cascade stands in, and past the last,
 *   lit ({@link cascadeLitFraction}).
 * - **The normal offset** is Holbert's (GDC 2011) basic variant: the receiver moves along its
 *   geometric normal by the cascade's `normalOffsetM` × saturate(1 − N·L) before it is projected
 *   ({@link receiverOffsetM}), so that the offset moves its compared depth as well as its uv, not
 *   his "offset in UV space only" refinement. L is star 0's own direction, not the fitted one.
 * - **The weight**: c′ = 1 − w (1 − c), w R10.T9.a's `cascadeWeight` ({@link weightedCascadeLit}).
 * - **The minimum** with the horizon map (decision-r10-t8b §2): the direct sun takes
 *   min(1, c′ ÷ m), 1 where m ≤ 0, m the map's visible share of the disc ({@link cascadeTerm}), so
 *   that its lit share is min(m, c′) ({@link directLitShare}): never brighter than the map alone.
 *   The cascades are fitted to the disc's top edge (decision-r10-t9b-penumbra; `cascades.ts`'
 *   `discTopDirection`), so their shadow begins where the horizon map's share m reaches 0, and
 *   min(m, c′) is m across the penumbra; non-terrain casters' shadows are short by about half a
 *   penumbra, ρL (4.7 cm at 10 m).
 */

import { add, dot, norm, scale, type Vec3 } from "../../../geometry/vec3";
import type { UniformSpec } from "../../engine/types";
import { CASCADE_SIZE_PX, type CascadeSet } from "./cascades";

/**
 * The filter's four taps, texels from the receiver, x then y with y down the map: the standard
 * four-sample rotated grid, (−2, −6), (6, −2), (−6, 2) and (2, 6) sixteenths of a pixel from its
 * centre (Vulkan's standard sample locations for four samples; Direct3D 11's standard multisample
 * pattern), scaled to the unit circle by 1 ÷ √40 sixteenths; `cascadeShadow.wgsl`'s
 * `CASCADE_TAP_0` to `3`.
 *
 * @remarks
 * Four points of a disc whose least distance apart is greatest are a square on its rim, √2 apart,
 * so this is the four-point maximin (best-spread) set on a disc of one texel. Its mean is 0, so it
 * moves no shadow edge, and turned by atan(1 ÷ 3) its four offsets differ along each axis (−3, −1,
 * 1 and 3 of 1 ÷ √10 texel, neighbours 2 ÷ √10 ≈ 0.63 texel apart), so that an edge along either
 * axis steps through five levels, not three. Each tap is a 2 × 2 PCF of the comparison sampler's
 * `linear` filter where the adapter filters comparisons (WebGPU leaves it to the adapter), so the
 * four taps carry the filtering alone where it does not.
 */
export const CASCADE_FILTER_TAPS: ReadonlyArray<readonly [number, number]> = [
  [-1 / Math.sqrt(10), -3 / Math.sqrt(10)],
  [3 / Math.sqrt(10), -1 / Math.sqrt(10)],
  [-3 / Math.sqrt(10), 1 / Math.sqrt(10)],
  [1 / Math.sqrt(10), 3 / Math.sqrt(10)],
];

/** The radius of the disc the taps lie on, texels: 1 (decision-r10-t9b-design D5). */
export const CASCADE_TAP_RADIUS_TEXELS = 1;

/**
 * The filter's reach about the receiver, texels: the taps' radius and the 2 × 2 PCF's texel beyond
 * each, so that a map covers a receiver only where every texel it reads lies inside.
 */
export const CASCADE_FOOTPRINT_TEXELS = CASCADE_TAP_RADIUS_TEXELS + 1;

/** A cascade's index, nearest first: layer i of the maps is cascade i. */
export type CascadeIndex = 0 | 1 | 2 | 3;

/** The cascades' indices, nearest first. */
const CASCADE_INDICES: readonly [CascadeIndex, CascadeIndex, CascadeIndex, CascadeIndex] = [
  0, 1, 2, 3,
];

/** A receiver's place in a cascade's map. */
export interface CascadeMapPoint {
  /** Across the map, 0 to 1 from its left edge: ½ + ½ of the clip x. */
  readonly u: number;
  /** Down the map, 0 to 1 from its top edge: ½ − ½ of the clip y, as a framebuffer's y runs down. */
  readonly v: number;
  /** The reversed orthographic depth: 1 at the near plane towards the sun, 0 at the far. */
  readonly depth: number;
}

/**
 * A camera-relative place in the body's non-rotating axes in a cascade's map, through its
 * `lightFromCamera` (`cascade_map_point` without the offset).
 *
 * @param lightFromCamera - A column-major 4 × 4 (`Cascade.lightFromCamera`), orthographic, w 1.
 * @param fromCameraM - The place less the camera, m.
 */
export function cascadeMapPoint(
  lightFromCamera: ArrayLike<number>,
  fromCameraM: Vec3,
): CascadeMapPoint {
  const at = (i: number): number => lightFromCamera[i] ?? Number.NaN;
  const p = fromCameraM;
  const x = at(0) * p.x + at(4) * p.y + at(8) * p.z + at(12);
  const y = at(1) * p.x + at(5) * p.y + at(9) * p.z + at(13);
  const depth = at(2) * p.x + at(6) * p.y + at(10) * p.z + at(14);
  return { u: 0.5 + 0.5 * x, v: 0.5 - 0.5 * y, depth };
}

/**
 * Whether a map point and the filter's footprint about it ({@link CASCADE_FOOTPRINT_TEXELS}) lie
 * inside the map's [0, 1]² (`cascade_covers`).
 */
export function coversWithFootprint(point: CascadeMapPoint): boolean {
  const margin = CASCADE_FOOTPRINT_TEXELS / CASCADE_SIZE_PX;
  return point.u >= margin && point.v >= margin && point.u <= 1 - margin && point.v <= 1 - margin;
}

/** x clamped to [0, 1], WGSL's `saturate`. */
function saturate(x: number): number {
  return Math.min(Math.max(x, 0), 1);
}

/**
 * Holbert's normal offset of a receiver in one cascade, m: the cascade's offset × saturate(1 − N·L),
 * full where the sun grazes the face or is behind it, none where it stands overhead.
 *
 * @param normalOffsetM - The cascade's `normalOffsetM`, m.
 * @param normal - The receiver's geometric unit normal.
 * @param sun - The unit direction to the sun, in the same axes.
 */
export function receiverOffsetM(normalOffsetM: number, normal: Vec3, sun: Vec3): number {
  return normalOffsetM * saturate(1 - dot(normal, sun));
}

/**
 * The cascade a receiver at `distanceAlongViewM` falls in: the first whose far end it does not
 * pass, or `null` beyond the last's, where it is lit.
 *
 * @param distanceAlongViewM - d, the receiver less the camera along the view axis, m.
 * @param farM - C₁ … C₄, each cascade's far end along the view axis, m.
 */
export function cascadeAt(
  distanceAlongViewM: number,
  farM: ArrayLike<number>,
): CascadeIndex | null {
  for (const i of CASCADE_INDICES) {
    if (distanceAlongViewM <= (farM[i] ?? Number.NaN)) {
      return i;
    }
  }
  return null;
}

/**
 * The share of the next cascade (or, for the last, of lit) at `distanceAlongViewM` in a cascade's
 * range: 0 to its fade start, rising linearly to 1 at its far end.
 *
 * @param distanceAlongViewM - d, m.
 * @param fadeStartM - The cascade's fade start along the view axis, m, before `farM`.
 * @param farM - The cascade's far end along the view axis, m.
 */
export function cascadeBlend(distanceAlongViewM: number, fadeStartM: number, farM: number): number {
  return saturate((distanceAlongViewM - fadeStartM) / (farM - fadeStartM));
}

/**
 * The cascades' lit fraction c at a receiver, before the weight (`cascade_lit_fraction`'s c): the
 * first covering cascade from its own, blended into the first covering cascade after its own over
 * its own blend band, and 1 where none covers it or beyond C₄.
 *
 * @param distanceAlongViewM - d, the receiver less the camera along the view axis, m.
 * @param farM - C₁ … C₄, m.
 * @param fadeStartM - Each cascade's fade start, m.
 * @param covers - Whether cascade j's map, with the footprint, holds the receiver.
 * @param litFraction - Cascade j's filtered lit fraction there, in [0, 1].
 */
export function cascadeLitFraction(
  distanceAlongViewM: number,
  farM: ArrayLike<number>,
  fadeStartM: ArrayLike<number>,
  covers: (cascade: CascadeIndex) => boolean,
  litFraction: (cascade: CascadeIndex) => number,
): number {
  const own = cascadeAt(distanceAlongViewM, farM);
  if (own === null) {
    return 1;
  }
  const firstFrom = (from: number): CascadeIndex | null =>
    CASCADE_INDICES.find((j) => j >= from && covers(j)) ?? null;
  const first = firstFrom(own);
  if (first === null) {
    return 1;
  }
  const c = litFraction(first);
  if (first !== own) {
    return c;
  }
  const blend = cascadeBlend(distanceAlongViewM, fadeStartM[own] ?? Number.NaN, farM[own] ?? 0);
  if (!(blend > 0)) {
    return c;
  }
  const next = firstFrom(own + 1);
  const after = next === null ? 1 : litFraction(next);
  return c + blend * (after - c);
}

/**
 * c′ = 1 − w (1 − c): the cascades' lit fraction at their weight against the horizon map, 1 at
 * w = 0.
 *
 * @param weight - w, in [0, 1] (`cascadeWeight`).
 * @param litFraction - c, in [0, 1].
 */
export function weightedCascadeLit(weight: number, litFraction: number): number {
  return 1 - weight * (1 - litFraction);
}

/**
 * The cascades' term on the direct sun beside the horizon map (`terrain_cascade_term`):
 * min(1, c′ ÷ m), 1 where m ≤ 0, so it never brightens.
 *
 * @param horizonShare - m, the horizon map's visible share of the star's disc, in [0, 1].
 * @param cascadeLit - c′, in [0, 1].
 */
export function cascadeTerm(horizonShare: number, cascadeLit: number): number {
  if (!(horizonShare > 0)) {
    return 1;
  }
  return Math.min(1, cascadeLit / horizonShare);
}

/**
 * The direct sun's lit share of the disc under the map and the cascades together: m times
 * {@link cascadeTerm}, which is min(m, c′), Design note 11's minimum.
 *
 * @param horizonShare - m, in [0, 1].
 * @param cascadeLit - c′, in [0, 1].
 */
export function directLitShare(horizonShare: number, cascadeLit: number): number {
  return horizonShare * cascadeTerm(horizonShare, cascadeLit);
}

/**
 * The cascades' fields of the lit terrain's `Draw`, after its own (`terrainLitShade.wgsl`), each
 * written in place each frame by {@link packCascadeUniforms}.
 */
export interface CascadeUniforms {
  /** Cascade 0's `lightFromCamera`, column-major, 16 floats. */
  readonly cascadeLight0: Float32Array;
  readonly cascadeLight1: Float32Array;
  readonly cascadeLight2: Float32Array;
  readonly cascadeLight3: Float32Array;
  /** C₁ … C₄, m. */
  readonly cascadeFarM: Float32Array;
  /** Each cascade's fade start, m. */
  readonly cascadeFadeStartM: Float32Array;
  /** Each cascade's normal offset, m. */
  readonly cascadeNormalOffsetM: Float32Array;
  /** The view axis in the body's non-rotating axes; w, the weight. */
  readonly cascadeViewAxis: Float32Array;
}

/** The {@link CascadeUniforms}' names and types, in the `Draw`'s order. */
export const CASCADE_UNIFORM_SPECS: ReadonlyArray<UniformSpec> = [
  { name: "cascadeLight0", type: "mat4x4f" },
  { name: "cascadeLight1", type: "mat4x4f" },
  { name: "cascadeLight2", type: "mat4x4f" },
  { name: "cascadeLight3", type: "mat4x4f" },
  { name: "cascadeFarM", type: "vec4f" },
  { name: "cascadeFadeStartM", type: "vec4f" },
  { name: "cascadeNormalOffsetM", type: "vec4f" },
  { name: "cascadeViewAxis", type: "vec4f" },
];

/** A 4 × 4 identity, column-major. */
const IDENTITY_4: ReadonlyArray<number> = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

/** Fresh {@link CascadeUniforms}, as for no cascades: identity matrices, zeros and w = 0. */
export function cascadeUniforms(): CascadeUniforms {
  return {
    cascadeLight0: Float32Array.from(IDENTITY_4),
    cascadeLight1: Float32Array.from(IDENTITY_4),
    cascadeLight2: Float32Array.from(IDENTITY_4),
    cascadeLight3: Float32Array.from(IDENTITY_4),
    cascadeFarM: new Float32Array(4),
    cascadeFadeStartM: new Float32Array(4),
    cascadeNormalOffsetM: new Float32Array(4),
    cascadeViewAxis: new Float32Array(4),
  };
}

/** The four matrices of `out`, nearest first. */
function lightsOf(
  out: CascadeUniforms,
): readonly [Float32Array, Float32Array, Float32Array, Float32Array] {
  return [out.cascadeLight0, out.cascadeLight1, out.cascadeLight2, out.cascadeLight3];
}

/**
 * Writes one view's cascades into the lit terrain's `Draw` fields, in place: each cascade's
 * `lightFromCamera`, far end, fade start and normal offset, and the view axis with the weight in
 * w; for `null` (no cascades this frame: the low setting, a body that does not take them, no star)
 * identity matrices, zeros and w = 0, which the receiver reads as lit without a sample.
 *
 * @param set - The view's cascades this frame (`fitCascades`), or `null`.
 * @param viewAxisNonRotating - The camera's forward in the body's non-rotating axes, the axis the
 *   splits are measured along; any length, normalised here.
 * @param out - The fields to write, as {@link cascadeUniforms} makes them.
 * @throws RangeError for a view axis that is not finite and of non-zero length, with a set.
 */
export function packCascadeUniforms(
  set: CascadeSet | null,
  viewAxisNonRotating: Vec3,
  out: CascadeUniforms,
): void {
  const lights = lightsOf(out);
  if (set === null) {
    for (const light of lights) {
      light.set(IDENTITY_4);
    }
    out.cascadeFarM.fill(0);
    out.cascadeFadeStartM.fill(0);
    out.cascadeNormalOffsetM.fill(0);
    out.cascadeViewAxis.fill(0);
    return;
  }
  const length = norm(viewAxisNonRotating);
  if (!(length > 0 && Number.isFinite(length))) {
    throw new RangeError(
      `the cascades need a view axis of finite non-zero length, not ${String(length)}`,
    );
  }
  for (const i of CASCADE_INDICES) {
    const cascade = set.cascades[i];
    lights[i].set(cascade.lightFromCamera.subarray(0, 16));
    out.cascadeFarM[i] = cascade.farM;
    out.cascadeFadeStartM[i] = cascade.fadeStartM;
    out.cascadeNormalOffsetM[i] = cascade.normalOffsetM;
  }
  const axis = scale(viewAxisNonRotating, 1 / length);
  out.cascadeViewAxis.set([axis.x, axis.y, axis.z, set.weight]);
}

/**
 * A cascade's 2 × 2 PCF at a point of its map: the share of the comparisons that pass (reference
 * at least the stored depth) about (u, v), in [0, 1], as the comparison sampler gives it.
 */
export type CascadeCompare = (cascade: CascadeIndex, u: number, v: number, depth: number) => number;

/**
 * c′ at a receiver from the packed fields, the twin of `cascadeShadow.wgsl`'s
 * `cascade_lit_fraction`: the cascade by d, each moved by its normal offset and projected, the four
 * taps' mean at the clamped depth, the overhang's fallback, the blend and the weight.
 *
 * @param uniforms - The `Draw`'s cascade fields ({@link packCascadeUniforms}).
 * @param fromCameraM - The receiver less the camera, in the body's non-rotating axes, m.
 * @param normal - Its geometric unit normal, in those axes.
 * @param sun - Star 0's unit direction, its centre's (not the disc's top edge, which the cascades
 *   are fitted to), in those axes.
 * @param compare - The comparison sampler's PCF of a cascade's map.
 */
export function cascadeReceiverLit(
  uniforms: CascadeUniforms,
  fromCameraM: Vec3,
  normal: Vec3,
  sun: Vec3,
  compare: CascadeCompare,
): number {
  const [ax = 0, ay = 0, az = 0, weight = 0] = uniforms.cascadeViewAxis;
  if (!(weight > 0)) {
    return 1;
  }
  const lights = lightsOf(uniforms);
  const lean = saturate(1 - dot(normal, sun));
  const pointIn = (j: CascadeIndex): CascadeMapPoint =>
    cascadeMapPoint(
      lights[j],
      add(fromCameraM, scale(normal, lean * (uniforms.cascadeNormalOffsetM[j] ?? 0))),
    );
  const sample = (j: CascadeIndex): number => {
    const { u, v, depth } = pointIn(j);
    const reference = saturate(depth);
    let sum = 0;
    for (const [tx, ty] of CASCADE_FILTER_TAPS) {
      sum += compare(j, u + tx / CASCADE_SIZE_PX, v + ty / CASCADE_SIZE_PX, reference);
    }
    return sum / CASCADE_FILTER_TAPS.length;
  };
  const d = dot(fromCameraM, { x: ax, y: ay, z: az });
  const c = cascadeLitFraction(
    d,
    uniforms.cascadeFarM,
    uniforms.cascadeFadeStartM,
    (j) => coversWithFootprint(pointIn(j)),
    sample,
  );
  return weightedCascadeLit(weight, c);
}
