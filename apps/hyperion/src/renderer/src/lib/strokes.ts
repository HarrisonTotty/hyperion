/**
 * The device widths at which the console draws its lines and outlines (decision-thin-line-contrast,
 * item 2; plan R07, R07.T16.d): a floor of 2 device pixels, so that every line and outline that a
 * canvas, an SVG or a view draws reaches its colour pair's contrast as drawn.
 *
 * @remarks
 * A width the guide gives in `px` is in CSS pixels. A line, a casing or a dash's length is drawn at
 * that width times {@link lineScale}, the larger of the device-pixel ratio and 2, so that lines keep
 * their ratios to one another. The outline of a symbol, a reticle or another mark sized in `rem`
 * is drawn at {@link markStrokeDevicePx}, the larger of its 1.5 CSS px and 2 device px, and widens
 * outward by {@link markShiftDevicePx}, so that every hole it encloses stays as built. Below 2 device
 * pixels an antialiased stroke's brightest pixel covers as little as half its width at its worst
 * position on the grid, and a `--text-muted` orbit then falls below 6:1 (4.11:1 at 1 device px in
 * the view). At 2 or more a stroke scores within 1% of its pair. Nothing changes at a ratio of 2 or
 * more.
 */
import { SYMBOL_STROKE_PX } from "../spatial/symbols";

/** The least width of a line, casing or outline that a canvas, an SVG or a view draws, device px. */
export const MIN_STROKE_DEVICE_PX = 2;

/**
 * The width of the `--surface-0` casing on each side of a stroke, CSS px: 1, the guide's casing
 * over a raster, which every mark over the image takes (the guide's drafted "Outlines for
 * symbology", R02.T2.b item 3; R02 Design note 9). Drawn at {@link lineScale} device px for each,
 * as every line width and dash is (R07.T16.d).
 */
export const CASING_PX = 1;

/** A ratio fit to scale by: one that is not a positive number is taken as 1. */
function ratioOf(devicePixelRatio: number): number {
  return devicePixelRatio > 0 && Number.isFinite(devicePixelRatio) ? devicePixelRatio : 1;
}

/**
 * The device pixels drawn for each CSS pixel of a line's or a casing's width, or of a dash's
 * length: the larger of the device-pixel ratio and {@link MIN_STROKE_DEVICE_PX}. It is 2 at
 * ratios of 0.78125, 1 and 2, and 3 at 3.
 */
export function lineScale(devicePixelRatio: number): number {
  return Math.max(MIN_STROKE_DEVICE_PX, ratioOf(devicePixelRatio));
}

/**
 * The width of a symbol's, a reticle's or another `rem`-sized mark's outline, device px: the larger
 * of its 1.5 CSS px (`SYMBOL_STROKE_PX`) and {@link MIN_STROKE_DEVICE_PX}. It is 2 at ratios of
 * 0.78125 and 1, 3 at 2 and 4.5 at 3.
 */
export function markStrokeDevicePx(devicePixelRatio: number): number {
  return Math.max(MIN_STROKE_DEVICE_PX, SYMBOL_STROKE_PX * ratioOf(devicePixelRatio));
}

/**
 * How far a mark's outline moves out, device px (δ): half what {@link markStrokeDevicePx} adds to
 * the outline's 1.5 CSS px, so that its inner edge stays where a 1.5 CSS px outline's would be. It
 * is 0.41 at a ratio of 0.78125, 0.25 at 1, and 0 from 4/3 up.
 *
 * @remarks
 * A symbol's line moves out by δ; a ringed circle's disc by δ and its ring by 3δ, so that the
 * disc's hole and the gap round it both stay; a reticle by 4δ, the ringed circle's growth.
 */
export function markShiftDevicePx(devicePixelRatio: number): number {
  const ratio = ratioOf(devicePixelRatio);
  return (markStrokeDevicePx(ratio) - SYMBOL_STROKE_PX * ratio) / 2;
}

/**
 * The least space between the centrelines of two reticles drawn about one mark, the destination's
 * outside the selection's, device px: a mark's outline and one casing, {@link markStrokeDevicePx}
 * + {@link CASING_PX} × {@link lineScale} (decision-r07-t16d-followups, item 2). It is 4 at ratios
 * of 0.78125 and 1, 5 at 2 and 7.5 at 3.
 *
 * @remarks
 * The least gap that keeps the outer reticle's casing, drawn after the inner one, off the inner
 * one's full-coverage core: the casing reaches m ÷ 2 + casing + 0.5 px from its centreline, and the
 * core lies within m ÷ 2 − 0.5 px of the inner one's. The view's draw list and the spatial displays
 * both take it, so that one pair of reticles is drawn ship-wide and is never told by colour alone.
 */
export function minReticleGapDevicePx(devicePixelRatio: number): number {
  return markStrokeDevicePx(devicePixelRatio) + CASING_PX * lineScale(devicePixelRatio);
}
