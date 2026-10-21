import { describe, expect, it } from "vitest";

import { localFrameAt } from "../geometry/frame";
import { add, scale, vec3, type Vec3 } from "../geometry/vec3";
import { apparentOffset } from "./apparent";
import { type Camera, PRESETS, type PresetName } from "./camera";

const FRAME = localFrameAt(vec3(26_000, 0, 0));
const K = 5;

/** A vector from coreward, spinward and north components. */
function local(coreward: number, spinward: number, north: number): Vec3 {
  return add(
    add(scale(FRAME.coreward, coreward), scale(FRAME.spinward, spinward)),
    scale(FRAME.north, north),
  );
}

function preset(name: PresetName, pxPerUnit = K): Camera {
  return { ...PRESETS[name], pxPerUnit };
}

/** A present position off the view centre, which the offset must not depend on. */
const PRESENT = local(12, -7, 4);

/** The apparent position: 1 ly coreward, 2 ly spinward and 3 ly north of the present one. */
const APPARENT = add(PRESENT, local(1, 2, 3));

describe("apparentOffset (plan 12, P12.T11)", () => {
  // Worked from `viewBasis`, with c, s and n the coreward, spinward and north components: TOP has
  // right = s and up = c, SIDE right = s and up = n, FRONT right = −c and up = n, and OBLIQUE
  // (030°, +30°) right = (√3/2)s − ½c and up = (√3/2)n + ½((√3/2)c + ½s). The screen's y runs down.
  it.each([
    ["top", 2 * K, -1 * K],
    ["side", 2 * K, -3 * K],
    ["front", -1 * K, -3 * K],
    ["oblique", K * (Math.sqrt(3) - 0.5), -K * ((7 * Math.sqrt(3)) / 4 + 0.5)],
  ] as const)("gives the offset worked by hand at the %s view", (name, dxPx, dyPx) => {
    const offset = apparentOffset(PRESENT, APPARENT, FRAME, preset(name));

    expect(offset.dxPx).toBeCloseTo(dxPx, 9);
    expect(offset.dyPx).toBeCloseTo(dyPx, 9);
  });

  it("scales with the zoom and does not depend on where the system lies", () => {
    const near = apparentOffset(vec3(0, 0, 0), local(1, 2, 3), FRAME, preset("top", 2 * K));

    expect(near.dxPx).toBeCloseTo(4 * K, 9);
    expect(near.dyPx).toBeCloseTo(-2 * K, 9);
  });

  it("is zero seen straight along the line from present to apparent", () => {
    const offset = apparentOffset(PRESENT, add(PRESENT, local(0, 0, 3)), FRAME, preset("top"));

    expect(Math.hypot(offset.dxPx, offset.dyPx)).toBeCloseTo(0, 12);
  });
});
