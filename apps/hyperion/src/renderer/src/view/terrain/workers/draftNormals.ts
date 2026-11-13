/**
 * A drafted bake's normals made whole by its refined ones, on the render thread (plan R10,
 * R10.T6.a's progressive bake).
 *
 * @remarks
 * A draft's packed normals hold each vertex's exact, at the even samples of the double grid, and
 * the vertices' mean between them. The pass keeps the vertices' half floats alone
 * ({@link draftVertexBits}, 16.9 KB a patch where the whole tile is 66.6 KB) until the patch's
 * {@link RefinedNormals} land, then rebuilds the tile ({@link mergeRefinedNormals}): the vertices'
 * bits as the draft packed them, and each sample between them rounded to a half float as
 * `packNormals` rounds a whole bake's, to nearest, ties to even. So the tile is the whole bake's
 * packed normals, bit for bit.
 */

import { NORMALS_BETWEEN, type RefinedNormals } from "./messages";

/** Normals along a side of the double grid: 129. */
const SIDE = 129;

/** Vertices along a patch's side: 65. */
const VERTICES = 65;

/**
 * The half-float bits of a drafted bake's normals at its vertices, the double grid's even samples,
 * row by row: 65 × 65 pairs.
 *
 * @throws RangeError for normals that are not the double grid's 129 × 129 pairs.
 */
export function draftVertexBits(normals: Float16Array): Uint16Array {
  if (normals.length !== 2 * SIDE * SIDE) {
    throw new RangeError(
      `${normals.length / 2} normals, where a drafted bake has ${SIDE} × ${SIDE}`,
    );
  }
  const bits = new Uint16Array(normals.buffer, normals.byteOffset, normals.length);
  const out = new Uint16Array(2 * VERTICES * VERTICES);
  for (let vy = 0; vy < VERTICES; vy += 1) {
    for (let vx = 0; vx < VERTICES; vx += 1) {
      const from = 2 * (2 * vy * SIDE + 2 * vx);
      const to = 2 * (vy * VERTICES + vx);
      out[to] = bits[from] ?? 0;
      out[to + 1] = bits[from + 1] ?? 0;
    }
  }
  return out;
}

/**
 * The whole bake's packed normals: `vertexBits` ({@link draftVertexBits}) at the vertices and
 * `between`, a refinement's pairs ({@link RefinedNormals.normals}), between them, in row order.
 *
 * @throws RangeError for vertex bits or pairs of another count than a patch's.
 */
export function mergeRefinedNormals(
  vertexBits: Uint16Array,
  between: RefinedNormals["normals"],
): Float16Array {
  if (vertexBits.length !== 2 * VERTICES * VERTICES) {
    throw new RangeError(`${vertexBits.length / 2} vertex normals, where a patch has 65 × 65`);
  }
  if (between.length !== 2 * NORMALS_BETWEEN) {
    throw new RangeError(
      `${between.length / 2} normals between the vertices, where a patch has ${NORMALS_BETWEEN}`,
    );
  }
  const out = new Float16Array(2 * SIDE * SIDE);
  const bits = new Uint16Array(out.buffer);
  let next = 0;
  for (let y = 0; y < SIDE; y += 1) {
    for (let x = 0; x < SIDE; x += 1) {
      const to = 2 * (y * SIDE + x);
      if (x % 2 === 0 && y % 2 === 0) {
        const from = 2 * ((y / 2) * VERTICES + x / 2);
        bits[to] = vertexBits[from] ?? 0;
        bits[to + 1] = vertexBits[from + 1] ?? 0;
      } else {
        out[to] = between[next] ?? 0;
        out[to + 1] = between[next + 1] ?? 0;
        next += 2;
      }
    }
  }
  return out;
}
