import { describe, expect, it } from "vitest";

import {
  type CellOut,
  childKeys,
  type CubePoint,
  type Face,
  faceCoords,
  faceOfAxis,
  facePoint,
  FACES,
  isValidPatchKey,
  MAX_LEVEL,
  parentKey,
  parsePatchKeyString,
  type PatchKey,
  patchKeyString,
  rootKey,
  stepCellInto,
} from "./patchKey";

const DEEP: PatchKey = { face: 5, level: MAX_LEVEL, i: 2 ** MAX_LEVEL - 1, j: 12_345 };

describe("patch keys", () => {
  it("round-trips through its string", () => {
    for (const key of [...FACES.map(rootKey), DEEP, { face: 2, level: 7, i: 3, j: 100 } as const]) {
      expect(parsePatchKeyString(patchKeyString(key))).toEqual(key);
    }
  });

  it("refuses strings that are not a valid key's", () => {
    for (const s of ["6/0/0/0", "0/1/2/0", "0/25/0/0", "0/01/0/0", "0/1/0", "a/b/c/d", ""]) {
      expect(parsePatchKeyString(s)).toBeNull();
    }
  });

  it("checks the key's ranges", () => {
    expect(isValidPatchKey(DEEP)).toBe(true);
    expect(isValidPatchKey({ face: 0, level: 3, i: 8, j: 0 })).toBe(false);
    expect(isValidPatchKey({ face: 0, level: 3, i: 1.5, j: 0 })).toBe(false);
  });

  it("makes every child's parent the patch itself", () => {
    const key: PatchKey = { face: 3, level: 9, i: 300, j: 17 };
    for (const child of childKeys(key)) {
      expect(parentKey(child)).toEqual(key);
    }
    expect(new Set(childKeys(key).map(patchKeyString)).size).toBe(4);
  });

  it("gives a root no parent and the deepest level no children", () => {
    expect(parentKey(rootKey(4))).toBeNull();
    expect(() => childKeys(DEEP)).toThrow(/deepest level/u);
  });
});

type Axis = 0 | 1 | 2;

/** The axis normal to `face`. */
function axisOf(face: Face): Axis {
  return face === 0 || face === 3 ? 0 : face === 1 || face === 4 ? 1 : 2;
}

/** `p` with its `axis` component replaced by `value`. */
function withAxis(p: CubePoint, axis: Axis, value: number): CubePoint {
  return axis === 0 ? [value, p[1], p[2]] : axis === 1 ? [p[0], value, p[2]] : [p[0], p[1], value];
}

/**
 * The step to a neighbouring cell in the array form it had before R05.T7 perf (c) wrote it in
 * scalars: the oracle `stepCellInto` must match.
 */
function referenceStep(k: PatchKey, di: number, dj: number): PatchKey | null {
  const half = 2 ** k.level;
  let p = facePoint(k.face, 2 * k.i + 1 - half + 2 * di, 2 * k.j + 1 - half + 2 * dj, half);
  const over = ([0, 1, 2] as const).filter((a) => Math.abs(p[a]) > half);
  const [first, second] = over;
  let face: Face;
  if (first === undefined) {
    face = k.face;
  } else if (second === undefined) {
    const own = axisOf(k.face);
    p = withAxis(p, first, Math.sign(p[first]) * half);
    p = withAxis(p, own, Math.sign(p[own]) * (half - 1));
    face = faceOfAxis(first, p[first]);
  } else {
    return null;
  }
  const [u, v] = faceCoords(face, p);
  return { face, level: k.level, i: (u + half - 1) / 2, j: (v + half - 1) / 2 };
}

describe("a step to a neighbouring cell", () => {
  it("folds as the array form did, into one reused cell, and writes nothing at a cube corner", () => {
    // Every cell of levels 0 to 4 of every face, and cells along the edges of deep levels.
    const keys: PatchKey[] = [];
    for (const face of FACES) {
      for (let level = 0; level <= 4; level += 1) {
        for (let i = 0; i < 2 ** level; i += 1) {
          for (let j = 0; j < 2 ** level; j += 1) {
            keys.push({ face, level, i, j });
          }
        }
      }
      for (const level of [12, 19, MAX_LEVEL]) {
        const last = 2 ** level - 1;
        for (const [i, j] of [
          [0, 0],
          [last, 0],
          [0, last],
          [last, last],
          [0, 777],
          [last, 777],
          [777, 0],
          [777, last],
          [777, 778],
        ] as const) {
          keys.push({ face, level, i, j });
        }
      }
    }
    const out: CellOut = { face: 0, i: -1, j: -1 };
    const wrong: string[] = [];
    let corners = 0;
    for (const key of keys) {
      for (let di = -1; di <= 1; di += 1) {
        for (let dj = -1; dj <= 1; dj += 1) {
          if (di === 0 && dj === 0) {
            continue;
          }
          const before = { ...out };
          const stepped = stepCellInto(out, key, di, dj);
          const got = stepped ? { face: out.face, level: key.level, i: out.i, j: out.j } : null;
          if (!stepped) {
            corners += 1;
            if (out.face !== before.face || out.i !== before.i || out.j !== before.j) {
              wrong.push(`${patchKeyString(key)} (${di}, ${dj}) wrote a cell at a corner`);
            }
          }
          if (JSON.stringify(got) !== JSON.stringify(referenceStep(key, di, dj))) {
            wrong.push(`${patchKeyString(key)} (${di}, ${dj}): ${JSON.stringify(got)}`);
          }
        }
      }
    }
    expect(wrong).toEqual([]);
    // Each face's four corner cells have one diagonal off the cube, at every level held whole and
    // at the deep levels' four corners.
    expect(corners).toBe(6 * 4 * (5 + 3));
  });
});
