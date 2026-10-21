import type { BodyIdHex } from "@hyperion/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PHOTOREAL_NOT_CREATED } from "../../displays/view/styleRefusals";
import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { readTokens } from "../../spatial/paint";
import { lookAlong } from "../camera/quaternion";
import { type RenderStyle, type ViewId, viewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { normalsAtlasOf } from "../terrain/gpu/resources";
import { planetGeometry } from "../terrain/planet";
import type { PatchRequest, Selection } from "../terrain/select";
import type { TerrainViewReport } from "../terrain/session";
import { type TerrainFrameInput, TerrainPass, type TerrainPool } from "../terrain/terrainPass";
import { type PatchKey, patchKeyString } from "../terrain/patchKey";
import type { BakedPatch } from "../terrain/workers/messages";
import {
  SurfaceQuery,
  type SurfaceQueryReply,
  type SurfaceQueryRequest,
  type SurfaceQueryWorker,
} from "./surfaceQuery";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import { SEGMENT_MEASURE_PREFIX } from "./metrics";
import { contactRule, TEST_PLANET_FIGURE } from "./spikeScene";
import { viewStrokesAt } from "../wireframe/drawList";
import { stretchKeys } from "./demandRecord";
import {
  DescentRefused,
  defaultSpikeWorkers,
  FRAME_MEASURE,
  prepareDescent,
  type PreparedDescent,
  type SpikeFrameInput,
  type SpikeFrameSample,
  type SpikePatchEvent,
  SpikeRun,
  type SpikeTerrainSession,
  SPIKE_VIEW_NAMES,
} from "./spikeRun";
import { TEST_PLANET_WORLD } from "./spikeWorld";

const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n));

const PREPARED: PreparedDescent = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: PROFILE,
  contact: contactRule(PROFILE),
  siteHeightM: 0,
  stretches: trackStretches(PROFILE),
  stretchMaxHeightsM: trackStretches(PROFILE).map(() => 0),
  trackMaxHeightM: 0,
  omittedSigmaM: new Float64Array(25),
};

/** A pool that records the demand and bakes nothing. */
class IdlePool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  terminated = false;
  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }
  onBaked(_cb: (bake: BakedPatch) => void): () => void {
    return () => undefined;
  }
  terminate(): void {
    this.terminated = true;
  }
}

const SIZES = {
  main: { widthPx: 64, heightPx: 36 },
  orbit: { widthPx: 32, heightPx: 24 },
  craft: { widthPx: 32, heightPx: 24 },
} as const;

function canvas(): HTMLCanvasElement {
  return document.createElement("canvas");
}

function input(nowMs: number): SpikeFrameInput {
  return {
    nowMs,
    sizes: SIZES,
    tokens: readTokens(document.documentElement),
    remPx: 16,
    strokes: viewStrokesAt(1),
    selection: null,
  };
}

interface Made {
  readonly run: SpikeRun;
  readonly engine: CountingRenderEngine;
  readonly pools: IdlePool[];
  readonly samples: SpikeFrameSample[];
  readonly patches: Array<readonly [SpikePatchEvent, string]>;
}

async function made(engine?: CountingRenderEngine): Promise<Made> {
  const counting = engine ?? (await countingRenderEngine());
  const pools: IdlePool[] = [];
  const samples: SpikeFrameSample[] = [];
  const patches: Array<readonly [SpikePatchEvent, string]> = [];
  const run = new SpikeRun(
    counting,
    PREPARED,
    {
      setting: "low",
      ridges: "off",
      createPool: () => {
        const pool = new IdlePool();
        pools.push(pool);
        return pool;
      },
    },
    { main: canvas(), orbit: canvas(), craft: canvas() },
    {
      onFrame: (sample) => samples.push(sample),
      onPatch: (event, key) => patches.push([event, `${key.face}/${key.level}/${key.i}/${key.j}`]),
    },
  );
  return { run, engine: counting, pools, samples, patches };
}

afterEach(() => {
  performance.clearMeasures();
});

describe("the spike's run", () => {
  it("starts the script at its first frame and holds its end", async () => {
    const { run } = await made();
    expect(run.frame(input(5_000)).tS).toBe(0);
    expect(run.frame(input(5_000 + 400_000)).tS).toBe(400);
    expect(run.frame(input(5_000 + 1e9)).tS).toBe(PROFILE.durationS);
    run.dispose();
  });

  it("tells every frame where script time starts: its first frame's timestamp", async () => {
    const { run, samples } = await made();
    run.frame(input(5_000));
    run.frame(input(5_000 + 400_000));
    expect(samples.map(({ scriptStartMs }) => scriptStartMs)).toEqual([5_000, 5_000]);
    run.dispose();
  });

  it("tells each frame's sample: the script's time, the passes and the patches", async () => {
    const { run, samples, engine } = await made();
    const before = engine.inner.views.flatMap((view) => view.frames).length;
    const frame = run.frame(input(0));
    expect(samples).toHaveLength(1);
    expect(samples[0]?.scriptTimeS).toBe(0);
    expect(samples[0]?.patchesHard).toBe(frame.terrain.selected);
    // The display pass and the two instruments into the views; terrain and sky into targets.
    expect(engine.inner.views.flatMap((view) => view.frames).length - before).toBe(3);
    expect(samples[0]?.passesSubmitted).toBe(5);
    run.dispose();
  });

  it("spans each frame's whole callback once, from its callbackStartMs for its callbackMs, and keeps no entry of it", async () => {
    const measure = vi.spyOn(performance, "measure");
    const { run, samples } = await made();
    run.frame(input(0));
    run.frame(input(16.7));
    expect(performance.getEntriesByName(FRAME_MEASURE)).toEqual([]);
    run.dispose();
    const spans = measure.mock.calls.flatMap(([name, options]) =>
      name === FRAME_MEASURE &&
      typeof options === "object" &&
      typeof options.start === "number" &&
      typeof options.end === "number"
        ? [{ startMs: options.start, durationMs: options.end - options.start }]
        : [],
    );
    expect(spans).toHaveLength(2);
    // The same numbers the measure received, which the trace carries as its `args.startTime`.
    expect(spans).toEqual(
      samples.map(({ callbackStartMs, callbackMs }) => ({
        startMs: callbackStartMs,
        durationMs: callbackMs,
      })),
    );
  });

  it("names the frame's span as the main process's reducer reads it", () => {
    // `reduceTrace.ts` holds its own copy of the literal, pinned by its test too.
    expect(FRAME_MEASURE).toBe("spike.frame");
  });

  it("tells of each patch it first requests", async () => {
    const { run, pools, patches } = await made();
    run.frame(input(0));
    const demanded = (pools[0]?.demand ?? []).map(
      ({ key }) => `${key.face}/${key.level}/${key.i}/${key.j}`,
    );
    expect(demanded.length).toBeGreaterThan(0);
    expect(patches).toEqual(demanded.map((key) => ["requested", key]));
    run.dispose();
  });

  it("spans every segment of the script once, the last closed at the script's end", async () => {
    const { run } = await made();
    run.frame(input(0));
    for (const span of PROFILE.segmentSpans()) {
      run.frame(input(1000 * ((span.startS + span.endS) / 2)));
    }
    run.frame(input(1000 * PROFILE.durationS));
    run.frame(input(1000 * PROFILE.durationS + 16));
    run.dispose();
    for (const span of PROFILE.segmentSpans()) {
      expect(performance.getEntriesByName(`${SEGMENT_MEASURE_PREFIX}${span.name}`)).toHaveLength(1);
    }
  });

  it("closes the open segment's span when disposed", async () => {
    const { run } = await made();
    run.frame(input(0));
    run.frame(input(30_000));
    run.dispose();
    expect(performance.getEntriesByName(`${SEGMENT_MEASURE_PREFIX}orbit coast`)).toHaveLength(1);
  });

  it("is a contact at touchdown", async () => {
    const { run } = await made();
    run.frame(input(0));
    expect(run.frame(input(1000 * PROFILE.durationS)).contact).toBe(true);
    run.dispose();
  });

  it("releases what it made when a later part cannot be made", async () => {
    const engine = await countingRenderEngine();
    engine.createRenderTarget = () => {
      throw new Error("refused");
    };
    const pools: IdlePool[] = [];
    expect(
      () =>
        new SpikeRun(
          engine,
          PREPARED,
          {
            setting: "low",
            ridges: "off",
            createPool: () => {
              const pool = new IdlePool();
              pools.push(pool);
              return pool;
            },
          },
          { main: canvas(), orbit: canvas(), craft: canvas() },
        ),
    ).toThrow(/refused/);
    expect(pools.map((pool) => pool.terminated)).toEqual([true]);
    expect(engine.inner.views.map((view) => view.disposed)).toEqual([true, true, true]);
  });

  it("disposes its views", async () => {
    const { run, engine, pools } = await made();
    run.dispose();
    expect(engine.inner.views.every((view) => view.disposed)).toBe(true);
    expect(pools.every((pool) => pool.terminated)).toBe(true);
  });
});

/** A frame 1,000 km above the test planet's north pole, looking down, `tiltRad` towards +x. */
function aboveThePole(nowMs: number, tiltRad = 0): TerrainFrameInput {
  return {
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM: { x: 0, y: 0, z: PREPARED.planet.figure.polarRadiusM + 1_000_000 },
      orientation: lookAlong(
        { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
        { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
      ),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    },
    grounded: [],
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    nowMs,
  };
}

/** A pool that bakes its demand flat on the test's command. */
class BakingPool extends IdlePool {
  readonly #listeners = new Set<(bake: BakedPatch) => void>();

  override onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  /** Bakes every patch demanded for `pass`, flat; returns their keys. */
  bakeDemand(pass: TerrainPass): string[] {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    return this.demand.map(({ key }) => {
      const bake: BakedPatch = {
        key,
        generation: 1,
        originM: { x: 1_000, y: 2_000, z: PREPARED.planet.figure.polarRadiusM },
        heights: new Float32Array(65 * 65 * 2),
        offsets: null,
        normals: new Float16Array(n * n * 2),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
        originHeightM: 0,
        skirtDepthM: 1,
        layers: null,
      };
      for (const listener of this.#listeners) {
        listener(bake);
      }
      return patchKeyString(key);
    });
  }
}

/**
 * A terrain session stand-in that logs what the run asks of it: its passes are real test-planet
 * passes, one for each style, each framed from above the pole at every step, as a session steps
 * the passes of the views that hand the body over.
 */
class LoggingSession implements SpikeTerrainSession {
  readonly log: string[] = [];
  readonly reports: Array<readonly [ViewId, TerrainViewReport]> = [];
  readonly forgotten: ViewId[] = [];
  readonly wanted: ReadonlySet<BodyIdHex> = new Set();
  readonly classMaps = { classMapOf: () => null };
  readonly reliefs = { reliefM: () => null };
  steps = 0;
  /** How far the passes' camera is tilted at each step, so that a later step selects again. */
  tiltRad = 0;

  constructor(
    readonly lit: TerrainPass,
    readonly wire: TerrainPass,
  ) {}

  noteView(id: ViewId, report: TerrainViewReport): void {
    this.log.push("note");
    this.reports.push([id, report]);
  }

  forgetView(id: ViewId): void {
    this.forgotten.push(id);
  }

  step(): void {
    this.log.push("step");
    this.steps += 1;
    for (const pass of [this.lit, this.wire]) {
      pass.frame(aboveThePole(16 * this.steps, this.tiltRad));
    }
  }

  failComputation(): void {
    this.log.push("fail");
  }

  survey(): null {
    return null;
  }

  coverageMask(): null {
    return null;
  }

  drawTerrain(): null {
    return null;
  }

  drawWireTerrain(): null {
    return null;
  }

  pass(): TerrainPass {
    return this.lit;
  }

  wirePass(): TerrainPass {
    return this.wire;
  }
}

/** The run over the stand-in session, in `style`, its main view's draws logged with the session's calls. */
interface BodyMade extends Made {
  readonly session: LoggingSession;
  readonly lit: { readonly pass: TerrainPass; readonly pool: BakingPool };
  readonly wire: { readonly pass: TerrainPass; readonly pool: BakingPool };
  readonly selected: Selection[];
}

async function bodyMade(style: RenderStyle): Promise<BodyMade> {
  const engine = await countingRenderEngine();
  const passOf = (): { readonly pass: TerrainPass; readonly pool: BakingPool } => {
    const pool = new BakingPool();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      planet: PREPARED.planet,
      source: { kind: "test-planet", ridges: "off" },
      createPool: () => pool,
    });
    return { pass, pool };
  };
  const lit = passOf();
  const wire = passOf();
  const session = new LoggingSession(lit.pass, wire.pass);
  const make = engine.createView.bind(engine);
  engine.createView = (element, name) => {
    const view = make(element, name);
    const render = view.render.bind(view);
    view.render = (frame) => {
      session.log.push(`draw ${name}`);
      render(frame);
    };
    return view;
  };
  const samples: SpikeFrameSample[] = [];
  const patches: Array<readonly [SpikePatchEvent, string]> = [];
  const selected: Selection[] = [];
  const run = new SpikeRun(
    engine,
    PREPARED,
    {
      setting: "low",
      ridges: "off",
      createPool: () => new IdlePool(),
      body: { world: TEST_PLANET_WORLD, session, style },
    },
    { main: canvas(), orbit: canvas(), craft: canvas() },
    {
      onFrame: (sample) => samples.push(sample),
      onPatch: (event, key) => patches.push([event, patchKeyString(key)]),
      onSelect: (_input, selection) => selected.push(selection),
    },
  );
  return { run, engine, pools: [], samples, patches, session, lit, wire, selected };
}

/** A frame's inputs with the main view's stage laid out at its size. */
function bodyInput(nowMs: number): SpikeFrameInput {
  return {
    ...input(nowMs),
    mainStage: { widthPx: 64, heightPx: 36, devicePixelRatio: 1, remPx: 16 },
  };
}

describe("the spike's run over a generated world (R10.T14.c)", () => {
  it("notes the main view's report, steps the session once, then draws, as VIEW's primary", async () => {
    const { run, session } = await bodyMade("wireframe");
    run.frame(bodyInput(0));
    expect(session.log).toEqual([
      "note",
      "step",
      "draw spike",
      "draw spike-orbit",
      "draw spike-craft",
    ]);
    const [id, report] = session.reports[0] ?? [];
    expect(id).toBe(viewId(SPIKE_VIEW_NAMES.main));
    expect(report).toMatchObject({
      style: "wireframe",
      drawnStyle: "wireframe",
      setting: "low",
      streamPriority: "primary",
      readoutBody: TEST_PLANET_WORLD.body,
      presentedPx: { widthPx: 64, heightPx: 36 },
    });
    expect(report?.regimes.size).toBe(0);
    run.dispose();
  });

  it("reports the style asked and the style it last drew in", async () => {
    const { run, session } = await bodyMade("photorealistic");
    const first = run.frame(bodyInput(0));
    // The first photorealistic frame stands in with the wireframe while its pipelines compile.
    expect(first.drawnStyle).toBe("wireframe");
    expect(first.cameras.main.style).toBe("photorealistic");
    run.frame(bodyInput(16));
    expect(session.reports.map(([, report]) => [report.style, report.drawnStyle])).toEqual([
      ["photorealistic", "wireframe"],
      ["photorealistic", "wireframe"],
    ]);
    run.dispose();
  });

  it("reads its terrain and its sample from the pass in the style drawn, at its last step", async () => {
    const { run, samples, wire } = await bodyMade("wireframe");
    const frame = run.frame(bodyInput(0));
    const step = wire.pass.lastStep;
    if (step === null) {
      throw new Error("the session stepped the wireframe's pass");
    }
    expect(step.selection.patches.size).toBeGreaterThan(0);
    expect(frame.terrain).toEqual({
      selected: step.selection.patches.size,
      drawn: step.drawSet.count,
      standingIn: step.drawSet.standingIn,
      missing: step.drawSet.missing,
      limited: step.selection.limited,
      annunciation: null,
    });
    expect(samples[0]).toMatchObject({
      patchesHard: step.selection.patches.size,
      passesSubmitted: null,
      streaming: false,
    });
    expect(frame.drawnStyle).toBe("wireframe");
    run.dispose();
  });

  it("reads the lit pass once the photorealistic style is drawn", async () => {
    const { run, lit } = await bodyMade("photorealistic");
    run.frame(bodyInput(0));
    await vi.waitFor(() => {
      expect(run.frame(bodyInput(16)).drawnStyle).toBe("photorealistic");
    });
    const frame = run.frame(bodyInput(32));
    expect(frame.terrain.selected).toBe(lit.pass.lastStep?.selection.patches.size);
    run.dispose();
  });

  it("tells its listeners of the pass's demand, selections and patches made resident", async () => {
    const { run, session, wire, patches, selected } = await bodyMade("wireframe");
    run.frame(bodyInput(0));
    const demanded = (wire.pass.lastStep?.selection.demand ?? []).map(({ key }) =>
      patchKeyString(key),
    );
    expect(demanded.length).toBeGreaterThan(0);
    expect(patches).toEqual(demanded.map((key) => ["requested", key]));
    // The first selection was made with the pass, before the run could observe it.
    expect(selected).toEqual([]);
    session.tiltRad = 0.4;
    run.frame(bodyInput(16));
    expect(selected).toEqual([wire.pass.lastStep?.selection]);
    patches.length = 0;
    const baked = wire.pool.bakeDemand(wire.pass);
    expect(baked.length).toBeGreaterThan(0);
    expect(patches).toEqual(
      baked.flatMap((key) => [
        ["baked", key],
        ["resident", key],
      ]),
    );
    run.dispose();
    patches.length = 0;
    wire.pool.bakeDemand(wire.pass);
    expect(patches).toEqual([]);
  });

  it("is the pass's contact by the session's rule: none in orbit, one at touchdown", async () => {
    const { run } = await bodyMade("wireframe");
    expect(run.frame(bodyInput(0)).contact).toBe(false);
    // Over no resident bake the pass's held ground is the datum, the site's height here.
    expect(run.frame(bodyInput(1000 * PROFILE.durationS)).contact).toBe(true);
    run.dispose();
  });

  it("makes no lit view or atmosphere of its own", async () => {
    const engine = await countingRenderEngine();
    const targets = vi.spyOn(engine, "createRenderTarget");
    const session = new LoggingSession(
      new TerrainPass({
        engine,
        setting: "low",
        planet: PREPARED.planet,
        source: { kind: "test-planet", ridges: "off" },
        createPool: () => new IdlePool(),
      }),
      new TerrainPass({
        engine,
        setting: "low",
        planet: PREPARED.planet,
        source: { kind: "test-planet", ridges: "off" },
        createPool: () => new IdlePool(),
      }),
    );
    const run = new SpikeRun(
      engine,
      PREPARED,
      {
        setting: "low",
        ridges: "off",
        createPool: () => new IdlePool(),
        body: { world: TEST_PLANET_WORLD, session, style: "wireframe" },
      },
      { main: canvas(), orbit: canvas(), craft: canvas() },
    );
    run.frame(bodyInput(0));
    // The lit view's targets are `spike:spike-hdr` and `spike:spike-sky`; the atmosphere's tables
    // are `atmosphere …` textures.
    expect(
      targets.mock.calls.map(([spec]) => spec.name).filter((name) => name.includes(":spike-")),
    ).toEqual([]);
    expect(engine.textureSpecs.filter((spec) => spec.name.startsWith("atmosphere"))).toEqual([]);
    run.dispose();
  });

  it("gives the style's refusal and draws nothing, rather than measure the wireframe, where the photorealistic pipelines failed", async () => {
    // The renderer says why its pipelines could not be made.
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { run, engine, session, samples } = await bodyMade("photorealistic");
    engine.failMaterials(() => true);
    expect(run.frame(bodyInput(0)).refusal).toBeNull();
    await vi.waitFor(() => {
      expect(run.frame(bodyInput(16)).refusal).toBe(PHOTOREAL_NOT_CREATED);
    });
    session.log.length = 0;
    const sampled = samples.length;
    const refused = run.frame(bodyInput(32));
    expect([refused.refusal, refused.terrain.selected, session.log, samples.length]).toEqual([
      PHOTOREAL_NOT_CREATED,
      0,
      [],
      sampled,
    ]);
    run.dispose();
  });

  it("gives no refusal over the test planet or in the wireframe, and no drawn terrain's interval or edges over the test planet", async () => {
    const { run } = await made();
    const frame = run.frame(input(0));
    expect([frame.refusal, frame.contourIntervalM, frame.surveyEdges]).toEqual([null, null, []]);
    run.dispose();
    const wire = await bodyMade("wireframe");
    expect(wire.run.frame(bodyInput(0)).refusal).toBeNull();
    wire.run.dispose();
  });

  it("disposes the drawer with its views, and the session forgets the main view", async () => {
    const { run, engine, session } = await bodyMade("wireframe");
    run.frame(bodyInput(0));
    run.dispose();
    expect(engine.inner.views.map((view) => view.disposed)).toEqual([true, true, true]);
    expect(session.forgotten).toEqual([viewId(SPIKE_VIEW_NAMES.main)]);
  });
});

describe("the default worker count", () => {
  it("is a quarter of the threads, from one to three (Design note 11)", () => {
    expect([2, 4, 8, 12, 16, 32].map(defaultSpikeWorkers)).toEqual([1, 1, 2, 3, 3, 3]);
  });
});

/** A query that answers from fixed values and records the floors' groups it was asked for. */
class FixedQueryWorker implements SurfaceQueryWorker {
  readonly groups: Array<ReadonlyArray<ReadonlyArray<PatchKey>>> = [];
  readonly #listeners: ((event: MessageEvent<SurfaceQueryReply>) => void)[] = [];

  constructor(readonly floorOf: (group: number) => number) {}

  postMessage(message: SurfaceQueryRequest, _transfer: Transferable[]): void {
    let reply: SurfaceQueryReply;
    switch (message.kind) {
      case "level-table":
        reply = { kind: "level-table", id: message.id, table: goldenLevelTable("off") };
        break;
      case "omitted-sigma":
        reply = { kind: "omitted-sigma", id: message.id, sigmaM: new Float64Array(25).fill(2) };
        break;
      case "height":
        reply = { kind: "height", id: message.id, heightsM: Float64Array.of(-412) };
        break;
      case "max-heights":
        this.groups.push(message.groups);
        reply = {
          kind: "max-heights",
          id: message.id,
          maxesM: Float64Array.from(message.groups, (_, k) => this.floorOf(k)),
          baked: 0,
        };
        break;
    }
    queueMicrotask(() => {
      for (const cb of this.#listeners) {
        cb(new MessageEvent("message", { data: reply }));
      }
    });
  }

  addEventListener(type: "message" | "error", cb: never): void {
    if (type === "message") {
      this.#listeners.push(cb);
    }
  }

  terminate(): void {}
}

describe("prepareDescent", () => {
  it("asks one floor a stretch, over the stretch's keys, and flies over what it answers", async () => {
    const worker = new FixedQueryWorker((k) => -412 + 10 * k);
    const prepared = await prepareDescent(new SurfaceQuery(worker, "off"), 5n);
    const stretches = trackStretches(PROFILE);
    expect(prepared.stretches).toEqual(stretches);
    expect(worker.groups).toHaveLength(1);
    expect(worker.groups[0]).toEqual(stretches.map((stretch) => stretchKeys(PROFILE, stretch)));
    expect(prepared.stretchMaxHeightsM).toEqual(stretches.map((_, k) => -412 + 10 * k));
    expect(prepared.siteHeightM).toBe(-412);
    expect(Array.from(prepared.omittedSigmaM)).toEqual(Array.from({ length: 25 }, () => 2));
    const stretch = stretches.find((each) => each.piece === "slowdown 3");
    if (stretch === undefined) {
      throw new Error("the plan has no slowdown 3");
    }
    const k = stretches.indexOf(stretch);
    expect(prepared.profile.poseAt((stretch.startS + stretch.endS) / 2).floorM).toBe(-412 + 10 * k);
  });

  it("takes a floor that is not finite for a measurement fault, not a refusal", async () => {
    const worker = new FixedQueryWorker(() => Number.POSITIVE_INFINITY);
    const prepared = prepareDescent(new SurfaceQuery(worker, "off"), 5n);
    await expect(prepared).rejects.toThrow(/not finite/);
    await expect(prepared).rejects.not.toBeInstanceOf(DescentRefused);
  });
});
