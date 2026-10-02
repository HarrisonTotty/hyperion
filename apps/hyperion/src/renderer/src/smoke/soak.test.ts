import { describe, expect, it } from "vitest";

import { percentile } from "./soak";

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
