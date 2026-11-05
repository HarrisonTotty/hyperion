/**
 * The cull's counts of its strides drawn past τ′ (plan R14, R14.T3.e): the bit-exact copy of each
 * pass's word past its buckets, read back every sample through a ring of three buffers.
 */

import { describe, expect, it } from "vitest";

import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import type { BufferHandle } from "../engine/types";
import { BUCKET_ARGS_BYTES, ClusterBuckets } from "./buckets";
import { ClusterOverCounts } from "./overCount";
import { terrainClusterMesh } from "./templates";

/** Terrain's eight strided templates. */
const templates = terrainClusterMesh().templates.filter((t) => !t.mayDiscard);

/** A terrain view's room: 10 instances, 64 blocks and 32 strips each. */
const CAPACITIES = templates.map((t) => 10 * (t.shape === "terrain-block" ? 64 : 32));

/** A counting engine whose reads answer with `answer`, or wait until `release` while `held`. */
class AnsweringEngine extends CountingRenderEngine {
  answer = new Uint32Array();
  held = false;
  readonly #waiting: Array<() => void> = [];

  override readBuffer(_buffer?: BufferHandle): Promise<ArrayBuffer> {
    const bytes = this.answer.slice().buffer;
    if (!this.held) {
      return Promise.resolve(bytes);
    }
    return new Promise((resolve) => {
      this.#waiting.push(() => {
        resolve(bytes);
      });
    });
  }

  /** Answers every read held so far. */
  release(): void {
    for (const answer of this.#waiting.splice(0)) {
      answer();
    }
  }
}

/** Lets the reads' promises settle: a task's turn, after every microtask before it. */
async function settle(): Promise<void> {
  await new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

describe("the cull's counts of its strides past τ′ (plan R14, R14.T3.e)", () => {
  it("reads each pass's count back every sample through a bit-exact copy of the words past its buckets", async () => {
    const engine = new AnsweringEngine(await fakeDevice());
    engine.answer = Uint32Array.from([0, 3, 7]);
    const buckets = new ClusterBuckets(engine, "test clusters", 3, templates, CAPACITIES);
    const over = new ClusterOverCounts(engine, "test clusters");
    expect(over.latest()).toBeNull();
    over.sample(buckets);
    await settle();
    const dispatched = engine.dispatched.at(-1);
    expect([
      dispatched?.kernel,
      dispatched?.bindings.buffers["args"],
      [...(dispatched?.bindings.uniforms["params"] ?? new Uint32Array())],
      over.latest(),
      engine.kernelPairs.at(-1)?.readback,
    ]).toEqual([
      "cluster over counts",
      buckets.args,
      [3 * templates.length * (BUCKET_ARGS_BYTES / 4), 3, 0, 0],
      [0, 3, 7],
      "bit-exact",
    ]);
    over.dispose();
    buckets.dispose();
  });

  it("skips a sample while all three of its ring's reads are out, and samples again once one is back", async () => {
    const engine = new AnsweringEngine(await fakeDevice());
    engine.held = true;
    engine.answer = Uint32Array.from([0, 0, 1]);
    const buckets = new ClusterBuckets(engine, "test clusters", 3, templates, CAPACITIES);
    const over = new ClusterOverCounts(engine, "test clusters");
    const copies = (): number =>
      engine.dispatched.filter((d) => d.kernel === "cluster over counts").length;
    for (let n = 0; n < 4; n += 1) {
      over.sample(buckets);
    }
    const whileOut = [copies(), over.latest()];
    engine.release();
    await settle();
    over.sample(buckets);
    expect([whileOut, copies(), over.latest()]).toEqual([[3, null], 4, [0, 0, 1]]);
    over.dispose();
    buckets.dispose();
  });

  it("makes a larger slot where the passes outgrow a free one, and releases the one it replaces once the ring is full", async () => {
    const engine = new AnsweringEngine(await fakeDevice());
    const sizes: number[] = [];
    const released: string[] = [];
    const off = engine.onAllocation((event) => {
      if (event.kind === "created" && event.name.includes("over counts")) {
        sizes.push(event.bytes);
      }
      if (event.kind === "destroyed" && event.name.includes("over counts")) {
        released.push(event.name);
      }
    });
    const over = new ClusterOverCounts(engine, "test clusters");
    for (const passes of [1, 2, 3, 4]) {
      const buckets = new ClusterBuckets(engine, "test clusters", passes, templates, CAPACITIES);
      over.sample(buckets);
      // Each sample's read back before the next, so that its slot is free for the next to outgrow.
      // oxlint-disable-next-line no-await-in-loop
      await settle();
      buckets.dispose();
    }
    off();
    expect([sizes, released.length]).toEqual([[4, 8, 12, 16], 1]);
    over.dispose();
  });

  it("answers a read still out when it is disposed into nothing", async () => {
    const engine = new AnsweringEngine(await fakeDevice());
    engine.held = true;
    engine.answer = Uint32Array.from([5]);
    const buckets = new ClusterBuckets(engine, "test clusters", 1, templates, CAPACITIES);
    const over = new ClusterOverCounts(engine, "test clusters");
    over.sample(buckets);
    over.dispose();
    engine.release();
    await settle();
    expect(over.latest()).toBeNull();
    buckets.dispose();
  });
});
