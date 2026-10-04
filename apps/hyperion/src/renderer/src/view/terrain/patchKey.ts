/**
 * Patch keys of the cube-sphere quadtree, the client's mirror of `hyperion_surface::cube::PatchKey`
 * (plan R05, Design note 2).
 *
 * @remarks
 * A key is (face, level, i, j) with `i, j < 2^level`. Rust packs it into a `u64`, which does not
 * fit a JavaScript number, so the client keys its maps by {@link patchKeyString}.
 */

/**
 * A cube face in S2's order: 0 = +x, 1 = +y, 2 = +z, 3 = −x, 4 = −y, 5 = −z (`hyperion_surface`'s
 * `Face::PosX` to `Face::NegZ`).
 */
export type Face = 0 | 1 | 2 | 3 | 4 | 5;

/**
 * Marks the end of an exhaustive `switch` over a face or an axis, whose numeric cases the linter
 * cannot see are exhaustive: the call compiles only while `value` is narrowed to `never`.
 *
 * @throws Error always; it is reached only by a value outside its union.
 */
export function unreachable(value: never): never {
  throw new Error(`unreachable: ${String(value)}`);
}

/** The six faces in index order. */
export const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The deepest level a key may have (`hyperion_surface::MAX_LEVEL`). */
export const MAX_LEVEL = 24;

/** A node of one face's quadtree: `i` along the face's u axis and `j` along its v axis. */
export interface PatchKey {
  readonly face: Face;
  /** 0 for the face itself, up to {@link MAX_LEVEL}. */
  readonly level: number;
  /** The column, from 0 at u = −1 to `2^level − 1`. */
  readonly i: number;
  /** The row, from 0 at v = −1 to `2^level − 1`. */
  readonly j: number;
}

/** The root patch of a face, the whole face at level 0. */
export function rootKey(face: Face): PatchKey {
  return { face, level: 0, i: 0, j: 0 };
}

/**
 * The string that keys a patch in the client's maps, `face/level/i/j` in decimal.
 *
 * @remarks
 * It is also the tie-break of the demand list's order (R05.T7.d), so its form is part of the
 * contract.
 */
export function patchKeyString(k: PatchKey): string {
  return `${k.face}/${k.level}/${k.i}/${k.j}`;
}

function isFace(n: number): n is Face {
  return Number.isInteger(n) && n >= 0 && n <= 5;
}

/** Whether a key's fields are in range: an integer level up to {@link MAX_LEVEL}, i and j below 2^level. */
export function isValidPatchKey(k: PatchKey): boolean {
  if (!isFace(k.face) || !Number.isInteger(k.level) || k.level < 0 || k.level > MAX_LEVEL) {
    return false;
  }
  const side = 2 ** k.level;
  return (
    Number.isInteger(k.i) &&
    Number.isInteger(k.j) &&
    k.i >= 0 &&
    k.j >= 0 &&
    k.i < side &&
    k.j < side
  );
}

const KEY_STRING = /^([0-5])\/(\d{1,2})\/(\d{1,8})\/(\d{1,8})$/u;

/**
 * Parses a {@link patchKeyString} back to its key, or `null` if the string is not one of a valid
 * key.
 */
export function parsePatchKeyString(s: string): PatchKey | null {
  const m = KEY_STRING.exec(s);
  if (m === null) {
    return null;
  }
  const face = Number(m[1]);
  if (!isFace(face)) {
    return null;
  }
  const key: PatchKey = { face, level: Number(m[2]), i: Number(m[3]), j: Number(m[4]) };
  return isValidPatchKey(key) && patchKeyString(key) === s ? key : null;
}

/** The patch's parent, one level up, or `null` for a face's root. */
export function parentKey(k: PatchKey): PatchKey | null {
  if (k.level === 0) {
    return null;
  }
  return { face: k.face, level: k.level - 1, i: Math.floor(k.i / 2), j: Math.floor(k.j / 2) };
}

/**
 * The patch's four children, in `hyperion_surface`'s order: (2i, 2j), (2i + 1, 2j), (2i, 2j + 1),
 * (2i + 1, 2j + 1).
 *
 * @throws Error if the patch is already at {@link MAX_LEVEL}.
 */
export function childKeys(k: PatchKey): readonly [PatchKey, PatchKey, PatchKey, PatchKey] {
  if (k.level >= MAX_LEVEL) {
    throw new Error(`patch ${patchKeyString(k)} is at the deepest level and has no children`);
  }
  const level = k.level + 1;
  const i = 2 * k.i;
  const j = 2 * k.j;
  return [
    { face: k.face, level, i, j },
    { face: k.face, level, i: i + 1, j },
    { face: k.face, level, i, j: j + 1 },
    { face: k.face, level, i: i + 1, j: j + 1 },
  ];
}

/**
 * A patch's index within its level, face · 2⁴⁸ + i · 2²⁴ + j: an exact integer below 2⁵¹, which
 * numeric maps per level key patches by without building a string.
 */
export function patchKeyIndex(key: PatchKey): number {
  return key.face * 2 ** 48 + key.i * 2 ** 24 + key.j;
}

/**
 * The index ({@link patchKeyIndex}) of `key`'s ancestor at `level`, computed without building the
 * ancestor's key.
 */
export function ancestorIndex(key: PatchKey, level: number): number {
  const shift = 2 ** (key.level - level);
  return key.face * 2 ** 48 + Math.floor(key.i / shift) * 2 ** 24 + Math.floor(key.j / shift);
}

/** The key's `hyperion_surface` `to_u64` word, for checks against the Rust goldens. */
export function patchKeyWord(k: PatchKey): bigint {
  return (BigInt(k.face) << 53n) | (BigInt(k.level) << 48n) | (BigInt(k.i) << 24n) | BigInt(k.j);
}

/** One of a patch's four edges, named by the face coordinate constant along it (Rust's `Edge`). */
export type Edge =
  /** The edge at the patch's smallest u (vertex column x = 0), towards i − 1. */
  | "UMin"
  /** The edge at the patch's largest u (x = 64), towards i + 1. */
  | "UMax"
  /** The edge at the patch's smallest v (vertex row y = 0), towards j − 1. */
  | "VMin"
  /** The edge at the patch's largest v (y = 64), towards j + 1. */
  | "VMax";

/** The four edges, in `Edge::ALL`'s order. */
export const EDGES: ReadonlyArray<Edge> = ["UMin", "UMax", "VMin", "VMax"];

/** An integer point on the cube, in body-fixed axes. */
export type CubePoint = readonly [number, number, number];

/** An axis of the body-fixed frame: 0 = x, 1 = y, 2 = z. */
export type Axis = 0 | 1 | 2;

const AXES: ReadonlyArray<Axis> = [0, 1, 2];

function edgeStep(edge: Edge): readonly [number, number] {
  switch (edge) {
    case "UMin":
      return [-1, 0];
    case "UMax":
      return [1, 0];
    case "VMin":
      return [0, -1];
    case "VMax":
      return [0, 1];
  }
  return unreachable(edge);
}

/**
 * The integer point (`u`, `v`) of `face` on the cube of half-width `half`, by S2's axes (Rust's
 * `face_point`).
 */
export function facePoint(face: Face, u: number, v: number, half: number): CubePoint {
  switch (face) {
    case 0:
      return [half, u, v];
    case 1:
      return [-u, half, v];
    case 2:
      return [-u, -v, half];
    case 3:
      return [-half, -v, -u];
    case 4:
      return [v, -half, -u];
    case 5:
      return [v, u, -half];
  }
  return unreachable(face);
}

/** The inverse of {@link facePoint}: the (u, v) of the integer point `p` on `face`. */
export function faceCoords(face: Face, p: CubePoint): readonly [number, number] {
  switch (face) {
    case 0:
      return [p[1], p[2]];
    case 1:
      return [-p[0], p[2]];
    case 2:
      return [-p[0], -p[1]];
    case 3:
      return [-p[2], -p[1]];
    case 4:
      return [-p[2], p[0]];
    case 5:
      return [p[1], p[0]];
  }
  return unreachable(face);
}

function faceAxis(face: Face): Axis {
  switch (face) {
    case 0:
    case 3:
      return 0;
    case 1:
    case 4:
      return 1;
    case 2:
    case 5:
      return 2;
  }
  return unreachable(face);
}

/** The face normal to `axis` on the side of `sign`'s sign. */
export function faceOfAxis(axis: Axis, sign: number): Face {
  switch (axis) {
    case 0:
      return sign > 0 ? 0 : 3;
    case 1:
      return sign > 0 ? 1 : 4;
    case 2:
      return sign > 0 ? 2 : 5;
  }
  return unreachable(axis);
}

/** The lowest-index face containing the integer point `p` of the cube of half-width `half`. */
export function canonicalFace(p: CubePoint, half: number): Face {
  let best: Face | null = null;
  for (const a of AXES) {
    if (Math.abs(p[a]) === half) {
      const face = faceOfAxis(a, p[a]);
      if (best === null || face < best) {
        best = face;
      }
    }
  }
  if (best === null) {
    throw new Error(`point (${p.join(", ")}) is not on the cube of half-width ${half}`);
  }
  return best;
}

/** A cell of a level that {@link stepCellInto} writes: its face and its column and row. */
export interface CellOut {
  face: Face;
  i: number;
  j: number;
}

/**
 * Writes into `out` the cell (i + `di`, j + `dj`) of the patch's level, folded onto the
 * neighbouring face when it leaves this one, and returns `true`; returns `false`, writing nothing,
 * when both coordinates leave it (a cube corner's diagonal).
 *
 * @remarks
 * Rust's `step_cell`, exact integer geometry in half-cell units, with nothing allocated: the
 * selection's balance steps to every new leaf's neighbours with it (R05.T7 perf (c)).
 *
 * @param di - The step in i, −1, 0 or 1.
 * @param dj - The step in j, −1, 0 or 1.
 */
export function stepCellInto(out: CellOut, k: PatchKey, di: number, dj: number): boolean {
  const half = 2 ** k.level;
  const cu = 2 * k.i + 1 - half + 2 * di;
  const cv = 2 * k.j + 1 - half + 2 * dj;
  // `facePoint`'s point, axis by axis, with no array.
  let x = 0;
  let y = 0;
  let z = 0;
  switch (k.face) {
    case 0:
      x = half;
      y = cu;
      z = cv;
      break;
    case 1:
      x = -cu;
      y = half;
      z = cv;
      break;
    case 2:
      x = -cu;
      y = -cv;
      z = half;
      break;
    case 3:
      x = -half;
      y = -cv;
      z = -cu;
      break;
    case 4:
      x = cv;
      y = -half;
      z = -cu;
      break;
    case 5:
      x = cv;
      y = cu;
      z = -half;
      break;
  }
  const overX = Math.abs(x) > half;
  const overY = Math.abs(y) > half;
  const overZ = Math.abs(z) > half;
  const over = (overX ? 1 : 0) + (overY ? 1 : 0) + (overZ ? 1 : 0);
  if (over > 1) {
    return false;
  }
  let face = k.face;
  if (over === 1) {
    // Onto the face the point left by: that axis back to the cube, this face's axis one cell in.
    const own = faceAxis(k.face);
    if (own === 0) {
      x = Math.sign(x) * (half - 1);
    } else if (own === 1) {
      y = Math.sign(y) * (half - 1);
    } else {
      z = Math.sign(z) * (half - 1);
    }
    if (overX) {
      x = Math.sign(x) * half;
      face = faceOfAxis(0, x);
    } else if (overY) {
      y = Math.sign(y) * half;
      face = faceOfAxis(1, y);
    } else {
      z = Math.sign(z) * half;
      face = faceOfAxis(2, z);
    }
  }
  // `faceCoords`' (u, v) on the face reached.
  let u = 0;
  let v = 0;
  switch (face) {
    case 0:
      u = y;
      v = z;
      break;
    case 1:
      u = -x;
      v = z;
      break;
    case 2:
      u = -x;
      v = -y;
      break;
    case 3:
      u = -z;
      v = -y;
      break;
    case 4:
      u = -z;
      v = x;
      break;
    case 5:
      u = y;
      v = x;
      break;
  }
  out.face = face;
  out.i = (u + half - 1) / 2;
  out.j = (v + half - 1) / 2;
  return true;
}

/**
 * The cell (i + `di`, j + `dj`) of the patch's level, folded onto the neighbouring face when it
 * leaves this one, or `null` when both coordinates leave it (a cube corner's diagonal).
 */
function stepCell(k: PatchKey, di: number, dj: number): PatchKey | null {
  const out: CellOut = { face: k.face, i: 0, j: 0 };
  return stepCellInto(out, k, di, dj)
    ? { face: out.face, level: k.level, i: out.i, j: out.j }
    : null;
}

/** The patch of the same level across `edge`, on the neighbouring face across a face edge. */
export function edgeNeighbour(k: PatchKey, edge: Edge): PatchKey {
  const [di, dj] = edgeStep(edge);
  const n = stepCell(k, di, dj);
  if (n === null) {
    throw new Error("a step across one edge leaves the face by one coordinate at most");
  }
  return n;
}

/** Whether two keys name the same patch. */
export function sameKey(a: PatchKey, b: PatchKey): boolean {
  return a.face === b.face && a.level === b.level && a.i === b.i && a.j === b.j;
}

/** The patch across `edge` and the edge of that patch which leads back to this one. */
export function edgeNeighbourAndBack(k: PatchKey, edge: Edge): readonly [PatchKey, Edge] {
  const neighbour = edgeNeighbour(k, edge);
  const back = EDGES.find((e) => sameKey(edgeNeighbour(neighbour, e), k));
  if (back === undefined) {
    throw new Error("the neighbour across an edge is a neighbour back across one of its edges");
  }
  return [neighbour, back];
}

/**
 * The patches of the same level across each corner, in the order (i − 1, j − 1), (i + 1, j − 1),
 * (i + 1, j + 1), (i − 1, j + 1); `null` at a cube corner, about which only three patches meet.
 */
export function cornerNeighbours(
  k: PatchKey,
): readonly [PatchKey | null, PatchKey | null, PatchKey | null, PatchKey | null] {
  return [stepCell(k, -1, -1), stepCell(k, 1, -1), stepCell(k, 1, 1), stepCell(k, -1, 1)];
}
