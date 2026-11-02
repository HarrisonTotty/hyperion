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
 * body's lit side, `dark` (`DARK`) only its unlit side, each from the image's histogram; and
 * `incident` (`INCIDENT`) reads the light falling on a body from the scene's host discs, with no
 * histogram (`view/lighting/incident.ts`; ux-calls-view, item 3b, the owner's delegated call,
 * 2026-11-01).
 */
export type MeterMode = "average" | "lit" | "dark" | "incident";

/** A meter that reads the image's histogram: every {@link MeterMode} but `incident`. */
export type HistogramMeter = Exclude<MeterMode, "incident">;

/** The histogram meter `mode` is, or `null` for `incident`, whose frames take no histogram. */
export function histogramMeterOf(mode: MeterMode): HistogramMeter | null {
  return mode === "incident" ? null : mode;
}

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
 * Every mode gives the host disc 0, so the disc's own light never steps the exposure as the star
 * enters the frame (brainstorm, "Exposure and tone mapping"); its glare's veil, which the histogram
 * weighs, does in a camera view, whole once the disc touches the frame (`histogram.wgsl`).
 */
export function meterWeights(mode: HistogramMeter): readonly [number, number, number, number] {
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

/** The weights of Rec. 709 luminance from linear red, green and blue (ITU-R BT.709-6, item 3.2). */
export const METER_LUMA: readonly [number, number, number] = [0.2126, 0.7152, 0.0722];

/** Rec. 709 luminance of a linear colour, the meter's ({@link METER_LUMA}). */
export function meterLuma(rgb: readonly [number, number, number]): number {
  return METER_LUMA[0] * rgb[0] + METER_LUMA[1] * rgb[1] + METER_LUMA[2] * rgb[2];
}
