import { afterEach, describe, expect, it } from "vitest";

import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { readTokens } from "../../spatial/paint";
import { planetGeometry } from "../terrain/planet";
import type { PatchRequest } from "../terrain/select";
import type { TerrainPool } from "../terrain/terrainPass";
import type { PatchKey } from "../terrain/patchKey";
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
import { stretchKeys } from "./demandRecord";
import {
  DescentRefused,
  defaultSpikeWorkers,
  prepareDescent,
  type PreparedDescent,
  type SpikeFrameInput,
  type SpikeFrameSample,
  type SpikePatchEvent,
  SpikeRun,
} from "./spikeRun";

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
