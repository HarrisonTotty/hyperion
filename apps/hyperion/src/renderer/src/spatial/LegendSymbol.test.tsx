import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { markShiftDevicePx } from "../lib/strokes";
import { LegendSymbol } from "./LegendSymbol";
import type { SymbolShape } from "./marks";

/** The legend's radius in its 10-unit box, as built. */
const MARK_RADIUS = 4.25;

/** The radii, in the box's units, of the paths a legend symbol draws, ring first. */
function radii(shape: SymbolShape, diameterRem: number, ratio: number): number[] {
  vi.stubGlobal("devicePixelRatio", ratio);
  const { unmount } = render(
    <LegendSymbol shape={shape} diameterRem={diameterRem} filled={false} name="Sample" />,
  );
  const sample = screen.getByRole("img", { name: "Sample" });
  const paths = [...sample.querySelectorAll("path")].map((path) => {
    // A circle starts "M -r 0"; a polygon's first corner is its first move.
    const [x = 0, y = 0] = (path.getAttribute("d") ?? "")
      .replace(/^M /u, "")
      .split(" ")
      .slice(0, 2)
      .map(Number);
    return Math.hypot(x, y);
  });
  unmount();
  return paths;
}

/** δ in the box's units for a symbol `diameterRem` across, at a rem of 16 px. */
function shiftUnits(diameterRem: number, ratio: number): number {
  return ((markShiftDevicePx(ratio) / ratio) * 10) / (diameterRem * 16);
}

describe("LegendSymbol (R07.T16.f)", () => {
  it.each([0.78125, 1, 2, 3])(
    "moves a circle's outline out by the painter's shift, in its box's units, at a ratio of %s",
    (ratio) => {
      expect(radii("circle", 1, ratio)[0]).toBeCloseTo(MARK_RADIUS + shiftUnits(1, ratio), 9);
    },
  );

  it("moves it 0.53 CSS px out at 0.78125, whatever the symbol's size", () => {
    for (const diameterRem of [0.5, 0.75, 1]) {
      const [radius = 0] = radii("circle", diameterRem, 0.78125);
      const cssPxPerUnit = (diameterRem * 16) / 10;
      expect(Math.round((radius - MARK_RADIUS) * cssPxPerUnit * 100) / 100).toBe(0.53);
    }
  });

  it("moves a polygon's sides out by the shift, its corners by the shift over its inradius", () => {
    const [corner = 0] = radii("triangle-down", 1, 0.78125);

    // The triangle's sides lie at half its corners' radius from its centroid.
    expect(corner).toBeCloseTo(MARK_RADIUS + shiftUnits(1, 0.78125) / 0.5, 9);
  });

  it("moves a ringed circle's ring by three shifts and its disc by one", () => {
    const [ring = 0, disc = 0] = radii("ringed-circle", 1, 0.78125);
    const shift = shiftUnits(1, 0.78125);

    expect(ring).toBeCloseTo(MARK_RADIUS + 3 * shift, 9);
    expect(disc).toBeCloseTo(MARK_RADIUS / 3 + shift, 9);
  });

  it("draws every outline as built from a ratio of 4/3 up", () => {
    const [ring = 0, disc = 0] = radii("ringed-circle", 1, 2);

    expect(ring).toBeCloseTo(MARK_RADIUS, 12);
    expect(disc).toBeCloseTo(MARK_RADIUS / 3, 12);
  });
});
