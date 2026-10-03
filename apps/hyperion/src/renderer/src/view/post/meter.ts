/**
 * The meter classes the photorealistic style writes in the HDR target's alpha, and the weight each
 * meter gives them (plan R07, Design note 10).
 *
 * @remarks
 * Shared file. R06.T13.e's disc pass writes {@link METER_CLASS}.hostDisc; R08's and R11's passes
 * write or keep the class beneath them. Later plans extend this file and never redeclare these
 * exports.
 */

/**
 * The operator's meter: `average` (`AVG`) weighs every class but the host disc, `lit` (`LIT`) only a
 * body's lit side, `dark` (`DARK`) only its unlit side.
 */
export type MeterMode = "average" | "lit" | "dark";

/**
 * The class each opaque pass writes in the HDR target's alpha, an exact small integer.
 *
 * @remarks
 * The target clears to alpha 1, so a pixel that no opaque pass writes meters as `other`. Blended
 * passes keep the destination alpha (R01's blend modes), so the class beneath them survives.
 */
export const METER_CLASS = { hostDisc: 0, other: 1, litBody: 2, unlitBody: 3 } as const;

/** One of {@link METER_CLASS}'s values. */
export type MeterClass = (typeof METER_CLASS)[keyof typeof METER_CLASS];

/**
 * The integer weight of each meter class under `mode`, indexed by class: the histogram kernel adds
 * this many counts for a pixel of that class.
 *
 * @remarks
 * Every mode gives the host disc 0, so the exposure does not step when the star enters the frame
 * (brainstorm, "Exposure and tone mapping").
 */
export function meterWeights(mode: MeterMode): readonly [number, number, number, number] {
  let weights: readonly [number, number, number, number];
  switch (mode) {
    case "average":
      weights = [0, 1, 1, 1];
      break;
    case "lit":
      weights = [0, 0, 1, 0];
      break;
    case "dark":
      weights = [0, 0, 0, 1];
      break;
  }
  return weights;
}
