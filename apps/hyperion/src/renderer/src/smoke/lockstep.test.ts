import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../test/terrainFixtures";
import { lookAlong } from "../view/camera/quaternion";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../view/engine/platform";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import type { DrawnPatch, DrawSet } from "../view/terrain/cache";
import { DEFAULT_TERRAIN_VIEW, normalsAtlasOf } from "../view/terrain/gpu/resources";
import { type PatchKey, patchKeyString } from "../view/terrain/patchKey";
import { planetGeometry } from "../view/terrain/planet";
import type { PatchRequest } from "../view/terrain/select";
import {
  type TerrainFrame,
  TerrainPass,
  type TerrainPool,
  type TerrainView,
} from "../view/terrain/terrainPass";
import type { BakedPatch, BakeSettings } from "../view/terrain/workers/messages";
import type { RefinedDraft, RefineRequest } from "../view/terrain/workers/pool";
import { drawnKey, LaneHold, type LockstepStep, settleInLockstep } from "./lockstep";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** The pass's cache, 8 MiB: a budget of 81 patches, which {@link VIEW} wants more than. */
const CACHE_BYTES = 2 ** 23;

/** 1.5 km up, 74° from straight down, 512 px across. */
const VIEW: TerrainView = {
  rotation: IDENTITY_ROTATION,
  cameraM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 1_500 },
  orientation: lookAlong(
    { x: Math.sin(1.3), y: 0, z: -Math.cos(1.3) },
    { x: Math.cos(1.3), y: 0, z: Math.sin(1.3) },
  ),
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 512, heightPx: 384 },
};

/**
 * A pool of two workers whose n-th bake takes `paceMs(n)` on the (faked) clock, taken in the
 * demand's order, and lands if its key is still asked for, as the height workers' do.
 */
class PacedPool implements TerrainPool {
  readonly #listeners = new Set<(bake: BakedPatch) => void>();
  #queue: PatchRequest[] = [];
  readonly #wanted = new Set<string>();
  readonly #inFlight = new Set<string>();
  #started = 0;
  /** The pass the bakes land in, set once it is made. */
  pass: TerrainPass | null = null;

  constructor(
    readonly settings: BakeSettings,
    readonly paceMs: (n: number) => number,
  ) {}

  /** Whether every bake asked for has landed: none queued and none in flight. */
  get idle(): boolean {
    return this.#queue.length === 0 && this.#inFlight.size === 0;
  }

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.#wanted.clear();
    for (const request of demand) {
      this.#wanted.add(patchKeyString(request.key));
    }
    this.#queue = demand.filter((request) => !this.#inFlight.has(patchKeyString(request.key)));
    this.#pump();
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  terminate(): void {
    this.#queue = [];
  }

  #pump(): void {
    while (this.#inFlight.size < 2) {
      const request = this.#queue.shift();
      if (request === undefined) {
        return;
      }
      const { key } = request;
      const keyString = patchKeyString(key);
      this.#inFlight.add(keyString);
      this.#started += 1;
      setTimeout(() => {
        this.#inFlight.delete(keyString);
        if (this.#wanted.has(keyString)) {
          this.#land(key);
        }
        this.#pump();
      }, this.paceMs(this.#started));
    }
  }

  /**
   * A flat bake whose height range differs from patch to patch, as real ground's does, so that
   * the patches' errors differ and the budget's refusals fall unevenly.
   */
  #land(key: PatchKey): void {
    const pass = this.pass;
    if (pass === null) {
      return;
    }
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    const share = ((key.i * 7_919 + key.j * 104_729 + key.level * 31) % 997) / 997;
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: { x: 1_000, y: 2_000, z: PLANET.figure.polarRadiusM },
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2),
      heightRangeM: [0, 50 + 128_000 * share * 2 ** -key.level],
      boundingRadiusM: 1,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: null,
    };
    for (const listener of this.#listeners) {
      listener(bake);
    }
  }
}

/** The selection a pass streamed at {@link VIEW} settles on, its bakes landing at `paceMs`. */
async function settledSelection(
  paceMs: (n: number) => number,
): Promise<{ readonly keys: string; readonly limited: boolean; readonly done: boolean }> {
  const engine = await countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
  const pools: PacedPool[] = [];
  const pass = new TerrainPass({
    engine,
    setting: "low",
    terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
    planet: PLANET,
    source: { kind: "test-planet", ridges: "off" },
    createPool: (bake) => {
      const pool = new PacedPool(bake, paceMs);
      pools.push(pool);
      return pool;
    },
  });
  await pass.ready();
  const pool = pools.at(-1);
  if (pool === undefined) {
    throw new Error("the pass made no pool");
  }
  pool.pass = pass;
  pass.setReducedMotion(true);
  const step = (): LockstepStep<TerrainFrame> => {
    const frame = pass.frame({
      view: VIEW,
      grounded: [],
      sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
      sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
      exposureScale: 1e-5,
      nowMs: performance.now(),
    });
    return {
      done: frame.drawSet.missing === 0 && frame.drawSet.standingIn === 0,
      frame,
      drawSet: frame.drawSet,
      resting: pass.bandsAtTarget(DEFAULT_TERRAIN_VIEW),
    };
  };
  const settling = settleInLockstep(step, () => pool.idle, 600_000);
  await vi.runAllTimersAsync();
  const { done, frame } = await settling;
  pass.dispose();
  return {
    keys: [...frame.selection.patches.keys()].toSorted().join(" "),
    limited: frame.selection.limited,
    done,
  };
}

/** A draw set of the level-9 patches at `columns` of row 0 on face 1, whole, seen by view 0. */
function drawSetOf(columns: ReadonlyArray<number>): DrawSet {
  const patches = columns.map((i, slot): DrawnPatch => {
    const key: PatchKey = { face: 1, level: 9, i, j: 0 };
    const keyString = patchKeyString(key);
    return {
      keyString,
      patch: {
        key,
        generation: 1,
        originM: { x: 0, y: 0, z: 0 },
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
        slot,
        keyString,
      },
      standIn: false,
      within: null,
    };
  });
  return {
    count: patches.length,
    patches,
    slots: Uint32Array.from(patches.map((_, slot) => slot)),
    standingIn: 0,
    missing: 0,
    views: Uint32Array.from(patches.map(() => 1)),
    standingInViews: 0,
    missingViews: 0,
  };
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the terrain checks' wait for a pass's streaming (R14.T1.a's captures)", () => {
  it("settles a pass the budget holds on one selection whatever its workers' pace", async () => {
    // Stepped every 50 ms whatever had landed, the third pace settled on 75 patches and the others
    // on 72: the budget's hold kept a split made while other bakes were still in flight.
    const paces = [
      (): number => 1,
      (): number => 40,
      (n: number): number => 10 + ((n * 37) % 90),
      (n: number): number => 5 + ((n * 61) % 150),
    ];
    const settled = [];
    for (const pace of paces) {
      // One pass at a time, on the one faked clock.
      // oxlint-disable-next-line no-await-in-loop
      settled.push(await settledSelection(pace));
    }
    const first = settled[0];
    expect([first?.limited, first?.done]).toEqual([true, true]);
    expect(settled).toEqual(paces.map(() => first));
  });

  it("takes the frame of the first step that draws what the step before drew", async () => {
    // The terms hold from the first step, whose draw set the time morph changes at the next.
    const drawn = [
      [1, 2],
      [2, 3],
      [2, 3],
    ];
    let n = 0;
    const settling = settleInLockstep(
      () => {
        const at = Math.min(n, drawn.length - 1);
        n += 1;
        return { done: true, frame: at, drawSet: drawSetOf(drawn[at] ?? []), resting: true };
      },
      () => true,
      10_000,
    );
    await vi.runAllTimersAsync();
    expect(await settling).toEqual({ done: true, frame: 2, steps: 3 });
  });

  it("steps on at once while the bands still slew towards the selection's τ′", async () => {
    let steps = 0;
    const settling = settleInLockstep(
      () => {
        steps += 1;
        return { done: true, frame: steps, drawSet: drawSetOf([1]), resting: steps >= 4 };
      },
      () => true,
      10_000,
    );
    await vi.runAllTimersAsync();
    expect(await settling).toEqual({ done: true, frame: 4, steps: 4 });
  });

  it("takes no step while a bake is still to land", async () => {
    let landed = false;
    let steps = 0;
    const settling = settleInLockstep(
      () => {
        steps += 1;
        return { done: true, frame: steps, drawSet: drawSetOf([1]), resting: true };
      },
      () => landed,
      10_000,
    );
    await vi.advanceTimersByTimeAsync(5_000);
    const waiting = steps;
    landed = true;
    await vi.runAllTimersAsync();
    expect([waiting, await settling]).toEqual([0, { done: true, frame: 2, steps: 2 }]);
  });

  it("ends at its limit, not done, where the bakes never land", async () => {
    const settling = settleInLockstep(
      () => ({ done: true, frame: 0, drawSet: drawSetOf([1]), resting: true }),
      () => false,
      1_000,
    );
    await vi.runAllTimersAsync();
    expect(await settling).toEqual({ done: false, frame: 0, steps: 1 });
  });

  it("names each drawn patch by its key, the area it draws and its views", () => {
    expect([drawnKey(drawSetOf([1, 2])), drawnKey(undefined)]).toEqual([
      "1/9/1/0<@1 1/9/2/0<@1",
      "",
    ]);
  });
});

/** A lane whose bakes a test lands by hand, each of no more than its key. */
class HandLane implements TerrainPool {
  readonly #listeners = new Set<(bake: BakedPatch) => void>();

  reprioritise(): void {}

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  terminate(): void {}

  /** Lands the bake of the level-9 patch at column `i` of row 0 on face 1. */
  land(i: number): void {
    const bake: BakedPatch = {
      key: { face: 1, level: 9, i, j: 0 },
      generation: 1,
      originM: { x: 0, y: 0, z: 0 },
      heights: new Float32Array(0),
      offsets: null,
      normals: new Float16Array(0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: null,
    };
    for (const listener of this.#listeners) {
      listener(bake);
    }
  }
}

/**
 * A {@link HandLane} that refines drafts and tells of failures: it notes each refinement asked and
 * dropped, and its count, and tells a failure or refined normals by hand.
 */
class RefiningHandLane extends HandLane {
  readonly asked: string[] = [];
  readonly dropped: string[] = [];
  refiningCount = 0;
  readonly #failed = new Set<(failure: { readonly keyString: string }) => void>();
  readonly #refined = new Set<(refined: RefinedDraft) => void>();

  refine(r: RefineRequest): void {
    this.asked.push(patchKeyString(r.key));
  }

  dropRefinement(keyString: string): void {
    this.dropped.push(keyString);
  }

  onFailed(cb: (failure: { readonly keyString: string }) => void): () => void {
    this.#failed.add(cb);
    return () => {
      this.#failed.delete(cb);
    };
  }

  onRefined(cb: (refined: RefinedDraft) => void): () => void {
    this.#refined.add(cb);
    return () => {
      this.#refined.delete(cb);
    };
  }

  /** Tells the failure of the patch at column `i`. */
  fail(i: number): void {
    for (const listener of this.#failed) {
      listener({ keyString: patchKeyString({ face: 1, level: 9, i, j: 0 }) });
    }
  }

  /** Tells the refined normals of the patch at column `i`. */
  refined(i: number): void {
    for (const listener of this.#refined) {
      listener({
        key: { face: 1, level: 9, i, j: 0 },
        ticket: i,
        normals: new Float32Array(0),
        complete: true,
      });
    }
  }
}

describe("the hold on a pass's lanes between its settles", () => {
  it("keeps a held lane's bakes from its pass, and lands them in their order at its release", () => {
    const hold = new LaneHold();
    const lane = new HandLane();
    const landed: number[] = [];
    hold.wrap(lane).onBaked((bake) => landed.push(bake.key.i));
    lane.land(1);
    hold.hold();
    lane.land(2);
    lane.land(3);
    const whileHeld = [...landed];
    hold.release();
    lane.land(4);
    expect([whileHeld, landed]).toEqual([[1], [1, 2, 3, 4]]);
  });

  it("keeps a held lane's failures and refined normals too, in the order they came", () => {
    const hold = new LaneHold();
    const lane = new RefiningHandLane();
    const wrapped = hold.wrap(lane);
    const told: string[] = [];
    wrapped.onFailed?.(({ keyString }) => told.push(`failed ${keyString}`));
    wrapped.onRefined?.(({ key }) => told.push(`refined ${patchKeyString(key)}`));
    hold.hold();
    lane.refined(1);
    lane.fail(2);
    const whileHeld = [...told];
    hold.release();
    expect([whileHeld, told]).toEqual([[], ["refined 1/9/1/0", "failed 1/9/2/0"]]);
  });

  it("gives a listener stopped while held none of the arrivals kept for it", () => {
    const hold = new LaneHold();
    const lane = new HandLane();
    const landed: number[] = [];
    const stop = hold.wrap(lane).onBaked((bake) => landed.push(bake.key.i));
    hold.hold();
    lane.land(1);
    stop();
    hold.release();
    expect(landed).toEqual([]);
  });

  it("gives a plain lane's pass no part the lane lacks", () => {
    const plain = new LaneHold().wrap(new HandLane());
    expect(
      ["refine", "dropRefinement", "onFailed", "onRefined"].map((part) => part in plain),
    ).toEqual([false, false, false, false]);
  });

  it("passes a refining lane's asks through, and reads its count as it stands", () => {
    const lane = new RefiningHandLane();
    const wrapped = new LaneHold().wrap(lane);
    wrapped.refine?.({ key: { face: 1, level: 9, i: 5, j: 0 }, priority: 1, ticket: 0 });
    wrapped.dropRefinement?.("1/9/6/0");
    lane.refiningCount = 2;
    expect([lane.asked, lane.dropped, wrapped.refiningCount]).toEqual([
      ["1/9/5/0"],
      ["1/9/6/0"],
      2,
    ]);
  });
});
