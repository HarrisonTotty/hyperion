/**
 * The shadow cascades as the cluster cull's passes (plan R14, R14.T1.b and T1.c), and T1.c's rule:
 * the casters of a cascade whose texels no receiver reads.
 *
 * @remarks
 * A receiver at view depth d (`cascadeShadow.wgsl`'s `cascade_lit_fraction`) takes slice i, the
 * first whose far end C₍ᵢ₊₁₎ d does not pass. It samples the first cascade from i that covers it
 * with its footprint, and over i's blend band, (fadeStart_i, C₍ᵢ₊₁₎], the first covering cascade
 * after i too. So a receiver samples cascade k only (a) at d in (fadeStart₍ₖ₋₁₎, C₍ₖ₊₁₎], any
 * d ≤ C₁ for k = 0, or (b) at d ≤ fadeStart₍ₖ₋₁₎ where cascade k − 1 does not cover it.
 *
 * A receiver R reads a texel a caster writes only where R's moved point (R along its normal by the
 * cascade's normal offset o_k × saturate(1 − N·L), at most o_k) lies within 1 + √2 texels t_k across
 * the light of a texel centre the caster covers: a tap is a texel from the point, and its 2 × 2
 * PCF's texels' centres within a texel of the tap along each axis. Under `castsInto`'s premise, that
 * a terrain caster lies within the caster reach ℓ of any receiver it shadows, R lies no more than ℓ
 * farther from the light than the caster, and to be shadowed at all its moved point lies farther.
 * With the caster's sphere at c of radius ρ, R then lies within ρ + {@link RECEIVER_READ_TEXELS}
 * t_k + o_k of c across the light, and at view depths in [c·v − ρ − ℓ max(0, s) − m_k,
 * c·v + ρ + ℓ max(0, −s) + m_k], s the light's axis on the view axis v and m_k = 2 o_k +
 * {@link RECEIVER_READ_TEXELS} t_k.
 *
 * So a caster is refused from cascade k where that interval lies wholly beyond C₍ₖ₊₁₎, or wholly at
 * or below fadeStart₍ₖ₋₁₎ with every such R, moved by o₍ₖ₋₁₎, inside cascade k − 1's covered
 * square, half-side r₍ₖ₋₁₎ − {@link CASCADE_FOOTPRINT_TEXELS} t₍ₖ₋₁₎ (`cascade_covers`). Either way
 * no receiver reads a texel it writes: the layers differ only where no receiver samples, and the
 * image not at all. The rule is exact under the premise, as `castsInto` is; non-terrain casters do
 * not take it.
 *
 * A far cascade that a later frame may hold (plan R14, R14.T6.b; `Cascade.holdSlackM` σ above 0)
 * has its far end widened by σ, and refuses nothing as covered below: its frames' receivers stand
 * up to σ off in view depth from its own frame's (`holdsFarCascade` holds it no longer), and the
 * cascade below may be rendered again, or held from another frame in other light axes, meanwhile.
 */

import type { Vec3 } from "../../geometry/vec3";
import { CASCADE_FOOTPRINT_TEXELS, type CascadeIndex } from "../terrain/shadows/cascadeShadow";
import type { CascadeSet } from "../terrain/shadows/cascades";
import type { CascadeCullPass, CullReceivers } from "./cullPass";
import type { Stride } from "./templates";

/**
 * How far across the light of a texel centre a receiver's moved point may read it, texels: 1 + √2
 * (a tap's texel and its 2 × 2 PCF's diagonal), taken whole.
 */
export const RECEIVER_READ_TEXELS = 3;

/** The cascades' indices, nearest first. */
const CASCADES: readonly [CascadeIndex, CascadeIndex, CascadeIndex, CascadeIndex] = [0, 1, 2, 3];

/**
 * Cascade `index`'s receivers as the cull takes them (see the module's remarks).
 *
 * @param set - The view's cascades this frame.
 * @param lightOnViewAxis - s, the light's axis z_L (towards the light) on the view axis.
 */
export function cascadeCullReceivers(
  set: CascadeSet,
  index: CascadeIndex,
  lightOnViewAxis: number,
): CullReceivers {
  const cascade = set.cascades[index];
  const marginM = 2 * cascade.normalOffsetM + RECEIVER_READ_TEXELS * cascade.texelM;
  const reachM = set.casterReachM;
  const below = index === 0 ? undefined : set.cascades[index - 1];
  const slackM = cascade.holdSlackM;
  return {
    depthM: [below?.fadeStartM ?? 0, cascade.farM + slackM],
    reachM: [
      reachM * Math.max(0, lightOnViewAxis) + marginM,
      reachM * Math.max(0, -lightOnViewAxis) + marginM,
    ],
    coveredBelow:
      below === undefined || slackM > 0
        ? null
        : {
            centreM: [below.centreFromCameraLightM.x, below.centreFromCameraLightM.y],
            halfSideM: below.radiusM - CASCADE_FOOTPRINT_TEXELS * below.texelM,
            reachM:
              RECEIVER_READ_TEXELS * cascade.texelM + cascade.normalOffsetM + below.normalOffsetM,
          },
  };
}

/**
 * `set`'s four cascades as the cull's passes, nearest first.
 *
 * @param axes - The light's axes x_L, y_L and z_L in the instances' frame: `set.basis`, turned.
 * @param viewAxis - The view axis, a unit vector in that frame.
 * @param strideCap - The coarsest stride a pass may take.
 * @param receivers - Whether T1.c's receivers' cull is on.
 */
export function cascadeCullPasses(
  set: CascadeSet,
  axes: readonly [Vec3, Vec3, Vec3],
  viewAxis: Vec3,
  strideCap: Stride,
  receivers: boolean,
): readonly [CascadeCullPass, CascadeCullPass, CascadeCullPass, CascadeCullPass] {
  const z = axes[2];
  const s = z.x * viewAxis.x + z.y * viewAxis.y + z.z * viewAxis.z;
  const [a, b, c, d] = CASCADES.map((index): CascadeCullPass => {
    const cascade = set.cascades[index];
    return {
      kind: "cascade",
      axes,
      centreFromCameraLightM: cascade.centreFromCameraLightM,
      radiusM: cascade.radiusM,
      depthFarM: cascade.depthFarM,
      toleranceM: cascade.texelM,
      strideCap,
      receivers: receivers ? cascadeCullReceivers(set, index, s) : null,
    };
  });
  if (a === undefined || b === undefined || c === undefined || d === undefined) {
    throw new Error("a cascade set has four cascades");
  }
  return [a, b, c, d];
}
