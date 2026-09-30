import { describe, expect, it } from "vitest";

import { localFrameAt } from "./frame";
import type { PlaneSpec } from "./marks";
import { gridLines, ringPolyline, type Segment } from "./plane";
import { cross, dot, norm, sub, vec3 } from "./vec3";

const PLANE: PlaneSpec = { spacing: 20, extent: 50, rings: [] };
// Off the x axis, so that coreward is not along x.
const FRAME = localFrameAt(vec3(15_600, 20_800, 0));

function length(segment: Segment): number {
  return norm(sub(segment.to, segment.from));
}

describe("gridLines", () => {
  const grid = gridLines(PLANE, FRAME);

  it("has five chords each way for extent 50 and spacing 20", () => {
    expect(grid.alongCoreward).toHaveLength(5);
    expect(grid.alongSpinward).toHaveLength(5);
  });

  it("makes the chord through the centre 100 long and those at ±40 60 long", () => {
    for (const family of [grid.alongCoreward, grid.alongSpinward]) {
      const lengths = family.map(length);

      expect(lengths[0]).toBeCloseTo(60, 9);
      expect(lengths[1]).toBeCloseTo(2 * Math.sqrt(2_100), 9);
      expect(lengths[2]).toBeCloseTo(100, 9);
      expect(lengths[3]).toBeCloseTo(2 * Math.sqrt(2_100), 9);
      expect(lengths[4]).toBeCloseTo(60, 9);
    }
  });

  it("ends every chord on the rim of the disc, in the plane", () => {
    for (const segment of [...grid.alongCoreward, ...grid.alongSpinward]) {
      for (const end of [segment.from, segment.to]) {
        expect(dot(end, FRAME.north)).toBeCloseTo(0, 12);
        expect(norm(end)).toBeCloseTo(50, 9);
      }
    }
  });

  it("aligns the first family to coreward, not to x, off the x axis", () => {
    for (const segment of grid.alongCoreward) {
      const direction = sub(segment.to, segment.from);

      expect(norm(cross(direction, FRAME.coreward))).toBeCloseTo(0, 9);
      expect(Math.abs(direction.x)).toBeGreaterThan(0);
      expect(Math.abs(direction.y)).toBeGreaterThan(0);
    }
    for (const segment of grid.alongSpinward) {
      expect(norm(cross(sub(segment.to, segment.from), FRAME.spinward))).toBeCloseTo(0, 9);
    }
  });

  it("leaves out a chord that would lie on the rim", () => {
    const rimmed = gridLines({ spacing: 20, extent: 40, rings: [] }, FRAME);

    expect(rimmed.alongCoreward).toHaveLength(3);
  });

  it("draws no grid for a plane without size", () => {
    expect(gridLines({ spacing: 0, extent: 50, rings: [] }, FRAME).alongCoreward).toHaveLength(0);
  });
});

describe("ringPolyline", () => {
  it("puts every point on the ring, in the plane, and closes it", () => {
    const points = ringPolyline(0.05, FRAME);

    expect(points).toHaveLength(97);
    expect(points.at(-1)).toEqual(points[0]);
    for (const point of points) {
      expect(norm(point)).toBeCloseTo(0.05, 12);
      expect(dot(point, FRAME.north)).toBeCloseTo(0, 12);
    }
  });

  it("starts at the ring's coreward point", () => {
    const [first] = ringPolyline(50, FRAME, 12);

    expect(first).toBeDefined();
    expect(dot(first ?? vec3(0, 0, 0), FRAME.coreward)).toBeCloseTo(50, 9);
  });
});
