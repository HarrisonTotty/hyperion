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
