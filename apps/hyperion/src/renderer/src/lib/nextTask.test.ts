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
});
