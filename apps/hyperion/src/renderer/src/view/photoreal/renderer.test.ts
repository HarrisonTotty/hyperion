import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { sphereFootprint } from "../bodies/regime";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { BODY_DISC_CELLS_KERNEL, DISC_CELLS_PASS, type LitBodyInput } from "../bodies/draw";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { Viewport } from "../camera/projection";
import { TIMING_FRAMES_IN_FLIGHT } from "../engine/webgpu/timing";
import type { DrawItem, FrameSubmission, RenderView } from "../engine/types";
import { AU_M } from "../scenes/kept";
import { BloomChain } from "../post/bloomChain";
import { histogramParams } from "../post/histogram";
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
      {
        id: lit.body,
        centreM: vec3(0, 0, -1e8),
        figure: lit.figure,
        photometry: lit.photometry,
        lighting: undefined,
      },
    ],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
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

  it("draws mesh bodies with depth in the bodies pass, between the sky and the painter's sequence", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const frame = frameWith([]);
    const [planet] = frame.bodies;
    if (planet === undefined) {
      throw new Error("no body");
    }
    // A depth writer over the planet, standing for R10's terrain.
    const writer = sphereFootprint(
      planet.centreM,
      planet.figure.equatorialRadiusM,
      camera,
      VIEWPORT,
    );
    const plan = renderer.render(view, { ...frame, depthWriters: writer === null ? [] : [writer] });
    expect(plan?.regimes.get(planet.id)).toBe("mesh");
    const passes = engine.targetFrames.map((f) => [
      f.label,
      f.colourLoad ?? "clear",
      f.draws.map((draw) => draw.material.name),
    ]);
    expect(passes.slice(0, 3)).toEqual([
      [SKY_PASS_LABEL, "clear", ["sky:starSpriteHdr"]],
      [PHOTOREAL_PASS_LABELS.bodies, "load", ["bodies:smoothMesh"]],
      [PHOTOREAL_PASS_LABELS.discs, "load", ["bodies:discLimb"]],
    ]);
    renderer.dispose();
  });

  it("lights each body by planetshine from two neighbours on the high setting and one on the low", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const frame = frameWith([]);
    const [planet] = frame.bodies;
    if (planet === undefined) {
      throw new Error("no body");
    }
    // Two moons on the planet's night side, full from it and clear of its shadow.
    const moonFigure = { equatorialRadiusM: 1.737e6, polarRadiusM: 1.737e6, pole: null };
    const moons = [
      {
        ...planet,
        id: "0200080020000000.0301",
        centreM: vec3(-4e8, -3e7, -1e8),
        figure: moonFigure,
      },
      {
        ...planet,
        id: "0200080020000000.0302",
        centreM: vec3(-4e8, 5e7, -1e8),
        figure: moonFigure,
      },
    ];
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const sources = (["high", "low"] as const).map((setting) => {
      const plan = renderer.render(view, { ...frame, setting, bodies: [planet, ...moons] });
      return plan?.discs.find((disc) => disc.body === planet.id)?.secondaries.length;
    });
    expect(sources).toEqual([2, 1]);
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

/** Lets the read-backs settle. */
async function settled(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
}

describe("the photorealistic renderer's histogram", () => {
  async function ready(): Promise<{
    readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    readonly renderer: PhotorealRenderer;
  }> {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    return { engine, renderer };
  }

  it("is dispatched once a frame with the operator's meter's weights", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), { ...frameWith([]), meter: "lit" });
    expect(
      engine.dispatched
        .filter((d) => d.pass === PHOTOREAL_PASS_LABELS.histogram)
        .map((d) => [d.pass, Array.from(d.bindings.uniforms["params"] ?? [])]),
    ).toEqual([["histogram", Array.from(histogramParams(VIEWPORT, "lit", 1))]]);
    renderer.dispose();
  });

  it("is not taken on a frame that meters nothing, as an instrument's (R07.T19.c)", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), { ...frameWith([]), meter: null });
    await settled();
    const histograms = engine.dispatched.filter((d) => d.pass === PHOTOREAL_PASS_LABELS.histogram);
    expect([histograms.length, renderer.takeHistogram()]).toEqual([0, undefined]);
    renderer.dispose();
  });

  it("is handed over once read back, under the frame's pre-exposure", async () => {
    const { renderer } = await ready();
    renderer.render(new RecordingView(), frameWith([]));
    await settled();
    expect([renderer.takeHistogram()?.preExposure, renderer.takeHistogram()]).toEqual([
      1e-4,
      undefined,
    ]);
    renderer.dispose();
  });

  it("is dropped by a device restore", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), frameWith([]));
    engine.restore();
    await settled();
    expect(renderer.takeHistogram()).toBeUndefined();
    renderer.dispose();
  });

  it("is dropped by a dispose, which releases the reader's buffers", async () => {
    const { engine, renderer } = await ready();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    renderer.render(new RecordingView(), frameWith([]));
    renderer.dispose();
    await settled();
    expect(renderer.takeHistogram()).toBeUndefined();
    expect(released.filter((name) => name.startsWith("test view histogram"))).toHaveLength(3);
  });
});

/**
 * An Earth-sized body `diameterPx` across on a view of `viewport` at 60°, `xPx` right of its
 * centre.
 */
function bodyAcross(
  diameterPx: number,
  xPx: number,
  id: string,
  viewport: Viewport = VIEWPORT,
): LitBodyInput {
  const lit = aLitBody();
  const pxPerRad = viewport.widthPx / (2 * Math.tan(Math.PI / 6));
  const distanceM = lit.figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad);
  return {
    id,
    centreM: vec3((xPx * distanceM) / pxPerRad, 0, -distanceM),
    figure: lit.figure,
    photometry: lit.photometry,
    lighting: undefined,
  };
}

/** A frame of `frameWith`'s sky and lights with these bodies, drawn as discs. */
function frameOfDiscs(bodies: ReadonlyArray<LitBodyInput>, viewport = VIEWPORT): PhotorealFrame {
  return {
    ...frameWith([]),
    viewport,
    bodies,
    previousRegimes: new Map(bodies.map((body) => [body.id, "disc"])),
  };
}

/** A 3.5 px disc (8 × 8) and a 13 px one (4 × 4) on `viewport`. */
function smallDiscs(viewport: Viewport = VIEWPORT): LitBodyInput[] {
  const third = viewport.widthPx / 4;
  return [
    bodyAcross(3.5, third, "0200080020000000.0401", viewport),
    bodyAcross(13, -third, "0200080020000000.0402", viewport),
  ];
}

describe("the photorealistic renderer's cell pass (R07.T8.d)", () => {
  it("shades the small discs' cells once, after the sky and before the discs", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    const plan = renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    expect(plan?.discs.map((disc) => disc.interiorSamples)).toEqual([8, 4]);
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect(cells.map((d) => [d.kernel, engine.targetFrames[d.framesBefore - 1]?.label])).toEqual([
      [BODY_DISC_CELLS_KERNEL.name, SKY_PASS_LABEL],
    ]);
    expect(engine.targetFrames[cells[0]?.framesBefore ?? -1]?.label).toBe(
      PHOTOREAL_PASS_LABELS.discs,
    );
    renderer.dispose();
  });

  it("dispatches nothing where every disc is 32 px or more", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    const plan = renderer.render(
      new RecordingView(),
      frameOfDiscs([bodyAcross(34, 0, "0200080020000000.0403")]),
    );
    expect([
      plan?.discs.length,
      engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS).length,
    ]).toEqual([1, 0]);
    renderer.dispose();
  });

  it("dispatches once in each of three photorealistic views", async () => {
    const engine = await countingRenderEngine();
    const renderers = ["primary", "instrument 1", "instrument 2"].map(
      (name) => new PhotorealRenderer(engine, name),
    );
    await Promise.all(
      renderers.map((renderer) => renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera)),
    );
    for (const renderer of renderers) {
      renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    }
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect([cells.length, new Set(cells.map((d) => d.bindings.buffers["cell_jobs"])).size]).toEqual(
      [3, 3],
    );
    for (const renderer of renderers) {
      renderer.dispose();
    }
  });

  it("makes the cell kernel again after a device restore, and dispatches it", async () => {
    const engine = await countingRenderEngine();
    const made = vi.spyOn(engine, "createComputeAsync");
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = frameWith([]).camera;
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    expect([
      made.mock.calls.filter(([pair]) => pair === BODY_DISC_CELLS_KERNEL).length,
      engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS).length,
    ]).toEqual([2, 1]);
    renderer.dispose();
  });

  it("keeps three frames of a photorealistic primary and two instruments in the timer's resolves", async () => {
    // Each render resolves its own pass times; a dispatch's wait for the next render's.
    const engine = await countingRenderEngine();
    const views = [
      { name: "primary", viewport: { widthPx: 1920, heightPx: 1080 }, role: "eye" as const },
      { name: "instrument 1", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
      { name: "instrument 2", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
    ];
    const camera = frameWith([]).camera;
    const drawn = await Promise.all(
      views.map(async ({ name, viewport, role }) => {
        const renderer = new PhotorealRenderer(engine, name);
        await renderer.prepare(viewport, "high", role, camera);
        return { viewport, role, renderer, canvas: new RecordingView() };
      }),
    );
    const overlay = {
      label: PHOTOREAL_PASS_LABELS.symbology,
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [],
      postProcesses: [],
    };
    for (const view of drawn) {
      view.renderer.render(view.canvas, {
        ...frameOfDiscs(smallDiscs(view.viewport), view.viewport),
        role: view.role,
        meter: view.role === "eye" ? "average" : null,
        overlay,
      });
    }
    const resolves =
      engine.targetFrames.length + drawn.reduce((sum, view) => sum + view.canvas.frames.length, 0);
    expect(engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS)).toHaveLength(3);
    expect(3 * resolves).toBeLessThanOrEqual(TIMING_FRAMES_IN_FLIGHT);
    for (const view of drawn) {
      view.renderer.dispose();
    }
  });
});
