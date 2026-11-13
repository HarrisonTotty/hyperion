import { describe, expect, it } from "vitest";

import { METER_CLASS } from "../view/post/meter";
import { againstUnshadowed } from "./cascadeDarkening";

/** A read-back of terrain texels lit by a star, each its three channels. */
function frame(...texels: ReadonlyArray<readonly [number, number, number]>): Float32Array {
  return Float32Array.from(texels.flatMap(([r, g, b]) => [r, g, b, METER_CLASS.litBody]));
}

describe("the cascades' darkening against the frame at w = 0", () => {
  it("counts no texel darker where both frames are alike, a channel under the floor included", () => {
    // The second texel's blue channel is under a millionth of the brightest's: as dark in both.
    const open = frame([1, 1, 1], [0.5, 0.5, 1e-9]);
    const seen = againstUnshadowed(open, open);
    expect(seen.terrain).toBe(2);
    expect(seen.darker).toBe(0);
    expect(seen.halved).toBe(0);
    expect(seen.brighter).toBe(0);
  });

  it("counts a texel darkened to a third as darker and darker by more than half", () => {
    const seen = againstUnshadowed(
      frame([1, 1, 1], [0.1, 0.1, 0.1]),
      frame([1, 1, 1], [0.3, 0.3, 0.3]),
    );
    expect(seen.darker).toBe(1);
    expect(seen.halved).toBe(1);
  });

  it("counts a texel 1% darker as darker but not by more than half", () => {
    const seen = againstUnshadowed(frame([0.99, 1, 1]), frame([1, 1, 1]));
    expect(seen.darker).toBe(1);
    expect(seen.halved).toBe(0);
  });

  it("leaves out texels no body draws", () => {
    const open = Float32Array.of(1, 1, 1, METER_CLASS.other, 1, 1, 1, METER_CLASS.litBody);
    const shadowed = Float32Array.of(0, 0, 0, METER_CLASS.other, 1, 1, 1, METER_CLASS.litBody);
    const seen = againstUnshadowed(shadowed, open);
    expect(seen.terrain).toBe(1);
    expect(seen.darker).toBe(0);
  });
});
