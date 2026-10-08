import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { type CanvasPointPx, type CanvasSizePx, dragTurn } from "./drag";
import type { ViewTurn } from "./look";
import { conjugate, multiply, quaternionFromAxisAngle, rotate } from "./quaternion";

const SIZE: CanvasSizePx = { widthPx: 1280, heightPx: 720 };

/** The focal length of a canvas of `size` at `fovXDeg`, CSS px. */
function focalPx(size: CanvasSizePx, fovXDeg: number): number {
  return size.widthPx / 2 / Math.tan((fovXDeg * Math.PI) / 360);
}

/** The direction, camera axes, that a pixel shows. */
function directionAt(point: CanvasPointPx, size: CanvasSizePx, fovXDeg: number): Vec3 {
  const f = focalPx(size, fovXDeg);
  return normalise(vec3(point.xPx - size.widthPx / 2, size.heightPx / 2 - point.yPx, -f));
}

/** Where a camera-axes direction lands on the canvas after the camera turns by `turn`. */
function landsAt(
  direction: Vec3,
  turn: ViewTurn,
  size: CanvasSizePx,
  fovXDeg: number,
): CanvasPointPx {
  // The free camera's turn: a yaw about its +y, then a pitch about its +x.
  const rotation = multiply(
    quaternionFromAxisAngle(vec3(0, 1, 0), turn.yawRad),
    quaternionFromAxisAngle(vec3(1, 0, 0), turn.pitchRad),
  );
  const seen = rotate(conjugate(rotation), direction);
  const f = focalPx(size, fovXDeg);
  return {
    xPx: size.widthPx / 2 + (f * seen.x) / -seen.z,
    yPx: size.heightPx / 2 - (f * seen.y) / -seen.z,
  };
}

describe("a drag's turn", () => {
  it("turns the line of sight left for a drag to the right, and up for a drag down", () => {
    const right = dragTurn({ xPx: 640, yPx: 360 }, { xPx: 700, yPx: 360 }, SIZE, 60);
    const down = dragTurn({ xPx: 640, yPx: 360 }, { xPx: 640, yPx: 400 }, SIZE, 60);
    expect([right.yawRad > 0, right.pitchRad, down.yawRad, down.pitchRad > 0]).toEqual([
      true,
      0,
      0,
      true,
    ]);
  });

  it("sizes the turn from the field of view: atan of the travel over the focal length", () => {
    for (const fovXDeg of [60, 15, 120]) {
      const turn = dragTurn({ xPx: 640, yPx: 360 }, { xPx: 740, yPx: 300 }, SIZE, fovXDeg);
      const f = focalPx(SIZE, fovXDeg);
      expect(turn.yawRad).toBeCloseTo(Math.atan(100 / f), 12);
      expect(turn.pitchRad).toBeCloseTo(Math.atan(-60 / f), 12);
    }
  });

  it("keeps a point dragged along the centre lines under the pointer, at any field of view", () => {
    for (const fovXDeg of [60, 15]) {
      const from = { xPx: 200, yPx: 360 };
      const to = { xPx: 1100, yPx: 360 };
      const landed = landsAt(
        directionAt(from, SIZE, fovXDeg),
        dragTurn(from, to, SIZE, fovXDeg),
        SIZE,
        fovXDeg,
      );
      expect(landed.xPx).toBeCloseTo(to.xPx, 6);
      const up = { xPx: 640, yPx: 600 };
      const top = { xPx: 640, yPx: 100 };
      const raised = landsAt(
        directionAt(up, SIZE, fovXDeg),
        dragTurn(up, top, SIZE, fovXDeg),
        SIZE,
        fovXDeg,
      );
      expect(raised.yPx).toBeCloseTo(top.yPx, 6);
    }
  });

  it("keeps a point off the centre lines under the pointer to first order", () => {
    const from = { xPx: 900, yPx: 200 };
    const to = { xPx: 908, yPx: 206 };
    const landed = landsAt(directionAt(from, SIZE, 60), dragTurn(from, to, SIZE, 60), SIZE, 60);
    // Off by second order in the 10 px travel: well under a pixel.
    expect(Math.hypot(landed.xPx - to.xPx, landed.yPx - to.yPx)).toBeLessThan(0.5);
  });

  it("adds up from move to move to the turn of the whole drag", () => {
    const points = [
      { xPx: 300, yPx: 500 },
      { xPx: 420, yPx: 470 },
      { xPx: 600, yPx: 300 },
      { xPx: 1000, yPx: 120 },
    ];
    let yawRad = 0;
    let pitchRad = 0;
    for (const [index, point] of points.slice(1).entries()) {
      const turn = dragTurn(points[index] ?? point, point, SIZE, 60);
      yawRad += turn.yawRad;
      pitchRad += turn.pitchRad;
    }
    const whole = dragTurn(points[0] ?? { xPx: 0, yPx: 0 }, { xPx: 1000, yPx: 120 }, SIZE, 60);
    expect(yawRad).toBeCloseTo(whole.yawRad, 12);
    expect(pitchRad).toBeCloseTo(whole.pitchRad, 12);
  });

  it("turns nothing on a canvas not yet laid out", () => {
    expect(
      dragTurn({ xPx: 0, yPx: 0 }, { xPx: 50, yPx: 50 }, { widthPx: 0, heightPx: 0 }, 60),
    ).toEqual({
      yawRad: 0,
      pitchRad: 0,
    });
  });
});
