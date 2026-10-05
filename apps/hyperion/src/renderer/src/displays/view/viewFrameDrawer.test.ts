import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { FrameSubmission, RenderView } from "../../view/engine/types";
import { makeViewFrameDrawer, type ViewFrameDrawer } from "./viewFrameDrawer";

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
