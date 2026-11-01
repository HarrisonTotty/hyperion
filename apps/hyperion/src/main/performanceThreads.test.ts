import { describe, expect, it } from "vitest";

import {
  EFFICIENCY_THREADS_NODE,
  PERFORMANCE_THREADS_NODE,
  performanceThreadsOf,
} from "./performanceThreads";

/**
 * A Mac's `sysctl` of `performance` and `efficiency` threads, recording the nodes it is asked; an
 * `efficiency` of `null` is a Mac whose `sysctl` has no efficiency level's node.
 */
function mac(performance: string, efficiency: string | null, asked: string[] = []) {
  return (node: string): string => {
    asked.push(node);
    if (node === PERFORMANCE_THREADS_NODE) {
      return performance;
    }
    if (node === EFFICIENCY_THREADS_NODE && efficiency !== null) {
      return efficiency;
    }
    throw new Error(`sysctl: unknown oid '${node}'`);
  };
}

describe("the machine's performance threads", () => {
  it("reads macOS's performance level's processors, and its efficiency level's", () => {
    const asked: string[] = [];
    const threads = performanceThreadsOf("darwin", mac("8\n", "2\n", asked));
    expect([threads, asked]).toEqual([8, [PERFORMANCE_THREADS_NODE, EFFICIENCY_THREADS_NODE]]);
  });

  it("counts an Apple chip's efficiency cores at a third, rounded down", () => {
    // An M5's four and six, an M1's four and four, an M1 Pro's eight and two.
    expect(
      [
        ["4", "6"],
        ["4", "4"],
        ["8", "2"],
      ].map(([p, e]) => performanceThreadsOf("darwin", mac(p ?? "", e ?? ""))),
    ).toEqual([6, 5, 8]);
  });

  it("takes the performance cores alone where the efficiency level names no whole number", () => {
    expect(performanceThreadsOf("darwin", mac("6\n", null))).toBe(6);
    for (const text of ["", "four", "2.5", "-3"]) {
      expect(performanceThreadsOf("darwin", mac("6", text))).toBe(6);
    }
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
