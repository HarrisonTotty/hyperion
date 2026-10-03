import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { DrawItem, FrameSubmission, RenderView } from "../engine/types";
import { AU_M } from "../scenes/kept";
import { BloomChain } from "../post/bloomChain";
import { type PhotorealFrame, PhotorealRenderer } from "./renderer";
import { PHOTOREAL_PASS_LABELS, SKY_PASS_LABEL } from "./passes";

const VIEWPORT = { widthPx: 64, heightPx: 36 };

/** A canvas view that records the frames submitted to it. */
class RecordingView implements RenderView {
  readonly name = "test view";
  readonly frames: FrameSubmission[] = [];
  resize(): void {}
  render(frame: FrameSubmission): void {
    this.frames.push(frame);
  }
  readBack(): Promise<Uint8Array> {
    return Promise.resolve(new Uint8Array(0));
  }
  dispose(): void {}
}

function frameWith(sky: ReadonlyArray<DrawItem>): PhotorealFrame {
  const lit = aLitBody();
  return {
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    viewport: VIEWPORT,
    role: "eye",
    setting: "high",
    exposureScale: 1e-4,
    sky,
    starSprites: [[10, 10, 0, 0, 1, 1, 1, 0]],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: aHostDisc(), centreM: vec3(AU_M, 0, -1e8) }],
    bodies: [
      { id: lit.body, centreM: vec3(0, 0, -1e8), figure: lit.figure, photometry: lit.photometry },
    ],
    previousRegimes: new Map(),
    overlay: null,
  };
}

describe("the photorealistic renderer", () => {
  it("draws nothing until its pipelines are made", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    expect(renderer.render(view, frameWith([]))).toBeNull();
    renderer.dispose();
  });

  it("draws the sky and the stars, then the bodies over them, then tones onto the canvas", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const band: DrawItem = {
      mesh: { kind: "mesh", name: "band triangle" },
      material: { kind: "material", name: "sky:band" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    expect(renderer.render(view, frameWith([band]))).not.toBeNull();
    const passes = engine.targetFrames.map((f) => [
      f.label,
      f.colourLoad ?? "clear",
      f.draws.map((draw) => draw.material.name),
    ]);
    expect(passes).toEqual([
      [SKY_PASS_LABEL, "clear", ["sky:band", "sky:starSpriteHdr"]],
      [PHOTOREAL_PASS_LABELS.discs, "load", ["bodies:disc", "bodies:discLimb"]],
      ...passes.slice(2),
    ]);
    expect(view.frames.map((f) => [f.label, f.encoding])).toEqual([["tonemap", "in-pass"]]);
    renderer.dispose();
  });

  it("makes its resources again after a device restore, and draws after them", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    expect(renderer.status).toBe("idle");
    expect(renderer.render(view, frameWith([]))).toBeNull();
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    expect([renderer.status, renderer.render(view, frameWith([])) !== null]).toEqual([
      "ready",
      true,
    ]);
    renderer.dispose();
  });

  it("abandons a making a restore overtakes", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const first = renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    await first;
    expect(renderer.status).toBe("idle");
    renderer.dispose();
  });

  it("releases its buffers and textures when disposed", async () => {
    const engine = await countingRenderEngine();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    renderer.dispose();
    expect(released).toEqual(
      expect.arrayContaining([
        "test view glare sources",
        "test view star sprites",
        "test view blue noise",
      ]),
    );
  });
});

describe("the photorealistic renderer's overlay", () => {
  it("loads the tone-mapped image for the symbology", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    const overlay = {
      label: PHOTOREAL_PASS_LABELS.symbology,
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [],
      postProcesses: [],
    };
    renderer.render(view, { ...frameWith([]), overlay });
    expect(view.frames.map((f) => [f.label, f.colourLoad])).toEqual([
      ["tonemap", undefined],
      ["symbology", "load"],
    ]);
    renderer.dispose();
  });
});

describe("the photorealistic renderer when a pipeline is refused", () => {
  it("stands failed, prepares nothing more and releases the bloom chain it made", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const chainDisposed = vi.spyOn(BloomChain.prototype, "dispose");
    vi.spyOn(engine, "createMaterialAsync").mockImplementation((spec) =>
      spec.name === "tone mapping"
        ? Promise.reject(new Error("refused"))
        : Promise.resolve({ kind: "material", name: spec.name }),
    );
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    // The chain's release follows its making, a few microtask turns later.
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
    expect(renderer.status).toBe("failed");
    const materials = engine.counts.materials;
    expect(renderer.render(new RecordingView(), frameWith([]))).toBeNull();
    expect(engine.counts.materials).toBe(materials);
    expect(chainDisposed).toHaveBeenCalled();
    renderer.dispose();
  });
});
