import { describe, expect, it } from "vitest";

import { keptBody } from "../../scenes/kept";
import { bakeOf, ScriptedHeightWorker } from "../../../test/scriptedHeightWorker";
import { type PatchKey, patchKeyString } from "../patchKey";
import type { PatchRequest } from "../select";
import {
  type FieldRefusal,
  HeightWorkerPool,
  MAX_CONSECUTIVE_WORKER_FAILURES,
  MAX_IN_FLIGHT_PER_WORKER,
} from "./pool";

/** Two bodies of the kept system, for the per-body field calls. */
const MOON = keptBody(1);
const PLANET = keptBody(2);

function keyAt(i: number): PatchKey {
  return { face: 0, level: 10, i, j: 0 };
}

function req(i: number, priority: number, forced = false): PatchRequest {
  return { key: keyAt(i), priority, forced };
}

function poolOf(workers: number): {
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
    bake: { vertexPath: "face-differences", normals: "mesh", ridges: "off" },
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

  it("bakes again what was baked before a field was started", () => {
    const { pool, created, baked } = poolOf(1);
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
