import { describe, expect, it } from "vitest";

import { halfStepAt, surfaceChannelOff, writeSlackSteps } from "./writeSlack";

/**
 * The RTX 3080's worst pixel's channel (`decision-r08-t8-leans.md` §8): colour × alpha and rgb
 * summing to 0.0325606, 1,066.95 half-float steps of 2⁻¹⁵ in its binade [2⁻⁵, 2⁻⁴).
 */
const COLOUR = 0.02;
const RGB = 0.012_560_6;
const EXPECTED = COLOUR + RGB;

/** The half float's step in that binade, 2⁻¹⁵. */
const BINADE_STEP = 2 ** -15;

/** The step rounding to nearest takes there: 2⁻¹⁰ of the value plus 2⁻²⁴. */
const NEAREST_STEP = 2 ** -10 * EXPECTED + 2 ** -24;

describe("the half float's step (decision-r08-t8-leans.md §8)", () => {
  it("is 2⁻¹⁰ of the binade's base for a truncating or unknown write", () => {
    expect(halfStepAt(EXPECTED, "toward-zero")).toBe(BINADE_STEP);
    expect(halfStepAt(-EXPECTED, "unknown")).toBe(BINADE_STEP);
  });

  it("is 2⁻²⁴ among the subnormals", () => {
    expect(halfStepAt(2 ** -15, "toward-zero")).toBe(2 ** -24);
  });

  it("is 2⁻¹⁰ of the value plus 2⁻²⁴ when the target rounds to nearest", () => {
    expect(halfStepAt(EXPECTED, "nearest")).toBe(NEAREST_STEP);
  });
});

describe("the write's slack under the target's rounding (decision-r08-t8-leans.md §8)", () => {
  it("is half a step either way when the target rounds to nearest", () => {
    expect(writeSlackSteps("nearest")).toEqual({ under: 0.5, over: 0.5 });
    const offs = [-0.55, -0.45, 0.45, 0.55].map((k) =>
      surfaceChannelOff(EXPECTED + k * NEAREST_STEP, COLOUR, RGB, "nearest"),
    );
    expect(offs.map((off) => off < 1)).toEqual([false, true, true, false]);
  });

  // The RTX 3080's worst pixel: a truncated write 0.95 of a step under the value, which the
  // half-step bound failed at 1.81 of it.
  it("takes a write up to one step under the value when the target truncates", () => {
    const off = surfaceChannelOff(EXPECTED - 0.95 * BINADE_STEP, COLOUR, RGB, "toward-zero");
    expect(off).toBeLessThan(1);
  });

  it("refuses a write over the value when the target truncates", () => {
    const off = surfaceChannelOff(EXPECTED + 0.1 * BINADE_STEP, COLOUR, RGB, "toward-zero");
    expect(off).toBeGreaterThan(1);
  });

  it("refuses a truncating write more than one step under the value", () => {
    const off = surfaceChannelOff(EXPECTED - 1.05 * BINADE_STEP, COLOUR, RGB, "toward-zero");
    expect(off).toBeGreaterThan(1);
  });

  it("takes a write up to one step either way when the target's rounding is unknown", () => {
    const offs = [-0.95, 0.95].map((k) =>
      surfaceChannelOff(EXPECTED + k * BINADE_STEP, COLOUR, RGB, "unknown"),
    );
    expect(offs.every((off) => off < 1)).toBe(true);
  });

  it("is 0 for a write that lands on the value", () => {
    expect(surfaceChannelOff(EXPECTED, COLOUR, RGB, "toward-zero")).toBe(0);
  });
});
