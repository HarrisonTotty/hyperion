import { describe, expect, it } from "vitest";

import { add, cross, dot, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { DEFAULT_FOV_DEG, project, viewRotation, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, rotate } from "../camera/quaternion";
import { CHASE_OFFSET_HULL_LENGTHS } from "../camera/state";
import { narrow } from "../coords/narrow";
import type { ViewPosition } from "../coords/position";
import { type CameraOrigins, originMinusCamera, relativeToCamera } from "../coords/relative";
import { separable } from "../depth/depth";
import { TEST_HULL, TEST_PLATE_DISTANCE_M } from "../scene/hull";
import { sceneOrigins } from "../scene/model";
import { HULL_OCCLUDER_DEPTH_FRACTION } from "../wireframe/drawList";
import { hullFaces } from "../wireframe/hulls";
import {
  PRECISION_DURATION_S,
  PRECISION_MOON,
  PRECISION_PLANET,
  PRECISION_SHIP,
  precisionScene,
} from "./precision";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;
const f = Math.fround;

/** A pixel position, px from the top left. */
interface PixelPx {
  readonly x: number;
  readonly y: number;
}

/** A mark's pixel as the exact `f64` path puts it and as the `f32` GPU path draws it. */
interface MarkPixels {
  readonly exact: PixelPx;
  readonly drawn: PixelPx;
}

/**
 * A point's pixel through the GPU's path, emulated in `f32`: the narrowed vector from the camera,
 * the rotation-only view matrix, and the projection, each operation rounded to `f32`.
 */
function gpuPixel(v32: Float32Array, camera: CameraPose): PixelPx {
  const m = viewRotation(camera.orientation);
  const at = (i: number): number => m[i] ?? Number.NaN;
  const x = v32[0] ?? Number.NaN;
  const y = v32[1] ?? Number.NaN;
  const z = v32[2] ?? Number.NaN;
  // Column-major: view = M · v.
  const vx = f(f(f(at(0) * x) + f(at(3) * y)) + f(at(6) * z));
  const vy = f(f(f(at(1) * x) + f(at(4) * y)) + f(at(7) * z));
  const vz = f(f(f(at(2) * x) + f(at(5) * y)) + f(at(8) * z));
  const s = f(1 / Math.tan(FOV_X_RAD / 2));
  const aspect = f(VIEWPORT.widthPx / VIEWPORT.heightPx);
  const w = f(-vz);
  const ndcX = f(f(s * vx) / w);
  const ndcY = f(f(f(s * aspect) * vy) / w);
  return {
    x: ((ndcX + 1) / 2) * VIEWPORT.widthPx,
    y: ((1 - ndcY) / 2) * VIEWPORT.heightPx,
  };
}

/** A mark of the scene: its exact `f64` vector from the camera, and its `f32` one as drawn. */
interface Mark {
  readonly name: string;
  exact(camera: CameraPose, origins: CameraOrigins): Vec3;
  drawn(camera: CameraPose, origins: CameraOrigins): Float32Array;
}

const SHIP_AT = (origins: CameraOrigins): ViewPosition => origins.craftPosition(PRECISION_SHIP);

/** A body's centre, drawn as a narrowed vector from the camera. */
function bodyMark(name: string, body: ViewPosition): Mark {
  return {
    name,
    exact: (camera, origins) => relativeToCamera(body, camera, origins),
    drawn: (camera, origins) => narrow(relativeToCamera(body, camera, origins)),
  };
}

/** A hull vertex, drawn as the hull's origin from the camera plus its `f32` offset (Design note 2). */
function hullMark(index: number): Mark {
  const vertex = TEST_HULL.vertices[index] ?? vec3(Number.NaN, 0, 0);
  return {
    name: `hull vertex ${String(index)}`,
    exact: (camera, origins) => add(relativeToCamera(SHIP_AT(origins), camera, origins), vertex),
    drawn: (camera, origins) => {
      const origin = originMinusCamera(
        { origin: SHIP_AT(origins), offsetsF32: narrow(vertex) },
        camera,
        origins,
      );
      const offset = narrow(vertex);
      return new Float32Array([0, 1, 2].map((i) => f((origin[i] ?? 0) + (offset[i] ?? 0))));
    },
  };
}

const MARKS: readonly Mark[] = [
  bodyMark("moon", { kind: "body", body: PRECISION_MOON, m: vec3(0, 0, 0) }),
  bodyMark("planet", { kind: "body", body: PRECISION_PLANET, m: vec3(0, 0, 0) }),
  hullMark(9),
  hullMark(10),
  hullMark(11),
  hullMark(12),
];

describe("the precision scene", () => {
  it("moves every mark smoothly: no step beyond the path's own motion larger than 0.1 px", () => {
    const kept = precisionScene();
    const worst = new Map<string, number>();
    let previous: Map<string, MarkPixels> | null = null;
    for (let frame = 0; frame <= PRECISION_DURATION_S * 60; frame += 1) {
      const tS = frame / 60;
      const camera = kept.cameraAt(tS);
      const origins = sceneOrigins(kept.sceneAt(tS));
      const current = new Map<string, MarkPixels>();
      for (const mark of MARKS) {
        const exact = project(
          mark.exact(camera, origins),
          { orientation: camera.orientation, fovXRad: FOV_X_RAD },
          VIEWPORT,
        );
        current.set(mark.name, {
          exact: { x: exact.xPx, y: exact.yPx },
          drawn: gpuPixel(mark.drawn(camera, origins), camera),
        });
      }
      if (previous !== null) {
        for (const [name, now] of current) {
          const before = previous.get(name);
          if (before !== undefined) {
            const excess = Math.hypot(
              now.drawn.x - before.drawn.x - (now.exact.x - before.exact.x),
              now.drawn.y - before.drawn.y - (now.exact.y - before.exact.y),
            );
            worst.set(name, Math.max(worst.get(name) ?? 0, excess));
          }
        }
      }
      previous = current;
    }
    const over = [...worst].filter(([, excess]) => excess > 0.1);
    expect(worst.size).toBe(MARKS.length);
    expect(over).toEqual([]);
  });

  it("keeps the plate, the moon and the planet pairwise separable in depth", () => {
    const kept = precisionScene();
    const camera = kept.cameraAt(0);
    const origins = sceneOrigins(kept.sceneAt(0));
    const forward = rotate(camera.orientation, vec3(0, 0, -1));
    const along = (mark: Mark): number => dot(mark.exact(camera, origins), forward);
    const [moon, planet, plate] = [MARKS[0], MARKS[1], MARKS[2]].map((mark) =>
      mark === undefined ? Number.NaN : along(mark),
    );
    const pairs = [
      [plate, moon],
      [plate, planet],
      [moon, planet],
    ].map(([a = Number.NaN, b = Number.NaN]) => separable(a, b, Math.max(a, b)));
    expect(pairs).toEqual([true, true, true]);
  });
});

/** A triangle's corners, m in hull axes. */
type Triangle = readonly [Vec3, Vec3, Vec3];

/** The test hull's faces with these indices, as triangles in hull axes. */
function facesAt(indices: ReadonlyArray<number>): Triangle[] {
  return indices.map((index) => {
    const [i = -1, j = -1, k = -1] = TEST_HULL.faces[index] ?? [];
    const a = TEST_HULL.vertices[i];
    const b = TEST_HULL.vertices[j];
    const c = TEST_HULL.vertices[k];
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error(`the test hull has no face ${String(index)}`);
    }
    return [a, b, c] as const;
  });
}

/** The test hull's plate: its two triangles, the last faces of `TEST_HULL`, and its window. */
const PLATE_FACES = facesAt([14, 15]);

/**
 * The hull's occluder, as the draw list builds it in both styles: its opaque faces, its window
 * left out (R07.T16.e), turned by no attitude.
 */
const OCCLUDER_FACES: ReadonlyArray<Triangle> = hullFaces(TEST_HULL, IDENTITY_QUATERNION);

/**
 * Where the ray from `eye` towards `point` first meets one of `faces` pushed away by the occluder's
 * constant (its reversed-Z depth times 1 − `depthFraction`, so its distance over 1 −
 * `depthFraction`), as a fraction of the way to `point`, or `null`.
 */
function firstHit(
  eye: Vec3,
  point: Vec3,
  faces: ReadonlyArray<Triangle>,
  depthFraction: number,
): number | null {
  const direction = sub(point, eye);
  let nearest: number | null = null;
  for (const [a, b, c] of faces) {
    // Möller–Trumbore.
    const e1 = sub(b, a);
    const e2 = sub(c, a);
    const p = cross(direction, e2);
    const det = dot(e1, p);
    if (Math.abs(det) < 1e-12) {
      continue;
    }
    const s = sub(eye, a);
    const u = dot(s, p) / det;
    const q = cross(s, e1);
    const v = dot(direction, q) / det;
    const t = dot(e2, q) / det;
    if (u >= 0 && v >= 0 && u + v <= 1 && t > 0) {
      const pushed = t / (1 - depthFraction);
      nearest = nearest === null ? pushed : Math.min(nearest, pushed);
    }
  }
  return nearest;
}

/** Points along every edge in `edges`, `per` to an edge, ends included. */
function edgeSamples(edges: ReadonlyArray<readonly [number, number]>, per: number): Vec3[] {
  return edges.flatMap(([i, j]) => {
    const a = TEST_HULL.vertices[i];
    const b = TEST_HULL.vertices[j];
    if (a === undefined || b === undefined) {
      throw new Error(`the test hull has no edge ${String(i)}, ${String(j)}`);
    }
    return Array.from({ length: per }, (_, n) => add(a, scale(sub(b, a), n / (per - 1))));
  });
}

/**
 * Whether the plate stands between the eye and `point`, worked out analytically: the ray crosses
 * the plate's plane inside its 1 m square before it reaches the point.
 */
function behindPlate(eye: Vec3, point: Vec3): boolean {
  const plateZ = eye.z - TEST_PLATE_DISTANCE_M;
  const t = (plateZ - eye.z) / (point.z - eye.z);
  if (!(t > 0 && t < 1)) {
    return false;
  }
  const at = add(eye, scale(sub(point, eye), t));
  return Math.abs(at.x) <= 0.5 && at.y >= 2 && at.y <= 3;
}

describe("the test hull's hidden lines", () => {
  // The hull faces' constant push, 2⁻¹⁶ of the depth on every backend (R07.T16.d), where the
  // hardware bias moved a face by 7.6 × 10⁻⁶ to 1.5 × 10⁻⁵ of its distance (Design note 5).
  const bias = HULL_OCCLUDER_DEPTH_FRACTION;
  const seat = TEST_HULL.eyePointM;
  // The chase camera's place, astern and above (R02's preset).
  const chase = scale(CHASE_OFFSET_HULL_LENGTHS, TEST_HULL.lengthM);
  const hiddenBy = (eye: Vec3, faces: ReadonlyArray<Triangle>) => (point: Vec3) => {
    const hit = firstHit(eye, point, faces, bias);
    return hit !== null && hit < 1;
  };

  it("hides nothing behind the plate, a window, from the seat, which its faces would hide were they opaque (R07.T16.e)", () => {
    const samples = edgeSamples(TEST_HULL.edges.slice(0, 16), 101);
    const behind = samples.filter((point) => behindPlate(seat, point));
    // Seen through the window: behind the plate, and hidden by none of the hull's opaque faces.
    const seen = behind.filter((point) => !hiddenBy(seat, OCCLUDER_FACES)(point));
    expect({
      behind: behind.length > 0,
      seen: seen.length > 0,
      // The control: the plate's faces, were they opaque, would hide each of them.
      hiddenByThePlate: seen.filter(hiddenBy(seat, PLATE_FACES)).length === seen.length,
    }).toEqual({ behind: true, seen: true, hiddenByThePlate: true });
  });

  it("keeps the hull's own edges, along their length, in front of its opaque faces, from the seat and from astern", () => {
    // Each edge against the opaque faces it bounds, those holding both of its ends.
    const hidden = [seat, chase].flatMap((eye) =>
      TEST_HULL.edges.slice(0, 16).flatMap(([i, j]): string[] => {
        const own = facesAt(
          TEST_HULL.faces.flatMap((face, index) =>
            face.includes(i) && face.includes(j) && !TEST_HULL.windows.includes(index)
              ? [index]
              : [],
          ),
        );
        const name = `edge ${String(i)}-${String(j)}`;
        if (own.length === 0) {
          return [`${name} bounds no opaque face`];
        }
        return edgeSamples([[i, j]], 101)
          .filter(hiddenBy(eye, own))
          .map((p) => `${name} at (${String(p.x)}, ${String(p.y)}, ${String(p.z)})`);
      }),
    );
    expect(hidden).toEqual([]);
  });
});
