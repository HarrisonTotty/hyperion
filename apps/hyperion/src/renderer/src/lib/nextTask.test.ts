import { afterEach, describe, expect, it, vi } from "vitest";

import { nextTask, untilNextTask } from "./nextTask";

afterEach(() => {
  vi.useRealTimers();
});

describe("nextTask (lane perf-long-tasks-3)", () => {
  it("runs its work after the task that asks for it and its microtasks", async () => {
    const order: string[] = [];
    nextTask(() => {
      order.push("next task");
    });
    queueMicrotask(() => {
      order.push("microtask");
    });
    order.push("this task");
    await untilNextTask();
    expect(order).toEqual(["this task", "microtask", "next task"]);
  });

  it("settles under a test's fake clock, which it does not wait on", async () => {
    vi.useFakeTimers();
    await expect(untilNextTask()).resolves.toBeUndefined();
  });

  it("runs its work once, at a fake clock's advance that gives the channel no turn", async () => {
    vi.useFakeTimers();
    let runs = 0;
    nextTask(() => {
      runs += 1;
    });
    await vi.advanceTimersByTimeAsync(0);
    const atAdvance = runs;
    vi.useRealTimers();
    // The channel's message, come since, finds the work run.
    await new Promise((resolve) => {
      setTimeout(resolve, 5);
    });
    expect([atAdvance, runs]).toEqual([1, 1]);
  });
});
