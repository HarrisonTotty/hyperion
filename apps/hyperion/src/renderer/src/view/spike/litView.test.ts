import { describe, expect, it } from "vitest";

import { exposureReading } from "../../displays/view/viewRun";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { controlEv100 } from "../photometry/exposure";
import type { FrameSubmission } from "../engine/types";
import { TERRAIN_PASS_LABEL } from "../terrain/gpu/material";
import { LIT_VIEW_DISPLAY_LABEL, LitView, renderSizeOf } from "./litView";

const SIZE = { widthPx: 64, heightPx: 36 };
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

describe("the spike's lit view", () => {
  it("shows its exposure as EV100 15.0 MAN", async () => {
    const lit = new LitView(await countingRenderEngine(), "spike", SIZE, null);
    expect(controlEv100(lit.exposure)).toBeCloseTo(15, 12);
    expect(exposureReading(lit.exposure)).toBe("EV100 15.0 MAN");
  });

  it("draws the terrain into its HDR target, then AgX of that target into the output", async () => {
    const engine = await countingRenderEngine();
    const lit = new LitView(engine, "spike", SIZE, null);
    await lit.ready();
    const frames: FrameSubmission[] = [];
    lit.render({ render: (frame) => frames.push(frame) }, IDENTITY, IDENTITY, []);
    expect(engine.targetFrames.at(-1)?.label).toBe(TERRAIN_PASS_LABEL);
    expect(frames).toHaveLength(1);
    expect(frames[0]?.label).toBe(LIT_VIEW_DISPLAY_LABEL);
    expect(frames[0]?.draws[0]?.textures).toEqual({ hdrColour: lit.target.colour });
  });

  it("leaves the display pass out until its material is ready", async () => {
    const lit = new LitView(await countingRenderEngine(), "spike", SIZE, null);
    const frames: FrameSubmission[] = [];
    lit.render({ render: (frame) => frames.push(frame) }, IDENTITY, IDENTITY, []);
    expect(frames[0]?.draws).toEqual([]);
  });

  it("pre-exposes by R02's scale of EV100 15", async () => {
    const lit = new LitView(await countingRenderEngine(), "spike", SIZE, null);
    // exposureScale(15) = 1 ÷ (1.2 × 2¹⁵), the saturation-based scale with q = 0.65 (R02).
    expect(lit.exposureScale).toBeCloseTo(1 / ((78 / 65) * 2 ** 15), 15);
  });
});

describe("the lit view's render size", () => {
  it("draws the low setting at 720p at the view's aspect", () => {
    expect(renderSizeOf({ widthPx: 1920, heightPx: 1080 }, 720)).toEqual({
      widthPx: 1280,
      heightPx: 720,
    });
  });

  it("draws at the view's own size with no render height", () => {
    expect(renderSizeOf({ widthPx: 1920, heightPx: 1080 }, null)).toEqual({
      widthPx: 1920,
      heightPx: 1080,
    });
  });

  it("never draws above the presented size", () => {
    expect(renderSizeOf({ widthPx: 640, heightPx: 360 }, 720)).toEqual({
      widthPx: 640,
      heightPx: 360,
    });
  });

  it("makes its target at the render size", async () => {
    const engine = await countingRenderEngine();
    const lit = new LitView(engine, "spike", { widthPx: 1920, heightPx: 1080 }, 720);
    expect(lit.renderSize).toEqual({ widthPx: 1280, heightPx: 720 });
    expect(engine.textureSpecs.at(-2)?.size).toEqual([1280, 720]);
  });
});
