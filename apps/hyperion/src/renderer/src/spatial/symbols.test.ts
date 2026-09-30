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

/** The shapes the later plans add, each drawn before the display that gives it its meaning. */
const ADDED_POLYGONS: ReadonlyArray<SymbolShape> = ["triangle-down", "pentagon", "hexagon"];
const EVERY_SHAPE: ReadonlyArray<SymbolShape> = [
  "circle",
  ...POLYGONS,
  "ringed-circle",
  ...ADDED_POLYGONS,
];

/** The screen size at 100%, where a `rem` is 16 px. */
const REM_PX = 16;

/** The radius of an outline's path at a size class, as the draw list sets it (plan 05, D14). */
function pathRadiusPx(sizeClass: SizeClass, remPx = REM_PX): number {
  return (SIZE_CLASS_REM[sizeClass] * remPx) / 2 - SYMBOL_STROKE_PX / 2;
}

/** The distance from the centre to the nearest side of a closed unit polygon. */
function inradius(points: ReadonlyArray<{ readonly x: number; readonly y: number }>): number {
  let nearest = Number.POSITIVE_INFINITY;
  for (const [index, from] of points.slice(0, -1).entries()) {
    const to = points[index + 1] ?? from;
    const side = Math.hypot(to.x - from.x, to.y - from.y);
    nearest = Math.min(nearest, Math.abs(from.x * to.y - from.y * to.x) / side);
  }
  return nearest;
}

/** How many of a polygon's corners share its topmost height on the screen. */
function cornersAtTop(shape: SymbolShape): number {
  const outline = symbolOutline(shape);
  const corners = outline.kind === "polygon" ? outline.points.slice(0, -1) : [];
  const top = Math.min(...corners.map((point) => point.y));
  return corners.filter((point) => Math.abs(point.y - top) < 1e-12).length;
}

describe("the outlines the later plans add", () => {
  it.each(ADDED_POLYGONS)(
    "draws the %s as a closed, centred polygon inside the unit circle",
    (shape) => {
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
    },
  );

  it("draws the ringed circle as a centred disc inside a ring on the unit circle", () => {
    const outline = symbolOutline("ringed-circle");
    if (outline.kind !== "ringed-circle") {
      throw new Error("the ringed circle should be a ringed circle");
    }

    expect(outline.discRadius).toBeGreaterThan(0);
    expect(outline.discRadius).toBeLessThan(1);
  });

  it("points the pentagon up, with one corner at the top", () => {
    expect(cornersAtTop("pentagon")).toBe(1);
  });

  it("gives the hexagon a flat top, so that it reads apart from the pentagon", () => {
    expect(cornersAtTop("hexagon")).toBe(2);
  });

  it("gives the triangle-down a flat top where the triangle has its point", () => {
    expect([cornersAtTop("triangle"), cornersAtTop("triangle-down")]).toEqual([1, 2]);
  });

  it("turns the triangle over for triangle-down, so that the two share a hit area", () => {
    const up = symbolOutline("triangle");
    const down = symbolOutline("triangle-down");
    if (up.kind !== "polygon" || down.kind !== "polygon") {
      throw new Error("both triangles should be polygons");
    }
    const mirrored = new Set(
      up.points.map((point) => `${point.x.toFixed(12)},${(-point.y).toFixed(12)}`),
    );

    for (const point of down.points) {
      expect(mirrored).toContain(`${point.x.toFixed(12)},${point.y.toFixed(12)}`);
    }
  });

  it("gives every shape, old and new, a distinct outline", () => {
    const outlines = EVERY_SHAPE.map((shape) => symbolOutline(shape));

    outlines.forEach((outline, index) => {
      for (const other of outlines.slice(index + 1)) {
        expect(outline).not.toEqual(other);
      }
    });
  });

  it.each(ADDED_POLYGONS)(
    "leaves the open %s a hole at least as wide as its outline at the smallest size class",
    (shape) => {
      const outline = symbolOutline(shape);
      if (outline.kind !== "polygon") {
        throw new Error(`${shape} should be a polygon`);
      }

      // The largest circle about the centre that the outline's line leaves clear, at 100%.
      const holePx = 2 * (inradius(outline.points) * pathRadiusPx(0) - SYMBOL_STROKE_PX / 2);

      expect(holePx).toBeGreaterThanOrEqual(SYMBOL_STROKE_PX);
    },
  );

  it("leaves the ringed circle's gap and its open disc's hole each wider than its line at size class 2", () => {
    const outline = symbolOutline("ringed-circle");
    if (outline.kind !== "ringed-circle") {
      throw new Error("the ringed circle should be a ringed circle");
    }
    const ringPx = pathRadiusPx(2);
    const discPx = outline.discRadius * ringPx;

    // Between the disc's line and the ring's, and inside the disc's line, at 100%.
    const gapPx = ringPx - SYMBOL_STROKE_PX / 2 - (discPx + SYMBOL_STROKE_PX / 2);
    const holePx = 2 * (discPx - SYMBOL_STROKE_PX / 2);

    expect(gapPx).toBeGreaterThanOrEqual(SYMBOL_STROKE_PX);
    expect(holePx).toBeGreaterThanOrEqual(SYMBOL_STROKE_PX);
  });
});
