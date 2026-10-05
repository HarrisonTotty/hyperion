import { describe, expect, it } from "vitest";

import { runPose, startRun } from "../../displays/view/viewRun";
import { normalise, vec3 } from "../../geometry/vec3";
import type { ColourTokens } from "../../spatial/paint";
import { DEFAULT_FOV_DEG } from "../camera/projection";
import { precisionScene } from "../scenes/precision";
import { buildWireframeDrawList, type SpriteStar } from "../wireframe/drawList";
import { internalViewport, spritesAtScale } from "./internalScale";

const CANVAS = { widthPx: 1280, heightPx: 720 };

/** Stand-in colours, one distinct string per token, as `readTokens` would read them. */
const TOKENS: ColourTokens = {
  text: "colour-text",
  textMuted: "colour-text-muted",
  accent: "colour-accent",
  target: "colour-target",
  line: "colour-line",
  surface0: "colour-surface-0",
};

/** Stars all round the sky, on a spiral, a few of them in any view. */
function skyStars(): SpriteStar[] {
  const stars: SpriteStar[] = [];
  for (let i = 0; i < 400; i += 1) {
    const z = 1 - (2 * (i + 0.5)) / 400;
    const r = Math.sqrt(1 - z * z);
    const phi = i * 2.399963;
    stars.push({
      id: `star ${String(i)}`,
      direction: normalise(vec3(r * Math.cos(phi), r * Math.sin(phi), z)),
      illuminanceRgbLx: [1e-8, 2e-8, 3e-8],
    });
  }
  return stars;
}

/** The draw list's sprites at a viewport, for the precision scene's first frame. */
function spritesAt(viewport: { readonly widthPx: number; readonly heightPx: number }) {
  const run = startRun(precisionScene());
  return buildWireframeDrawList(
    run.scene,
    { pose: runPose(run), fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180 },
    viewport,
    TOKENS,
    {
      lowSetting: false,
      ev100: -1,
      selection: null,
      destination: null,
      remPx: 16,
      skyStars: skyStars(),
    },
  ).sprites;
}

describe("internalViewport", () => {
  it("is the render resolution itself at a scale of 1", () => {
    expect(internalViewport(CANVAS, 1)).toBe(CANVAS);
  });

  it("scales both axes by the width's factor, keeping at least a pixel", () => {
    expect([
      internalViewport(CANVAS, 0.5),
      internalViewport({ widthPx: 1, heightPx: 1 }, 0.5),
    ]).toEqual([
      { widthPx: 640, heightPx: 360 },
      { widthPx: 1, heightPx: 1 },
    ]);
  });
});

describe("spritesAtScale", () => {
  it("places the canvas's stars where the internal viewport's list would, with their light per pixel", () => {
    const to = internalViewport(CANVAS, 0.61);
    const scaled = new Map(spritesAtScale(spritesAt(CANVAS), CANVAS, to).map((s) => [s.id, s]));
    const truth = spritesAt(to).filter((s) => scaled.has(s.id));
    expect(truth.length > 3).toBe(true);
    const worst = Math.max(
      ...truth.map((s) => {
        const at = scaled.get(s.id);
        return at === undefined
          ? Infinity
          : Math.max(
              Math.abs(at.xPx - s.xPx),
              Math.abs(at.yPx - s.yPx),
              Math.abs(at.exposedRgb[1] / s.exposedRgb[1] - 1),
            );
      }),
    );
    expect(worst < 1e-6).toBe(true);
  });

  it("returns the sprites themselves where the viewports are one", () => {
    const sprites = spritesAt(CANVAS);
    expect(spritesAtScale(sprites, CANVAS, { ...CANVAS })).toBe(sprites);
  });
});
