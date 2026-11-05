/**
 * The levers' comparisons' list of the texels beyond a third of a stop (plan R14, R14.T2.d's "the
 * rest listed").
 */

import { describe, expect, it } from "vitest";

import { METER_CLASS } from "../view/post/meter";
import { beyondThirdStop, stopsApart } from "./leverCaptures";

/** A read-back of grey texels of luminance `values`, each a body's but where `values` is null. */
function texels(values: ReadonlyArray<number | null>): Float32Array {
  const out = new Float32Array(4 * values.length);
  values.forEach((value, p) => {
    out.set([value ?? 0, value ?? 0, value ?? 0, value === null ? 0 : METER_CLASS.litBody], 4 * p);
  });
  return out;
}

describe("the texels beyond a third of a stop (plan R14, R14.T2.d)", () => {
  it("lists each terrain texel stopsApart counts beyond, by its place and its luminances", () => {
    // A frame 3 wide: (1, 0) a stop darker, (0, 1) black, (2, 1) a tenth of a stop off, (1, 1) sky.
    const before = texels([1, 0.5, 0.25, 0.5, null, 0.5]);
    const after = texels([1, 0.25, 0.25, 0, null, 0.5 * 2 ** 0.1]);
    expect([
      beyondThirdStop(before, after, 3),
      stopsApart(before, after).terrain - stopsApart(before, after).within,
    ]).toEqual(["(1, 0) 5.0e-1 → 2.5e-1, (0, 1) 5.0e-1 → 1.0e-6", 2]);
  });

  it("lists the first of many, then how many more, and none where none are beyond", () => {
    const before = texels([1, 1, 1, 1]);
    expect([
      beyondThirdStop(before, texels([0.1, 0.1, 0.1, 1]), 2, before, 2),
      beyondThirdStop(before, before, 2),
    ]).toEqual(["(0, 0) 1.0e+0 → 1.0e-1, (1, 0) 1.0e+0 → 1.0e-1 and 1 more", "none"]);
  });
});
