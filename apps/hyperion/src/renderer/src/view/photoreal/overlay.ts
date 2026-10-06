/**
 * The symbology over a photorealistic image (plan R07, T16.a; Design notes 16–17): R02's draw
 * list with every mark cased, drawn by R02's renderer as the frame's last canvas pass, over the
 * tone-mapped image.
 *
 * @remarks
 * Every mark over the image is stroked twice, a `--surface-0` casing of {@link CASING_PX} on each
 * side beneath its coloured stroke, never a blur or a glow (the guide's "Outlines for symbology"),
 * so that it carries its own dark surface over any part of the image; the image is never dimmed
 * under it. The wireframe style's list cases every batch but the hull edges, whose own faces hide
 * the stars behind them; over the image they are cased like every mark, and the hull faces'
 * slope bias covers their cased width (`HULL_OCCLUDER_BIAS`, the UX decisions, item 12). Until R11
 * draws them, rings stay R02's ellipses and craft R02's hull outlines, cased, never omitted
 * (Design note 17). The star sprites are left out: the image holds the stars. The occluders stay,
 * so that a mark behind a body or a hull stays hidden.
 */
import type { Viewport } from "../camera/projection";
import type { FrameSubmission } from "../engine/types";
import {
  CASING_PX,
  type DrawCamera,
  type LineBatch,
  type WireframeDrawList,
} from "../wireframe/drawList";
import type { WireframeRenderer } from "../wireframe/submit";
import { PHOTOREAL_PASS_LABELS } from "./passes";

/** A batch as the photorealistic style strokes it: cased by at least {@link CASING_PX}. */
function cased(line: LineBatch): LineBatch {
  return line.casingWidthPx >= CASING_PX ? line : { ...line, casingWidthPx: CASING_PX };
}

/**
 * R02's draw list as the overlay draws it over the image: every line batch cased in its
 * `--surface-0` casing colour at {@link CASING_PX}, the hull edges included, and no star sprite.
 */
export function overlayDrawList(list: WireframeDrawList): WireframeDrawList {
  return { ...list, lines: list.lines.map(cased), sprites: [] };
}

/**
 * The symbology's canvas pass (Design note 8's "symbology cased over the result"): the overlay's
 * draw list packed and bound by R02's renderer, labelled `symbology` for the pass timer, loading
 * the tone-mapped image beneath it through the canvas's sRGB view.
 *
 * @param viewport - The canvas's size, px: the symbology is drawn at the display's resolution,
 *   never at the scene target's internal one.
 */
export function overlaySubmission(
  renderer: Pick<WireframeRenderer, "frame">,
  list: WireframeDrawList,
  camera: DrawCamera,
  viewport: Viewport,
): FrameSubmission {
  return {
    ...renderer.frame(overlayDrawList(list), camera, viewport),
    label: PHOTOREAL_PASS_LABELS.symbology,
    colourLoad: "load",
  };
}
