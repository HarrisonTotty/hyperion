import { describe, expect, it } from "vitest";

import { GraphicsStatusStore, initialGraphicsStatus } from "../../../view/engine/status";
import type { AllocationEvent } from "../../../view/engine/memory";
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
    const record = probe.phase(
      "photoreal-two-wireframe",
      0,
      200,
      [
        { name: "view", style: "photorealistic" },
        { name: "instrument-1", style: "wireframe" },
      ],
      [],
    );
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

  it("gives each pass label's time in each frame timed in full, 0 where a frame has none (the owner's report)", async () => {
    const { probe, page, engine, timed } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
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
    // A frame whose resolve never reports is left out, as the window's GPU times leave it.
    inFrame(page, 133, () => {
      primary.render(frame("tonemap"));
    });
    expect(probe.passTimes(0, 200)).toEqual([
      { label: "view:hdr", ms: [1, 0], kernels: [], workgroups: [] },
      { label: "view", ms: [1, 1], kernels: [], workgroups: [] },
      { label: "instrument-1", ms: [0.5, 0], kernels: [], workgroups: [] },
    ]);
    expect(probe.window(0, 200, [{ name: "view", style: "photorealistic" }]).frameGpuMs).toEqual([
      2.5, 1,
    ]);
  });

  it("numbers a label its frame repeats, in the frame's order (the owner's report)", async () => {
    const { probe, page, engine, timed } = await probed();
    const target = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 100, heightPx: 50 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    inFrame(page, 100, () => {
      target.render(frame("atmosphere view"));
      target.render(frame("atmosphere view"));
      target.render(frame("atmosphere view"));
    });
    timed.deliver(() => 2);
    expect(probe.passTimes(0, 200).map((pass) => pass.label)).toEqual([
      "view:hdr",
      "view:hdr #2",
      "view:hdr #3",
    ]);
  });

  it("carries a dispatched label's kernels and workgroups, numbered as its passes are (main's ask)", async () => {
    const { probe, page, engine, timed } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    const kernel = { kind: "compute", name: "atmosphere ray march", path: "reference" } as const;
    const bindings = { buffers: {}, sampled: {}, uniforms: {}, storage: {} };
    inFrame(page, 100, () => {
      engine.dispatch(kernel, bindings, [4, 2, 1], "atmosphere view");
      engine.dispatch(kernel, bindings, [10, 10, 1], "atmosphere view");
      primary.render(frame("tonemap"));
    });
    // The render's resolve holds the dispatches' passes before its own, as R01's timer encodes them.
    timed.engines[0]?.reportPassTimes({
      frame: engine.passTimesFrame,
      timer: "quantized",
      passes: [
        { label: "atmosphere view", ns: 2e6, bracketed: false },
        { label: "atmosphere view", ns: 5e6, bracketed: false },
        { label: "tonemap", ns: 1e6, bracketed: false },
      ],
    });
    expect(probe.passTimes(0, 200)).toEqual([
      { label: "atmosphere view", ms: [2], kernels: ["atmosphere ray march"], workgroups: [8] },
      {
        label: "atmosphere view #2",
        ms: [5],
        kernels: ["atmosphere ray march"],
        workgroups: [100],
      },
      { label: "tonemap", ms: [1], kernels: [], workgroups: [] },
    ]);
  });

  it("sums the engine's writes, allocations and releases within a window, by resource (the owner's report)", async () => {
    const { probe, page, timed } = await probed();
    const engine = timed.engines[0];
    const report = (atMs: number, event: AllocationEvent): void => {
      page.nowMs = atMs;
      engine?.reportAllocation(event);
    };
    report(50, { kind: "created", name: "early", bytes: 64, category: "other" });
    report(100, { kind: "created", name: "view star sprites", bytes: 256, category: "other" });
    report(110, { kind: "uploaded", name: "view star sprites", bytes: 64 });
    report(120, { kind: "uploaded", name: "view star sprites", bytes: 32 });
    report(130, { kind: "destroyed", name: "early", bytes: 64, category: "other" });
    report(300, { kind: "uploaded", name: "view star sprites", bytes: 16 });
    expect(probe.memoryTraffic(100, 200)).toEqual({
      uploaded: { count: 2, bytes: 96 },
      created: { count: 1, bytes: 256 },
      destroyed: { count: 1, bytes: 64 },
      uploads: [{ name: "view star sprites", count: 2, bytes: 96 }],
    });
  });

  it("counts a resolve that never reports as a drop, and leaves its frame untimed", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    inFrame(page, 100, () => {
      primary.render(frame("view:wireframe"));
    });
    const record = probe.phase(
      "wireframe-alone",
      0,
      200,
      [{ name: "view", style: "wireframe" }],
      [],
    );
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
    const record = probe.phase(
      "wireframe-alone",
      0,
      200,
      [{ name: "view", style: "wireframe" }],
      [],
    );
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
    const record = probe.phase(
      "wireframe-alone",
      80,
      200,
      [{ name: "view", style: "wireframe" }],
      [],
    );
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
    const record = probe.phase(
      "photoreal-two-wireframe",
      100,
      200,
      [{ name: "view", style: "photorealistic" }],
      [],
    );
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

  it("keeps every fault and restore with its time, for the terrain run (R10.T16.d)", async () => {
    const { probe, page, timed } = await probed();
    page.nowMs = 40;
    timed.engines[0]?.loseDevice("destroyed");
    page.nowMs = 90;
    timed.engines[0]?.raiseRestored();
    expect(probe.timedFaults()).toEqual([
      { atMs: 40, fault: { kind: "device-lost", reason: "destroyed", message: "fake loss" } },
    ]);
    expect(probe.restoresAtMs()).toEqual([90]);
  });

  it("tells a watcher of each submission in its animation frame, and each target made", async () => {
    const { probe, page, engine } = await probed();
    const seen: Array<readonly [string, string, number | null]> = [];
    const targets: string[] = [];
    const unwatch = probe.watch({
      submitted: (owner, submission, frameMs) => {
        seen.push([owner, submission.label, frameMs]);
      },
      madeTarget: (spec) => {
        targets.push(spec.name);
      },
    });
    const primary = engine.createView(document.createElement("canvas"), "view");
    const target = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 8, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    inFrame(page, 100, () => {
      target.render(frame("discs"));
      primary.render(frame("tonemap"));
    });
    primary.render(frame("outside"));
    unwatch();
    inFrame(page, 116, () => {
      primary.render(frame("tonemap"));
    });
    expect(targets).toEqual(["view:hdr"]);
    expect(seen).toEqual([
      ["view:hdr", "discs", 100],
      ["view", "tonemap", 100],
      ["view", "outside", null],
    ]);
  });

  it("gives every animation frame's time, and a view's draws', within a window", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    inFrame(page, 100, () => {
      primary.render(frame("tonemap"));
    });
    inFrame(page, 116, () => undefined);
    inFrame(page, 133, () => {
      primary.render(frame("tonemap"));
    });
    expect(probe.frameTimes(0, 200)).toEqual([100, 116, 133]);
    expect(probe.drawTimes("view", 0, 200)).toEqual([100, 133]);
    expect(probe.drawTimes("view", 110, 200)).toEqual([133]);
    const { name, statements, ...window } = probe.phase(
      "wireframe-alone",
      0,
      200,
      [{ name: "view", style: "wireframe" }],
      ["IMAGE: DETAIL LIMITED"],
    );
    expect([name, statements]).toEqual(["wireframe-alone", ["IMAGE: DETAIL LIMITED"]]);
    expect(probe.window(0, 200, [{ name: "view", style: "wireframe" }])).toEqual(window);
  });

  it("gives each view's draws within a window, each frame once, from its own times (lane perf-render-thread-ground)", async () => {
    const { probe, page, engine } = await probed();
    const primary = engine.createView(document.createElement("canvas"), "view");
    const instrument = engine.createView(document.createElement("canvas"), "instrument-1");
    for (let k = 0; k < 40; k += 1) {
      inFrame(page, 100 + 16 * k, () => {
        if (k % 2 === 0) {
          primary.render(frame("tonemap"));
          primary.render(frame("tonemap"));
        }
        if (k % 3 === 0) {
          instrument.render(frame("tonemap"));
        }
      });
    }
    expect([
      probe.drawTimes("view", 0, Number.POSITIVE_INFINITY).length,
      probe.drawTimes("view", 132, 228),
      probe.drawTimes("instrument-1", 148, 244),
      probe.drawTimes("instrument-1", 244, 244),
      probe.drawTimes("nothing", 0, 1_000),
    ]).toEqual([20, [132, 164, 196], [148, 196], [], []]);
  });

  it("summarises a window of more resolves than an argument list takes (a terrain run's minutes)", async () => {
    const { probe, page } = await probed();
    // Two hundred renders of a thousand resolves each: some 10⁵ resolves in a window, as a
    // terrain run's minutes hold, which an argument spread over them could not take.
    inFrame(page, 100, () => {
      for (let render = 0; render < 200; render += 1) {
        probe.rendered("view:hdr", render * 1_000, (render + 1) * 1_000, 0);
      }
    });
    const record = probe.window(0, 200, [{ name: "view", style: "photorealistic" }]);
    expect([record.droppedResolves, record.unattributedGpuMs]).toEqual([200_000, 0]);
  });

  it("counts each preparation until it settles, failed or not (R10.T14.d)", async () => {
    const { probe, page } = await probed();
    const settles: Array<() => void> = [];
    probe.preparing(
      new Promise<void>((resolve) => {
        settles.push(resolve);
      }),
    );
    probe.preparing(
      new Promise<void>((_, reject) => {
        settles.push(() => {
          reject(new Error("refused"));
        });
      }),
    );
    const before = probe.preparations();
    page.nowMs = 40;
    for (const settle of settles) {
      settle();
    }
    await Promise.resolve();
    await Promise.resolve();
    expect([before, probe.preparations()]).toEqual([
      { seen: 2, inFlight: 2, lastSettledAtMs: null },
      { seen: 2, inFlight: 0, lastSettledAtMs: 40 },
    ]);
  });

  it("is told of each preparation its probed engine passes on (R10.T14.d)", async () => {
    const { probe, engine } = await probed();
    const material = engine.createMaterial({
      name: "prepared",
      displayName: "PREPARED",
      vertexWgsl: "",
      fragmentWgsl: "",
      uniforms: [],
      samplers: [],
      cullMode: "none",
      depthWrite: false,
      colourWrites: true,
      blend: "none",
    });
    const prepared = engine.prepareMaterial(material, ["rgba16float"], []);
    const during = probe.preparations();
    await prepared;
    expect([during.inFlight, probe.preparations()]).toEqual([
      1,
      { seen: 1, inFlight: 0, lastSettledAtMs: 0 },
    ]);
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
