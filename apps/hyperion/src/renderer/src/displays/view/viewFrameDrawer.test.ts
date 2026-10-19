import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { FrameSubmission, RenderView } from "../../view/engine/types";
import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { PHASE_GIANT, PHASE_PLANETS, phaseScene } from "../../view/scenes/phaseScene";
import {
  makeViewFrameDrawer,
  skyFaceSizePx,
  ViewFrameDrawer,
  type ViewFrameInputs,
} from "./viewFrameDrawer";
import { startRun } from "./viewRun";

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

/** The phase scene's frame drawn photorealistically, at 320 × 180, with the view's `sources`. */
async function drawPhotoreal(
  sources: Pick<ViewFrameInputs, "classMaps" | "reliefs" | "terrain">,
): Promise<void> {
  const engine = await countingRenderEngine();
  const drawer = new ViewFrameDrawer(engine, new RecordingView("view"), "view");
  drawer.draw({
    run: startRun(phaseScene()),
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

  it("hands its lit bodies over by the view's reliefs", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      reliefs: {
        reliefM: (body) => {
          asked.push(body);
          return null;
        },
      },
    });
    expect(asked).toEqual(LIT);
  });

  it("draws its lit bodies over the view's terrain", async () => {
    const asked: string[] = [];
    await drawPhotoreal({
      terrain: {
        surveyCoverage: (body) => {
          asked.push(body);
          return null;
        },
        draws: () => [],
      },
    });
    expect(asked).toEqual(LIT);
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
