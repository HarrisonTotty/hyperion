import { describe, expect, it } from "vitest";

import { wheelSteps } from "./ViewCanvas";

describe("the wheel's zoom steps (plan R15, T4)", () => {
  it("makes a step of 100 px, nearer for a scroll up", () => {
    expect([wheelSteps(-100, 0), wheelSteps(250, 0)]).toEqual([1, -2.5]);
  });

  it("makes a step of a line or a page, whatever its size", () => {
    expect([wheelSteps(-3, 1), wheelSteps(1, 2)]).toEqual([1, -1]);
  });
});
