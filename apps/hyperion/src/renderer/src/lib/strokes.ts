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
 * the view). At 2 or more a stroke scores within 1% of its pair under exact area coverage, as the
 * view's ramp gives it; Chromium's 2D canvas peaks at about 15/16 for a 2 px line at 45° (6.45:1 for
 * `--text-muted`, as R07.T16.f's capture check reads it), still above 6:1 on `--surface-0`. Nothing
 * changes at a ratio of 2 or more.
 *
 * The view's draw list (R07.T16.d), the spatial displays' painter and the DOM's SVG strokes
 * (R07.T16.f, through {@link strokeProperties}) all take their widths from here.
 */
import { useSyncExternalStore } from "react";

import { SYMBOL_STROKE_PX } from "../spatial/symbols";
import { rootRemPx } from "./useElementSize";

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
 * The least space between the selection's bracket's centreline and the destination's chevrons'
 * apices about one mark, device px: a mark's outline and one casing, {@link markStrokeDevicePx}
 * + {@link CASING_PX} × {@link lineScale} (decision-r07-t16d-followups, item 2;
 * decision-r07-quality-and-destination, Q2). It is 4 at ratios of 0.78125 and 1, 5 at 2 and 7.5
 * at 3.
 *
 * @remarks
 * The least gap that keeps the chevrons' casing, drawn after the bracket, off the bracket's
 * full-coverage core: the casing reaches m ÷ 2 + casing + 0.5 px from its centreline, and the core
 * lies within m ÷ 2 − 0.5 px of the bracket's. The chevrons come nearest the bracket near its arms'
 * inner ends, at (H ÷ 3 + the gap) ÷ √2 for a bracket of half-size H, which is the gap or more
 * wherever H is at least 1.25 times it. The view's draw list and the spatial displays both take
 * it, so that one pair of marks is drawn ship-wide.
 */
export function minReticleGapDevicePx(devicePixelRatio: number): number {
  return markStrokeDevicePx(devicePixelRatio) + CASING_PX * lineScale(devicePixelRatio);
}

/**
 * How many outline shifts δ ({@link markShiftDevicePx}) a ringed circle's ring moves out: 3, its
 * disc moving one, so that the disc's hole and the gap round it both stay (decision-thin-line-
 * contrast, item 2). The view's symbology and the spatial displays' painter both take it.
 */
export const RING_SHIFTS = 3;

/**
 * How many outline shifts δ a reticle moves out: 4, a ringed circle's growth, its ring moved out by
 * {@link RING_SHIFTS} δ and widened by δ (decision-thin-line-contrast, item 2). The view's symbology
 * and the spatial displays' painter both take it.
 */
export const RETICLE_SHIFTS = 4;

/**
 * A spatial display's marks' strokes at a device-pixel ratio, CSS px, which its reticles and the
 * labels beside them are placed by (R07.T16.f and T16.h).
 */
export interface ReticleStrokesCss {
  /** The outline shift δ: {@link markShiftDevicePx} ÷ the ratio. */
  readonly shiftPx: number;
  /** A mark's outline width: {@link markStrokeDevicePx} ÷ the ratio. */
  readonly markStrokePx: number;
  /** The reticles' least gap about one mark: {@link minReticleGapDevicePx} ÷ the ratio. */
  readonly minGapPx: number;
}

/**
 * The marks' strokes at a device-pixel ratio, CSS px: δ 0.53, 0.25, 0 and 0, the outline 2.56, 2,
 * 1.5 and 1.5, and the least gap 5.12, 4, 2.5 and 2.5, at 0.78125, 1, 2 and 3.
 *
 * @remarks
 * `SpatialView` gives the least gap to its draw list, so that its destination's chevrons stand
 * where the view's do, and all three to `placeLabels`, so that a label's text starts 0.125 rem
 * outside the outer edge of the outermost reticle about its mark, as on every display
 * (decision-r07-quality-and-destination, Q3).
 */
export function reticleStrokesCssPx(devicePixelRatio: number): ReticleStrokesCss {
  const ratio = ratioOf(devicePixelRatio);
  return {
    shiftPx: markShiftDevicePx(ratio) / ratio,
    markStrokePx: markStrokeDevicePx(ratio) / ratio,
    minGapPx: minReticleGapDevicePx(ratio) / ratio,
  };
}

/**
 * The root's custom properties that size the DOM's SVG strokes at a device-pixel ratio (R07.T16.f):
 * the line scale and the mark stroke, each as CSS px, since a stylesheet's widths are CSS px.
 */
export interface StrokeProperties {
  /** CSS px drawn for each CSS px of a line's width: {@link lineScale} ÷ the ratio, unitless. */
  readonly "--line-scale": string;
  /** A mark's outline, CSS px: {@link markStrokeDevicePx} ÷ the ratio, with its `px`. */
  readonly "--mark-stroke": string;
}

/**
 * The stroke properties at a device-pixel ratio: `2.56` and `2.56px` at 0.78125, `2` and `2px` at
 * 1, and `1` and `1.5px` at 2 and 3, the values `styles.css`'s `:root` holds.
 */
export function strokeProperties(devicePixelRatio: number): StrokeProperties {
  const ratio = ratioOf(devicePixelRatio);
  return {
    "--line-scale": String(lineScale(ratio) / ratio),
    "--mark-stroke": `${String(markStrokeDevicePx(ratio) / ratio)}px`,
  };
}

/**
 * Calls `onChange` each time the window's device-pixel ratio changes, as when the window moves to
 * another display or the page is zoomed, until the returned function is called.
 *
 * @remarks
 * A `matchMedia` query for the ratio in force fires once when the ratio leaves it, so each change
 * arms a query for the new ratio.
 */
function watchDevicePixelRatio(onChange: () => void): () => void {
  let disarm: (() => void) | null = null;
  function arm(): void {
    const list = window.matchMedia(`(resolution: ${String(window.devicePixelRatio)}dppx)`);
    const changed = (): void => {
      list.removeEventListener("change", changed);
      arm();
      onChange();
    };
    list.addEventListener("change", changed);
    disarm = () => {
      list.removeEventListener("change", changed);
    };
  }
  arm();
  return () => {
    disarm?.();
  };
}

/**
 * Sets {@link strokeProperties} of the window's device-pixel ratio on `root`, now and each time the
 * ratio changes, until the returned function is called.
 *
 * @remarks
 * `main.tsx` calls it on the document's root before the first render, so that the stylesheet's SVG
 * strokes are never drawn under 2 device px.
 */
export function watchStrokeProperties(root: HTMLElement): () => void {
  const apply = (): void => {
    for (const [name, value] of Object.entries(strokeProperties(window.devicePixelRatio))) {
      root.style.setProperty(name, value);
    }
  };
  apply();
  return watchDevicePixelRatio(apply);
}

/** What a drawing in a stroke's own units needs to place an outline in device px. */
export interface StrokeMetrics {
  /** Device px in one CSS px. */
  readonly devicePixelRatio: number;
  /** CSS px in one `rem` at the current interface scale. */
  readonly remPx: number;
}

function windowRatio(): number {
  return window.devicePixelRatio;
}

function subscribeRem(onChange: () => void): () => void {
  window.addEventListener("resize", onChange);
  return () => {
    window.removeEventListener("resize", onChange);
  };
}

/**
 * The device-pixel ratio and the root's rem, kept current as either changes, for a drawing that
 * converts δ ({@link markShiftDevicePx}) into its own units, as `LegendSymbol` does.
 *
 * @remarks
 * The ratio is watched through a `matchMedia` resolution query, and the rem on each window resize,
 * which is what a change of interface scale fires (as `useElementSize` measures it).
 */
export function useStrokeMetrics(): StrokeMetrics {
  const devicePixelRatio = useSyncExternalStore(watchDevicePixelRatio, windowRatio);
  const remPx = useSyncExternalStore(subscribeRem, rootRemPx);
  return { devicePixelRatio, remPx };
}
