import { describe, expect, it } from "vitest";

import { viewPixelSize } from "./viewSize";

describe("a view's pixel size", () => {
  it("scales by the device pixel ratio and rounds", () => {
    expect(viewPixelSize({ width: 100.4, height: 50.2 }, 1.5, 8192)).toEqual({
      widthPx: 151,
      heightPx: 75,
    });
  });

  it("clamps to the device's limit", () => {
    expect(viewPixelSize({ width: 6000, height: 100 }, 2, 8192)).toEqual({
      widthPx: 8192,
      heightPx: 200,
    });
  });

  it("never returns zero", () => {
    expect(viewPixelSize({ width: 0, height: 0.2 }, 1, 8192)).toEqual({ widthPx: 1, heightPx: 1 });
  });

  it("gives one pixel for a size that is not finite", () => {
    expect(viewPixelSize({ width: Number.NaN, height: 10 }, 1, 8192).widthPx).toBe(1);
  });
});
