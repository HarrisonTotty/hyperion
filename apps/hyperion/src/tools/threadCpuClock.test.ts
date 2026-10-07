import { describe, expect, it } from "vitest";

import { type CpuUsageSource, threadCpuClockMs } from "./threadCpuClock";

/**
 * A kernel that counts CPU time in ticks: the thread's reading is its runtime as of the last
 * update, which a tick or the process's reading makes.
 */
function tickedKernel(): {
  source: CpuUsageSource;
  run: (durationUs: number) => void;
  tick: () => void;
} {
  let runtimeUs = 0;
  let updatedUs = 0;
  const update = (): void => {
    updatedUs = runtimeUs;
  };
  return {
    source: {
      cpuUsage: () => {
        update();
        return { user: updatedUs, system: 0 };
      },
      threadCpuUsage: () => ({ user: updatedUs, system: 0 }),
    },
    run: (durationUs) => {
      runtimeUs += durationUs;
    },
    tick: update,
  };
}

describe("the demand record's thread CPU clock", () => {
  it("reads a selection shorter than a tick at its own length", () => {
    const kernel = tickedKernel();
    const clock = threadCpuClockMs(kernel.source);
    kernel.run(1_000);
    kernel.tick();
    kernel.run(250);
    const start = clock?.() ?? NaN;
    kernel.run(370);
    expect((clock?.() ?? NaN) - start).toBeCloseTo(0.37, 12);
  });

  it("sums the thread's user and system time, in milliseconds", () => {
    const clock = threadCpuClockMs({
      cpuUsage: () => ({ user: 0, system: 0 }),
      threadCpuUsage: () => ({ user: 1_500, system: 250 }),
    });
    expect(clock?.()).toBe(1.75);
  });

  it("is absent where Node has no thread CPU time", () => {
    expect(threadCpuClockMs({ cpuUsage: () => ({ user: 0, system: 0 }) })).toBeUndefined();
  });
});
