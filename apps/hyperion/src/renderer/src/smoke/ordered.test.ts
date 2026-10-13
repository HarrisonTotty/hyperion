import { describe, expect, it } from "vitest";

import { Checks } from "./harness";
import { attempt, inOrder } from "./ordered";

/** A promise and its resolver, so that a test settles work in the order it chooses. */
function deferred(): { readonly promise: Promise<void>; readonly resolve: () => void } {
  const settles: Array<() => void> = [];
  const promise = new Promise<void>((settle) => {
    settles.push(settle);
  });
  return {
    promise,
    resolve: () => {
      for (const settle of settles) {
        settle();
      }
    },
  };
}

/** The names of a run's checks, in order. */
function names(checks: Checks): string[] {
  return checks.list.map((check) => check.name);
}

describe("checks in the order their work began", () => {
  it("records each slot's checks in the order begun, though the work ends in another", async () => {
    const checks = new Checks();
    const first = deferred();
    const second = deferred();
    const run = inOrder(checks, async (ordered) => {
      ordered.later(async (slot) => {
        await first.promise;
        slot.check("first a", true, "");
        slot.check("first b", true, "");
      });
      ordered.now((slot) => {
        slot.check("between", true, "");
      });
      ordered.later(async (slot) => {
        await second.promise;
        slot.check("second", true, "");
      });
      await Promise.resolve();
    });
    second.resolve();
    await Promise.resolve();
    first.resolve();
    await run;
    expect(names(checks)).toEqual(["first a", "first b", "between", "second"]);
  });

  it("stops at a slot that throws: the slots before it and its own checks so far, then its throw", async () => {
    const checks = new Checks();
    const run = inOrder(checks, async (ordered) => {
      ordered.now((slot) => {
        slot.check("before", true, "");
      });
      ordered.later(async (slot) => {
        await Promise.resolve();
        slot.check("its own", true, "");
        throw new RangeError("the twin refused");
      });
      ordered.later(async (slot) => {
        await Promise.resolve();
        slot.check("after", true, "");
      });
      await Promise.resolve();
    });
    await expect(run).rejects.toThrow(new RangeError("the twin refused"));
    expect(names(checks)).toEqual(["before", "its own"]);
  });

  it("records every slot begun before the group's own throw, then fails with that throw", async () => {
    const checks = new Checks();
    const run = inOrder(checks, async (ordered) => {
      ordered.later(async (slot) => {
        await Promise.resolve();
        slot.check("begun", true, "");
      });
      await Promise.resolve();
      throw new Error("the GPU step failed");
    });
    await expect(run).rejects.toThrow("the GPU step failed");
    expect(names(checks)).toEqual(["begun"]);
  });

  it("fails with a slot's throw over the group's later one, as the sequence would have", async () => {
    const checks = new Checks();
    const run = inOrder(checks, async (ordered) => {
      ordered.later(async () => {
        await Promise.resolve();
        throw new Error("the earlier twin");
      });
      await Promise.resolve();
      throw new Error("the later GPU step");
    });
    await expect(run).rejects.toThrow("the earlier twin");
  });

  it("begins no more work once a slot has failed, and fails with that slot's throw", async () => {
    const checks = new Checks();
    let steps = 0;
    const run = inOrder(checks, async (ordered) => {
      ordered.later(async () => {
        await Promise.resolve();
        throw new Error("the first twin");
      });
      for (let i = 0; i < 5; i += 1) {
        // Each step waits a turn, as a GPU read-back does, before it begins its slot.
        // oxlint-disable-next-line no-await-in-loop
        await new Promise((resolve) => {
          setTimeout(resolve, 0);
        });
        ordered.now((slot) => {
          slot.check(`step ${i}`, true, "");
        });
        steps += 1;
      }
    });
    await expect(run).rejects.toThrow("the first twin");
    expect(steps).toBe(0);
    expect(names(checks)).toEqual([]);
  });

  it("turns a computation's throw into a rejection read where it is awaited", async () => {
    const refused = attempt((): number => {
      throw new RangeError("refused");
    });
    await expect(refused).rejects.toThrow(new RangeError("refused"));
    await expect(attempt(() => 7)).resolves.toBe(7);
  });
});
