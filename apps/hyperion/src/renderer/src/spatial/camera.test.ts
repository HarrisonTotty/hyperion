import { describe, expect, it } from "vitest";

import { between, seededRandom } from "../test/seededRandom";
import {
  type Camera,
  clampElevationDeg,
  fitPxPerUnit,
  matchesPreset,
  PRESETS,
  project,
  rotateCamera,
  viewBasis,
  type Viewport,
  wrapAzimuthDeg,
  zoomCamera,
  zoomLimits,
} from "./camera";
import { localFrameAt } from "./frame";
import { cross, dot, norm, scale, sub, vec3, type Vec3 } from "./vec3";

// A frame off the x axis, so that nothing passes by lining up with x and y.
const FRAME = localFrameAt(vec3(15_600, 20_800, 0));
const VIEWPORT: Viewport = { widthPx: 800, heightPx: 600, remPx: 16 };
const CENTRE = { xPx: 400, yPx: 300 };
const K = 4;

function cameraAt(azimuthDeg: number, elevationDeg: number): Camera {
  return { azimuthDeg, elevationDeg, pxPerUnit: K };
}

function screenOffset(point: Vec3, camera: Camera): { dx: number; dy: number } {
  const projected = project(point, viewBasis(FRAME, camera), camera, VIEWPORT);
  return { dx: projected.xPx - CENTRE.xPx, dy: projected.yPx - CENTRE.yPx };
}

const ANGLE_GRID: Array<[number, number]> = [];
for (const azimuthDeg of [0, 30, 90, 135, 180, 270, 359]) {
  for (const elevationDeg of [-90, -45, -1, 0, 1, 30, 89.999, 90]) {
    ANGLE_GRID.push([azimuthDeg, elevationDeg]);
  }
}

describe("viewBasis", () => {
  it.each(ANGLE_GRID)("is orthonormal and right-handed at %f°, %f°", (azimuthDeg, elevationDeg) => {
    const { right, up, forward } = viewBasis(FRAME, cameraAt(azimuthDeg, elevationDeg));

    for (const axis of [right, up, forward]) {
      expect(norm(axis)).toBeCloseTo(1, 12);
    }
    expect(dot(right, up)).toBeCloseTo(0, 12);
    expect(dot(right, forward)).toBeCloseTo(0, 12);
    expect(dot(up, forward)).toBeCloseTo(0, 12);
    const rightCrossForward = cross(right, forward);
    expect(rightCrossForward.x).toBeCloseTo(up.x, 12);
    expect(rightCrossForward.y).toBeCloseTo(up.y, 12);
    expect(rightCrossForward.z).toBeCloseTo(up.z, 12);
  });

  it("looks exactly south onto the plane at TOP", () => {
    const { forward } = viewBasis(FRAME, PRESETS.top);

    expect(dot(forward, FRAME.north)).toBe(-1);
    expect(Math.abs(forward.x)).toBe(0);
    expect(Math.abs(forward.y)).toBe(0);
  });

  it("looks exactly along the plane at SIDE and FRONT", () => {
    expect(Math.abs(viewBasis(FRAME, PRESETS.side).forward.z)).toBe(0);
    expect(Math.abs(viewBasis(FRAME, PRESETS.front).forward.z)).toBe(0);
    expect(dot(viewBasis(FRAME, PRESETS.front).forward, FRAME.coreward)).toBe(0);
  });
});

describe("project", () => {
  it("puts coreward straight up and spinward to the right at TOP", () => {
    const camera = { ...PRESETS.top, pxPerUnit: K };

    const coreward = screenOffset(FRAME.coreward, camera);
    const spinward = screenOffset(FRAME.spinward, camera);

    expect(coreward.dx).toBeCloseTo(0, 12);
    expect(coreward.dy).toBeCloseTo(-K, 12);
    expect(spinward.dx).toBeCloseTo(K, 12);
    expect(spinward.dy).toBeCloseTo(0, 12);
  });

  it("puts north up and spinward to the right at SIDE", () => {
    const camera = { ...PRESETS.side, pxPerUnit: K };

    const north = screenOffset(FRAME.north, camera);
    const spinward = screenOffset(FRAME.spinward, camera);

    expect(north.dx).toBeCloseTo(0, 12);
    expect(north.dy).toBeCloseTo(-K, 12);
    expect(spinward.dx).toBeCloseTo(K, 12);
  });

  it("puts north up and rimward to the right at FRONT", () => {
    const camera = { ...PRESETS.front, pxPerUnit: K };

    const north = screenOffset(FRAME.north, camera);
    const rimward = screenOffset(scale(FRAME.coreward, -1), camera);

    expect(north.dy).toBeCloseTo(-K, 12);
    expect(rimward.dx).toBeCloseTo(K, 12);
    expect(rimward.dy).toBeCloseTo(0, 12);
  });

  it("mirrors the TOP view about the horizontal axis from the south", () => {
    const top = { ...PRESETS.top, pxPerUnit: K };
    const south = { azimuthDeg: 0, elevationDeg: -90, pxPerUnit: K };
    const random = seededRandom(90);

    for (let i = 0; i < 50; i += 1) {
      const point = vec3(between(random, -9, 9), between(random, -9, 9), between(random, -9, 9));
      const fromTop = screenOffset(point, top);
      const fromSouth = screenOffset(point, south);

      expect(fromSouth.dx).toBeCloseTo(fromTop.dx, 9);
      expect(fromSouth.dy).toBeCloseTo(-fromTop.dy, 9);
    }
  });

  it("keeps a sphere inside a circle of k·R, touching it on the limb, from 1,000 orientations", () => {
    const random = seededRandom(1_000);
    const radius = 50;

    for (let i = 0; i < 1_000; i += 1) {
      const camera = cameraAt(between(random, 0, 360), between(random, -90, 90));
      const basis = viewBasis(FRAME, camera);
      const direction = vec3(
        between(random, -1, 1),
        between(random, -1, 1),
        between(random, -1, 1),
      );
      const onSphere = scale(direction, radius / norm(direction));
      const toward = scale(basis.forward, dot(onSphere, basis.forward));
      const onLimb = scale(sub(onSphere, toward), radius / norm(sub(onSphere, toward)));

      const inside = screenOffset(onSphere, camera);
      const limb = screenOffset(onLimb, camera);

      expect(Math.hypot(inside.dx, inside.dy)).toBeLessThanOrEqual(K * radius * (1 + 1e-12));
      expect(Math.hypot(limb.dx, limb.dy)).toBeCloseTo(K * radius, 9);
    }
  });

  it("never rolls: north stays vertical at every angle short of ±90°", () => {
    for (let azimuthDeg = 0; azimuthDeg < 360; azimuthDeg += 15) {
      for (let elevationDeg = -89; elevationDeg <= 89; elevationDeg += 8) {
        const north = screenOffset(FRAME.north, cameraAt(azimuthDeg, elevationDeg));

        expect(north.dx).toBeCloseTo(0, 12);
        expect(north.dy).toBeLessThan(0);
      }
    }
  });

  describe("on the galactic axes, worked by hand", () => {
    // At (26,000, 0, 0) ly coreward is -x, spinward +y and north +z. The expected pixels were
    // worked out on paper from the guide's definitions, not from viewBasis.
    const ON_X = localFrameAt(vec3(26_000, 0, 0));

    function at(point: Vec3, azimuthDeg: number, elevationDeg: number) {
      const camera = cameraAt(azimuthDeg, elevationDeg);
      return project(point, viewBasis(ON_X, camera), camera, VIEWPORT);
    }

    it("shows +x down, +y right and +z towards the viewer at TOP, a view from the north", () => {
      const plusX = at(vec3(10, 0, 0), 0, 90);
      const plusY = at(vec3(0, 10, 0), 0, 90);
      const plusZ = at(vec3(0, 0, 10), 0, 90);

      expect([plusX.xPx, plusX.yPx, plusX.depth]).toEqual([400, 340, 0]);
      expect([plusY.xPx, plusY.yPx, plusY.depth]).toEqual([440, 300, 0]);
      expect([plusZ.xPx, plusZ.yPx, plusZ.depth]).toEqual([400, 300, -10]);
    });

    it("turns the galaxy counter-clockwise on screen at TOP", () => {
      // A point a little further round the galaxy, counter-clockwise seen from the north, lies
      // spinward: on screen it is to the right of the centre, and the core is straight up.
      const angleRad = 1e-4;
      const ahead = vec3(26_000 * Math.cos(angleRad) - 26_000, 26_000 * Math.sin(angleRad), 0);
      const core = vec3(-26_000, 0, 0);

      expect(at(ahead, 0, 90).xPx).toBeGreaterThan(400);
      expect(at(core, 0, 90).xPx).toBeCloseTo(400, 9);
      expect(at(core, 0, 90).yPx).toBeLessThan(300);
    });

    it.each([
      // (10, 0, 0): right 0.5, up −0.4330, forward −0.75.
      [vec3(10, 0, 0), 420, 317.320_508_075_688_8, -7.5],
      // (0, 10, 0): right 0.8660, up 0.25, forward 0.4330.
      [vec3(0, 10, 0), 434.641_016_151_377_5, 290, 4.330_127_018_922_193],
      // (0, 0, 10): right 0, up 0.8660, forward −0.5.
      [vec3(0, 0, 10), 400, 265.358_983_848_622_5, -5],
    ])("projects %o at OBLIQUE to (%f, %f) at depth %f", (point, xPx, yPx, depth) => {
      const projected = at(point, 30, 30);

      expect(projected.xPx).toBeCloseTo(xPx, 9);
      expect(projected.yPx).toBeCloseTo(yPx, 9);
      expect(projected.depth).toBeCloseTo(depth, 9);
    });

    it("looks spinward, towards -x, at FRONT on the +y axis, with rimward (+y) to the right", () => {
      const onY = localFrameAt(vec3(0, 26_000, 0));
      const camera = cameraAt(90, 0);
      const basis = viewBasis(onY, camera);

      const rimward = project(vec3(0, 10, 0), basis, camera, VIEWPORT);
      const spinward = project(vec3(-10, 0, 0), basis, camera, VIEWPORT);

      expect([rimward.xPx, rimward.yPx, rimward.depth]).toEqual([440, 300, 0]);
      expect([spinward.xPx, spinward.yPx, spinward.depth]).toEqual([400, 300, 10]);
    });
  });

  it("gives larger depths to points further from the viewer", () => {
    const camera = cameraAt(30, 30);
    const basis = viewBasis(FRAME, camera);

    const near = project(scale(basis.forward, -5), basis, camera, VIEWPORT);
    const far = project(scale(basis.forward, 5), basis, camera, VIEWPORT);

    expect(far.depth).toBeGreaterThan(near.depth);
    expect(far.xPx).toBeCloseTo(near.xPx, 12);
    expect(far.yPx).toBeCloseTo(near.yPx, 12);
  });
});

describe("camera angles", () => {
  it.each([
    [361, 1],
    [720, 0],
    [-1, 359],
    [-360, 0],
    [-1e-15, 0],
    [359.5, 359.5],
  ])("wraps an azimuth of %f° to %f°", (azimuthDeg, expected) => {
    expect(wrapAzimuthDeg(azimuthDeg)).toBeCloseTo(expected, 12);
  });

  it.each([
    [95, 90],
    [-120, -90],
    [45, 45],
  ])("clamps an elevation of %f° to %f°", (elevationDeg, expected) => {
    expect(clampElevationDeg(elevationDeg)).toBe(expected);
  });

  it("wraps the azimuth from 359 + 2 to 1", () => {
    expect(rotateCamera(cameraAt(359, 0), 2, 0).azimuthDeg).toBe(1);
  });

  it("stops the elevation at +90° however far it is turned", () => {
    let camera = cameraAt(30, 30);
    for (let i = 0; i < 13; i += 1) {
      camera = rotateCamera(camera, 0, 5);
    }

    expect(camera.elevationDeg).toBe(90);
    expect(camera.pxPerUnit).toBe(K);
  });
});

describe("zoom", () => {
  it("fits a sphere inside the shorter side of the viewport", () => {
    expect(fitPxPerUnit(50, VIEWPORT, 20)).toBe((300 - 20) / 50);
    expect(fitPxPerUnit(0.05, { widthPx: 400, heightPx: 900, remPx: 16 }, 0)).toBeCloseTo(4_000, 9);
  });

  it("refuses a radius that is not positive", () => {
    expect(() => fitPxPerUnit(0, VIEWPORT, 0)).toThrow(RangeError);
  });

  it("zooms by a factor within the limits", () => {
    const limits = zoomLimits(K);

    expect(zoomCamera(cameraAt(0, 0), 1.25, limits).pxPerUnit).toBe(5);
    expect(zoomCamera(cameraAt(0, 0), 1_000, limits).pxPerUnit).toBe(K * 100);
    expect(zoomCamera(cameraAt(0, 0), 0.001, limits).pxPerUnit).toBe(K * 0.5);
    expect(zoomCamera(cameraAt(10, 20), 2, limits)).toMatchObject({
      azimuthDeg: 10,
      elevationDeg: 20,
    });
  });
});

describe("matchesPreset", () => {
  it("matches a preset whatever the zoom", () => {
    const zoomed: Camera = { ...PRESETS.top, pxPerUnit: 99 };

    expect(matchesPreset(zoomed, PRESETS.top)).toBe(true);
    expect(matchesPreset(PRESETS.oblique, PRESETS.oblique)).toBe(true);
  });

  it("matches across the azimuth's wrap and not a degree away", () => {
    expect(matchesPreset({ azimuthDeg: 360 - 1e-9, elevationDeg: 0 }, PRESETS.side)).toBe(true);
    expect(matchesPreset({ azimuthDeg: 1, elevationDeg: 0 }, PRESETS.side)).toBe(false);
    expect(matchesPreset({ azimuthDeg: 0, elevationDeg: 89 }, PRESETS.top)).toBe(false);
  });
});
