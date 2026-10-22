/**
 * How far an `rgba16float` write may move a channel the composite forms in `f32`, under the
 * target's probed rounding (plan R08, R08.T8.c; `decision-r08-t8-leans.md` §8): R01's Design note
 * 22 found that Gen9 truncates `rgba16float` writes toward zero, the RTX 3080 does the same (T9.i's
 * probe), and SwiftShader rounds to nearest. Low's pick check (`atmosphere.ts`'s `checkLowPick`)
 * holds each surface channel to this bound.
 */

import type { TargetRounding } from "../view/engine/status";

/** The half float's least normal, 2⁻¹⁴. */
const HALF_LEAST_NORMAL = 2 ** -14;

/** The half float's subnormal spacing, 2⁻²⁴: its step below its least normal. */
const HALF_SUBNORMAL_STEP = 2 ** -24;

/** `f32`'s two roundings of colour × alpha + rgb, as a share of |colour × alpha| + |rgb|: 2⁻²³. */
const F32_TWO_ROUNDINGS = 2 ** -23;

/** A write's slack, in steps of the half float, under a value's magnitude and over it. */
export interface WriteSlack {
  readonly under: number;
  readonly over: number;
}

/**
 * How far a write may move a value under the target's rounding, in steps (§8): rounded to
 * nearest, half a step either way; toward zero, up to one step under the magnitude and none over,
 * so that a write that rounds away from zero still fails; unknown, one step either way.
 */
export function writeSlackSteps(rounding: TargetRounding): WriteSlack {
  let slack: WriteSlack;
  switch (rounding) {
    case "nearest":
      slack = { under: 0.5, over: 0.5 };
      break;
    case "toward-zero":
      slack = { under: 1, over: 0 };
      break;
    case "unknown":
      slack = { under: 1, over: 1 };
      break;
  }
  return slack;
}

/**
 * The half float's step at `value` (§8): 2⁻¹⁰ of its binade's base, 2⁻²⁴ among the subnormals;
 * rounded to nearest it is taken, as before §8, as 2⁻¹⁰ of the value plus 2⁻²⁴, which is never
 * smaller.
 */
export function halfStepAt(value: number, rounding: TargetRounding): number {
  const magnitude = Math.abs(value);
  if (rounding === "nearest") {
    return 2 ** -10 * magnitude + HALF_SUBNORMAL_STEP;
  }
  return magnitude < HALF_LEAST_NORMAL
    ? HALF_SUBNORMAL_STEP
    : 2 ** (Math.floor(Math.log2(magnitude)) - 10);
}

/**
 * A surface channel's written `actual` against colour × alpha + rgb as a fraction of its bound, 0
 * where it lands on the value: the write's slack on the side the output lies, in half-float steps
 * at the expected value ({@link halfStepAt}), plus `f32`'s two roundings of its terms either way.
 *
 * @param colour - The surface's colour times the march's alpha, the term `f32` forms first.
 * @param rgb - The march's in-scatter times the composite's factor.
 */
export function surfaceChannelOff(
  actual: number,
  colour: number,
  rgb: number,
  rounding: TargetRounding,
): number {
  const expected = colour + rgb;
  const slack = writeSlackSteps(rounding);
  const terms = F32_TWO_ROUNDINGS * (Math.abs(colour) + Math.abs(rgb));
  const step = halfStepAt(expected, rounding);
  // Over the expected magnitude, or under it: the write's slack differs by side.
  const outward = (actual - expected) * (expected < 0 ? -1 : 1);
  const bound = (outward > 0 ? slack.over : slack.under) * step + terms;
  return outward === 0 ? 0 : Math.abs(outward) / bound;
}
