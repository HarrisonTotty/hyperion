import { describe, expect, it } from "vitest";

import { draftVertexBits, mergeRefinedNormals } from "./draftNormals";
import { packNormals } from "./heightBake";
import { NORMALS_BETWEEN } from "./messages";

/** A double grid's 129 × 129 pairs, each sample's a value of its own, not a half float exactly. */
function wholePairs(): Float32Array {
  const pairs = new Float32Array(2 * 129 * 129);
  for (let n = 0; n < pairs.length; n += 1) {
    pairs[n] = Math.sin(0.37 * n + 0.1) * 0.999_9;
  }
  return pairs;
}

/** The samples between the vertices of `pairs`, row by row, as a refinement lays them out. */
function between(pairs: Float32Array): Float32Array {
  const out: number[] = [];
  for (let y = 0; y < 129; y += 1) {
    for (let x = 0; x < 129; x += 1) {
      if (x % 2 === 1 || y % 2 === 1) {
        const n = 2 * (y * 129 + x);
        out.push(pairs[n] ?? Number.NaN, pairs[n + 1] ?? Number.NaN);
      }
    }
  }
  return Float32Array.from(out);
}

/** A half-float array's bits. */
function bits(n: Float16Array): Uint16Array {
  return new Uint16Array(n.buffer, n.byteOffset, n.length);
}

describe("a drafted bake's normals made whole (R10.T6.a)", () => {
  it("is the whole bake's packed normals, bit for bit, from the draft's vertices and the refinement", () => {
    const whole = wholePairs();
    // A draft: the vertices' pairs exact, every other sample something else.
    const draft = whole.map((v, n) => (Math.floor(n / 2) % 2 === 1 ? 0.25 : v));
    const refined = between(whole);
    expect(refined).toHaveLength(2 * NORMALS_BETWEEN);
    const merged = mergeRefinedNormals(draftVertexBits(packNormals(draft)), refined);
    expect(bits(merged)).toEqual(bits(packNormals(whole)));
  });

  it("keeps the draft's vertices alone, a quarter of its tile", () => {
    const packed = packNormals(wholePairs());
    const kept = draftVertexBits(packed);
    expect(kept).toHaveLength(2 * 65 * 65);
    expect([kept[0], kept[1], kept[2], kept[2 * 65]]).toEqual([
      bits(packed)[0],
      bits(packed)[1],
      bits(packed)[4],
      bits(packed)[2 * 2 * 129],
    ]);
  });

  it("refuses normals of another grid, and refinements of another count", () => {
    expect(() => draftVertexBits(new Float16Array(2 * 65 * 65))).toThrow(RangeError);
    const kept = draftVertexBits(packNormals(wholePairs()));
    expect(() => mergeRefinedNormals(kept, new Float32Array(2))).toThrow(RangeError);
    expect(() =>
      mergeRefinedNormals(new Uint16Array(4), new Float32Array(2 * NORMALS_BETWEEN)),
    ).toThrow(RangeError);
  });
});
