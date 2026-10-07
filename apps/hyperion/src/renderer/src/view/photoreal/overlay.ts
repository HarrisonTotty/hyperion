/**
 * The symbology over a photorealistic image (plan R07, T16.a; Design notes 16–17): R02's draw
 * list with every mark cased, drawn by R02's renderer as the frame's last canvas pass, over the
 * tone-mapped image.
 *
 * @remarks
 * Every mark over the image is stroked twice, a `--surface-0` casing of {@link CASING_PX} CSS px
 * on each side, drawn at the list's stroke scale, beneath its coloured stroke, never a blur or a
 * glow (the guide's "Outlines for symbology"), so that it carries its own dark surface over any
 * part of the image; the image is never dimmed under it. The wireframe style's list cases every
 * batch but the hull edges, whose own faces hide the stars behind them; over the image they are
 * cased like every mark, and the hull faces' slope term covers their cased width at every scale
 * (`occluderSlopePx`; the UX decisions, item 12; R07.T16.d). Until R11
 * draws them, rings stay R02's ellipses, cased, never omitted (Design note 17). Until hull art
 * exists, a craft is R02's hull outline, cased, on a `--surface-0` silhouette of its opaque faces,
 * so that nothing behind them shows through: the one exception to the image's never being dimmed
 * under symbology (Design notes 16 and 17; decision-r07-t16a, item 3; R07.T16.e). Its windows are
 * in no mesh, and hide nothing. The star sprites are left out: the image holds the stars. The
 * occluders stay, so that a mark behind a body or a hull stays hidden.
 */
import type { ColourTokens } from "../../spatial/paint";
import type { Viewport } from "../camera/projection";
import type { FrameSubmission } from "../engine/types";
import {
  CASING_PX,
  type DrawCamera,
  type LineBatch,
  type OccluderMesh,
  type WireframeDrawList,
} from "../wireframe/drawList";
import type { WireframeRenderer } from "../wireframe/submit";
import { PHOTOREAL_PASS_LABELS } from "./passes";

/**
 * A batch as the photorealistic style strokes it: cased by at least `casingPx`, device px.
 */
function cased(line: LineBatch, casingPx: number): LineBatch {
  return line.casingWidthPx >= casingPx ? line : { ...line, casingWidthPx: casingPx };
}

/**
 * R02's draw list as the overlay draws it over the image: every line batch cased in its
 * `--surface-0` casing colour at {@link CASING_PX} times the list's stroke scale, the hull edges
 * included; every hull's opaque faces filled in `--surface-0`, its silhouette (R07.T16.e); and no
 * star sprite.
 *
 * @param tokens - The tokens the list was built with, whose `--surface-0` its casings take: the
 *   silhouette is one surface with them (decision-r07-t16a, item 3).
 */
export function overlayDrawList(
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
): WireframeDrawList {
  const casingPx = CASING_PX * list.strokeScale;
  const silhouette = (mesh: OccluderMesh): OccluderMesh => ({ ...mesh, fill: tokens.surface0 });
  return {
    ...list,
    occluderMeshes: list.occluderMeshes.map(silhouette),
    lines: list.lines.map((line) => cased(line, casingPx)),
    sprites: [],
  };
}

/**
 * The symbology's canvas pass (Design note 8's "symbology cased over the result"): the overlay's
 * draw list packed and bound by R02's renderer, labelled `symbology` for the pass timer, loading
 * the tone-mapped image beneath it through the canvas's sRGB view. It draws the bodies' occluder
 * spheres, then the hulls' silhouettes, then the lines, after tone mapping, so that the meter and
 * the HDR scene are unchanged (R07.T16.e).
 *
 * @param tokens - The tokens the list was built with ({@link overlayDrawList}).
 * @param viewport - The canvas's size, px: the symbology is drawn at the display's resolution,
 *   never at the scene target's internal one.
 */
export function overlaySubmission(
  renderer: Pick<WireframeRenderer, "frame">,
  list: WireframeDrawList,
  tokens: Pick<ColourTokens, "surface0">,
  camera: DrawCamera,
  viewport: Viewport,
): FrameSubmission {
  return {
    ...renderer.frame(overlayDrawList(list, tokens), camera, viewport),
    label: PHOTOREAL_PASS_LABELS.symbology,
    colourLoad: "load",
  };
}
