import { describe, expect, it } from "vitest";

import { f32Digest, f64Digest, fnv1a64 } from "./f32Digest";

const encode = (s: string): Uint8Array => new TextEncoder().encode(s);

describe("the float digests", () => {
  it("matches FNV-1a 64's published vectors", () => {
    // The FNV test suite's "", "a" and "foobar" (Fowler, Noll and Vo, isthe.com/chongo/tech/comp/fnv).
    expect(fnv1a64(encode(""))).toBe(0xcbf29ce484222325n);
    expect(fnv1a64(encode("a"))).toBe(0xaf63dc4c8601ec8cn);
    expect(fnv1a64(encode("foobar"))).toBe(0x85944171f73967e8n);
  });

  it("matches the testkit's hand-computed f32 digest of 1.0", () => {
    expect(f32Digest(new Float32Array([1]))).toBe(0x4b72477f9c5c2f98n);
  });

  it("hashes each f64's little-endian bits", () => {
    expect(f64Digest(new Float64Array([1]))).toBe(
      fnv1a64(new Uint8Array([0, 0, 0, 0, 0, 0, 0xf0, 0x3f])),
    );
  });

  it("tells a negative zero from a positive one", () => {
    expect(f32Digest(new Float32Array([-0]))).not.toBe(f32Digest(new Float32Array([0])));
    expect(f64Digest(new Float64Array([-0]))).not.toBe(f64Digest(new Float64Array([0])));
  });
});
