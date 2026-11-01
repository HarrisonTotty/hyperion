import { describe, expect, it } from "vitest";

import { PERFORMANCE_THREADS_NODE, performanceThreadsOf } from "./performanceThreads";

describe("the machine's performance threads", () => {
  it("reads macOS's performance level's processors", () => {
    const asked: string[] = [];
    const threads = performanceThreadsOf("darwin", (node) => {
      asked.push(node);
      return "8\n";
    });
    expect([threads, asked]).toEqual([8, [PERFORMANCE_THREADS_NODE]]);
  });

  it("is none off macOS, where nothing is read", () => {
    for (const platform of ["linux", "win32"] as const) {
      expect(
        performanceThreadsOf(platform, () => {
          throw new Error("read off macOS");
        }),
      ).toBeNull();
    }
  });

  it("is none on a Mac with no performance level, or one that names no whole number of them", () => {
    expect(
      performanceThreadsOf("darwin", () => {
        throw new Error("sysctl: unknown oid 'hw.perflevel0.logicalcpu'");
      }),
    ).toBeNull();
    for (const text of ["0\n", "", "four", "2.5"]) {
      expect(performanceThreadsOf("darwin", () => text)).toBeNull();
    }
  });
});
