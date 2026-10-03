import { describe, expect, it } from "vitest";

import { exposureReading } from "../../displays/view/viewRun";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { controlEv100 } from "../photometry/exposure";
import type { DrawItem, FrameSubmission } from "../engine/types";
import { TERRAIN_PASS_LABEL } from "../terrain/gpu/material";
import {
  LIT_VIEW_ATMOSPHERE_LABEL,
  LIT_VIEW_DISPLAY_LABEL,
  type LitScene,
  LitView,
  renderSizeOf,
} from "./litView";

const SIZE = { widthPx: 64, heightPx: 36 };
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** A draw for a composite the test makes up; its handles are never drawn. */
const DUMMY_DRAW: DrawItem = {
  mesh: { kind: "mesh", name: "composite" },
  material: { kind: "material", name: "composite" },
  offsetFromCameraM: new Float32Array(3),
  uniforms: {},
  textures: {},
};

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

  it("lays an atmosphere over the terrain target into its sky target, and maps that", async () => {
    const engine = await countingRenderEngine();
    const lit = new LitView(engine, "spike", SIZE, null, true);
    await lit.ready();
    const frames: FrameSubmission[] = [];
    const seen: LitScene[] = [];
    const composite = engine.targetFrames.length;
    lit.render({ render: (frame) => frames.push(frame) }, IDENTITY, IDENTITY, [], (scene) => {
      seen.push(scene);
      return { ...DUMMY_DRAW, textures: { marker: scene.colour } };
    });
    const labels = engine.targetFrames.slice(composite).map((frame) => frame.label);
    expect(labels).toEqual([TERRAIN_PASS_LABEL, LIT_VIEW_ATMOSPHERE_LABEL]);
    expect(seen).toEqual([{ colour: lit.target.colour, depth: lit.target.depth }]);
    const display = frames[0]?.draws[0]?.textures["hdrColour"];
    expect(display).toBeDefined();
    expect(display).not.toBe(lit.target.colour);
  });

  it("draws from its targets' new textures after a resize", async () => {
    const engine = await countingRenderEngine();
    const lit = new LitView(engine, "spike", SIZE, null, true);
    await lit.ready();
    lit.resize({ widthPx: 128, heightPx: 72 });
    const seen: LitScene[] = [];
    const frames: FrameSubmission[] = [];
    lit.render({ render: (frame) => frames.push(frame) }, IDENTITY, IDENTITY, [], (scene) => {
      seen.push(scene);
      return DUMMY_DRAW;
    });
    expect(seen[0]?.colour).toBe(lit.target.colour);
    expect(seen[0]?.depth).toBe(lit.target.depth);
    const sky = engine.targetFrames.at(-1);
    expect(sky?.label).toBe(LIT_VIEW_ATMOSPHERE_LABEL);
    expect(frames[0]?.draws[0]?.textures["hdrColour"]).not.toBe(lit.target.colour);
  });

  it("refuses a composite it was not made for", async () => {
    const lit = new LitView(await countingRenderEngine(), "spike", SIZE, null);
    expect(() => {
      lit.render({ render: () => undefined }, IDENTITY, IDENTITY, [], () => DUMMY_DRAW);
    }).toThrow(/atmosphere/);
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
