import { describe, expect, it } from "vitest";

import { performanceThreadsFromArgv, performanceThreadsSwitch } from "./machineLaunch";

describe("the performance threads' hand-off", () => {
  it("carries a count and its absence through the renderer's argv", () => {
    for (const threads of [8, 1, null]) {
      expect(performanceThreadsFromArgv(["--a", performanceThreadsSwitch(threads)])).toBe(threads);
    }
  });

  it("reads none from an argv that carries none, or no whole number", () => {
    expect(performanceThreadsFromArgv(["--hyperion-quality=high"])).toBeNull();
    for (const text of ["0", "-2", "3.5", "x", ""]) {
      expect(performanceThreadsFromArgv([`--hyperion-performance-threads=${text}`])).toBeNull();
    }
  });
});
