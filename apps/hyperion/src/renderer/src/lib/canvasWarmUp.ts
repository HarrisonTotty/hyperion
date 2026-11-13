/**
 * The page's 2D canvases' GPU context made as the page starts (lane perf-page-long-tasks, part 2):
 * a throwaway canvas's first draw, before the view's engine starts compiling.
 *
 * @remarks
 * An accelerated 2D canvas draws through one GPU context of the page's, which Chromium makes at the
 * first draw of the first such canvas with a synchronous call to the GPU process. Made there by the
 * orbit map's first paint, as the run opened `SYSTEM` 1.7 s in, it waited for the GPU process,
 * busy compiling the engine's pipelines on SwiftShader: the paint's clear took 9,965 ms and its 26
 * ops 1.2 ms, every later paint under 3 ms. Made at the page's start, the context took 1.2 ms and
 * that first clear 0.1 ms. The canvas is never shown and is let go; the context stays the page's,
 * and every canvas draws through it as it would have, so nothing drawn changes.
 */

/**
 * The warm canvas's side, px: past Chromium's least accelerated 2D canvas (about 128 × 129 px), so
 * that its draw makes the GPU context rather than a software canvas's.
 */
export const WARM_CANVAS_PX = 512;

/** A 2D context as far as the warm-up draws on it. */
export interface WarmContext {
  fillRect(x: number, y: number, width: number, height: number): void;
}

/** A canvas as far as the warm-up makes and draws on it. */
export interface WarmCanvas {
  width: number;
  height: number;
  getContext(contextId: "2d"): WarmContext | null;
}

/** The part of a document the warm-up uses, so that a test can stand in for one. */
export interface WarmDocument {
  createElement(tagName: "canvas"): WarmCanvas;
}

/**
 * Makes the page's 2D GPU context now, by one texel's draw on a canvas of
 * {@link WARM_CANVAS_PX} square that is never shown; nothing where the page has no 2D context.
 */
export function warmCanvas2d(document: WarmDocument): void {
  const canvas = document.createElement("canvas");
  canvas.width = WARM_CANVAS_PX;
  canvas.height = WARM_CANVAS_PX;
  canvas.getContext("2d")?.fillRect(0, 0, 1, 1);
}
