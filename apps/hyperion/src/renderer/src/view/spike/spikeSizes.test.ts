import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import type {
  SpikeSizesColumn,
  SpikeSizesCountColumns,
  SpikeSizesMemory,
} from "../../../../preload/api";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { goldenLevelTable } from "../../test/terrainFixtures";
import type { RenderEngine } from "../engine/types";
import { lookAlong } from "../camera/quaternion";
import { newCameraState, viewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES, requestAdapterOutcome } from "../engine/platform";
import { cameraSceneOf } from "../scene/model";
import { bindingCeilingSlots, WEBGPU_DEFAULT_STORAGE_BINDING_BYTES } from "../terrain/cacheBudget";
import type { AllocationEvent, MemoryCategory } from "../engine/memory";
import { planetGeometry } from "../terrain/planet";
import type { PatchRequest } from "../terrain/select";
import { CASCADE_BYTES, CASCADE_TEXTURE } from "../terrain/shadows/cascades";
import { TerrainPass, type TerrainPassView, type TerrainPool } from "../terrain/terrainPass";
import type { BakedPatch } from "../terrain/workers/messages";
import { DescentProfile, landingSiteOf } from "./descentProfile";
import { spikeScene, TEST_PLANET_FIGURE } from "./spikeScene";
import {
  matchViewsByWeight,
  SECOND_VIEW_ID,
  secondsOf,
  secondViewStyle,
  segmentRunsOf,
  type SpikeSizesFrame,
  type SpikeSizesSession,
  type SpikeSizesTerrainPass,
  SpikeSizes,
  sizesColumn,
  STAND_IN_ONLY,
  TOUCHDOWN_TOLERANCE_M,
  touchedDown,
  withheldFailures,
} from "./spikeSizes";
import { TEST_PLANET_WORLD } from "./spikeWorld";

const PLANET = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n));
const MAIN_ID = viewId("spike");
const BODY = TEST_PLANET_WORLD.body;
const VIEWPORT = { widthPx: 640, heightPx: 360 } as const;
const CRAFT_VIEWPORT = { widthPx: 320, heightPx: 180 } as const;

/** A pool that records the demand and bakes nothing. */
class IdlePool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }
  onBaked(_cb: (bake: BakedPatch) => void): () => void {
    return () => undefined;
  }
  terminate(): void {}
}

/** A test-planet pass on `engine`. */
function passOn(engine: RenderEngine): TerrainPass {
  return new TerrainPass({
    engine,
    setting: "high",
    planet: PLANET,
    source: { kind: "test-planet", ridges: "off" },
    createPool: () => new IdlePool(),
  });
}

/** The pass's views 1,000 km above the pole: `down` looking at the planet, the other away from it. */
function views(order: "main first" | "second first", tiltRad = 0): TerrainPassView[] {
  const cameraM = { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 1_000_000 };
  const main: TerrainPassView = {
    id: MAIN_ID,
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM,
      orientation: lookAlong(
        { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
        { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
      ),
      fovXRad: Math.PI / 3,
      viewport: VIEWPORT,
    },
    tauPx: 1,
    weight: 1,
  };
  const second: TerrainPassView = {
    id: SECOND_VIEW_ID,
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM,
      // Looking straight up, away from the planet: it sees nothing.
      orientation: lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 }),
      fovXRad: Math.PI / 3,
      viewport: CRAFT_VIEWPORT,
    },
    tauPx: 1,
    weight: 0.25,
  };
  return order === "main first" ? [main, second] : [second, main];
}

/** A creation or destruction of `bytes` under `category`, named `name`. */
function allocation(
  kind: "created" | "destroyed",
  category: MemoryCategory,
  bytes: number,
  name = `a test's ${category}`,
): AllocationEvent {
  return { kind, name, bytes, category };
}

/** A session stand-in whose passes the test sets, its memory a test's too. */
class SizedSession implements SpikeSizesSession {
  lit: SpikeSizesTerrainPass | null = null;
  wire: SpikeSizesTerrainPass | null = null;
  workerField = 1_000;
  /** The session's textures by category, as its memory reports them. */
  readonly gpu = new Map<MemoryCategory, number>([["coarse-field-gpu", 4_096]]);
  readonly workerCount = 2;

  pass(_body: BodyIdHex): SpikeSizesTerrainPass | null {
    return this.lit;
  }

  wirePass(_body: BodyIdHex): SpikeSizesTerrainPass | null {
    return this.wire;
  }

  memory(): {
    workerFieldBytes: number;
    keptFieldBytes: number;
    gpuBytes: Map<MemoryCategory, number>;
  } {
    return {
      workerFieldBytes: this.workerField,
      keptFieldBytes: 500,
      gpuBytes: new Map(this.gpu),
    };
  }

  survey(): null {
    return null;
  }
}

/** The frame at script time `tS`, its clock `nowMs`. */
function frameAt(session: SpikeSizesSession, tS: number, nowMs = 1000 * tS): SpikeSizesFrame {
  const pose = PROFILE.poseAt(tS);
  return {
    session,
    body: BODY,
    nowMs,
    tS,
    pose,
    underWay: tS < PROFILE.durationS,
    touchedDown: touchedDown(PROFILE, pose),
    mainViewport: VIEWPORT,
    secondViewport: CRAFT_VIEWPORT,
  };
}

/** One frame of the run: the moment, the passes stepped as the session would, then the reading. */
function play(
  sizes: SpikeSizes,
  session: SizedSession,
  tS: number,
  step: () => void = () => undefined,
): void {
  const frame = frameAt(session, tS);
  sizes.beforeStep(frame);
  step();
  sizes.afterStep(frame);
}

/** A count's five columns, in their order. */
function countsOf(columns: SpikeSizesCountColumns): SpikeSizesColumn[] {
  return [
    columns.frustum,
    columns.frustumNoMargin,
    columns.allRound,
    columns.frustumDiagnostic,
    columns.allRoundDiagnostic,
  ];
}

function sizesOf(setting: "high" | "low", style: "photorealistic" | "wireframe"): SpikeSizes {
  return new SpikeSizes({
    setting,
    style,
    mainViewId: MAIN_ID,
    secondViewName: "spike-craft",
    nowMs: () => 0,
  });
}

describe("the second view's style (decision-r10-t14b-measure §4)", () => {
  it.each([
    ["high", "photorealistic", "photorealistic"],
    ["high", "wireframe", "wireframe"],
    ["low", "photorealistic", "wireframe"],
    ["low", "wireframe", "wireframe"],
  ] as const)("on %s with the main view %s, is %s", (setting, main, second) => {
    expect(secondViewStyle(setting, main)).toBe(second);
    expect(sizesOf(setting, main).secondStyle).toBe(second);
  });
});

describe("finding the views in a selection's input", () => {
  const main = { weight: 1, viewport: VIEWPORT };
  const second = { weight: 0.25, viewport: CRAFT_VIEWPORT };

  it("finds each view by its weight, wherever the input holds it", () => {
    expect(
      matchViewsByWeight(
        [main, second],
        [
          { weight: 0.25, viewport: CRAFT_VIEWPORT },
          { weight: 1, viewport: VIEWPORT },
        ],
      ),
    ).toEqual([1, 0]);
  });

  it("finds a view by its viewport where weights tie, and none where the tie stands", () => {
    const tied = [
      { weight: 1, viewport: CRAFT_VIEWPORT },
      { weight: 1, viewport: VIEWPORT },
    ];
    expect(matchViewsByWeight([main], tied)).toEqual([1]);
    expect(matchViewsByWeight([{ weight: 1, viewport: null }], tied)).toEqual([null]);
  });

  it("finds none of a view the input does not hold, and never one input view twice", () => {
    expect(matchViewsByWeight([main, second], [{ weight: 1, viewport: VIEWPORT }])).toEqual([
      0,
      null,
    ]);
    expect(matchViewsByWeight([main, main], [{ weight: 1, viewport: VIEWPORT }])).toEqual([
      0,
      null,
    ]);
  });
});

describe("the samples' seconds and segments", () => {
  it("writes the seconds as the first and those skipped", () => {
    expect([secondsOf([4, 5, 7, 10]), secondsOf([])]).toEqual([
      { first: 4, skipped: [6, 8, 9] },
      { first: 0, skipped: [] },
    ]);
  });

  it("writes the segments as runs from each one's first sample", () => {
    expect(segmentRunsOf([0, 0, 1, 1, 1, 2, -1])).toEqual([
      { segment: 0, from: 0 },
      { segment: 1, from: 2 },
      { segment: 2, from: 5 },
      { segment: -1, from: 6 },
    ]);
  });
});

describe("a column of the samples", () => {
  it("writes a missing sample as -1 within a gap, a run of one reason one gap", () => {
    const reasons = ["a", "a", "a", "b", "b"];
    expect(sizesColumn([3, null, null, null, 7.4, null], (i) => reasons[i] ?? "c")).toEqual({
      samples: [3, -1, -1, -1, 7, -1],
      gaps: [
        { from: 1, to: 2, reason: "a" },
        { from: 3, to: 3, reason: "b" },
        { from: 5, to: 5, reason: "c" },
      ],
    });
  });
});

describe("the measured sizes of a run (decision-r10-t14b-measure §2–§6)", () => {
  it("counts each cache's pass from its first selection after it appears, and again on a new pass object", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("high", "photorealistic");
    play(sizes, session, 0);
    const first = passOn(engine);
    session.lit = first;
    // The session makes and steps a pass in one step, before it can be observed.
    play(sizes, session, 1, () => first.step({ views: views("main first"), grounded: [] }));
    play(sizes, session, 2, () => first.step({ views: views("main first", 0.3), grounded: [] }));
    const again = passOn(engine);
    session.lit = again;
    play(sizes, session, 3, () => again.step({ views: views("main first"), grounded: [] }));
    play(sizes, session, 4, () => again.step({ views: views("main first", 0.3), grounded: [] }));
    const [lit, ...others] = sizes.report().passes;
    expect([lit?.cache, lit?.attachments, lit?.samples, lit?.series?.seconds, others]).toEqual([
      "lit",
      2,
      2,
      // Seconds 2 and 4: second 3's step, the new pass's first, came before it was observed.
      { first: 2, skipped: [3] },
      [],
    ]);
  });

  it("describes each cache's views in its style, and finds them in the input by their weight", async () => {
    const engine = await countingRenderEngine();
    const counted = (order: "main first" | "second first"): ReadonlyArray<number | undefined> => {
      const session = new SizedSession();
      const sizes = sizesOf("high", "photorealistic");
      const pass = passOn(engine);
      session.lit = pass;
      play(sizes, session, 0, () => pass.step({ views: views(order), grounded: [] }));
      play(sizes, session, 1, () => pass.step({ views: views(order, 0.3), grounded: [] }));
      const lit = sizes.report().passes[0];
      expect(lit?.views).toEqual([
        { id: MAIN_ID, style: "photorealistic", weight: 1 },
        { id: SECOND_VIEW_ID, style: "photorealistic", weight: 0.25 },
      ]);
      return lit?.series?.views.map((view) => view.frustum.samples[0]) ?? [];
    };
    const inOrder = counted("main first");
    // The main view looks down at the planet and the second away from it, so that their counts
    // differ: each is found where the input holds it.
    expect(inOrder[0]).toBeGreaterThan(inOrder[1] ?? 0);
    expect(counted("second first")).toEqual(inOrder);
  });

  it("gives a cache of one described view its own columns for the unions", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    // Low's photorealistic run: the lit cache's view the main one, the wireframe's the second.
    const sizes = sizesOf("low", "photorealistic");
    const lit = passOn(engine);
    const wire = passOn(engine);
    session.lit = lit;
    session.wire = wire;
    play(sizes, session, 0, () => {
      lit.step({ views: views("main first").slice(0, 1), grounded: [] });
      wire.step({ views: views("main first").slice(1), grounded: [] });
    });
    play(sizes, session, 1, () => {
      lit.step({ views: views("main first", 0.3).slice(0, 1), grounded: [] });
      wire.step({ views: views("main first", 0.3).slice(1), grounded: [] });
    });
    const report = sizes.report();
    expect(
      report.passes.map((pass) => [
        pass.cache,
        pass.views.map((view) => view.id),
        pass.series?.union,
        pass.series?.unionDiffers,
      ]),
    ).toEqual([
      ["lit", [MAIN_ID], null, 0],
      ["wireframe", [SECOND_VIEW_ID], null, 0],
    ]);
  });

  it("compares the two passes' forced sets after each step (T14.c's finding 4)", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("low", "photorealistic");
    const lit = passOn(engine);
    const wire = passOn(engine);
    session.lit = lit;
    session.wire = wire;
    const contact = { positionM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 5 }, radiusM: 20 };
    play(sizes, session, 0, () => {
      lit.step({ views: views("main first"), grounded: [contact] });
      wire.step({ views: views("main first"), grounded: [contact] });
    });
    // The same frame read again: no pass selected since.
    play(sizes, session, 0.5);
    play(sizes, session, 1, () => {
      lit.step({ views: views("main first", 0.3), grounded: [contact] });
      wire.step({ views: views("main first", 0.3), grounded: [] });
    });
    expect(sizes.report().forcedSets).toEqual({
      value: {
        comparedFrames: 3,
        differingFrames: 1,
        firstDifferingS: 1,
        largestDifference: expect.any(Number),
      },
      reason: null,
    });
  });

  it("states no comparison where the two passes never stood in the same frame", () => {
    const sizes = sizesOf("high", "wireframe");
    play(sizes, new SizedSession(), 0);
    expect(sizes.report().forcedSets.value).toBeNull();
  });

  it("reads the session's memory once a second, the caches' allocations, and the cascades as the engine allocated them, counted once", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    // The session's own figure for the set the engine allocates.
    session.gpu.set("shadows", CASCADE_BYTES);
    const sizes = sizesOf("high", "photorealistic");
    const pass = passOn(engine);
    session.lit = pass;
    sizes.noteAllocation(allocation("created", "render-targets", 1_000));
    for (const [tS, field] of [
      [0, 1_000],
      [0.5, 9_000],
      [1, 3_000],
      [2.5, 2_000],
    ] as const) {
      session.workerField = field;
      play(sizes, session, tS, () => {
        pass.step({ views: views("main first"), grounded: [] });
        if (tS === 1) {
          sizes.noteAllocation(
            allocation("created", "shadows", CASCADE_BYTES, CASCADE_TEXTURE.name),
          );
        }
      });
    }
    const { memory } = sizes.report();
    const slots = pass.layout.slots;
    const cache = slots.slotCount * slots.bytesPerSlot;
    expect(memory).toEqual({
      samples: 3,
      workerFieldBytes: { bytes: 3_000, atS: 1 },
      keptFieldBytes: { bytes: 500, atS: 0 },
      sessionGpu: [
        { category: "coarse-field-gpu", bytes: 4_096, atS: 0 },
        { category: "shadows", bytes: CASCADE_BYTES, atS: 0 },
      ],
      caches: [
        {
          cache: "lit",
          slotCount: slots.slotCount,
          bytesPerSlot: slots.bytesPerSlot,
          allocationBytes: cache,
        },
      ],
      cascades: {
        bytes: CASCADE_BYTES,
        source: "engine",
        reason: null,
        specBytes: CASCADE_BYTES,
        differsFromSpec: false,
      },
      engine: {
        categories: [
          { category: "render-targets", bytes: 1_000, atS: null },
          { category: "shadows", bytes: CASCADE_BYTES, atS: 1 },
        ],
        peak: { bytes: 1_000 + CASCADE_BYTES, atS: 1 },
        unseenDestroyed: 0,
      },
      itemizedGpuBytes: cache + 4_096 + CASCADE_BYTES,
      itemizedTotalBytes: cache + 4_096 + CASCADE_BYTES + 3_000,
      workersStarted: 2,
    });
  });

  it("states the adapter's, the request's and the device's storage binding, and each cache's ceiling", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("high", "photorealistic");
    const adapterBytes = 2 * MAX_REQUESTED_BUFFER_BYTES;
    sizes.noteAdapter(
      await requestAdapterOutcome(
        new FakeGpu([
          new FakeAdapter({
            info: SWIFTSHADER_INFO,
            features: [],
            maxStorageBufferBindingSize: adapterBytes,
          }),
        ]),
      ),
    );
    sizes.noteDevice({ maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES });
    const pass = passOn(engine);
    session.lit = pass;
    play(sizes, session, 0, () => pass.step({ views: views("main first"), grounded: [] }));
    const fields = pass.layout.slots.fields;
    expect(sizes.report().binding).toEqual({
      adapterBytes: { value: adapterBytes, reason: null },
      requestedBytes: { value: MAX_REQUESTED_BUFFER_BYTES, reason: null },
      deviceBytes: { value: MAX_REQUESTED_BUFFER_BYTES, reason: null },
      caches: [
        {
          cache: "lit",
          slotCount: pass.layout.slots.slotCount,
          ceilingAtRequestedSlots: bindingCeilingSlots(fields, MAX_REQUESTED_BUFFER_BYTES),
          ceilingAtDefaultSlots: bindingCeilingSlots(fields, WEBGPU_DEFAULT_STORAGE_BINDING_BYTES),
        },
      ],
    });
  });

  it("states why the binding is missing where no adapter answered", async () => {
    const sizes = sizesOf("high", "photorealistic");
    sizes.noteAdapter(await requestAdapterOutcome(new FakeGpu([])));
    expect(sizes.report().binding.adapterBytes).toEqual({
      value: null,
      reason: "no adapter (no-adapter)",
    });
  });

  it("starts the height workers on the bake clock, its flight from the first frame", () => {
    const sizes = sizesOf("high", "photorealistic");
    const made: ScriptedHeightWorker[] = [];
    const createWorker = sizes.heightWorkers(() => {
      const worker = new ScriptedHeightWorker();
      made.push(worker);
      return worker;
    });
    const worker = createWorker();
    worker.postMessage({ kind: "field-release", id: 1, body: BODY }, []);
    play(sizes, new SizedSession(), 0);
    expect([
      made.length,
      made[0]?.received.length,
      sizes.report().bakes.workersMade,
      sizes.clock.summary().flightStartMs,
    ]).toEqual([1, 1, 1, 0]);
    // The message before the flight's start is left out of its figures.
    expect(sizes.report().bakes.requests.find(({ kind }) => kind === "field-release")).toEqual({
      kind: "field-release",
      count: 0,
    });
  });

  it("marks the contact by the session's rule for each counter once", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("high", "photorealistic");
    const pass = passOn(engine);
    session.lit = pass;
    play(sizes, session, 0, () => pass.step({ views: views("main first"), grounded: [] }));
    play(sizes, session, 7);
    sizes.markContact();
    play(sizes, session, 9);
    sizes.markContact();
    expect(sizes.report().passes[0]?.contact.sessionContact?.atS).toBe(7);
  });

  it("marks touchdown at the first frame whose pose has reached the ground", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("high", "photorealistic");
    const pass = passOn(engine);
    session.lit = pass;
    play(sizes, session, 0, () => pass.step({ views: views("main first"), grounded: [] }));
    play(sizes, session, PROFILE.durationS - 40);
    play(sizes, session, PROFILE.durationS - 5);
    play(sizes, session, PROFILE.durationS);
    expect(sizes.report().passes[0]?.contact.touchdown?.atS).toBe(PROFILE.durationS - 5);
  });

  it("counts the readout's publishes and those holding an answer", () => {
    const sizes = sizesOf("high", "photorealistic");
    const kept = spikeScene(PROFILE, TEST_PLANET_WORLD);
    const scene = kept.sceneAt(0);
    sizes.publishReadout(
      null,
      {
        source: { kind: "kept", kept },
        tS: 0,
        scene,
        camera: newCameraState(cameraSceneOf(scene), "camera"),
      },
      0,
    );
    expect(sizes.report().readouts).toEqual({
      intervalMs: 250,
      publishes: 1,
      answeredPublishes: 0,
      asked: 0,
      answered: 0,
      failed: 0,
      failComputationCalls: 0,
    });
  });

  it("reports a block of finite numbers whose every column is as long as its samples", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    const sizes = sizesOf("low", "photorealistic");
    const lit = passOn(engine);
    const wire = passOn(engine);
    session.lit = lit;
    session.wire = wire;
    for (const tS of [0, 1, 2, 3]) {
      play(sizes, session, tS, () => {
        lit.step({ views: views("main first", tS / 10).slice(0, 1), grounded: [] });
        wire.step({ views: views("main first", tS / 10), grounded: [] });
      });
    }
    const report = sizes.report();
    expect(JSON.parse(JSON.stringify(report))).toEqual(report);
    const sized = report.passes.flatMap(({ series, samples }) =>
      series === null ? [] : [{ series, samples }],
    );
    expect(sized).toHaveLength(2);
    for (const { series, samples } of sized) {
      const length = series.heightAboveGroundM.length;
      const columns = [
        ...series.views.flatMap(countsOf),
        ...(series.union === null ? [] : countsOf(series.union)),
        series.forcedResident,
        series.allRoundResident,
        series.usedSlots,
      ];
      expect([length, columns.every((column) => column.samples.length === length)]).toEqual([
        samples,
        true,
      ]);
    }
    expect(report.segments).toEqual([PROFILE.poseAt(0).segment]);
  });

  it("reports a pass with no view of its cache's style as a stand-in, not sized, its samples left out", async () => {
    const engine = await countingRenderEngine();
    const session = new SizedSession();
    // High's photorealistic run: its wireframe pass only stands in while the pipelines compile.
    const sizes = sizesOf("high", "photorealistic");
    const lit = passOn(engine);
    const wire = passOn(engine);
    session.lit = lit;
    session.wire = wire;
    for (const tS of [0, 1]) {
      play(sizes, session, tS, () => {
        lit.step({ views: views("main first", tS / 10), grounded: [] });
        wire.step({ views: views("main first", tS / 10), grounded: [] });
      });
    }
    const [sized, standIn] = sizes.report().passes;
    expect(sized?.notSized).toBeNull();
    expect(standIn).toMatchObject({
      cache: "wireframe",
      views: [],
      rule: null,
      ruleDiagnostic: null,
      occluderPrice: null,
      series: null,
    });
    expect(standIn?.notSized?.startsWith(`${STAND_IN_ONLY}: `)).toBe(true);
    expect(standIn?.samples).toBeGreaterThan(0);
  });
});

describe("the engine's allocations under the measured sizes (R10.T9.b as built)", () => {
  /** `sizes` told of `events` at script times, each in its own frame's step. */
  function noted(
    sizes: SpikeSizes,
    events: ReadonlyArray<readonly [number, AllocationEvent]>,
  ): SpikeSizesMemory {
    const session = new SizedSession();
    for (const [tS, event] of events) {
      play(sizes, session, tS, () => {
        sizes.noteAllocation(event);
      });
    }
    return sizes.report().memory;
  }

  it.each([
    ["low", "photorealistic", /^the low setting has no shadow cascades/],
    ["low", "wireframe", /^the low setting has no shadow cascades/],
    ["high", "wireframe", /^the wireframe alone was drawn/],
  ] as const)(
    "states none and why where none was allocated, on %s in the %s",
    (setting, style, why) => {
      const sizes = sizesOf(setting, style);
      const memory = noted(sizes, [[0, allocation("created", "height-cache", 2_048)]]);
      expect(memory.cascades).toEqual({
        bytes: 0,
        source: "none",
        reason: expect.stringMatching(why),
        specBytes: CASCADE_BYTES,
        differsFromSpec: false,
      });
    },
  );

  it("itemizes no cascades where none was allocated", () => {
    const memory = noted(sizesOf("low", "photorealistic"), [
      [0, allocation("created", "height-cache", 2_048)],
    ]);
    expect([memory.itemizedGpuBytes, memory.itemizedTotalBytes]).toEqual([4_096, 4_096 + 1_000]);
  });

  it("flags cascades whose allocated bytes differ from T9.a's spec", () => {
    const memory = noted(sizesOf("high", "photorealistic"), [
      [0, allocation("created", "shadows", CASCADE_BYTES / 4, CASCADE_TEXTURE.name)],
    ]);
    expect(memory.cascades).toEqual({
      bytes: CASCADE_BYTES / 4,
      source: "engine",
      reason: null,
      specBytes: CASCADE_BYTES,
      differsFromSpec: true,
    });
  });

  it("lowers a category's live bytes at a destruction, the whole tally's peak the peak of the sum", () => {
    const memory = noted(sizesOf("high", "photorealistic"), [
      [0, allocation("created", "height-cache", 100, "first cache")],
      [1, allocation("destroyed", "height-cache", 100, "first cache")],
      [2, allocation("created", "shadows", 200)],
      [3, allocation("created", "height-cache", 80, "second cache")],
    ]);
    // The categories' peaks sum to 300; the tally never held more than 280 at once.
    expect(memory.engine).toEqual({
      categories: [
        { category: "height-cache", bytes: 100, atS: 0 },
        { category: "shadows", bytes: 200, atS: 2 },
      ],
      peak: { bytes: 280, atS: 3 },
      unseenDestroyed: 0,
    });
  });

  it("takes nothing away for an upload, or for a destruction whose creation it never saw", () => {
    const memory = noted(sizesOf("high", "photorealistic"), [
      [0, allocation("created", "height-cache", 100, "seen")],
      [1, { kind: "uploaded", name: "seen", bytes: 100 }],
      [2, allocation("destroyed", "height-cache", 100, "made before the subscription")],
      [3, allocation("created", "other", 10)],
    ]);
    expect(memory.engine).toEqual({
      categories: [
        { category: "height-cache", bytes: 100, atS: 0 },
        { category: "other", bytes: 10, atS: 3 },
      ],
      peak: { bytes: 110, atS: 3 },
      unseenDestroyed: 1,
    });
  });
});

/** Two height-worker factories alike, but for their identity. */
function makeWorker(): ScriptedHeightWorker {
  return new ScriptedHeightWorker();
}
function makeOtherWorker(): ScriptedHeightWorker {
  return new ScriptedHeightWorker();
}

describe("the height workers' factory under the measured sizes", () => {
  it("is the same function for the same factory, and another for another", () => {
    const sizes = sizesOf("high", "photorealistic");
    expect([
      sizes.heightWorkers(makeWorker) === sizes.heightWorkers(makeWorker),
      sizes.heightWorkers(makeWorker) === sizes.heightWorkers(makeOtherWorker),
    ]).toEqual([true, false]);
  });
});

describe("touchdown (T14.c's finding 3)", () => {
  it("is the first 64 Hz pose at the script's last clearance, within the hover's last second", () => {
    const hover = PROFILE.segmentSpans().at(-1);
    const restM = PROFILE.segments.at(-1)?.endAltitudeM ?? 0;
    let firstS: number | null = null;
    for (let k = 0; k <= 64 * PROFILE.durationS && firstS === null; k += 1) {
      const pose = PROFILE.poseAt(k / 64);
      firstS = touchedDown(PROFILE, pose) ? pose.tS : null;
    }
    // The hover descends 0.05 m/s from 2 m to its stop at 19.5 s and halts in the second after.
    const stopS = (hover?.startS ?? 0) + 20.5;
    expect([firstS ?? 0, PROFILE.poseAt(firstS ?? 0).clearanceM]).toEqual([
      expect.closeTo(stopS - Math.sqrt(TOUCHDOWN_TOLERANCE_M / 0.025), 1),
      expect.closeTo(restM + TOUCHDOWN_TOLERANCE_M, 2),
    ]);
  });

  it("is the script's end where its pose never comes down to its rest", () => {
    expect(touchedDown(PROFILE, { tS: PROFILE.durationS, clearanceM: 50 })).toBe(true);
    expect(touchedDown(PROFILE, { tS: 0, clearanceM: 50 })).toBe(false);
  });
});

describe("the readout's stand-in of the session (§6(f))", () => {
  /** A session stand-in with a private field its methods read, as `TerrainSession`'s do. */
  class PrivateSession {
    readonly #failures: string[] = [];
    readonly name = "session";

    failComputation(): void {
      this.#failures.push("failed");
    }

    get failures(): number {
      return this.#failures.length;
    }

    record(): number {
      this.#failures.push("recorded");
      return this.#failures.length;
    }
  }

  it("counts a call to fail the session's computation and passes none on", () => {
    const session = new PrivateSession();
    let withheld = 0;
    const standIn = withheldFailures(session, () => {
      withheld += 1;
    });
    standIn.failComputation();
    standIn.failComputation();
    expect([withheld, session.failures]).toEqual([2, 0]);
  });

  it("is the session in every other member, its methods reading its own private fields", () => {
    const session = new PrivateSession();
    const standIn = withheldFailures(session, () => undefined);
    expect([standIn.name, standIn.record(), standIn.failures, session.failures]).toEqual([
      "session",
      1,
      1,
      1,
    ]);
  });
});
