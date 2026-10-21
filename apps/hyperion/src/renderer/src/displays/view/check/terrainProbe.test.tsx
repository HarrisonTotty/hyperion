import { describe, expect, it, vi } from "vitest";

import type { TerrainCheckViewRecord } from "../../../../../preload/api";
import { fakeViewEngineSource } from "../../../test/fakeViewEngine";
import { FakeRenderEngine } from "../../../test/fakeRenderEngine";
import { ScriptedHeightWorker } from "../../../test/scriptedHeightWorker";
import { timedEngineSource } from "../../../test/viewDisplayHarness";
import { GraphicsStatusStore, initialGraphicsStatus } from "../../../view/engine/status";
import type { AllocationEvent } from "../../../view/engine/memory";
import { EngineUnavailable } from "../../../view/engine/resilientEngine";
import type {
  DrawItem,
  FrameSubmission,
  RenderEngine,
  TexelRect,
  TextureHandle,
} from "../../../view/engine/types";
import { TERRAIN_SCENE_NAME, terrainTestGolden } from "../../../view/scenes/terrainScene";
import { type ViewId, viewId } from "../../../view/camera/state";
import {
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
  type TerrainFault,
} from "../../../view/terrain/annunciation";
import { toHalfBits } from "../../../view/sky/half";
import type { ViewEngineSource } from "../useViewEngine";
import { SCENE_OPTIONS } from "../viewRun";
import {
  bakesInFlight,
  countedWorker,
  longestGapOutside,
  longestGapThrough,
  missedAtCadence,
  nonFiniteTexels,
  TERRAIN_LIT_PREFIX,
  TERRAIN_WIRE_PREFIX,
  TerrainProbe,
  type WatchedSession,
  type WorkerTraffic,
} from "./terrainProbe";
import { ViewsProbe } from "./viewsProbe";

/** Counts with nothing counted. */
function noTraffic(): { -readonly [K in keyof WorkerTraffic]: number } {
  return {
    made: 0,
    live: 0,
    bakesPosted: 0,
    baked: 0,
    refused: 0,
    inits: 0,
    chunks: 0,
    releases: 0,
  };
}

const STORE = (): GraphicsStatusStore =>
  new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));

/** A page whose animation frames the test runs by hand, on a clock it moves. */
class FakePage {
  nowMs = 0;
  readonly #callbacks: FrameRequestCallback[] = [];

  requestAnimationFrame = (callback: FrameRequestCallback): number => {
    this.#callbacks.push(callback);
    return this.#callbacks.length;
  };

  /** Runs `draw` in the animation frame stamped `timeMs`. */
  frame(timeMs: number, draw: () => void): void {
    this.requestAnimationFrame(draw);
    this.nowMs = timeMs;
    for (const callback of this.#callbacks.splice(0)) {
      callback(timeMs);
    }
  }
}

/** A frame whose draws use the materials named. */
function frame(label: string, materials: ReadonlyArray<string> = []): FrameSubmission {
  return {
    label,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: materials.map((name): DrawItem => ({
      mesh: { kind: "mesh", name: "patch" },
      material: { kind: "material", name },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    })),
    postProcesses: [],
  };
}

/**
 * A session stand-in whose pass draws for the views in `handing`, `patches` patches, a drawn patch
 * in the view of each of `handing` but those in `blind`.
 */
function watchedSession(
  handing: Set<string>,
  patches: { count: number },
  blind: ReadonlySet<string> = new Set(),
): WatchedSession {
  return {
    pass: () => ({
      viewIds: handing,
      lastStep: { drawSet: patches },
      seesPatch: (id) => handing.has(id) && !blind.has(id),
    }),
    wirePass: () => null,
    coverageMask: () => null,
    survey: () => null,
    memory: () => ({
      workerFieldBytes: 1_000,
      keptFieldBytes: 200,
      gpuBytes: new Map([["coarse-field-gpu", 24_576]]),
    }),
    workerCount: 3,
    terrainFaults: new Map(),
    subscribeFaults: () => () => undefined,
    compiled: Promise.resolve(3),
    compileStartedAtMs: 0,
  };
}

/** A session stand-in whose terrain's fault the test sets, its listeners told of each change. */
class FaultingSession {
  readonly #listeners = new Set<() => void>();
  terrainFaults: ReadonlyMap<ViewId, TerrainFault> = new Map();

  /** The session as the probe watches it. */
  get watched(): WatchedSession {
    const read = (): ReadonlyMap<ViewId, TerrainFault> => this.terrainFaults;
    const listeners = this.#listeners;
    return {
      ...watchedSession(new Set(), { count: 0 }),
      get terrainFaults() {
        return read();
      },
      subscribeFaults: (listener) => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    };
  }

  /** How many listen now. */
  get listening(): number {
    return this.#listeners.size;
  }

  /** Sets the fault and tells the listeners. */
  set(faults: ReadonlyMap<ViewId, TerrainFault>): void {
    this.terrainFaults = faults;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

describe("the terrain probe's counted height workers (R10.T16.d)", () => {
  it("counts each message posted and answered, and each worker made and terminated", () => {
    const traffic = noTraffic();
    const inner = new ScriptedHeightWorker();
    const worker = countedWorker(inner, traffic);
    const body = terrainTestGolden().body;
    worker.postMessage(
      { kind: "field-init", id: 1, body, block: new ArrayBuffer(8), form: "payload" },
      [],
    );
    worker.postMessage({ kind: "field-chunk", id: 2, body, block: new ArrayBuffer(8) }, []);
    worker.postMessage({ kind: "field-release", id: 3, body }, []);
    inner.reply({ kind: "bake-failed", id: 4, message: "no" });
    expect(inner.received.map((message) => message.kind)).toEqual([
      "field-init",
      "field-chunk",
      "field-release",
    ]);
    expect(traffic).toMatchObject({
      made: 1,
      live: 1,
      inits: 1,
      chunks: 1,
      releases: 1,
      refused: 1,
    });
    worker.terminate();
    worker.terminate();
    expect([traffic.live, inner.terminated]).toEqual([0, true]);
  });

  it("gives the bakes posted and not answered as in flight", () => {
    expect(bakesInFlight({ ...noTraffic(), bakesPosted: 7, baked: 3, refused: 1 })).toBe(3);
    expect(bakesInFlight({ ...noTraffic(), bakesPosted: 1, baked: 2 })).toBe(0);
  });
});

describe("a scene target's read-back, checked finite", () => {
  it("counts a half texel with an infinity or a NaN in any channel", () => {
    const finite = [0.5, 1, 2, 1].map(toHalfBits);
    const infinite = [toHalfBits(0.5), 0x7c00, toHalfBits(2), toHalfBits(1)];
    const nan = [0x7e00, 0, 0, 0];
    const halves = Uint16Array.from([finite, infinite, nan].flat());
    expect(nonFiniteTexels(halves.buffer, "rgba16float")).toBe(2);
  });

  it("counts a float texel that is not finite, and reads no other format", () => {
    const floats = Float32Array.from([1, 2, 3, 1, Number.NaN, 0, 0, 1]);
    expect(nonFiniteTexels(floats.buffer, "rgba32float")).toBe(1);
    expect(nonFiniteTexels(new ArrayBuffer(8), "rgba8unorm")).toBeNull();
  });
});

/** `count` animation frames 10 ms apart, each time's index. */
function index(count: number): Map<number, number> {
  return new Map(Array.from({ length: count }, (_, i) => [i * 10, i]));
}

describe("the frames a view missed at its own cadence", () => {
  it("counts none for a view drawing every frame or every second one", () => {
    expect(missedAtCadence([0, 10, 20, 30, 40], index(5), null)).toBe(0);
    expect(missedAtCadence([0, 20, 40, 60, 80], index(9), null)).toBe(0);
  });

  it("counts the draws its cadence missed, outside the quiet span", () => {
    expect(missedAtCadence([0, 10, 20, 50, 60, 70], index(8), null)).toBe(2);
    expect(missedAtCadence([0, 20, 40, 100, 120, 140], index(15), null)).toBe(2);
    expect(missedAtCadence([0, 10, 20, 50, 60, 70], index(8), { fromMs: 25, toMs: 45 })).toBe(0);
  });
});

describe("the longest time between frames over the judged span (R10.T14.d)", () => {
  it("counts from the span's start to its end, and gives where the longest ended", () => {
    expect(longestGapOutside([10, 20, 400, 410], 0, 500, [])).toEqual({
      gapMs: 380,
      endMs: 400,
    });
  });

  it("leaves out a gap that crosses a quiet span", () => {
    expect(
      longestGapOutside([10, 20, 400, 410], 0, 500, [
        { fromMs: 100, toMs: 300 },
        { fromMs: 450, toMs: 460 },
      ]),
    ).toEqual({ gapMs: 10, endMs: 10 });
  });

  it("reads no frame outside the span", () => {
    expect(longestGapOutside([0, 5, 2_000], 100, 200, [])).toEqual({ gapMs: 100, endMs: 200 });
  });
});

describe("the longest time between frames through a compile (R10.T14.d)", () => {
  it("counts from the start, through the first frame after the compile's end", () => {
    expect(longestGapThrough([5, 20, 30, 400, 410, 900], 0, 300)).toBe(370);
  });

  it("counts no frame before the start and none after the first past the compile", () => {
    expect(longestGapThrough([0, 100, 116, 133, 800], 100, 120)).toBe(17);
  });

  it("gives none where no frame follows the compile's end", () => {
    expect(longestGapThrough([10, 20], 0, 300)).toBeNull();
  });
});

/** A probe over a views probe whose animation frames the test runs by hand on `page`'s clock. */
async function probedPage(page: FakePage): Promise<{
  readonly probe: TerrainProbe;
  readonly primary: ReturnType<RenderEngine["createView"]>;
  readonly stop: () => void;
}> {
  const timed = timedEngineSource();
  const views = new ViewsProbe(timed.source, () => page.nowMs);
  const probe = new TerrainProbe(views, { paceMs: 0, readBackMs: 1_000, nowMs: () => page.nowMs });
  const outcome = await probe.source.requestAdapter();
  if (outcome.kind !== "adapter") {
    throw new Error("the fake gave no adapter");
  }
  const engine: RenderEngine = await probe.source.load(outcome, STORE());
  views.installFrameClock(page);
  const stop = probe.start();
  const primary = engine.createView(document.createElement("canvas"), "view");
  return { probe, primary, stop };
}

describe("the terrain probe's longest gap and its session's compile (R10.T14.d)", () => {
  it("gives the script's time at the end of a view's longest gap", async () => {
    const page = new FakePage();
    const { probe, primary, stop } = await probedPage(page);
    probe.watchTerrain(watchedSession(new Set(), { count: 0 }));
    const start = probe.mark();
    probe.fixture.notePlayed?.(10);
    page.frame(10, () => {
      primary.render(frame("tonemap"));
    });
    page.frame(26, () => {
      primary.render(frame("tonemap"));
    });
    probe.fixture.notePlayed?.(11);
    page.frame(310, () => {
      primary.render(frame("tonemap"));
    });
    page.nowMs = 320;
    const record = probe.phase(
      "terrain-alone",
      start,
      probe.mark(),
      [{ name: "view", style: "photorealistic" }],
      null,
    );
    stop();
    expect(record.views[0]).toMatchObject({ longestGapMs: 284, longestGapEndScriptS: 11 });
    // A frame's script time as it drew, and none for a moment no frame was submitted at.
    expect([probe.scriptSAt(310), probe.scriptSAt(200)]).toEqual([11, null]);
  });

  it("follows a session's compile from its making, with the longest time between frames", async () => {
    const page = new FakePage();
    const { probe, primary, stop } = await probedPage(page);
    const settles: Array<(compiled: number) => void> = [];
    const compiled = new Promise<number>((resolve) => {
      settles.push(resolve);
    });
    const finish = (count: number): void => {
      for (const settle of settles) {
        settle(count);
      }
    };
    page.nowMs = 60;
    // Made at 50 ms, a commit before its watch.
    probe.watchTerrain({
      ...watchedSession(new Set(), { count: 0 }),
      compiled,
      compileStartedAtMs: 50,
    });
    page.frame(100, () => {
      primary.render(frame("tonemap"));
    });
    const before = probe.compiled;
    page.nowMs = 300;
    finish(3);
    await compiled;
    await Promise.resolve();
    page.frame(450, () => {
      primary.render(frame("tonemap"));
    });
    stop();
    expect([before, probe.compiled, probe.sessions()]).toEqual([
      false,
      true,
      [
        {
          madeAtMs: 50,
          compile: { kind: "ended", compileMs: 250, compiled: 3, longestFrameGapMs: 350 },
        },
      ],
    ]);
  });
});

describe("the terrain probe (R10.T16.d)", () => {
  it("serves TERRAIN TEST's fixture as its own, and leaves every other kept scene as it is", () => {
    const probe = new TerrainProbe(new ViewsProbe(), { paceMs: 10, readBackMs: 100 });
    expect(probe.sceneOptions.map((option) => option.name)).toEqual(
      SCENE_OPTIONS.map((option) => option.name),
    );
    expect(
      probe.sceneOptions.map((option, i) =>
        option.name === TERRAIN_SCENE_NAME
          ? option.terrain === probe.fixture
          : option === SCENE_OPTIONS[i],
      ),
    ).toEqual(SCENE_OPTIONS.map(() => true));
    probe.fixture.notePlayed?.(12.5);
    expect(probe.scriptS).toBe(12.5);
  });

  it("holds the device its engine is made on, and destroys it on the harness's word", async () => {
    const fake = fakeViewEngineSource();
    const devices: GPUDevice[] = [];
    const inner: ViewEngineSource = {
      requestAdapter: fake.source.requestAdapter,
      load: async (outcome) => {
        const device = await outcome.adapter.requestDevice();
        devices.push(device);
        return new FakeRenderEngine(device);
      },
    };
    const probe = new TerrainProbe(new ViewsProbe(inner, () => 0), { paceMs: 0, readBackMs: 100 });
    expect(probe.destroyDevice()).toBe(false);
    const outcome = await probe.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error("the fake gave no adapter");
    }
    await probe.source.load(outcome, STORE());
    expect(probe.destroyDevice()).toBe(true);
    await expect(devices[0]?.lost).resolves.toMatchObject({ reason: "destroyed" });
  });

  it("counts each view's handed-over draws, its terrain draws and the pass's patches", async () => {
    const timed = timedEngineSource();
    const page = new FakePage();
    const views = new ViewsProbe(timed.source, () => page.nowMs);
    const probe = new TerrainProbe(views, {
      paceMs: 0,
      readBackMs: 1_000,
      nowMs: () => page.nowMs,
    });
    const outcome = await probe.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error("the fake gave no adapter");
    }
    const engine: RenderEngine = await probe.source.load(outcome, STORE());
    views.installFrameClock(page);
    const stop = probe.start();
    const handing = new Set<string>();
    const blind = new Set<string>();
    const patches = { count: 0 };
    const release = probe.watchTerrain(watchedSession(handing, patches, blind));
    const primary = engine.createView(document.createElement("canvas"), "view");
    const target = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 8, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    const draw = (terrain: boolean): void => {
      target.render(frame("discs", terrain ? [`${TERRAIN_LIT_PREFIX}baked-offsets`] : ["disc"]));
      primary.render(frame("tonemap"));
    };
    const start = probe.mark();
    page.frame(100, () => {
      draw(false);
    });
    handing.add("view");
    patches.count = 12;
    page.frame(116, () => {
      draw(false);
    });
    page.frame(133, () => {
      draw(true);
    });
    patches.count = 15;
    page.frame(150, () => {
      draw(true);
    });
    page.frame(166, () => {
      draw(false);
    });
    // Past the survey: handed over, no drawn patch in view, no terrain, and no gap for it.
    blind.add("view");
    page.frame(183, () => {
      draw(false);
    });
    page.nowMs = 200;
    const record = probe.phase(
      "terrain-alone",
      start,
      probe.mark(),
      [{ name: "view", style: "photorealistic" }],
      null,
    );
    expect(record.views[0]).toMatchObject({
      draws: 6,
      handedOverDraws: 4,
      offSurveyDraws: 1,
      terrainDraws: 2,
      terrainGaps: 1,
      terrainGapSpansS: [[0, 0]],
      firstHandoverMs: 116,
      firstTerrainMs: 133,
      terrainHandoverMs: 116,
      missedFrames: 0,
    });
    expect(record.patchesDrawn).toEqual([0, 12, 12, 15, 15, 15]);
    expect(record.memory).toMatchObject({
      workerFieldBytes: 1_000,
      sessionGpuBytes: [{ category: "coarse-field-gpu", bytes: 24_576 }],
    });
    release();
    expect(probe.session).toBeNull();
    expect(probe.teardown()).toMatchObject({
      disposed: true,
      workersLive: 0,
      sessionGpuBytes: 24_576,
    });
    stop();
  });

  it("times the terrain from the hand-over it was first drawn in, past a lone handed-over draw (R10.T14.d)", async () => {
    const page = new FakePage();
    const { probe, primary, stop } = await probedPage(page);
    const handing = new Set<string>();
    probe.watchTerrain(watchedSession(handing, { count: 12 }));
    const draw = (terrain: boolean): void => {
      primary.render(frame("tonemap", terrain ? [`${TERRAIN_LIT_PREFIX}baked-offsets`] : []));
    };
    const start = probe.mark();
    // The frame the script's loop moves the camera in: the pass still holds the view.
    handing.add("view");
    page.frame(100, () => {
      draw(false);
    });
    handing.delete("view");
    page.frame(116, () => {
      draw(false);
    });
    handing.add("view");
    page.frame(133, () => {
      draw(false);
    });
    page.frame(150, () => {
      draw(true);
    });
    page.nowMs = 160;
    const record = probe.phase(
      "terrain-alone",
      start,
      probe.mark(),
      [{ name: "view", style: "photorealistic" }],
      null,
    );
    stop();
    expect(record.views[0]).toMatchObject({
      firstHandoverMs: 100,
      firstTerrainMs: 150,
      terrainHandoverMs: 133,
    });
  });

  describe("a wireframe view's terrain (R10.T11)", () => {
    /**
     * The record of a wireframe view's five frames into its canvas: before any hand-over; handed
     * over without its terrain; twice with it (drawing `material`); then past the survey, seeing no
     * patch. The session hands the view over through its wireframe pass, or its lit pass where
     * `through` says so.
     */
    async function wireframeRecord(
      material: string,
      through: "wire" | "lit" = "wire",
    ): Promise<TerrainCheckViewRecord> {
      const timed = timedEngineSource();
      const page = new FakePage();
      const views = new ViewsProbe(timed.source, () => page.nowMs);
      const probe = new TerrainProbe(views, {
        paceMs: 0,
        readBackMs: 1_000,
        nowMs: () => page.nowMs,
      });
      const outcome = await probe.source.requestAdapter();
      if (outcome.kind !== "adapter") {
        throw new Error("the fake gave no adapter");
      }
      const engine: RenderEngine = await probe.source.load(outcome, STORE());
      views.installFrameClock(page);
      const stop = probe.start();
      const handing = new Set<string>();
      const blind = new Set<string>();
      const patches = { count: 0 };
      const lit = watchedSession(handing, patches, blind);
      probe.watchTerrain(
        through === "wire" ? { ...lit, pass: () => null, wirePass: (body) => lit.pass(body) } : lit,
      );
      const canvas = engine.createView(document.createElement("canvas"), "view");
      const draw = (terrain: boolean): void => {
        canvas.render(frame("wireframe", terrain ? ["wireframe:occluderSphere", material] : []));
      };
      const start = probe.mark();
      page.frame(100, () => {
        draw(false);
      });
      handing.add("view");
      patches.count = 12;
      page.frame(116, () => {
        draw(false);
      });
      page.frame(133, () => {
        draw(true);
      });
      page.frame(150, () => {
        draw(true);
      });
      blind.add("view");
      page.frame(166, () => {
        draw(false);
      });
      page.nowMs = 200;
      const record = probe.phase(
        "terrain-photoreal-wireframe",
        start,
        probe.mark(),
        [{ name: "view", style: "wireframe" }],
        null,
      );
      stop();
      const view = record.views[0];
      if (view === undefined) {
        throw new Error("the phase records the view");
      }
      return view;
    }

    it("counts a frame with the wireframe's terrain material as drawing the terrain", async () => {
      expect(await wireframeRecord(`${TERRAIN_WIRE_PREFIX}face-differences`, "lit")).toMatchObject({
        terrainDraws: 2,
        firstTerrainMs: 133,
      });
    });

    it("counts a view its wireframe pass took as handed over where it sees a patch, and off the survey where it sees none", async () => {
      expect(await wireframeRecord(`${TERRAIN_WIRE_PREFIX}face-differences`)).toMatchObject({
        draws: 5,
        handedOverDraws: 3,
        offSurveyDraws: 1,
        firstHandoverMs: 116,
      });
    });

    it("counts no terrain in a frame of the wireframe's other materials", async () => {
      expect(await wireframeRecord("wireframe:linesSurveyed")).toMatchObject({
        terrainDraws: 0,
        firstTerrainMs: null,
      });
    });
  });

  it("tallies the terrain's own allocations, by category, and nothing else's", async () => {
    const fake = fakeViewEngineSource();
    const listeners = new Set<(event: AllocationEvent) => void>();
    const emit = (event: AllocationEvent): void => {
      for (const listener of listeners) {
        listener(event);
      }
    };
    const inner: ViewEngineSource = {
      requestAdapter: fake.source.requestAdapter,
      load: async (outcome) =>
        Object.assign(new FakeRenderEngine(await outcome.adapter.requestDevice()), {
          onAllocation: (listener: (event: AllocationEvent) => void): (() => void) => {
            listeners.add(listener);
            return () => {
              listeners.delete(listener);
            };
          },
        }),
    };
    const probe = new TerrainProbe(new ViewsProbe(inner, () => 0), { paceMs: 0, readBackMs: 100 });
    const outcome = await probe.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error("the fake gave no adapter");
    }
    await probe.source.load(outcome, STORE());
    emit({ kind: "created", name: "terrain slots", bytes: 1_000, category: "height-cache" });
    emit({ kind: "created", name: "view:hdr", bytes: 500, category: "render-targets" });
    emit({ kind: "created", name: "coverage mask x", bytes: 24, category: "coarse-field-gpu" });
    emit({ kind: "destroyed", name: "terrain slots", bytes: 1_000, category: "height-cache" });
    probe.watchTerrain(watchedSession(new Set(), { count: 0 }));
    expect(probe.memory()).toMatchObject({
      liveBytes: [
        { category: "coarse-field-gpu", bytes: 24 },
        { category: "height-cache", bytes: 0 },
      ],
      peakBytes: [
        { category: "coarse-field-gpu", bytes: 24 },
        { category: "height-cache", bytes: 1_000 },
      ],
    });
  });
});

/** `view`'s scene target, as a photorealistic view makes it. */
function sceneTarget(engine: RenderEngine, view: string): { render(f: FrameSubmission): void } {
  return engine.createRenderTarget({
    name: `${view}:hdr`,
    size: { widthPx: 2, heightPx: 2 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
}

/** Lets every read-back answered so far settle: its check, its catch and its release. */
function settled(): Promise<void> {
  return Array.from({ length: 8 }).reduce<Promise<void>>(
    (before) => before.then(() => undefined),
    Promise.resolve(),
  );
}

/** Four finite half texels. */
function finiteTexels(): Promise<ArrayBuffer> {
  return Promise.resolve(new ArrayBuffer(2 * 2 * 4 * 2));
}

describe("the terrain probe's read-backs (R10.T16.d; the batch review's D-C)", () => {
  /** A probe over a fake engine whose read-backs `readTexture` answers, on a page the test runs. */
  async function probed(readTexture: (texture: TextureHandle) => Promise<ArrayBuffer>): Promise<{
    readonly probe: TerrainProbe;
    readonly page: FakePage;
    readonly engine: RenderEngine;
    readonly stop: () => void;
  }> {
    const timed = timedEngineSource();
    const page = new FakePage();
    const inner: ViewEngineSource = {
      requestAdapter: timed.source.requestAdapter,
      load: async (outcome, store) =>
        Object.assign(await timed.source.load(outcome, store), { readTexture }),
    };
    const views = new ViewsProbe(inner, () => page.nowMs);
    const probe = new TerrainProbe(views, {
      paceMs: 0,
      readBackMs: 1_000,
      nowMs: () => page.nowMs,
    });
    const outcome = await probe.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error("the fake gave no adapter");
    }
    const engine = await probe.source.load(outcome, STORE());
    views.installFrameClock(page);
    return { probe, page, engine, stop: probe.start() };
  }

  it("reads each view's scene target back once an interval, and credits it to the view and the phase", async () => {
    const { probe, page, engine, stop } = await probed(finiteTexels);
    const primary = sceneTarget(engine, "view");
    const instrument = sceneTarget(engine, "instrument-1");
    const start = probe.mark();
    const draw = (): void => {
      primary.render(frame("discs"));
      instrument.render(frame("discs"));
    };
    page.frame(100, draw);
    await settled();
    page.frame(600, draw);
    await settled();
    page.frame(1_200, draw);
    await settled();
    page.nowMs = 1_300;
    const record = probe.phase(
      "terrain-alone",
      start,
      probe.mark(),
      [
        { name: "view", style: "photorealistic" },
        { name: "instrument-1", style: "photorealistic" },
      ],
      null,
    );
    stop();
    expect(record.views.map((view) => [view.name, view.readBacks])).toEqual([
      ["view", 2],
      ["instrument-1", 2],
    ]);
  });

  it("reads a target back one at a time, a read-back in flight holding the next", async () => {
    let asked = 0;
    const { page, engine, stop } = await probed(() => {
      asked += 1;
      return new Promise<ArrayBuffer>(() => undefined);
    });
    const primary = sceneTarget(engine, "view");
    for (const atMs of [100, 1_200, 2_400]) {
      page.frame(atMs, () => {
        primary.render(frame("discs"));
      });
    }
    stop();
    expect(asked).toBe(1);
  });

  it.each([
    ["the loss refuses", new EngineUnavailable("RenderView.readBack"), 0],
    ["fails for another reason", new Error("a mapping refused"), 1],
  ] as const)(
    "counts no read-back the engine %s, and warns only of a failure",
    async (_name, refusal, warnings) => {
      const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
      const { probe, page, engine, stop } = await probed(() => Promise.reject(refusal));
      const primary = sceneTarget(engine, "view");
      const start = probe.mark();
      page.frame(100, () => {
        primary.render(frame("discs"));
      });
      await settled();
      page.nowMs = 200;
      const record = probe.phase(
        "terrain-alone",
        start,
        probe.mark(),
        [{ name: "view", style: "photorealistic" }],
        null,
      );
      stop();
      expect([record.views[0]?.readBacks, warn.mock.calls.length]).toEqual([0, warnings]);
    },
  );

  it("reads a region of a scene target back for the hand-over check (R10.T10.g)", async () => {
    const asked: Array<readonly [number | undefined, TexelRect | undefined]> = [];
    const { probe, engine, stop } = await probed(
      (_texture: TextureHandle, level?: number, rect?: TexelRect) => {
        asked.push([level, rect]);
        return finiteTexels();
      },
    );
    sceneTarget(engine, "view");
    const rect = { x: 0, y: 1, width: 2, height: 1 };
    const bytes = await probe.readSceneTarget("view:hdr", rect);
    stop();
    expect([bytes.byteLength, asked]).toEqual([32, [[0, rect]]]);
  });

  it("refuses to read back a scene target it was not made, or another target", async () => {
    const { probe, engine, stop } = await probed(finiteTexels);
    engine.createRenderTarget({
      name: "view:bloom",
      size: { widthPx: 2, heightPx: 2 },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    const rect = { x: 0, y: 0, width: 1, height: 1 };
    await expect(probe.readSceneTarget("view:hdr", rect)).rejects.toThrow(/no scene target/);
    await expect(probe.readSceneTarget("view:bloom", rect)).rejects.toThrow(/no scene target/);
    stop();
  });

  it("reads the coverage mask's texels that are not zero as its surveyed cells", async () => {
    const mask: TextureHandle = { kind: "texture", name: "coverage mask" };
    const texels = new Uint8Array(16);
    texels[3] = 255;
    texels[7] = 255;
    const { probe, stop } = await probed((texture) =>
      Promise.resolve(texture === mask ? texels.buffer : new ArrayBuffer(0)),
    );
    probe.watchTerrain({ ...watchedSession(new Set(), { count: 0 }), coverageMask: () => mask });
    const reading = await probe.readMask();
    stop();
    expect([...(reading?.cells ?? [])]).toEqual([3, 7]);
  });
});

describe("the terrain probe's watch of the session's terrain fault (R10.T16.e; decision-r10-t16-leans §8)", () => {
  const PRIMARY = viewId("view");
  const INSTRUMENT = viewId("instrument-1");

  it("records each time the fault holds a view or changes, with its time, its text and its views", () => {
    let nowMs = 1_000;
    const probe = new TerrainProbe(new ViewsProbe(), {
      paceMs: 0,
      readBackMs: 100,
      nowMs: () => nowMs,
    });
    const session = new FaultingSession();
    probe.watchTerrain(session.watched);
    const none = probe.terrainFaults();
    nowMs = 2_000;
    session.set(new Map([[PRIMARY, TERRAIN_COMPUTATION_FAILED]]));
    nowMs = 2_500;
    session.set(
      new Map([
        [PRIMARY, TERRAIN_COMPUTATION_FAILED],
        [INSTRUMENT, TERRAIN_COMPUTATION_FAILED],
      ]),
    );
    expect([none, probe.terrainFaults()]).toEqual([
      [],
      [
        { atMs: 2_000, text: TERRAIN_COMPUTATION_FAILED, views: ["view"] },
        { atMs: 2_500, text: TERRAIN_COMPUTATION_FAILED, views: ["view", "instrument-1"] },
      ],
    ]);
  });

  it("records a fault standing when the watch begins, and stops listening at its release", () => {
    const probe = new TerrainProbe(new ViewsProbe(), {
      paceMs: 0,
      readBackMs: 100,
      nowMs: () => 0,
    });
    const session = new FaultingSession();
    session.set(new Map([[PRIMARY, TERRAIN_NOT_AVAILABLE]]));
    const release = probe.watchTerrain(session.watched);
    release();
    session.set(new Map());
    expect([probe.terrainFaults(), session.listening]).toEqual([
      [{ atMs: 0, text: TERRAIN_NOT_AVAILABLE, views: ["view"] }],
      0,
    ]);
  });
});
