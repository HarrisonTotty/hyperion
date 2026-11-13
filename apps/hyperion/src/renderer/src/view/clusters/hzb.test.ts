import { describe, expect, it } from "vitest";

import { add, scale, type Vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { between, seededRandom } from "../../test/seededRandom";
import { NEAR_PLANE_M } from "../camera/projection";
import { quaternionFromAxisAngle } from "../camera/quaternion";
import type { TextureHandle } from "../engine/types";
import {
  DEPTH_PYRAMID_KERNEL,
  DEPTH_PYRAMID_LABEL,
  DepthPyramid,
  depthPyramidTwin,
  HZB_PARAMS_BYTES,
  type HzbAxes,
  hzbAxesOf,
  hzbFootprint,
  hzbRefuses,
  type HzbTestInput,
  packHzbParams,
  packPyramidParams,
  pyramidLevels,
  pyramidProjectionOf,
  pyramidTexels,
} from "./hzb";

describe("the depth pyramid's levels (plan R14, R14.T3.d)", () => {
  it("halves 1080p from half its resolution to one texel, each odd side rounded up, in 2.8 MB", () => {
    const levels = pyramidLevels({ widthPx: 1920, heightPx: 1080 });
    expect([
      levels.map((level) => level.widthPx),
      levels.map((level) => level.heightPx),
      Math.round((4 * pyramidTexels(levels)) / 1e5) / 10,
    ]).toEqual([
      [960, 480, 240, 120, 60, 30, 15, 8, 4, 2, 1],
      [540, 270, 135, 68, 34, 17, 9, 5, 3, 2, 1],
      2.8,
    ]);
  });

  it("lays the levels one after another in the buffer", () => {
    const levels = pyramidLevels({ widthPx: 7, heightPx: 5 });
    expect(levels).toEqual([
      { widthPx: 4, heightPx: 3, offset: 0 },
      { widthPx: 2, heightPx: 2, offset: 12 },
      { widthPx: 1, heightPx: 1, offset: 16 },
    ]);
    expect(pyramidTexels(levels)).toBe(17);
  });

  it("has no pyramid for a depth of no pixels", () => {
    expect(() => pyramidLevels({ widthPx: 0, heightPx: 4 })).toThrow(RangeError);
  });
});

describe("the depth pyramid's twin", () => {
  it.each([
    [16, 16],
    [97, 61],
    [33, 1],
    [1, 9],
  ])(
    "holds at each texel of a %i × %i depth the farthest of every pixel it covers, the odd row or column included",
    (width, height) => {
      const random = seededRandom(width * 131 + height);
      const depth = Float32Array.from({ length: width * height }, () => random());
      const size = { widthPx: width, heightPx: height };
      const texels = depthPyramidTwin(depth, size);
      const levels = pyramidLevels(size);
      let worst = 0;
      levels.forEach((level, l) => {
        const span = 2 ** (l + 1);
        for (let y = 0; y < level.heightPx; y += 1) {
          for (let x = 0; x < level.widthPx; x += 1) {
            let farthest = Infinity;
            for (let py = y * span; py < Math.min((y + 1) * span, height); py += 1) {
              for (let px = x * span; px < Math.min((x + 1) * span, width); px += 1) {
                farthest = Math.min(farthest, depth[py * width + px] ?? Infinity);
              }
            }
            const held = texels[level.offset + y * level.widthPx + x] ?? Number.NaN;
            worst = Math.max(worst, Math.abs(held - farthest));
          }
        }
      });
      expect(worst).toBe(0);
    },
  );
});

/** The view the scenes are drawn in: 97 × 61 px across 60°, its axes turned off the frame's. */
const SIZE = { widthPx: 97, heightPx: 61 };
const PROJECTION = pyramidProjectionOf(Math.PI / 3, SIZE);
const AXES: HzbAxes = hzbAxesOf(quaternionFromAxisAngle({ x: 0.3, y: 1, z: -0.2 }, 0.7));

/** A sphere in the view's own axes: right, up and forward, m. */
interface ViewSphere {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly r: number;
}

/** `sphere`'s centre in the frame the axes are in, camera-relative, m. */
function centreOf(sphere: ViewSphere): Vec3 {
  return add(
    add(scale(AXES.right, sphere.x), scale(AXES.up, sphere.y)),
    scale(AXES.forward, sphere.z),
  );
}

/**
 * The distance along the view axis at which pixel (px, py)'s centre's ray first meets `sphere`, or
 * `null` where it misses: the place a fragment of anything within the sphere is nearest at.
 */
function rayHitZ(sphere: ViewSphere, px: number, py: number): number | null {
  const u = (((px + 0.5) / SIZE.widthPx) * 2 - 1) / PROJECTION.scaleX;
  const v = (1 - ((py + 0.5) / SIZE.heightPx) * 2) / PROJECTION.scaleY;
  // The ray t (u, v, 1): |t d − c|² = r².
  const a = u * u + v * v + 1;
  const b = -2 * (u * sphere.x + v * sphere.y + sphere.z);
  const c = sphere.x ** 2 + sphere.y ** 2 + sphere.z ** 2 - sphere.r ** 2;
  const disc = b * b - 4 * a * c;
  if (disc < 0) {
    return null;
  }
  const t = (-b - Math.sqrt(disc)) / (2 * a);
  return t > 0 ? t : null;
}

/** The reversed-Z depth of `occluders` drawn over the view, 0 where none is. */
function drawn(occluders: ReadonlyArray<ViewSphere>): Float32Array {
  const depth = new Float32Array(SIZE.widthPx * SIZE.heightPx);
  for (let py = 0; py < SIZE.heightPx; py += 1) {
    for (let px = 0; px < SIZE.widthPx; px += 1) {
      for (const sphere of occluders) {
        const z = rayHitZ(sphere, px, py);
        if (z !== null && z > NEAR_PLANE_M) {
          const at = py * SIZE.widthPx + px;
          depth[at] = Math.max(depth[at] ?? 0, NEAR_PLANE_M / z);
        }
      }
    }
  }
  return depth;
}

/** Whether every fragment `sphere` could draw fails the depth test against `depth`. */
function hidden(sphere: ViewSphere, depth: Float32Array): boolean {
  for (let py = 0; py < SIZE.heightPx; py += 1) {
    for (let px = 0; px < SIZE.widthPx; px += 1) {
      const z = rayHitZ(sphere, px, py);
      if (z !== null && !((depth[py * SIZE.widthPx + px] ?? 0) > NEAR_PLANE_M / z)) {
        return false;
      }
    }
  }
  return true;
}

/** The test's input over `depth`. */
function inputOf(depth: Float32Array): HzbTestInput {
  return {
    texels: depthPyramidTwin(depth, SIZE),
    levels: pyramidLevels(SIZE),
    size: SIZE,
    projection: PROJECTION,
    axes: AXES,
  };
}

/** A wall: a sphere of 40 m 60 m out, across the middle of the view. */
const WALL: ViewSphere = { x: 0, y: 0, z: 60, r: 40 };

describe("the occlusion's test against the pyramid", () => {
  it("refuses a sphere wholly behind a nearer one", () => {
    const input = inputOf(drawn([WALL]));
    expect(hzbRefuses(input, centreOf({ x: 1, y: -1, z: 150, r: 2 }), 2)).toBe(true);
  });

  it("keeps one that shows past the nearer one's edge", () => {
    const input = inputOf(drawn([{ x: 0, y: 0, z: 60, r: 10 }]));
    expect(hzbRefuses(input, centreOf({ x: 11, y: 0, z: 120, r: 6 }), 6)).toBe(false);
  });

  it("keeps a sphere across the near plane, and one about the camera, untested", () => {
    const input = inputOf(drawn([WALL]));
    expect([
      hzbFootprint(input, centreOf({ x: 0, y: 0, z: 0.3, r: 0.5 }), 0.5),
      hzbFootprint(input, centreOf({ x: 0.2, y: 0, z: 0, r: 2 }), 2),
      hzbRefuses(input, centreOf({ x: 0, y: 0, z: 0.3, r: 0.5 }), 0.5),
    ]).toEqual([null, null, false]);
  });

  it("keeps a sphere wholly off the depth", () => {
    const input = inputOf(drawn([WALL]));
    expect(hzbFootprint(input, centreOf({ x: 0, y: 0, z: -50, r: 3 }), 3)).toBeNull();
  });

  it("refuses, over random scenes, only spheres every fragment of which the drawn depth hides, and refuses many", () => {
    const random = seededRandom(0x4b2);
    let refused = 0;
    let kept = 0;
    let seen = 0;
    for (let scene = 0; scene < 6; scene += 1) {
      const occluders = Array.from({ length: 12 }, () => ({
        x: between(random, -30, 30),
        y: between(random, -20, 20),
        z: between(random, 20, 60),
        r: between(random, 4, 16),
      }));
      const depth = drawn(occluders);
      const input = inputOf(depth);
      for (let n = 0; n < 150; n += 1) {
        const sphere = {
          x: between(random, -60, 60),
          y: between(random, -40, 40),
          z: between(random, 30, 160),
          r: between(random, 0.5, 6),
        };
        if (hzbRefuses(input, centreOf(sphere), sphere.r, 0)) {
          refused += 1;
          seen += hidden(sphere, depth) ? 0 : 1;
        } else {
          kept += 1;
        }
      }
    }
    expect([refused > 100, kept > 100, seen]).toEqual([true, true, 0]);
  });
});

describe("the occlusion's packed parameters", () => {
  it("lays the axes, the scales, the near plane, the size and the levels' places as HzbParams", () => {
    const out = new ArrayBuffer(HZB_PARAMS_BYTES);
    const levels = pyramidLevels(SIZE);
    packHzbParams({ levels, size: SIZE, projection: PROJECTION, axes: AXES }, out);
    const floats = new Float32Array(out);
    const words = new Uint32Array(out);
    expect([
      floats[0],
      floats[3],
      floats[7],
      floats[11],
      [...words.subarray(12, 15)],
      [...words.subarray(16, 16 + levels.length)],
    ]).toEqual([
      Math.fround(AXES.right.x),
      Math.fround(PROJECTION.scaleX),
      Math.fround(PROJECTION.scaleY),
      Math.fround(NEAR_PLANE_M),
      [97, 61, levels.length],
      levels.map((level) => level.offset),
    ]);
    packHzbParams(null, out);
    expect(words.every((word) => word === 0)).toBe(true);
  });

  it("gives each level's dispatch its source and itself", () => {
    const levels = pyramidLevels(SIZE);
    expect([
      [...packPyramidParams(levels, SIZE, 0)],
      [...packPyramidParams(levels, SIZE, 2)],
    ]).toEqual([
      [97, 61, 0, 0, 49, 31, 0, 0],
      [25, 16, 2, levels[1]?.offset, 13, 8, levels[2]?.offset, 0],
    ]);
  });
});

describe("a view's depth pyramid", () => {
  const DEPTH: TextureHandle = { kind: "texture", name: "scene depth" };

  it("builds every level from the depth, a dispatch a level, and holds its build for the late pass", async () => {
    const engine = await countingRenderEngine();
    const kernel = await engine.createComputeAsync(DEPTH_PYRAMID_KERNEL);
    const pyramid = new DepthPyramid(engine, kernel, "view");
    pyramid.begin(SIZE, false);
    pyramid.build(DEPTH, PROJECTION);
    const levels = pyramidLevels(SIZE);
    expect([
      engine.dispatched.map((d) => [d.pass, d.workgroups, d.bindings.sampled["depth"]?.name]),
      pyramid.frame?.buffer.bytes,
      pyramid.frame?.levels,
      pyramid.builds,
    ]).toEqual([
      levels.map((level) => [
        DEPTH_PYRAMID_LABEL,
        [Math.ceil(level.widthPx / 8), Math.ceil(level.heightPx / 8), 1],
        "scene depth",
      ]),
      4 * pyramidTexels(levels),
      levels,
      1,
    ]);
    pyramid.dispose();
  });

  it("drops the last frame's visibility at its first frame, a new size and a cut, and keeps it otherwise", async () => {
    const engine = await countingRenderEngine();
    const kernel = await engine.createComputeAsync(DEPTH_PYRAMID_KERNEL);
    const pyramid = new DepthPyramid(engine, kernel, "view");
    const frame = (size: typeof SIZE, cut: boolean): boolean => {
      const fresh = pyramid.begin(size, cut);
      pyramid.build(DEPTH, PROJECTION);
      return fresh;
    };
    const larger = { widthPx: 120, heightPx: 61 };
    expect([
      frame(SIZE, false),
      frame(SIZE, false),
      frame(larger, false),
      frame(larger, false),
      frame(larger, true),
      frame(larger, false),
    ]).toEqual([true, false, true, false, true, false]);
    const live = engine.live.buffers;
    pyramid.dispose();
    expect(live - engine.live.buffers).toBe(1);
  });

  it("refuses a build before it is begun", async () => {
    const engine = await countingRenderEngine();
    const kernel = await engine.createComputeAsync(DEPTH_PYRAMID_KERNEL);
    const pyramid = new DepthPyramid(engine, kernel, "view");
    expect(() => {
      pyramid.build(DEPTH, PROJECTION);
    }).toThrow(Error);
  });
});
