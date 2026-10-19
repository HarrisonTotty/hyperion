import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { FrameSubmission, RenderView } from "../../view/engine/types";
import { viewId } from "../../view/camera/state";
import { IDENTITY_ROTATION } from "../../view/coords/rotation";
import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { PHASE_GIANT, PHASE_PLANETS, phaseScene } from "../../view/scenes/phaseScene";
import {
  makeViewFrameDrawer,
  presentedViewport,
  skyFaceSizePx,
  terrainReportOf,
  ViewFrameDrawer,
  type ViewFrameInputs,
} from "./viewFrameDrawer";
import { runPose, startRun } from "./viewRun";

/** A canvas view that records whether it was disposed. */
class RecordingView implements RenderView {
  readonly name: string;
  readonly frames: FrameSubmission[] = [];
  disposed = false;

  constructor(name: string) {
    this.name = name;
  }
  resize(): void {}
  render(frame: FrameSubmission): void {
    this.frames.push(frame);
  }
  readBack(): Promise<Uint8Array> {
    return Promise.resolve(new Uint8Array(0));
  }
  dispose(): void {
    this.disposed = true;
  }
}

/** Refuses the engine's materials, as `ResilientEngine` does while it has no device. */
function loseDevice(engine: Awaited<ReturnType<typeof countingRenderEngine>>): () => void {
  const spy = vi.spyOn(engine, "createMaterial").mockImplementation(() => {
    throw new EngineUnavailable("createMaterial");
  });
  return () => {
    spy.mockRestore();
  };
}

describe("a view's frame drawer, made during a device loss (R07.T19.c)", () => {
  it("is made after the restore, its handles once", async () => {
    const engine = await countingRenderEngine();
    // What one drawer makes, on a device.
    engine.resetCounts();
    makeViewFrameDrawer(engine, new RecordingView("measured"), "measured", () => undefined)();
    const once = { ...engine.counts };
    const regain = loseDevice(engine);
    const made: ViewFrameDrawer[] = [];
    const release = makeViewFrameDrawer(
      engine,
      new RecordingView("waiting"),
      "waiting",
      (drawer) => {
        made.push(drawer);
      },
    );
    regain();
    engine.resetCounts();
    engine.restore();
    await Promise.resolve();
    expect([made.length, engine.counts]).toEqual([1, once]);
    release();
  });

  it("is not made, and its view is disposed of, when released before the restore", async () => {
    const engine = await countingRenderEngine();
    const regain = loseDevice(engine);
    const view = new RecordingView("waiting");
    const made: ViewFrameDrawer[] = [];
    const release = makeViewFrameDrawer(engine, view, "waiting", (drawer) => {
      made.push(drawer);
    });
    regain();
    release();
    engine.restore();
    await Promise.resolve();
    expect([made.length, view.disposed]).toEqual([0, true]);
  });
});

/** Stand-in colours, one distinct colour per token, as `readTokens` would read them. */
const TOKENS = {
  text: "#e0e0e0",
  textMuted: "#909090",
  accent: "#40a0ff",
  target: "#ff40ff",
  line: "#404040",
  surface0: "#000000",
} as const;

/** The phase scene's run, its planets turning (their rotations the identity) where `rotated`. */
function phaseRun(rotated: boolean): ReturnType<typeof startRun> {
  const run = startRun(phaseScene());
  return rotated
    ? {
        ...run,
        scene: {
          ...run.scene,
          bodies: run.scene.bodies.map((body) =>
            body.kind === "star" ? body : { ...body, rotation: IDENTITY_ROTATION },
          ),
        },
      }
    : run;
}

/** The phase scene's frame drawn photorealistically, at 320 × 180, with the view's `sources`. */
async function drawPhotoreal(
  sources: Pick<ViewFrameInputs, "classMaps" | "reliefs" | "terrain">,
  rotated = false,
): Promise<void> {
  const engine = await countingRenderEngine();
  const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
  drawer.draw({
    run: phaseRun(rotated),
    size: { widthPx: 320, heightPx: 180, devicePixelRatio: 1, remPx: 16 },
    tokens: TOKENS,
    exposure: DEFAULT_EXPOSURE,
    stars: [],
    sky: null,
    selection: null,
    style: "photorealistic",
    setting: "low",
    renderScale: 1,
    availability: { wireframe: true, photorealistic: true },
    meter: null,
    ...sources,
  });
  drawer.dispose();
}

describe("a view's photorealistic frame (R10.T10.d, T10.e)", () => {
  const LIT = [...PHASE_PLANETS, PHASE_GIANT];

  it("shades its lit bodies with the view's class maps", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      classMaps: {
        classMapOf: (body) => {
          asked.push(body);
          return null;
        },
      },
    });
    expect(asked).toEqual(LIT);
  });

  it("hands its lit bodies with a rotation over by the view's reliefs", async () => {
    const asked: string[] = [];
    await drawPhotoreal(
      {
        reliefs: {
          reliefM: (body) => {
            asked.push(body);
            return null;
          },
        },
      },
      true,
    );
    expect(asked).toEqual(LIT);
  });

  it("draws its lit bodies over the view's own terrain of the session (R10.T16.c)", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      terrain: {
        source: {
          wanted: new Set(),
          survey: () => null,
          drawTerrain: () => null,
          coverageMask: () => null,
          pass: (body) => {
            asked.push(body);
            return null;
          },
        },
        view: viewId("primary"),
        nowMs: 0,
      },
    });
    expect(asked).toEqual(LIT);
  });
});

describe("what a view drew, for the terrain session (R10.T16.a)", () => {
  const LIT = [...PHASE_PLANETS, PHASE_GIANT];

  /** The phase scene's frame inputs at 320 × 180 CSS px and a ratio of `ratio`, in `style`. */
  function inputsOf(style: "photorealistic" | "wireframe", ratio = 1): ViewFrameInputs {
    return {
      run: startRun(phaseScene()),
      size: { widthPx: 320, heightPx: 180, devicePixelRatio: ratio, remPx: 16 },
      tokens: TOKENS,
      exposure: DEFAULT_EXPOSURE,
      stars: [],
      sky: null,
      selection: null,
      style,
      setting: "low",
      renderScale: 1,
      availability: { wireframe: true, photorealistic: true },
      meter: null,
    };
  }

  /** A drawer that has drawn the phase scene's photorealistic frame, its pipelines made. */
  async function plannedDrawer(): Promise<ViewFrameDrawer> {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    // The first frame draws the wireframe while its pipelines compile, and plans no regimes.
    drawer.draw(inputsOf("photorealistic"));
    await vi.waitFor(() => {
      expect(drawer.photorealStatus).toBe("ready");
    });
    drawer.draw(inputsOf("photorealistic"));
    return drawer;
  }

  it("plans no regimes before a photorealistic frame", async () => {
    const engine = await countingRenderEngine();
    const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
    const regimes = drawer.regimes.size;
    drawer.dispose();
    expect(regimes).toBe(0);
  });

  it("keeps its last photorealistic plan's regimes", async () => {
    const drawer = await plannedDrawer();
    const regimes = [...drawer.regimes.keys()];
    drawer.dispose();
    expect(regimes).toEqual(LIT);
  });

  it("keeps them through a frame drawn in the wireframe", async () => {
    const drawer = await plannedDrawer();
    const planned = drawer.regimes;
    drawer.draw(inputsOf("wireframe"));
    const kept = drawer.regimes;
    drawer.dispose();
    expect([planned.size > 0, kept === planned]).toEqual([true, true]);
  });

  it("reports its scene, pose, field of view, presented size, style, setting and priority", () => {
    const inputs = inputsOf("wireframe", 1.5);
    const regimes = new Map();
    expect(terrainReportOf(inputs, regimes, "secondary")).toEqual({
      scene: inputs.run.scene,
      pose: runPose(inputs.run),
      fovXRad: (inputs.run.camera.fovDeg * Math.PI) / 180,
      presentedPx: { widthPx: 480, heightPx: 270 },
      style: "wireframe",
      setting: "low",
      streamPriority: "secondary",
      regimes,
    });
  });

  it("reports nothing while its stage is not laid out", () => {
    expect(terrainReportOf({ ...inputsOf("wireframe"), size: null }, new Map(), "primary")).toBe(
      null,
    );
  });

  it("presents at its stage's size times its ratio, rounded, at least a pixel", () => {
    expect([
      presentedViewport({ widthPx: 320.4, heightPx: 180, devicePixelRatio: 1.25, remPx: 16 }),
      presentedViewport({ widthPx: 0.2, heightPx: 0.1, devicePixelRatio: 1, remPx: 16 }),
    ]).toEqual([
      { widthPx: 401, heightPx: 225 },
      { widthPx: 1, heightPx: 1 },
    ]);
  });
});

describe("the sky cube's face size (R07.T17)", () => {
  it.each([
    { setting: "high", blendable: true, faceSizePx: 3_072 },
    { setting: "low", blendable: true, faceSizePx: 1_024 },
    { setting: "high", blendable: false, faceSizePx: 1_024 },
    { setting: "low", blendable: false, faceSizePx: 1_024 },
  ] as const)(
    "is $faceSizePx² on the $setting setting where float32-blendable is $blendable",
    ({ setting, blendable, faceSizePx }) => {
      expect(skyFaceSizePx(setting, blendable)).toBe(faceSizePx);
    },
  );
});
