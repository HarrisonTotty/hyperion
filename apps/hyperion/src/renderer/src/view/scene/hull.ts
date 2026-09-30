import { type Vec3, vec3 } from "../../geometry/vec3";

/**
 * A craft's hull as the wireframe draws it: vertices in the hull's own frame, edges as index pairs,
 * and triangulated faces for the depth-only occluder that hides its hidden lines (plan R02, Design
 * note 15).
 *
 * @remarks
 * Hull axes are the camera's convention (`OwnShip`): +x to starboard, +y dorsal, −z forward, in
 * metres from the craft's reference point. The real definitions are the flight model's data; the
 * plan that builds craft is asked to emit this outline from them. Build one with
 * {@link hullOutline}, which checks its indices.
 */
export interface HullOutline {
  /** The hull's name as the view labels it. */
  readonly name: string;
  /** The vertices, m in hull axes. */
  readonly vertices: ReadonlyArray<Vec3>;
  /** The edges drawn as lines, as pairs of vertex indices. */
  readonly edges: ReadonlyArray<readonly [number, number]>;
  /** The faces, as triangles of vertex indices, drawn depth-only. */
  readonly faces: ReadonlyArray<readonly [number, number, number]>;
  /** The pilot's eye point, m in hull axes. */
  readonly eyePointM: Vec3;
  /** The hull's length along its z axis, m. */
  readonly lengthM: number;
}

/**
 * Builds a {@link HullOutline}, checking that every edge and face names vertices it has.
 *
 * @throws RangeError if an index is not an integer within the vertices, or a vertex is not finite.
 */
export function hullOutline(outline: HullOutline): HullOutline {
  const count = outline.vertices.length;
  for (const v of outline.vertices) {
    if (!(Number.isFinite(v.x) && Number.isFinite(v.y) && Number.isFinite(v.z))) {
      throw new RangeError(`hull ${outline.name} has a vertex that is not finite`);
    }
  }
  const indices = [...outline.edges.flat(), ...outline.faces.flat()];
  for (const index of indices) {
    if (!(Number.isInteger(index) && index >= 0 && index < count)) {
      throw new RangeError(
        `hull ${outline.name} names vertex ${String(index)} of ${String(count)}`,
      );
    }
  }
  return outline;
}

/** The half-width and half-height of the test hull's cross-section at its widest, m. */
const BEAM_M = { x: 3, y: 2 } as const;

/**
 * The seat's eye point on the test hull: above its forward section, on the centreline, outside the
 * faces, as an open cockpit's is, so that the hull's own occluder never encloses the seat camera.
 */
const TEST_EYE_POINT_M = vec3(0, 2.5, -4);

/** How far forward of the eye the test hull's plate stands, m: 1, the precision scene's figure. */
export const TEST_PLATE_DISTANCE_M = 1;

/**
 * The hand-made test hull (plan R02, Design note 15): a 20 m craft, a wedge from its nose at z =
 * −10 m to a 6 m × 4 m section at z = −4 m and a flared 8 m × 5 m stern at z = +10 m, with a 1 m
 * square plate (a windscreen) standing 1 m forward of the seat's eye point, facing it, above the
 * nose.
 *
 * @remarks
 * Labelled `TEST HULL` wherever it appears. It stands for R03's ship stand-in and for any craft
 * whose hull names no known outline until the plan that builds craft supplies definitions.
 */
export const TEST_HULL: HullOutline = hullOutline({
  name: "TEST HULL",
  vertices: [
    // 0: the nose.
    vec3(0, 0, -10),
    // 1–4: the forward section, port-low, starboard-low, starboard-high, port-high.
    vec3(-BEAM_M.x, -BEAM_M.y, -4),
    vec3(BEAM_M.x, -BEAM_M.y, -4),
    vec3(BEAM_M.x, BEAM_M.y, -4),
    vec3(-BEAM_M.x, BEAM_M.y, -4),
    // 5–8: the stern, in the same order.
    vec3(-4, -2.5, 10),
    vec3(4, -2.5, 10),
    vec3(4, 2.5, 10),
    vec3(-4, 2.5, 10),
    // 9–12: the plate, 1 m forward of the eye, port-low, starboard-low, starboard-high, port-high.
    vec3(-0.5, 2, TEST_EYE_POINT_M.z - TEST_PLATE_DISTANCE_M),
    vec3(0.5, 2, TEST_EYE_POINT_M.z - TEST_PLATE_DISTANCE_M),
    vec3(0.5, 3, TEST_EYE_POINT_M.z - TEST_PLATE_DISTANCE_M),
    vec3(-0.5, 3, TEST_EYE_POINT_M.z - TEST_PLATE_DISTANCE_M),
  ],
  edges: [
    [0, 1],
    [0, 2],
    [0, 3],
    [0, 4],
    [1, 2],
    [2, 3],
    [3, 4],
    [4, 1],
    [1, 5],
    [2, 6],
    [3, 7],
    [4, 8],
    [5, 6],
    [6, 7],
    [7, 8],
    [8, 5],
    [9, 10],
    [10, 11],
    [11, 12],
    [12, 9],
  ],
  faces: [
    // The nose's four faces.
    [0, 2, 1],
    [0, 3, 2],
    [0, 4, 3],
    [0, 1, 4],
    // The four sides from section to stern.
    [1, 2, 6],
    [1, 6, 5],
    [2, 3, 7],
    [2, 7, 6],
    [3, 4, 8],
    [3, 8, 7],
    [4, 1, 5],
    [4, 5, 8],
    // The stern.
    [5, 6, 7],
    [5, 7, 8],
    // The plate.
    [9, 10, 11],
    [9, 11, 12],
  ],
  eyePointM: TEST_EYE_POINT_M,
  lengthM: 20,
});
