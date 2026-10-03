import { describe, expect, it } from "vitest";

import {
  childKeys,
  FACES,
  isValidPatchKey,
  MAX_LEVEL,
  parentKey,
  parsePatchKeyString,
  type PatchKey,
  patchKeyString,
  rootKey,
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
