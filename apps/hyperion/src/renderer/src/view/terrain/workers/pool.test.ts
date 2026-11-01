import { describe, expect, it, vi } from "vitest";

import { keptBody } from "../../scenes/kept";
import { bakeOf, ScriptedHeightWorker } from "../../../test/scriptedHeightWorker";
import { SELECTION_RULE, sigmaSurveyOf } from "../levelBounds";
import { childKeys, type PatchKey, patchKeyString, rootKey } from "../patchKey";
import { compareRequests } from "../priority";
import type { PatchRequest } from "../select";
import type {
  BakedPatch,
  BakeSettings,
  ClassMapAnswer,
  GroundReadoutQuery,
  HeightSourceSetting,
  SurfaceGroundReadout,
  TerrainLayers,
} from "./messages";
import { NORMALS_BETWEEN } from "./messages";
import {
  defaultHeightWorkers,
  type FieldChange,
  type FieldRefusal,
  HeightWorkerPool,
  MAX_CONSECUTIVE_WORKER_FAILURES,
  MAX_IN_FLIGHT_PER_WORKER,
} from "./pool";

/** Two bodies of the kept system, for the per-body field calls. */
const MOON = keptBody(1);
const PLANET = keptBody(2);

/** The test planet, the pools' source unless a test names a body. */
const TEST_PLANET: HeightSourceSetting = { kind: "test-planet", ridges: "off" };

/** The moon as a pool's height source. */
const MOON_SOURCE: HeightSourceSetting = {
  kind: "body",
  body: MOON,
  detailSeed: "0123456789abcdef",
  weights: "baked",
};

function keyAt(i: number): PatchKey {
  return { face: 0, level: 10, i, j: 0 };
}

function req(i: number, priority: number, forced = false): PatchRequest {
  return { key: keyAt(i), priority, forced };
}

function poolOf(
  workers: number,
  source: HeightSourceSetting = TEST_PLANET,
): {
  pool: HeightWorkerPool;
  created: ScriptedHeightWorker[];
  baked: string[];
} {
  const created: ScriptedHeightWorker[] = [];
  const pool = new HeightWorkerPool({
    workers,
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      created.push(worker);
      return worker;
    },
    bake: { vertexPath: "face-differences", normals: "mesh", source },
  });
  const baked: string[] = [];
  pool.onBaked((bake) => baked.push(patchKeyString(bake.key)));
  return { pool, created, baked };
}

function bakedKeys(worker: ScriptedHeightWorker): string[] {
  return worker.received.flatMap((m) => (m.kind === "bake" ? [patchKeyString(m.key)] : []));
}

function workerAt(created: readonly ScriptedHeightWorker[], n: number): ScriptedHeightWorker {
  const w = created[n];
  if (w === undefined) {
    throw new Error(`no worker ${n}`);
  }
  return w;
}

/** A block of `bytes` bytes, each `fill`. */
function blockOf(bytes: number, fill: number): ArrayBuffer {
  return new Uint8Array(bytes).fill(fill).buffer;
}

/** The forms of the `field-init`s a worker received. */
function forms(w: ScriptedHeightWorker): string[] {
  return w.fieldMessages.flatMap((m) => (m.kind === "field-init" ? [m.form] : []));
}

/**
 * The layers of a body's bake, complete or not, with `unsurveyed` cells, its horizon map complete
 * or not.
 */
function layers(
  complete: boolean,
  unsurveyed: readonly number[],
  horizonComplete = true,
): TerrainLayers {
  return {
    palette: new Uint8Array(16),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete,
    unsurveyedCells: Uint32Array.from(unsurveyed),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete,
    horizonRefusedCells: horizonComplete ? new Uint32Array() : Uint32Array.of(3),
  };
}

/** Answers a worker's oldest pending bake as one the field cannot make yet. */
function replyUnsurveyed(w: ScriptedHeightWorker): void {
  const pending = w.pendingBakes[0];
  if (pending === undefined) {
    throw new Error("no bake is pending");
  }
  w.reply({ kind: "bake-unsurveyed", id: pending.id, message: "a margin to come" });
}

/**
 * Answers a worker's `n`th field message as a payload's block taken, surveying the cells `first`
 * and `first + 1`, with a level table of `first`s.
 */
function answerBlock(w: ScriptedHeightWorker, n: number, first: number): void {
  const message = w.fieldMessages[n];
  if (message === undefined) {
    throw new Error(`no field message ${String(n)}`);
  }
  w.reply({
    kind: "field-held",
    id: message.id,
    body: message.body,
    heldBytes: 8,
    block: {
      fieldLevel: 6,
      newlySurveyed: Uint32Array.from([first, first + 2]),
      newlyHeld: Uint32Array.from([first, first + 3]),
      levels: new Float64Array(100).fill(first),
      sigmaM: new Float64Array(25).fill(first / 10),
    },
  });
}

/** Each field message a worker received, as its kind, its body and its block's first byte. */
function fieldLog(w: ScriptedHeightWorker): string[] {
  return w.fieldMessages.map((m) =>
    m.kind === "field-release"
      ? `${m.kind} ${m.body}`
      : `${m.kind} ${m.body} ${new Uint8Array(m.block)[0] ?? -1}`,
  );
}

describe("the height-worker pool", () => {
  it("hands requests to idle workers in priority order, forced first", () => {
    const { pool, created } = poolOf(1);
    pool.reprioritise([req(1, 5), req(2, 9), req(3, 1, true), req(4, 7)]);
    expect(bakedKeys(workerAt(created, 0))).toEqual([
      patchKeyString(keyAt(3)),
      patchKeyString(keyAt(2)),
    ]);
    workerAt(created, 0).bakeOldest();
    expect(bakedKeys(workerAt(created, 0))[2]).toBe(patchKeyString(keyAt(4)));
  });

  it("breaks a tie of priority by the time to need, sooner first, before the key (R10.T14.b)", () => {
    const { pool, created } = poolOf(1);
    pool.reprioritise([
      { ...req(1, 5), needS: 0.5 },
      { ...req(2, 5), needS: 0.1 },
      { ...req(3, 6), needS: 2 },
      req(4, 5),
    ]);
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    // 3 outranks the rest whatever its time; then 4, needed now, then 2 and 1 by their times.
    expect(bakedKeys(w)).toEqual([3, 4, 2, 1].map((i) => patchKeyString(keyAt(i))));
  });

  it("never gives a worker more than two requests at once", () => {
    const { pool, created } = poolOf(2);
    pool.reprioritise(Array.from({ length: 10 }, (_, i) => req(i, i)));
    for (const w of created) {
      expect(w.pendingBakes.length).toBeLessThanOrEqual(MAX_IN_FLIGHT_PER_WORKER);
    }
    expect(pool.inFlightCount).toBe(4);
    expect(pool.queuedCount).toBe(6);
  });

  it("reorders the queue and drops what left the demand, leaving requests in flight alone", () => {
    const { pool, created, baked } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7), req(4, 6)]);
    // 1 and 2 are in flight; 3 and 4 queued. Now 4 outranks 3, 3 is gone, 1 stays wanted.
    pool.reprioritise([req(1, 1), req(4, 9), req(5, 2)]);
    expect(pool.queuedCount).toBe(2);
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    // 1 is delivered (still wanted), 2 dropped (no longer wanted), then 4 before 5.
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
    expect(bakedKeys(w).slice(2)).toEqual([patchKeyString(keyAt(4)), patchKeyString(keyAt(5))]);
  });

  it("never sends a cancelled request to a worker", () => {
    const { pool, created } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7)]);
    pool.cancelStale(new Set([patchKeyString(keyAt(1)), patchKeyString(keyAt(2))]));
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    expect(bakedKeys(w)).not.toContain(patchKeyString(keyAt(3)));
  });

  it("keeps a stale generation's result whose key is still wanted, and drops one that is not", () => {
    const { pool, created, baked } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8)]);
    const firstGeneration = pool.currentGeneration;
    pool.reprioritise([req(1, 9)]);
    expect(pool.currentGeneration).toBeGreaterThan(firstGeneration);
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  it("posts each block of a body's field to every worker, its first block as field-init", () => {
    const { pool, created } = poolOf(3);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    pool.fieldInit(PLANET, blockOf(2, 7));
    pool.fieldChunk(MOON, blockOf(4, 2));
    for (const w of created) {
      expect(fieldLog(w)).toEqual([
        `field-init ${MOON} 0`,
        `field-chunk ${MOON} 1`,
        `field-init ${PLANET} 7`,
        `field-chunk ${MOON} 2`,
      ]);
    }
    expect(pool.fieldBodies).toEqual([MOON, PLANET]);
  });

  it("releases a body's field from every worker and keeps no copy of it", () => {
    const { pool, created } = poolOf(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    pool.fieldRelease(MOON);
    pool.fieldRelease(MOON);
    for (const w of created) {
      expect(fieldLog(w).at(-1)).toBe(`field-release ${MOON}`);
      expect(w.fieldMessages.filter((m) => m.kind === "field-release")).toHaveLength(1);
      w.answerFields();
      expect(w.fields.heldBytes).toBe(0);
    }
    expect(pool.fieldBodies).toEqual([]);
    expect(pool.fieldResidentBytes).toBe(0);
  });

  it("refuses a block before its body's field is started, and a second start", () => {
    const { pool } = poolOf(1);
    expect(() => {
      pool.fieldChunk(MOON, blockOf(1, 0));
    }).toThrow(/before its field was started/u);
    pool.fieldInit(MOON, blockOf(1, 0));
    expect(() => {
      pool.fieldInit(MOON, blockOf(1, 0));
    }).toThrow(/already started/u);
  });

  it("reports the workers' copies of every field as each worker last held them", () => {
    const { pool, created } = poolOf(3);
    pool.fieldInit(MOON, blockOf(100, 0));
    pool.fieldChunk(MOON, blockOf(50, 1));
    pool.fieldInit(PLANET, blockOf(10, 2));
    expect(pool.fieldResidentBytes).toBe(0);
    workerAt(created, 0).answerFields();
    expect(pool.fieldResidentBytes).toBe(160);
    for (const w of created) {
      w.answerFields();
    }
    expect(pool.fieldResidentBytes).toBe(3 * 160);
    pool.fieldRelease(MOON);
    for (const w of created) {
      w.answerFields();
    }
    expect(pool.fieldResidentBytes).toBe(3 * 10);
  });

  it("posts a bake after the field's blocks, so that the worker makes it with them", () => {
    const { pool, created } = poolOf(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9), req(2, 8)]);
    for (const w of created) {
      expect(w.received.map((m) => m.kind)).toEqual(["field-init", "bake"]);
    }
  });

  it("replaces a worker that errors, re-posting every field before its re-queued requests", () => {
    const { pool, created, baked } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    pool.fieldInit(PLANET, blockOf(4, 2));
    pool.fieldChunk(MOON, blockOf(4, 3));
    workerAt(created, 0).answerFields();
    pool.reprioritise([req(1, 9), req(2, 8)]);
    expect(bakedKeys(workerAt(created, 0))).toHaveLength(2);
    workerAt(created, 0).fail();
    expect(workerAt(created, 0).terminated).toBe(true);
    expect(workerAt(created, 0).listening).toEqual({ message: 0, error: 0 });
    expect(pool.fieldResidentBytes).toBe(0);
    const replacement = workerAt(created, 1);
    // Each body's blocks in the order posted, a body after the body started before it.
    expect(fieldLog(replacement)).toEqual([
      `field-init ${MOON} 0`,
      `field-chunk ${MOON} 1`,
      `field-chunk ${MOON} 3`,
      `field-init ${PLANET} 2`,
    ]);
    expect(replacement.received.slice(4).map((m) => m.kind)).toEqual(["bake", "bake"]);
    replacement.answerFields();
    expect(pool.fieldResidentBytes).toBe(16);
    replacement.bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  // R10.T4.c (decision-r10-item30 §4): the workers survey σ only where the rule in force reads it.
  it("starts every field with the σ survey of the rule in force, a replacement worker's too", () => {
    const { pool, created } = poolOf(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    pool.fieldInit(PLANET, blockOf(4, 2), "stand-in");
    workerAt(created, 0).fail();
    expect(created).toHaveLength(3);
    const surveys = created.map((w) =>
      w.fieldMessages.flatMap((m) => (m.kind === "field-init" ? [m.sigma] : [])),
    );
    const rule = sigmaSurveyOf(SELECTION_RULE);
    expect(surveys).toEqual(created.map(() => [rule, rule]));
  });

  it("gives a worker's place up after it fails three times in a row", () => {
    const { pool, created } = poolOf(2);
    pool.reprioritise([req(1, 9)]);
    for (let n = 0; n < MAX_CONSECUTIVE_WORKER_FAILURES; n += 1) {
      workerAt(created, created.length === 2 ? 0 : created.length - 1).fail();
    }
    expect(created).toHaveLength(2 + MAX_CONSECUTIVE_WORKER_FAILURES - 1);
    expect(pool.lostWorkers).toBe(1);
    // The request went to the surviving worker.
    expect(bakedKeys(workerAt(created, 1))).toContain(patchKeyString(keyAt(1)));
  });

  it("keeps every worker busy before giving any a second request", () => {
    const { pool, created } = poolOf(3);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7)]);
    expect(created.map((w) => w.pendingBakes.length)).toEqual([1, 1, 1]);
  });

  it("bakes again what was baked before its body's field was started anew", () => {
    const { pool, created, baked } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    pool.fieldRelease(MOON);
    pool.fieldInit(MOON, blockOf(4, 1));
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([]);
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
    expect(bakedKeys(workerAt(created, 0))).toEqual([
      patchKeyString(keyAt(1)),
      patchKeyString(keyAt(1)),
    ]);
  });

  it("bakes nothing of a body again for another body's field", () => {
    const { pool, created, baked } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    pool.fieldInit(PLANET, blockOf(4, 1));
    pool.fieldChunk(PLANET, blockOf(4, 2));
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  it("bakes nothing of the test planet again for any field, which it never reads", () => {
    const { pool, created, baked } = poolOf(1);
    pool.reprioritise([req(2, 9)]);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(2))]);
  });

  it("sends no bake of a body before its field is started", () => {
    const { pool, created } = poolOf(2, MOON_SOURCE);
    pool.reprioritise([req(1, 9), req(2, 8)]);
    expect(created.map((w) => w.pendingBakes.length)).toEqual([0, 0]);
    expect(pool.queuedCount).toBe(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    expect(created.map((w) => w.received.map((m) => m.kind))).toEqual([
      ["field-init", "bake"],
      ["field-init", "bake"],
    ]);
  });

  it("asks again, once the body has a field, a bake made with a field since released", () => {
    const { pool, created, baked } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    pool.fieldRelease(MOON);
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([]);
    expect(pool.queuedCount).toBe(1);
    expect(workerAt(created, 0).pendingBakes).toHaveLength(0);
  });

  it("holds a bake the field cannot make yet until the body's next block", () => {
    const { pool, created, baked } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    const w = workerAt(created, 0);
    replyUnsurveyed(w);
    expect(pool.waitingCount).toBe(1);
    // Each frame's demand leaves it waiting rather than asking again.
    pool.reprioritise([req(1, 9)]);
    expect(w.pendingBakes).toHaveLength(0);
    pool.fieldChunk(MOON, blockOf(4, 1));
    expect(pool.waitingCount).toBe(0);
    expect(w.received.map((m) => m.kind)).toEqual(["field-init", "bake", "field-chunk", "bake"]);
    w.bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  it("drops a waiting request that leaves the demand", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    replyUnsurveyed(workerAt(created, 0));
    pool.reprioritise([]);
    expect(pool.waitingCount).toBe(0);
  });

  it("drops a waiting request that a cancellation does not keep", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9), req(2, 8)]);
    const w = workerAt(created, 0);
    replyUnsurveyed(w);
    replyUnsurveyed(w);
    expect(pool.waitingCount).toBe(2);
    pool.cancelStale(new Set([patchKeyString(keyAt(2))]));
    expect(pool.waitingCount).toBe(1);
    pool.fieldChunk(MOON, blockOf(4, 1));
    expect(bakedKeys(w).slice(2)).toEqual([patchKeyString(keyAt(2))]);
  });

  it("re-scores a waiting request asked for again, still waiting", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 1)]);
    const w = workerAt(created, 0);
    replyUnsurveyed(w);
    pool.request(req(1, 9, true));
    expect(pool.waitingCount).toBe(1);
    expect(w.pendingBakes).toHaveLength(0);
  });

  it("forgets what waited for a released field, the next demand held until a field starts", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    const w = workerAt(created, 0);
    replyUnsurveyed(w);
    pool.fieldRelease(MOON);
    expect(pool.waitingCount).toBe(0);
    pool.reprioritise([req(1, 9)]);
    expect(pool.queuedCount).toBe(1);
    pool.fieldInit(MOON, blockOf(4, 1));
    expect(w.pendingBakes).toHaveLength(1);
  });

  it("asks again at once a bake refused for a block that came after it was sent", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    const w = workerAt(created, 0);
    pool.fieldChunk(MOON, blockOf(4, 1));
    replyUnsurveyed(w);
    expect(pool.waitingCount).toBe(0);
    expect(w.pendingBakes).toHaveLength(1);
  });

  it("asks again, held, a bake refused after its body's field was released", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    const w = workerAt(created, 0);
    pool.fieldRelease(MOON);
    replyUnsurveyed(w);
    expect(pool.waitingCount).toBe(0);
    expect(pool.queuedCount).toBe(1);
    expect(w.pendingBakes).toHaveLength(0);
  });

  it("bakes again a bake in flight while a block came, unless it is complete, its map complete and surveyed whole", () => {
    const cases: ReadonlyArray<readonly [TerrainLayers, boolean]> = [
      [layers(true, []), true],
      [layers(true, [5]), false],
      [layers(false, []), false],
      [layers(true, [], false), false],
    ];
    const outcomes = cases.map(([made]) => {
      const { pool, created, baked } = poolOf(1, MOON_SOURCE);
      pool.fieldInit(MOON, blockOf(4, 0));
      pool.reprioritise([req(1, 9)]);
      pool.fieldChunk(MOON, blockOf(4, 1));
      const w = workerAt(created, 0);
      const pending = w.pendingBakes[0];
      if (pending === undefined) {
        throw new Error("no bake was sent");
      }
      const bake: BakedPatch = { ...bakeOf(pending.key, pending.generation), layers: made };
      w.reply({ kind: "baked", id: pending.id, bake });
      return [baked.length === 1, w.pendingBakes.length === 1];
    });
    expect(outcomes).toEqual(cases.map(([, kept]) => [kept, !kept]));
  });

  it("tells every listener of a bake and pumps its queue on, whatever one listener throws", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { pool, created, baked } = poolOf(1);
    pool.onBaked(() => {
      throw new Error("a listener's own bug");
    });
    pool.reprioritise([req(0, 3), req(1, 2), req(2, 1)]);
    const worker = workerAt(created, 0);
    worker.bakeOldest();
    expect([baked, bakedKeys(worker), error.mock.calls.length]).toEqual([
      [patchKeyString(keyAt(0))],
      [0, 1, 2].map((i) => patchKeyString(keyAt(i))),
      1,
    ]);
  });

  it("reports each block's change once, from the first worker to answer it", () => {
    const { pool, created } = poolOf(2, MOON_SOURCE);
    const changes: FieldChange[] = [];
    pool.onFieldChange((c) => changes.push(c));
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    answerBlock(workerAt(created, 1), 0, 10);
    answerBlock(workerAt(created, 0), 0, 10);
    answerBlock(workerAt(created, 0), 1, 20);
    answerBlock(workerAt(created, 1), 1, 20);
    expect(
      changes.map((c) => [c.body, c.sequence, [...c.newlySurveyed], [...c.newlyHeld]]),
    ).toEqual([
      [MOON, 0, [10, 12], [10, 13]],
      [MOON, 1, [20, 22], [20, 23]],
    ]);
  });

  it("keeps the level bounds of the latest block reported, and none once released", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    expect(pool.fieldLevels(MOON)).toBeNull();
    expect(pool.fieldBounds(MOON)).toBeNull();
    answerBlock(workerAt(created, 0), 0, 10);
    answerBlock(workerAt(created, 0), 1, 20);
    expect(pool.fieldLevels(MOON)?.[0]).toBe(20);
    expect(pool.fieldBounds(MOON)?.table).toBe(pool.fieldLevels(MOON));
    expect(pool.fieldBounds(MOON)?.sigmaM[24]).toBe(2);
    pool.fieldRelease(MOON);
    expect(pool.fieldLevels(MOON)).toBeNull();
    expect(pool.fieldBounds(MOON)).toBeNull();
  });

  it("reports no change again from a replacement's answers to the blocks posted again", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    const changes: FieldChange[] = [];
    pool.onFieldChange((c) => changes.push(c));
    pool.fieldInit(MOON, blockOf(4, 0));
    answerBlock(workerAt(created, 0), 0, 10);
    workerAt(created, 0).fail();
    answerBlock(workerAt(created, 1), 0, 10);
    expect(changes).toHaveLength(1);
  });

  it("reports no change of a released start's block, only the new start's", () => {
    const { pool, created } = poolOf(1, MOON_SOURCE);
    const changes: FieldChange[] = [];
    pool.onFieldChange((c) => changes.push(c));
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldRelease(MOON);
    pool.fieldInit(MOON, blockOf(4, 1));
    // The worker answers the first start's block 0 after the second start was posted.
    answerBlock(workerAt(created, 0), 0, 10);
    expect(changes).toEqual([]);
    answerBlock(workerAt(created, 0), 2, 30);
    expect(changes.map((c) => c.sequence)).toEqual([0]);
  });

  it("counts the blocks the workers have still to answer, and none of a replaced worker", () => {
    const { pool, created } = poolOf(2, MOON_SOURCE);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    expect(pool.blocksInFlight).toBe(4);
    workerAt(created, 0).answerFields();
    expect(pool.blocksInFlight).toBe(2);
    workerAt(created, 1).fail();
    // The replacement is posted both blocks afresh; the failed worker's are forgotten.
    expect(pool.blocksInFlight).toBe(2);
    workerAt(created, 2).answerFields();
    expect(pool.blocksInFlight).toBe(0);
  });

  it("posts a stand-in field's first block as a stand-in, to a replacement too", () => {
    const { pool, created } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0), "stand-in");
    pool.fieldChunk(MOON, blockOf(4, 1));
    expect(forms(workerAt(created, 0))).toEqual(["stand-in"]);
    workerAt(created, 0).fail();
    expect(forms(workerAt(created, 1))).toEqual(["stand-in"]);
    pool.fieldInit(PLANET, blockOf(4, 2));
    expect(forms(workerAt(created, 1))).toEqual(["stand-in", "payload"]);
  });

  it("reports a worker's refusal of a field once for each start of it", () => {
    const { pool, created } = poolOf(2);
    const refusals: FieldRefusal[] = [];
    pool.onFieldRefused((r) => refusals.push(r));
    pool.fieldInit(MOON, blockOf(4, 0));
    const init = workerAt(created, 0).fieldMessages[0];
    if (init === undefined) {
      throw new Error("no field-init was posted");
    }
    for (const w of created) {
      const message = w.fieldMessages[0];
      if (message === undefined) {
        throw new Error("no field-init was posted");
      }
      w.reply({
        kind: "field-refused",
        id: message.id,
        body: MOON,
        message: "unreadable",
        heldBytes: 0,
      });
    }
    expect(refusals).toEqual([{ body: MOON, message: "unreadable" }]);
    // A refusal of the field's earlier start is not the new start's.
    pool.fieldRelease(MOON);
    pool.fieldInit(MOON, blockOf(4, 0));
    workerAt(created, 0).reply({
      kind: "field-refused",
      id: init.id,
      body: MOON,
      message: "late",
      heldBytes: 0,
    });
    expect(refusals).toHaveLength(1);
  });

  it("reports a failed bake and gives the request up", () => {
    const { pool, created } = poolOf(1);
    const failures: string[] = [];
    pool.onFailed((f) => failures.push(f.keyString));
    pool.reprioritise([req(1, 9)]);
    const pending = workerAt(created, 0).pendingBakes[0];
    if (pending === undefined) {
      throw new Error("no bake was sent");
    }
    workerAt(created, 0).reply({ kind: "bake-failed", id: pending.id, message: "out of range" });
    expect(failures).toEqual([patchKeyString(keyAt(1))]);
    expect(pool.inFlightCount).toBe(0);
    pool.reprioritise([req(1, 9)]);
    expect(pool.queuedCount + pool.inFlightCount).toBe(0);
  });

  it("leaves no listener behind when terminated", () => {
    const { pool, created, baked } = poolOf(2);
    pool.reprioritise([req(1, 9)]);
    const pending = workerAt(created, 0).pendingBakes[0];
    pool.terminate();
    for (const w of created) {
      expect(w.terminated).toBe(true);
      expect(w.listening).toEqual({ message: 0, error: 0 });
    }
    if (pending !== undefined) {
      workerAt(created, 0).reply({ kind: "baked", id: pending.id, bake: bakeOf(keyAt(1), 1) });
    }
    expect(baked).toEqual([]);
    expect(() => pool.reprioritise([])).toThrow(/terminated/u);
  });
});

/** A worker's answer of a class map of no texel, at the map's level. */
function emptyMap(): ClassMapAnswer {
  return {
    level: 6,
    fieldLevel: 6,
    classes: Uint8Array.from([1, 0]),
    photometry: new Float32Array(5),
    texels: new Uint32Array(0),
    fractions: new Float32Array(0),
    weights: new Uint8Array(0),
    complete: true,
    normalLevel: 6,
    normalCells: new Uint32Array(0),
    normalPairs: new Float16Array(0),
    normalsComplete: true,
  };
}

/** Answers `worker`'s oldest class-map query with `map`. */
function answerClassMap(worker: ScriptedHeightWorker, map: ClassMapAnswer | null): void {
  const query = worker.pendingClassMaps[0];
  if (query === undefined) {
    throw new Error("no class map is asked");
  }
  worker.reply({ kind: "class-map", id: query.id, body: query.body, map });
}

describe("the height-worker pool's class maps (R10.T10.d)", () => {
  const SEED = "0123456789abcdef";

  it("asks one worker, the least loaded, after the body's blocks, and answers with its map", async () => {
    const { pool, created } = poolOf(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.reprioritise([req(1, 9)]);
    const asked = pool.classMap(MOON, SEED);
    // The bake went to the first worker; the query to the second, after the field's block.
    const second = workerAt(created, 1);
    expect(second.received.map((m) => m.kind)).toEqual(["field-init", "class-map"]);
    expect(second.pendingClassMaps[0]).toMatchObject({ body: MOON, detailSeed: SEED });
    expect(workerAt(created, 0).pendingClassMaps).toEqual([]);
    const map = emptyMap();
    answerClassMap(second, map);
    await expect(asked).resolves.toBe(map);
  });

  it("answers null at once for a body whose field it does not keep", async () => {
    const { pool, created } = poolOf(1);
    await expect(pool.classMap(MOON, SEED)).resolves.toBeNull();
    expect(workerAt(created, 0).received).toEqual([]);
  });

  it("answers null where the worker holds no field of the body", async () => {
    const { pool, created } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const none = pool.classMap(MOON, SEED);
    answerClassMap(workerAt(created, 0), null);
    await expect(none).resolves.toBeNull();
  });

  it("rejects with the module's refusal", async () => {
    const { pool, created } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const refused = pool.classMap(MOON, SEED);
    const query = workerAt(created, 0).pendingClassMaps[0];
    if (query === undefined) {
      throw new Error("no class map is asked");
    }
    workerAt(created, 0).reply({
      kind: "class-map-failed",
      id: query.id,
      body: MOON,
      message: "a body index above 65,535",
    });
    await expect(refused).rejects.toThrow(/class map of .* was refused: a body index above/u);
  });

  it("asks a failed worker's replacement again, once the fields are posted to it", async () => {
    const { pool, created } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    const asked = pool.classMap(MOON, SEED);
    workerAt(created, 0).fail();
    const replacement = workerAt(created, 1);
    expect(replacement.received.map((m) => m.kind)).toEqual([
      "field-init",
      "field-chunk",
      "class-map",
    ]);
    const map = emptyMap();
    answerClassMap(replacement, map);
    await expect(asked).resolves.toBe(map);
  });

  /**
   * A pool of `workers` asked for the Moon's map, its first worker and each of that one's
   * replacements failing until its place is given up; the queries each failing worker held.
   */
  function failingFirst(workers: number): {
    readonly pool: ReturnType<typeof poolOf>["pool"];
    readonly created: ReturnType<typeof poolOf>["created"];
    readonly asked: ReturnType<ReturnType<typeof poolOf>["pool"]["classMap"]>;
    readonly held: ReadonlyArray<number>;
  } {
    const { pool, created } = poolOf(workers);
    pool.fieldInit(MOON, blockOf(4, 0));
    const asked = pool.classMap(MOON, SEED);
    const held: number[] = [];
    let failing = workerAt(created, 0);
    for (let n = 0; n < MAX_CONSECUTIVE_WORKER_FAILURES; n += 1) {
      held.push(failing.pendingClassMaps.length);
      failing.fail();
      failing = workerAt(created, created.length - 1);
    }
    return { pool, created, asked, held };
  }

  it("asks each of a failing worker's replacements again until its place is given up", () => {
    const { pool, held } = failingFirst(2);
    expect([held, pool.lostWorkers]).toEqual([
      Array.from({ length: MAX_CONSECUTIVE_WORKER_FAILURES }, () => 1),
      1,
    ]);
  });

  it("asks another worker where a failed one's place is given up", async () => {
    const { created, asked } = failingFirst(2);
    const map = emptyMap();
    answerClassMap(workerAt(created, 1), map);
    await expect(asked).resolves.toBe(map);
  });

  it("answers null once its only worker's place is given up", async () => {
    await expect(failingFirst(1).asked).resolves.toBeNull();
  });

  it("answers null to a query out when terminated", async () => {
    const { pool } = poolOf(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const asked = pool.classMap(MOON, SEED);
    pool.terminate();
    await expect(asked).resolves.toBeNull();
  });
});

/** A pool made without bake settings, as the terrain session makes its own (R10.T16.a). */
function fieldsOnly(workers: number): {
  readonly pool: HeightWorkerPool;
  readonly created: ScriptedHeightWorker[];
} {
  const created: ScriptedHeightWorker[] = [];
  const pool = new HeightWorkerPool({
    workers,
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      created.push(worker);
      return worker;
    },
  });
  return { pool, created };
}

describe("the height-worker pool of the terrain session (R10.T16.a)", () => {
  it("holds fields without bake settings", () => {
    const { pool, created } = fieldsOnly(2);
    pool.fieldInit(MOON, blockOf(8, 1));
    expect(created.map((w) => w.fieldMessages.map((m) => m.kind))).toEqual([
      ["field-init"],
      ["field-init"],
    ]);
  });

  it("refuses a bake request without bake settings", () => {
    expect(() => {
      fieldsOnly(1).pool.request(req(0, 1));
    }).toThrow(/without bake settings/u);
  });

  it("refuses a demand without bake settings", () => {
    expect(() => {
      fieldsOnly(1).pool.reprioritise([req(0, 1)]);
    }).toThrow(/without bake settings/u);
  });

  it("refuses a cancellation without bake settings", () => {
    expect(() => {
      fieldsOnly(1).pool.cancelStale(new Set());
    }).toThrow(/without bake settings/u);
  });

  it("counts its own copy of every field's blocks, and none once released", () => {
    const { pool } = fieldsOnly(3);
    pool.fieldInit(MOON, blockOf(100, 0));
    pool.fieldChunk(MOON, blockOf(50, 1));
    pool.fieldInit(PLANET, blockOf(10, 2));
    const kept = pool.keptFieldBytes;
    pool.fieldRelease(MOON);
    expect([kept, pool.keptFieldBytes]).toEqual([160, 10]);
  });

  it("runs Design note 11's count up to eight threads, and every thread past eight up to eight", () => {
    expect(
      [1, 4, 7, 8, 10, 12, 16, 20, 32].map((threads) => defaultHeightWorkers(threads)),
    ).toEqual([1, 1, 1, 2, 2, 4, 8, 8, 8]);
  });

  it("runs an Apple chip's performance threads less two, up to eight, whatever its efficiency cores", () => {
    // An M1's four of eight threads, an M3 Pro's six of twelve, an M1 Pro's eight of ten, an M3
    // Max's twelve of sixteen.
    expect(
      [
        [8, 4],
        [12, 6],
        [10, 8],
        [16, 12],
        [2, 1],
      ].map(([threads, p]) => defaultHeightWorkers(threads ?? 0, p ?? null)),
    ).toEqual([2, 4, 6, 8, 1]);
    expect(defaultHeightWorkers(10, null)).toBe(2);
  });

  it("runs one worker where the browser reports no thread count", () => {
    expect(defaultHeightWorkers(Number.NaN)).toBe(1);
  });
});

/** The moon's bakes with the wireframe's shading: a second lane's settings (R10.T16.b). */
const MOON_WIREFRAME: HeightSourceSetting = { ...MOON_SOURCE, weights: "skipped" };

/** The bake settings of a lane over `source`. */
function laneSettings(source: HeightSourceSetting = MOON_SOURCE): BakeSettings {
  return { vertexPath: "face-differences", normals: "mesh", source };
}

/** A bake's key and its settings' class weights, as {@link bakesOf} lists them. */
function bakeLine(key: PatchKey, weights: "baked" | "skipped"): string {
  return `${patchKeyString(key)} ${weights}`;
}

/** Each bake a worker received, as its key and its settings' class weights. */
function bakesOf(w: ScriptedHeightWorker): string[] {
  return w.received.flatMap((m) =>
    m.kind === "bake" && m.settings.source.kind === "body"
      ? [bakeLine(m.key, m.settings.source.weights)]
      : [],
  );
}

/** Answers the pending bake of `key` with `weights` from a worker. */
function bakeKey(w: ScriptedHeightWorker, key: PatchKey, weights: "baked" | "skipped"): void {
  const pending = w.pendingBakes.find(
    (m) =>
      patchKeyString(m.key) === patchKeyString(key) &&
      m.settings.source.kind === "body" &&
      m.settings.source.weights === weights,
  );
  if (pending === undefined) {
    throw new Error(`no bake of ${patchKeyString(key)} is pending`);
  }
  w.reply({ kind: "baked", id: pending.id, bake: bakeOf(pending.key, pending.generation) });
}

describe("the height-worker pool's lanes (R10.T16.b)", () => {
  it("replaces a lane's demand alone", () => {
    const { pool, created } = fieldsOnly(1);
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    lit.reprioritise([req(1, 1), req(2, 1)]);
    wire.reprioritise([req(3, 1)]);
    lit.reprioritise([req(4, 1)]);
    expect([lit.queuedCount, wire.queuedCount]).toEqual([1, 1]);
    pool.fieldInit(MOON, blockOf(8, 0));
    expect(bakesOf(workerAt(created, 0)).toSorted()).toEqual(
      [bakeLine(keyAt(3), "skipped"), bakeLine(keyAt(4), "baked")].toSorted(),
    );
  });

  it("takes forced requests first, then the larger priority whatever the lane, then the older lane's, then the key's", () => {
    const { pool, created } = fieldsOnly(1);
    const older = pool.lane(laneSettings());
    const newer = pool.lane(laneSettings(MOON_WIREFRAME));
    older.reprioritise([req(1, 2), req(5, 1), req(6, 3)]);
    newer.reprioritise([req(2, 2), req(3, 0, true), req(4, 5)]);
    // Nothing is sent before the moon's field starts; then the queue's two best, and one a bake.
    pool.fieldInit(MOON, blockOf(8, 0));
    const worker = workerAt(created, 0);
    for (let n = 0; n < 4; n += 1) {
      worker.bakeOldest();
    }
    expect(bakesOf(worker)).toEqual([
      bakeLine(keyAt(3), "skipped"),
      bakeLine(keyAt(4), "skipped"),
      bakeLine(keyAt(6), "baked"),
      bakeLine(keyAt(1), "baked"),
      bakeLine(keyAt(2), "skipped"),
      bakeLine(keyAt(5), "baked"),
    ]);
  });

  it("hands a lane's bakes to its own listeners alone, each lane baking the key with its settings", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    const litBakes: string[] = [];
    const wireBakes: string[] = [];
    lit.onBaked((bake) => litBakes.push(patchKeyString(bake.key)));
    wire.onBaked((bake) => wireBakes.push(patchKeyString(bake.key)));
    lit.reprioritise([req(1, 1)]);
    wire.reprioritise([req(1, 1)]);
    const worker = workerAt(created, 0);
    expect(worker.pendingBakes).toHaveLength(2);
    bakeKey(worker, keyAt(1), "skipped");
    expect([litBakes, wireBakes]).toEqual([[], [patchKeyString(keyAt(1))]]);
    bakeKey(worker, keyAt(1), "baked");
    expect(litBakes).toEqual([patchKeyString(keyAt(1))]);
  });

  it("drops a terminated lane's queued, waiting and in-flight requests, and leaves the fields, the class maps and the other lanes", async () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const ended = pool.lane(laneSettings());
    const kept = pool.lane(laneSettings(MOON_WIREFRAME));
    const endedBakes: string[] = [];
    ended.onBaked((bake) => endedBakes.push(patchKeyString(bake.key)));
    const keptBakes: string[] = [];
    kept.onBaked((bake) => keptBakes.push(patchKeyString(bake.key)));
    const worker = workerAt(created, 0);
    // Two of the ended lane's in flight, one answered as waiting for a block, which sends the third.
    ended.reprioritise([req(1, 9), req(2, 8), req(3, 7), req(5, 6)]);
    replyUnsurveyed(worker);
    expect([ended.waitingCount, ended.queuedCount]).toEqual([1, 1]);
    kept.reprioritise([req(4, 1)]);
    const map = pool.classMap(MOON, "0123456789abcdef");
    ended.terminate();
    expect([ended.terminated, ended.waitingCount, ended.queuedCount]).toEqual([true, 0, 0]);
    // Its bakes still out are dropped as they arrive; the kept lane's is sent and delivered.
    bakeKey(worker, keyAt(2), "baked");
    bakeKey(worker, keyAt(3), "baked");
    expect(endedBakes).toEqual([]);
    bakeKey(worker, keyAt(4), "skipped");
    expect(keptBakes).toEqual([patchKeyString(keyAt(4))]);
    expect(pool.fieldBodies).toEqual([MOON]);
    const query = worker.pendingClassMaps[0];
    if (query === undefined) {
      throw new Error("no class map was asked");
    }
    worker.reply({ kind: "class-map", id: query.id, body: MOON, map: null });
    await expect(map).resolves.toBeNull();
    expect(() => {
      ended.reprioritise([]);
    }).toThrow(/lane was terminated/u);
    // A second termination does nothing.
    ended.terminate();
  });

  it("clears the waiting of the lanes of a released field's body alone", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    pool.fieldInit(PLANET, blockOf(8, 1));
    const moon = pool.lane(laneSettings());
    const planet = pool.lane(
      laneSettings({
        kind: "body",
        body: PLANET,
        detailSeed: "fedcba9876543210",
        weights: "baked",
      }),
    );
    const worker = workerAt(created, 0);
    moon.reprioritise([req(1, 2)]);
    planet.reprioritise([req(2, 1)]);
    replyUnsurveyed(worker);
    replyUnsurveyed(worker);
    pool.fieldRelease(MOON);
    expect([moon.waitingCount, planet.waitingCount]).toEqual([0, 1]);
  });

  it("asks a lane's waiting request again after the next block of its own body alone", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    pool.fieldInit(PLANET, blockOf(8, 1));
    const moon = pool.lane(laneSettings());
    const worker = workerAt(created, 0);
    moon.reprioritise([req(1, 1)]);
    replyUnsurveyed(worker);
    pool.fieldChunk(PLANET, blockOf(8, 2));
    expect(moon.waitingCount).toBe(1);
    pool.fieldChunk(MOON, blockOf(8, 3));
    expect([moon.waitingCount, worker.pendingBakes.length]).toEqual([0, 1]);
  });

  it("says it can bake nothing once every worker is given up", () => {
    const { pool, created } = fieldsOnly(2);
    const lane = pool.lane(laneSettings());
    expect(lane.unavailable).toBe(false);
    for (let failure = 0; failure < MAX_CONSECUTIVE_WORKER_FAILURES; failure += 1) {
      for (const worker of created.slice(-2)) {
        worker.fail();
      }
    }
    expect([pool.lostWorkers, pool.workerCount, lane.unavailable]).toEqual([2, 2, true]);
  });

  it("refuses every lane's call, and a new lane, once the pool is terminated", () => {
    const { pool } = fieldsOnly(1);
    const lane = pool.lane(laneSettings());
    pool.terminate();
    expect(lane.terminated).toBe(true);
    expect(() => {
      lane.reprioritise([]);
    }).toThrow(/pool was terminated/u);
    expect(() => pool.lane(laneSettings())).toThrow(/pool was terminated/u);
  });
});

/** A request of `key` at `priority`, not forced. */
function reqOf(key: PatchKey, priority: number, forced = false): PatchRequest {
  return { key, priority, forced };
}

/** A lit lane's demand on the moon whose every priority, 1,000 and above, outranks the wireframe's. */
function heavyDemand(count: number): PatchRequest[] {
  return Array.from({ length: count }, (_, i) => req(i, 1000 + i));
}

/** Answers the pool's oldest bake in flight, over every worker, as workers that bake in turn. */
function answerOldest(created: readonly ScriptedHeightWorker[]): void {
  let oldest: { readonly worker: ScriptedHeightWorker; readonly id: number } | null = null;
  for (const worker of created) {
    const pending = worker.pendingBakes[0];
    if (pending !== undefined && (oldest === null || pending.id < oldest.id)) {
      oldest = { worker, id: pending.id };
    }
  }
  if (oldest === null) {
    throw new Error("no bake is in flight");
  }
  oldest.worker.bakeOldest();
}

/** Every bake sent with the wireframe's settings, over every worker, in the order sent. */
function wireframeSent(created: readonly ScriptedHeightWorker[]): string[] {
  return created
    .flatMap((w) => w.received)
    .flatMap((m) =>
      m.kind === "bake" &&
      m.settings.source.kind === "body" &&
      m.settings.source.weights === "skipped"
        ? [{ id: m.id, key: patchKeyString(m.key) }]
        : [],
    )
    .toSorted((a, b) => a.id - b.id)
    .map((m) => m.key);
}

describe("the height-worker pool's per-pass share (R10.T14.b)", () => {
  it("bakes a second lane's roots within 2W + R bakes ahead of the first's refinements, and its finer patches wait", () => {
    const workers = 3;
    const { pool, created } = fieldsOnly(workers);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    const wireBakes: string[] = [];
    wire.onBaked((bake) => wireBakes.push(patchKeyString(bake.key)));
    lit.reprioritise(heavyDemand(200));
    // Every worker is full of the lit lane's bakes when the wireframe attaches with nothing resident.
    expect(pool.inFlightCount).toBe(workers * MAX_IN_FLIGHT_PER_WORKER);
    const roots = [rootKey(0), rootKey(4)];
    const finer: PatchKey[] = [...childKeys(rootKey(0)), { face: 4, level: 5, i: 3, j: 7 }];
    wire.reprioritise([
      ...roots.map((key) => reqOf(key, 0.5)),
      ...finer.map((key, n) => reqOf(key, 2 + n)),
    ]);
    const rootStrings = roots.map(patchKeyString);
    // The roots take the next R free workers, one a bake answered.
    for (let n = 0; n < roots.length; n += 1) {
      answerOldest(created);
    }
    expect(wireframeSent(created).toSorted()).toEqual(rootStrings.toSorted());
    let answered = roots.length;
    while (wireBakes.length < roots.length) {
      answerOldest(created);
      answered += 1;
    }
    // Every bake in flight before them is answered first, at most 2W, then the R roots.
    expect(answered).toBeLessThanOrEqual(2 * workers + roots.length);
    expect(wireBakes.toSorted()).toEqual(rootStrings.toSorted());
    // Its coarsest wanted level whole, its finer patches wait while the lit lane outranks them.
    for (let n = 0; n < 50; n += 1) {
      answerOldest(created);
    }
    expect([wireframeSent(created).toSorted(), wire.queuedCount]).toEqual([
      rootStrings.toSorted(),
      finer.length,
    ]);
  });

  it("keeps one lane's bake order, request for request, its roots among its refinements", () => {
    const [, child] = childKeys(rootKey(2));
    const demand = [
      reqOf(rootKey(2), 0.25),
      reqOf(rootKey(5), 40),
      reqOf(child, 9),
      reqOf(keyAt(4), 3, true),
      reqOf(keyAt(5), 12),
      reqOf(keyAt(6), 0.5),
      reqOf(rootKey(1), 12),
      reqOf(keyAt(7), 1, true),
    ];
    const order = (withSecondLane: boolean): string[] => {
      const { pool, created } = fieldsOnly(1);
      pool.fieldInit(MOON, blockOf(8, 0));
      const lane = pool.lane(laneSettings());
      if (withSecondLane) {
        // A second lane with no demand changes nothing.
        pool.lane(laneSettings(MOON_WIREFRAME)).reprioritise([]);
      }
      lane.reprioritise(demand);
      const worker = workerAt(created, 0);
      while (worker.pendingBakes.length > 0) {
        worker.bakeOldest();
      }
      return bakedKeys(worker);
    };
    const asBuilt = demand.toSorted(compareRequests).map((r) => patchKeyString(r.key));
    expect([order(false), order(true)]).toEqual([asBuilt, asBuilt]);
  });

  it("bakes every lane's forced requests before another lane's hole", () => {
    const { pool, created } = fieldsOnly(1);
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    lit.reprioritise([req(1, 0.1, true), req(2, 0.2, true), req(3, 50)]);
    wire.reprioritise([reqOf(rootKey(3), 0.01), req(4, 0, true)]);
    // Nothing is sent before the moon's field starts; then the queue's two best, and one a bake.
    pool.fieldInit(MOON, blockOf(8, 0));
    const worker = workerAt(created, 0);
    for (let n = 0; n < 3; n += 1) {
      worker.bakeOldest();
    }
    expect(bakesOf(worker)).toEqual([
      bakeLine(keyAt(2), "baked"),
      bakeLine(keyAt(1), "baked"),
      bakeLine(keyAt(4), "skipped"),
      bakeLine(rootKey(3), "skipped"),
      bakeLine(keyAt(3), "baked"),
    ]);
  });

  it("serves another lane's hole ahead of a lane's refinement, and never a lane's own hole ahead of its own", () => {
    const { pool, created } = fieldsOnly(1);
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    lit.reprioritise([req(1, 9), reqOf(rootKey(0), 1), req(2, 0.5)]);
    wire.reprioritise([reqOf(rootKey(1), 2), reqOf(rootKey(2), 3), req(3, 8)]);
    pool.fieldInit(MOON, blockOf(8, 0));
    const worker = workerAt(created, 0);
    for (let n = 0; n < 4; n += 1) {
      worker.bakeOldest();
    }
    // The wireframe's holes by priority while the lit lane's best is a refinement; the lit lane's
    // root behind its own refinement, then ahead of the wireframe's; then Design note 24's order.
    expect(bakesOf(worker)).toEqual([
      bakeLine(rootKey(2), "skipped"),
      bakeLine(rootKey(1), "skipped"),
      bakeLine(keyAt(1), "baked"),
      bakeLine(rootKey(0), "baked"),
      bakeLine(keyAt(3), "skipped"),
      bakeLine(keyAt(2), "baked"),
    ]);
  });

  it("never sends a hole cancelled from its lane's demand", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    lit.reprioritise(heavyDemand(10));
    wire.reprioritise([rootKey(0), rootKey(1), rootKey(2)].map((key) => reqOf(key, 1)));
    // Root 0 cancelled, then root 2 left out of the next demand: root 1 alone is still wanted.
    wire.cancelStale(new Set([rootKey(1), rootKey(2)].map(patchKeyString)));
    wire.reprioritise([reqOf(rootKey(1), 1)]);
    for (let n = 0; n < 6; n += 1) {
      answerOldest(created);
    }
    expect([wireframeSent(created), wire.queuedCount]).toEqual([[patchKeyString(rootKey(1))], 0]);
  });

  it("asks a hole made stale by its body's block again ahead of another lane's refinements", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lit = pool.lane(laneSettings());
    const wire = pool.lane(laneSettings(MOON_WIREFRAME));
    const wireBakes: string[] = [];
    wire.onBaked((bake) => wireBakes.push(patchKeyString(bake.key)));
    lit.reprioritise(heavyDemand(10));
    wire.reprioritise([reqOf(rootKey(0), 1)]);
    const worker = workerAt(created, 0);
    answerOldest(created);
    // The root is in flight behind one lit bake when a block comes; it lands incomplete, so stale.
    pool.fieldChunk(MOON, blockOf(8, 1));
    answerOldest(created);
    const pending = worker.pendingBakes.find((m) => m.key.level === 0);
    if (pending === undefined) {
      throw new Error("the root is not in flight");
    }
    const bake: BakedPatch = {
      ...bakeOf(pending.key, pending.generation),
      layers: layers(false, []),
    };
    worker.reply({ kind: "baked", id: pending.id, bake });
    expect([wireBakes, wireframeSent(created)]).toEqual([
      [],
      [patchKeyString(rootKey(0)), patchKeyString(rootKey(0))],
    ]);
  });
});

/** The query of a readout straight down from 2,000 km on +y, over 2 m, under 1 m, at a season. */
const READOUT_QUERY: GroundReadoutQuery = {
  originM: { x: 0, y: 2e6, z: 0 },
  direction: { x: 0, y: -1, z: 0 },
  maxM: 1e7,
  slopeBaselineM: 2,
  padWidthM: 1,
  seasonMeanAnomalyRad: 0.75,
};

/** A readout off the ground, as a worker answers one that meets no surveyed ground. */
const NOT_SURVEYED: SurfaceGroundReadout = { kind: "not-surveyed" };

/** Answers `worker`'s oldest ground-readout query with `readout`. */
function answerReadout(worker: ScriptedHeightWorker, readout: SurfaceGroundReadout): void {
  const query = worker.pendingGroundReadouts[0];
  if (query === undefined) {
    throw new Error("no ground readout is asked");
  }
  worker.reply({ kind: "ground-readout", id: query.id, body: query.body, readout });
}

describe("the height-worker pool's ground readouts (R10.T12)", () => {
  const SEED = "0123456789abcdef";

  it("asks one worker, the least loaded, after the body's blocks, and answers with its readout", async () => {
    const { pool, created } = fieldsOnly(2);
    pool.fieldInit(MOON, blockOf(4, 0));
    const map = pool.classMap(MOON, SEED);
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    // The class map went to the first worker; the readout to the second, after the block.
    const second = workerAt(created, 1);
    expect(second.received.map((m) => m.kind)).toEqual(["field-init", "ground-readout"]);
    expect(second.pendingGroundReadouts[0]).toEqual({
      ...READOUT_QUERY,
      kind: "ground-readout",
      id: second.pendingGroundReadouts[0]?.id,
      body: MOON,
      detailSeed: SEED,
    });
    expect(workerAt(created, 0).pendingGroundReadouts).toEqual([]);
    answerReadout(second, NOT_SURVEYED);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
    answerClassMap(workerAt(created, 0), null);
    await expect(map).resolves.toBeNull();
  });

  it("answers null at once for a body whose field it does not keep, posting nothing", async () => {
    const { pool, created } = fieldsOnly(1);
    await expect(pool.groundReadout(MOON, SEED, READOUT_QUERY)).resolves.toBeNull();
    expect(workerAt(created, 0).received).toEqual([]);
  });

  it("rejects with the module's refusal", async () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const refused = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    const query = workerAt(created, 0).pendingGroundReadouts[0];
    if (query === undefined) {
      throw new Error("no ground readout is asked");
    }
    workerAt(created, 0).reply({
      kind: "ground-readout-failed",
      id: query.id,
      body: MOON,
      message: "a pad width of 0 m is not finite and positive",
    });
    await expect(refused).rejects.toThrow(/ground readout of .* was refused: a pad width/u);
  });

  it("asks a failed worker's replacement again, with the class map, once the fields are posted to it", async () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    pool.fieldChunk(MOON, blockOf(4, 1));
    const map = pool.classMap(MOON, SEED);
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    workerAt(created, 0).fail();
    const replacement = workerAt(created, 1);
    expect(replacement.received.map((m) => m.kind)).toEqual([
      "field-init",
      "field-chunk",
      "class-map",
      "ground-readout",
    ]);
    expect(replacement.pendingGroundReadouts[0]).toMatchObject(READOUT_QUERY);
    answerReadout(replacement, NOT_SURVEYED);
    answerClassMap(replacement, null);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
    await expect(map).resolves.toBeNull();
  });

  it("answers null to a readout out when terminated, as to a class map", async () => {
    const { pool } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    const map = pool.classMap(MOON, SEED);
    pool.terminate();
    await expect(asked).resolves.toBeNull();
    await expect(map).resolves.toBeNull();
    expect(() => pool.groundReadout(MOON, SEED, READOUT_QUERY)).toThrow(/pool was terminated/u);
  });

  it("answers null once its only worker's place is given up", async () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    for (let n = 0; n < MAX_CONSECUTIVE_WORKER_FAILURES; n += 1) {
      workerAt(created, created.length - 1).fail();
    }
    expect(pool.lostWorkers).toBe(1);
    await expect(asked).resolves.toBeNull();
  });

  it("rejects a query answered with another query's kind, rather than leaving it to wait", async () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(4, 0));
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    const query = workerAt(created, 0).pendingGroundReadouts[0];
    if (query === undefined) {
      throw new Error("no ground readout is asked");
    }
    workerAt(created, 0).reply({ kind: "class-map", id: query.id, body: MOON, map: null });
    await expect(asked).rejects.toThrow(/a ground-readout query of .* was answered class-map/u);
  });
});

/** A pool of `workers` bake workers and a readout worker, holding fields, as the session's is. */
function withReadoutWorker(workers: number): {
  readonly pool: HeightWorkerPool;
  readonly created: ScriptedHeightWorker[];
} {
  const created: ScriptedHeightWorker[] = [];
  const pool = new HeightWorkerPool({
    workers,
    readoutWorker: true,
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      created.push(worker);
      return worker;
    },
  });
  return { pool, created };
}

describe("the height-worker pool's readout worker (R10.T12.c)", () => {
  const SEED = "0123456789abcdef";

  /** The moon's field held and a lane's ten requests sent, each bake worker two deep. */
  function busy(): ReturnType<typeof withReadoutWorker> {
    const made = withReadoutWorker(2);
    made.pool.fieldInit(MOON, blockOf(8, 0));
    made.pool.lane(laneSettings()).reprioritise(Array.from({ length: 10 }, (_, i) => req(i, 1)));
    return made;
  }

  /** Fails the worker in `created`'s place `n`, and then each replacement, until it is given up. */
  function giveUp(created: ScriptedHeightWorker[], n: number): void {
    workerAt(created, n).fail();
    for (let round = 1; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
      workerAt(created, created.length - 1).fail();
    }
  }

  it("runs one worker more than its bake workers, which its size leaves out", () => {
    const { pool, created } = withReadoutWorker(2);
    expect([pool.workerCount, pool.slotCount, created.length]).toEqual([2, 3, 3]);
  });

  it("posts every field to the readout worker too", () => {
    const { pool, created } = withReadoutWorker(2);
    pool.fieldInit(MOON, blockOf(8, 1));
    expect(created.map((w) => w.fieldMessages.map((m) => m.kind))).toEqual([
      ["field-init"],
      ["field-init"],
      ["field-init"],
    ]);
  });

  it("bakes nothing on the readout worker, made first, each bake worker two deep", () => {
    const { created } = busy();
    expect(created.map((w) => w.pendingBakes.length)).toEqual([0, 2, 2]);
  });

  it("asks a readout of the readout worker at once while the bake workers hold their bakes, and answers with it", async () => {
    const { pool, created } = busy();
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    expect(created.map((w) => w.pendingGroundReadouts.length)).toEqual([1, 0, 0]);
    answerReadout(workerAt(created, 0), NOT_SURVEYED);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
  });

  it("asks a class map of the least loaded bake worker, keeping the readout worker for the readouts", async () => {
    const { pool, created } = withReadoutWorker(2);
    pool.fieldInit(MOON, blockOf(8, 0));
    const map = pool.classMap(MOON, SEED);
    expect(created.map((w) => w.pendingClassMaps.length)).toEqual([0, 1, 0]);
    answerClassMap(workerAt(created, 1), null);
    await expect(map).resolves.toBeNull();
  });

  it("asks a readout out of a failed readout worker's replacement again, once the fields are posted to it", async () => {
    const { pool, created } = withReadoutWorker(2);
    pool.fieldInit(MOON, blockOf(8, 0));
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    workerAt(created, 0).fail();
    const replacement = workerAt(created, 3);
    expect(replacement.received.map((m) => m.kind)).toEqual(["field-init", "ground-readout"]);
    answerReadout(replacement, NOT_SURVEYED);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
  });

  it("moves a readout out to a bake worker once the readout worker's place is given up", async () => {
    const { pool, created } = withReadoutWorker(2);
    pool.fieldInit(MOON, blockOf(8, 0));
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    giveUp(created, 0);
    expect(workerAt(created, 1).pendingGroundReadouts).toHaveLength(1);
    answerReadout(workerAt(created, 1), NOT_SURVEYED);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
  });

  it("asks a readout of the least loaded bake worker once the readout worker's place is given up", async () => {
    const { pool, created } = withReadoutWorker(2);
    pool.fieldInit(MOON, blockOf(8, 0));
    giveUp(created, 0);
    const map = pool.classMap(MOON, SEED);
    const asked = pool.groundReadout(MOON, SEED, READOUT_QUERY);
    // The class map loads the first bake worker; the readout goes to the second.
    expect([1, 2].map((n) => workerAt(created, n).pendingGroundReadouts.length)).toEqual([0, 1]);
    answerReadout(workerAt(created, 2), NOT_SURVEYED);
    answerClassMap(workerAt(created, 1), null);
    await expect(asked).resolves.toBe(NOT_SURVEYED);
    await expect(map).resolves.toBeNull();
  });

  it("asks a class map of the readout worker once every bake worker's place is given up", async () => {
    const { pool, created } = withReadoutWorker(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    giveUp(created, 1);
    const map = pool.classMap(MOON, SEED);
    expect(workerAt(created, 0).pendingClassMaps).toHaveLength(1);
    answerClassMap(workerAt(created, 0), null);
    await expect(map).resolves.toBeNull();
  });

  it("can bake nothing more once its bake workers are given up, whatever the readout worker", () => {
    const { pool, created } = withReadoutWorker(1);
    const lane = pool.lane(laneSettings());
    giveUp(created, 1);
    expect([lane.unavailable, workerAt(created, 0).terminated]).toEqual([true, false]);
  });
});

/** The moon's lit bakes on the high setting: double normals, which a lane may draft. */
function highSettings(source: HeightSourceSetting = MOON_SOURCE): BakeSettings {
  return { vertexPath: "baked-offsets", normals: "double", source };
}

/** The bakes a worker was sent, as each key and whether it was drafted. */
function draftsOf(w: ScriptedHeightWorker): string[] {
  return w.received.flatMap((m) =>
    m.kind === "bake" ? [`${patchKeyString(m.key)}${m.draft === true ? " drafted" : ""}`] : [],
  );
}

/** Answers a worker's oldest refinement, complete or not. */
function answerRefinement(w: ScriptedHeightWorker, complete = true): void {
  const next = w.pendingRefinements[0];
  if (next === undefined) {
    throw new Error("no refinement is pending");
  }
  w.reply({
    kind: "normals-refined",
    id: next.id,
    refined: {
      key: next.key,
      generation: next.generation,
      normals: new Float32Array(2 * NORMALS_BETWEEN),
      complete,
    },
  });
}

describe("the height-worker pool's progressive bakes (R10.T6.a)", () => {
  it("drafts a body's bakes at double normals while others wait behind them, and the last whole", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lane = pool.lane(highSettings());
    lane.reprioritise([req(1, 3), req(2, 2), req(3, 1)]);
    const worker = workerAt(created, 0);
    worker.bakeOldest();
    worker.bakeOldest();
    expect(draftsOf(worker)).toEqual([
      `${patchKeyString(keyAt(1))} drafted`,
      `${patchKeyString(keyAt(2))} drafted`,
      patchKeyString(keyAt(3)),
    ]);
  });

  it("drafts nothing at mesh normals, with its normals skipped, or of the test planet", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    for (const settings of [
      laneSettings(),
      highSettings({ ...MOON_SOURCE, normals: "skipped" }),
      highSettings(TEST_PLANET),
    ]) {
      const lane = pool.lane(settings);
      lane.reprioritise([req(1, 3), req(2, 2), req(3, 1)]);
      const worker = workerAt(created, 0);
      while (worker.pendingBakes.length > 0) {
        worker.bakeOldest();
      }
      lane.terminate();
    }
    expect(draftsOf(workerAt(created, 0)).filter((line) => line.endsWith("drafted"))).toEqual([]);
  });

  it("refines once no bake is queued, to workers with nothing in flight, best first, with their tickets", () => {
    const { pool, created } = fieldsOnly(2);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lane = pool.lane(highSettings());
    const told: string[] = [];
    lane.onRefined((r) => told.push(`${patchKeyString(r.key)} ${String(r.ticket)}`));
    lane.reprioritise([req(1, 1), req(2, 1), req(3, 1)]);
    lane.refine({ key: keyAt(7), priority: 1, ticket: 70 });
    lane.refine({ key: keyAt(8), priority: 5, ticket: 80 });
    const [a, b] = [workerAt(created, 0), workerAt(created, 1)];
    // The three bakes go first, two to one worker; nothing refines while any waits.
    expect([a.pendingRefinements.length, b.pendingRefinements.length]).toEqual([0, 0]);
    a.bakeOldest();
    b.bakeOldest();
    expect(lane.queuedCount).toBe(0);
    // Worker b is idle: the best refinement; worker a still has a bake in flight, so none.
    expect([a.pendingRefinements.length, b.pendingRefinements.map((m) => m.key)]).toEqual([
      0,
      [keyAt(8)],
    ]);
    a.bakeOldest();
    expect(a.pendingRefinements.map((m) => m.key)).toEqual([keyAt(7)]);
    answerRefinement(a);
    answerRefinement(b);
    expect([told, lane.refiningCount]).toEqual([
      [`${patchKeyString(keyAt(7))} 70`, `${patchKeyString(keyAt(8))} 80`],
      0,
    ]);
  });

  it("tells no refinement dropped, and one asked again in flight once, with the later ticket", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lane = pool.lane(highSettings());
    const told: number[] = [];
    lane.onRefined((r) => told.push(r.ticket));
    lane.refine({ key: keyAt(1), priority: 1, ticket: 1 });
    const worker = workerAt(created, 0);
    lane.dropRefinement(patchKeyString(keyAt(1)));
    answerRefinement(worker);
    lane.refine({ key: keyAt(2), priority: 1, ticket: 2 });
    // Asked again in flight, for a newer draft: the normals refined are the key's over the field,
    // whichever draft asked, so the flight answers the later ticket and none is sent again.
    lane.refine({ key: keyAt(2), priority: 1, ticket: 3 });
    answerRefinement(worker);
    expect([told, worker.pendingRefinements.length, lane.refiningCount]).toEqual([[3], 0, 0]);
  });

  it("asks an incomplete refinement made before a block again, and waits one the field cannot make for its next", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lane = pool.lane(highSettings());
    const told: boolean[] = [];
    lane.onRefined((r) => told.push(r.complete));
    lane.refine({ key: keyAt(1), priority: 1, ticket: 1 });
    const worker = workerAt(created, 0);
    pool.fieldChunk(MOON, blockOf(8, 1));
    answerRefinement(worker, false);
    expect([told, worker.pendingRefinements.length]).toEqual([[], 1]);
    const pending = worker.pendingRefinements[0];
    if (pending === undefined) {
      throw new Error("no refinement is pending");
    }
    worker.reply({ kind: "bake-unsurveyed", id: pending.id, message: "a margin to come" });
    expect(worker.pendingRefinements).toEqual([]);
    pool.fieldChunk(MOON, blockOf(8, 2));
    answerBlock(worker, 2, 0);
    answerRefinement(worker, false);
    expect(told).toEqual([false]);
  });

  it("asks a failed worker's refinement again, and drops a refused one", () => {
    const { pool, created } = fieldsOnly(1);
    pool.fieldInit(MOON, blockOf(8, 0));
    const lane = pool.lane(highSettings());
    const told: number[] = [];
    lane.onRefined((r) => told.push(r.ticket));
    lane.refine({ key: keyAt(1), priority: 1, ticket: 1 });
    workerAt(created, 0).fail();
    const replacement = workerAt(created, 1);
    replacement.answerFields();
    const pending = replacement.pendingRefinements[0];
    if (pending === undefined) {
      throw new Error("no refinement is pending");
    }
    replacement.reply({ kind: "bake-failed", id: pending.id, message: "refused" });
    expect([told, lane.refiningCount]).toEqual([[], 0]);
  });
});
