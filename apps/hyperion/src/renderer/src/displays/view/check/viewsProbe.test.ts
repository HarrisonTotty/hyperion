import { describe, expect, it } from "vitest";

import { GraphicsStatusStore, initialGraphicsStatus } from "../../../view/engine/status";
import type { FrameSubmission, RenderEngine } from "../../../view/engine/types";
import { timedEngineSource } from "../../../test/viewDisplayHarness";
import { viewOf, ViewsProbe } from "./viewsProbe";

/** A frame with nothing in it, whose label names it. */
function frame(label: string): FrameSubmission {
  return {
    label,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: [],
    postProcesses: [],
  };
}

/** A page whose animation frames the test runs by hand, on a clock it moves. */
class FakePage {
  nowMs = 0;
  readonly #callbacks: FrameRequestCallback[] = [];

  requestAnimationFrame = (callback: FrameRequestCallback): number => {
    this.#callbacks.push(callback);
    return this.#callbacks.length;
  };

  /** Runs every callback asked for, at `timeMs`. */
  runFrame(timeMs: number): void {
    this.nowMs = timeMs;
    for (const callback of this.#callbacks.splice(0)) {
      callback(timeMs);
    }
  }
}

/** A probe over a timed fake engine, its clock the page's, and the engine it wrapped. */
async function probed(): Promise<{
  readonly probe: ViewsProbe;
  readonly page: FakePage;
  readonly engine: RenderEngine;
  readonly timed: ReturnType<typeof timedEngineSource>;
}> {
  const timed = timedEngineSource();
  const page = new FakePage();
  const probe = new ViewsProbe(timed.source, () => page.nowMs);
  const outcome = await probe.source.requestAdapter();
  if (outcome.kind !== "adapter") {
    throw new Error("the fake gave no adapter");
  }
  const engine = await probe.source.load(
    outcome,
    new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
  );
  probe.installFrameClock(page);
  return { probe, page, engine, timed };
}

/** Draws `draw` from the page's next animation frame at `timeMs`, taking `costMs` of the clock. */
function inFrame(page: FakePage, timeMs: number, draw: () => void, costMs = 0): void {
  page.requestAnimationFrame(() => {
    draw();
    page.nowMs += costMs;
  });
  page.runFrame(timeMs);
}

describe("viewOf", () => {
  it("names the view a target belongs to", () => {
    expect(viewOf("view")).toBe("view");
    expect(viewOf("view:hdr")).toBe("view");
    expect(viewOf("instrument-1 blue noise")).toBe("instrument-1");
  });
});

describe("the views probe", () => {
  it("keys each view's renders by their animation frame and sums their GPU time", async () => {
    const { probe, page, engine, timed } = await probed();
    const canvas = document.createElement("canvas");
    const primary = engine.createView(canvas, "view");
    const instrument = engine.createView(document.createElement("canvas"), "instrument-1");
    const target = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 100, heightPx: 50 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    inFrame(page, 100, () => {
      target.render(frame("discs"));
      primary.render(frame("tonemap"));
      instrument.render(frame("view:wireframe"));
    });
    inFrame(page, 116, () => {
      primary.render(frame("tonemap"));
    });
    timed.deliver((name) => (name === "instrument-1" ? 0.5 : 1));
    const record = probe.phase("photoreal-two-wireframe", 0, 200, [
      { name: "view", style: "photorealistic" },
      { name: "instrument-1", style: "wireframe" },
    ]);
    expect(record.frameGpuMs).toEqual([2.5, 1]);
    expect(record.frameIntervalsMs).toEqual([16]);
    expect(record.primaryIntervalsMs).toEqual([16]);
    expect(
      record.views.map(({ name, draws, gpuMs, passLabels }) => [name, draws, gpuMs, passLabels]),
    ).toEqual([
      ["view", 2, [2, 1], ["view", "view:hdr"]],
      ["instrument-1", 1, [0.5], ["instrument-1"]],
    ]);
    expect(record.droppedResolves).toBe(0);
    expect(probe.timer()).toBe("quantized");
  });

  it("counts a resolve that never reports as a drop, and leaves its frame untimed", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    inFrame(page, 100, () => {
      primary.render(frame("view:wireframe"));
    });
    const record = probe.phase("wireframe-alone", 0, 200, [{ name: "view", style: "wireframe" }]);
    expect(record).toMatchObject({ droppedResolves: 1, untimedFrames: 1, frameGpuMs: [] });
    expect(probe.timer()).toBe("absent");
  });

  it("times each animation frame's callbacks on the main thread", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    inFrame(
      page,
      100,
      () => {
        primary.render(frame("view:wireframe"));
      },
      3,
    );
    const record = probe.phase("wireframe-alone", 0, 200, [{ name: "view", style: "wireframe" }]);
    expect(record.mainThreadMs).toEqual([3]);
  });

  it("leaves out the frames outside the window, and resolves no view made", async () => {
    const { probe, page, engine, timed } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    inFrame(page, 50, () => {
      primary.render(frame("view:wireframe"));
    });
    inFrame(page, 100, () => {
      primary.render(frame("view:wireframe"));
      // A resolve no view's render takes (a mipmap's), between two that do.
      const fake = timed.engines[0];
      if (fake === undefined) {
        throw new Error("no engine was made");
      }
      const free = engine.passTimesFrame + 1;
      fake.passTimesFrame = free;
      fake.reportPassTimes({
        frame: free,
        timer: "full",
        passes: [{ label: "mipmaps", ns: 2e5, bracketed: false }],
      });
      primary.render(frame("view:wireframe"));
    });
    timed.deliver(() => 1);
    const record = probe.phase("wireframe-alone", 80, 200, [{ name: "view", style: "wireframe" }]);
    expect(record.frameGpuMs).toEqual([2]);
    expect(record.unattributedGpuMs).toBeCloseTo(0.2);
  });

  it("follows the scene target's scale against its view's canvas", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    primary.resize({ widthPx: 1000, heightPx: 800 });
    page.nowMs = 10;
    const target = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 1000, heightPx: 800 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    page.nowMs = 120;
    target.resize({ widthPx: 800, heightPx: 640 });
    const record = probe.phase("photoreal-two-wireframe", 100, 200, [
      { name: "view", style: "photorealistic" },
    ]);
    expect(record.views[0]?.scales).toEqual([1, 0.8]);
    expect(probe.canvasOf("view")).toEqual({ widthPx: 1000, heightPx: 800 });
    expect(probe.canvasOf("instrument-2")).toEqual({ widthPx: 0, heightPx: 0 });
  });

  it("keeps the allocations after a mark", async () => {
    const { probe, timed } = await probed();
    const mark = probe.allocationMark();
    timed.engines[0]?.dispose();
    expect(probe.allocationsSince(mark).map(({ kind, name }) => [kind, name])).toEqual([
      ["destroyed", "fake engine memory"],
    ]);
  });

  it("keeps every fault as a line", async () => {
    const { probe, timed } = await probed();
    timed.engines[0]?.loseDevice("destroyed");
    expect(probe.faults()).toEqual(["device-lost (destroyed): fake loss"]);
  });

  it("gives the page's own requestAnimationFrame back", () => {
    const probe = new ViewsProbe();
    const page = new FakePage();
    const own = page.requestAnimationFrame;
    const restore = probe.installFrameClock(page);
    expect(page.requestAnimationFrame).not.toBe(own);
    restore();
    expect(page.requestAnimationFrame).toBe(own);
  });
});
