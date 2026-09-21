import { describe, expect, it } from "vitest";

import { localFrameAt } from "./frame";
import { isAbovePlane, type SizeClass, type SymbolShape } from "./marks";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX, symbolOutline } from "./symbols";
import { vec3 } from "./vec3";

const POLYGONS: ReadonlyArray<SymbolShape> = ["diamond", "square", "triangle"];
const SIZE_CLASSES: ReadonlyArray<SizeClass> = [0, 1, 2, 3, 4];

describe("symbolOutline", () => {
  it("marks a circle as a circle", () => {
    expect(symbolOutline("circle")).toEqual({ kind: "circle" });
  });

  it.each(POLYGONS)("draws the %s as a closed, centred polygon inside the unit circle", (shape) => {
    const outline = symbolOutline(shape);
    if (outline.kind !== "polygon") {
      throw new Error(`${shape} should be a polygon`);
    }
    const { points } = outline;
    const vertices = points.slice(0, -1);

    expect(points.at(-1)).toEqual(points[0]);
    expect(vertices.length).toBeGreaterThanOrEqual(3);
    const centroidX = vertices.reduce((sum, point) => sum + point.x, 0) / vertices.length;
    const centroidY = vertices.reduce((sum, point) => sum + point.y, 0) / vertices.length;
    expect(centroidX).toBeCloseTo(0, 12);
    expect(centroidY).toBeCloseTo(0, 12);
    for (const point of points) {
      expect(Math.hypot(point.x, point.y)).toBeLessThanOrEqual(1 + 1e-12);
    }
  });

  it("gives every shape a distinct outline", () => {
    const outlines = (["circle", ...POLYGONS] as const).map((shape) => symbolOutline(shape));

    outlines.forEach((outline, index) => {
      for (const other of outlines.slice(index + 1)) {
        expect(outline).not.toEqual(other);
      }
    });
  });
});

describe("SIZE_CLASS_REM", () => {
  it("grows strictly with the size class from at least 0.5 rem", () => {
    const sizes = SIZE_CLASSES.map((sizeClass) => SIZE_CLASS_REM[sizeClass]);

    expect(sizes[0]).toBeGreaterThanOrEqual(0.5);
    for (let i = 1; i < sizes.length; i += 1) {
      expect(sizes[i]).toBeGreaterThan(sizes[i - 1] ?? Number.POSITIVE_INFINITY);
    }
  });

  it("leaves the smallest open symbol a visible hole at 80% scale", () => {
    const diameterPx = SIZE_CLASS_REM[0] * 16 * 0.8;

    expect(diameterPx - 2 * SYMBOL_STROKE_PX).toBeGreaterThanOrEqual(3);
  });
});

describe("isAbovePlane", () => {
  const frame = localFrameAt(vec3(26_000, 0, 0));

  it.each([
    [vec3(1, 2, 0.5), true],
    [vec3(1, 2, 0), true],
    [vec3(1, 2, -0.001), false],
  ])("puts %o above the plane: %s", (position, above) => {
    expect(isAbovePlane(frame, position)).toBe(above);
  });
});
