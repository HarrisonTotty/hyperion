import { describe, expect, it } from "vitest";

import type { Camera } from "./camera";
import { easeOut, TRANSITION_MS, tweenCamera } from "./transition";

function camera(azimuthDeg: number, elevationDeg: number, pxPerUnit = 4): Camera {
  return { azimuthDeg, elevationDeg, pxPerUnit };
}

describe("TRANSITION_MS", () => {
  it("is inside the guide's 80 to 150 ms", () => {
    expect(TRANSITION_MS).toBeGreaterThanOrEqual(80);
    expect(TRANSITION_MS).toBeLessThanOrEqual(150);
  });
});

describe("easeOut", () => {
  it("runs from exactly 0 to exactly 1", () => {
    expect(easeOut(0)).toBe(0);
    expect(easeOut(1)).toBe(1);
  });

  it("is monotonic and eases out", () => {
    let previous = easeOut(0);
    for (let step = 1; step <= 100; step += 1) {
      const value = easeOut(step / 100);
      expect(value).toBeGreaterThanOrEqual(previous);
      previous = value;
    }
    expect(easeOut(0.5)).toBe(0.875);
  });

  it("clamps progress outside 0 to 1", () => {
    expect(easeOut(-0.5)).toBe(0);
    expect(easeOut(1.7)).toBe(1);
  });
});

describe("tweenCamera", () => {
  it("turns from 350° to 010° through 000°, not 180°", () => {
    const from = camera(350, 30);
    const to = camera(10, 30);

    expect(tweenCamera(from, to, 0.5).azimuthDeg).toBeCloseTo(0, 9);
    expect(tweenCamera(from, to, 0.25).azimuthDeg).toBeCloseTo(355, 9);
    expect(tweenCamera(from, to, 0.75).azimuthDeg).toBeCloseTo(5, 9);
  });

  it("turns the other way from 010° to 350°", () => {
    expect(tweenCamera(camera(10, 0), camera(350, 0), 0.25).azimuthDeg).toBeCloseTo(5, 9);
  });

  it("moves the elevation linearly and the zoom evenly in its logarithm", () => {
    const middle = tweenCamera(camera(0, -30, 1), camera(0, 90, 100), 0.5);

    expect(middle.elevationDeg).toBeCloseTo(30, 9);
    expect(middle.pxPerUnit).toBeCloseTo(10, 9);
  });

  it("starts at from and ends exactly at to", () => {
    const from = camera(30, 30, 3.3);
    const to = camera(0, 90, 3.3);

    expect(tweenCamera(from, to, 0)).toBe(from);
    expect(tweenCamera(from, to, 1)).toBe(to);
    expect(tweenCamera(from, to, easeOut(1))).toBe(to);
  });
});
