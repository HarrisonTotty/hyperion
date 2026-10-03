import { describe, expect, it } from "vitest";

import { nnls } from "./nnls";

describe("nnls", () => {
  it("matches ordinary least squares when its solution is positive", () => {
    // x = (1, 2) solves the square system exactly.
    const x = nnls(
      [
        [2, 1],
        [1, 3],
      ],
      [4, 7],
    );
    expect(x[0]).toBeCloseTo(1, 12);
    expect(x[1]).toBeCloseTo(2, 12);
  });

  it("clamps a component that least squares would make negative", () => {
    // b = A (2, −1). Constrained: x₂ = 0, x₁ = (a₁ · b) ÷ |a₁|² = 1 ÷ 2.
    const x = nnls(
      [
        [1, 1],
        [1, 2],
      ],
      [1, 0],
    );
    expect(x[1]).toBe(0);
    expect(x[0]).toBeCloseTo(0.5, 12);
  });

  it("returns zero when every column points away from the target", () => {
    const x = nnls(
      [
        [1, 2],
        [3, 1],
      ],
      [-1, -2],
    );
    expect(x).toEqual([0, 0]);
  });
});
