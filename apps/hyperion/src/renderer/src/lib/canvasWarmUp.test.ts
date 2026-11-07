import { describe, expect, it } from "vitest";

import { WARM_CANVAS_PX, type WarmCanvas, warmCanvas2d } from "./canvasWarmUp";

/** A document whose canvases record their size and their draws, with a 2D context or none. */
function fakeDocument(hasContext: boolean): {
  readonly document: { createElement(tagName: "canvas"): WarmCanvas };
  readonly made: Array<{ readonly canvas: WarmCanvas; readonly draws: string[] }>;
} {
  const made: Array<{ readonly canvas: WarmCanvas; readonly draws: string[] }> = [];
  return {
    document: {
      createElement: () => {
        const draws: string[] = [];
        const canvas: WarmCanvas = {
          width: 300,
          height: 150,
          getContext: () =>
            hasContext
              ? {
                  fillRect: (x, y, width, height) => {
                    draws.push(
                      `fillRect ${String(x)} ${String(y)} ${String(width)} ${String(height)}`,
                    );
                  },
                }
              : null,
        };
        made.push({ canvas, draws });
        return canvas;
      },
    },
    made,
  };
}

describe("the 2D canvases' GPU context made as the page starts (lane perf-page-long-tasks)", () => {
  it("draws one texel on one canvas past the least accelerated size", () => {
    const { document, made } = fakeDocument(true);
    warmCanvas2d(document);
    expect(made.map(({ canvas, draws }) => [canvas.width, canvas.height, draws])).toEqual([
      [WARM_CANVAS_PX, WARM_CANVAS_PX, ["fillRect 0 0 1 1"]],
    ]);
    expect(WARM_CANVAS_PX * WARM_CANVAS_PX).toBeGreaterThan(128 * 129);
  });

  it("does nothing more where the page has no 2D context", () => {
    const { document, made } = fakeDocument(false);
    expect(() => {
      warmCanvas2d(document);
    }).not.toThrow();
    expect(made.map(({ draws }) => draws)).toEqual([[]]);
  });
});
