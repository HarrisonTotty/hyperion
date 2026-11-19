import { describe, expect, it } from "vitest";

import { overlappedPasses, serialPasses } from "../test/passTimes";
import { ownBeside, percentile, recordPassTimes } from "./soak";

describe("a nearest-rank percentile", () => {
  it("takes the value at the rank ceil(p × n)", () => {
    const values = [5, 1, 4, 2, 3];
    expect(percentile(values, 0.5)).toBe(3);
    expect(percentile(values, 0.95)).toBe(5);
    expect(percentile(values, 1)).toBe(5);
  });

  it("takes the smallest value at p = 0, and is NaN with no values", () => {
    expect(percentile([7, 3], 0)).toBe(3);
    expect(percentile([], 0.5)).toBeNaN();
  });
});

/** What a smoke check states beside the bodies' and the composite's spans, for one resolve. */
function ownStated(passes: ReturnType<typeof serialPasses>): readonly [string, string] {
  const spans = new Map<string, number[]>();
  const own = new Map<string, number[]>();
  recordPassTimes({ frame: 1, timer: "full", passes }, spans, own);
  return [
    ownBeside(spans.get("bodies") ?? [], own.get("bodies") ?? []),
    ownBeside(spans.get("atmosphere composite") ?? [], own.get("atmosphere composite") ?? []),
  ];
}

describe("a smoke run's pass times (lane fix-floor-calm)", () => {
  it("states a pass's own time beside its span where Metal's spans overlap, and nothing on a serial timer", () => {
    expect([
      ownStated(overlappedPasses(4e6, 12e6, 0.5e6)),
      ownStated(
        serialPasses([
          ["bodies", 4e6],
          ["atmosphere composite", 0.5e6],
        ]),
      ),
    ]).toEqual([
      ["", ", own 500.0"],
      ["", ""],
    ]);
  });
});
