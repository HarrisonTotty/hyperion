/**
 * The resolve's place probe's comparison (plan R14, R14.T2.d; `resolveProbe.ts`): the units in the
 * last place two probes' places stand apart, and the pixels one probe alone drew.
 */

import { describe, expect, it } from "vitest";

import { ID_NONE } from "../view/engine/idTarget";
import { placeAgreement, ulpOf } from "./resolveProbe";

/** The bits of `f32` `x`. */
function bitsOf(x: number): number {
  return new Uint32Array(Float32Array.of(x).buffer)[0] ?? ID_NONE;
}

/** Three axes of one pixel each, from places. */
function axes(...places: ReadonlyArray<readonly [number, number, number]>): Uint32Array[] {
  return [0, 1, 2].map((axis) => Uint32Array.from(places, (p) => bitsOf(p[axis] ?? 0)));
}

describe("the resolve's place probe (plan R14, R14.T2.d)", () => {
  it("finds no unit apart where the two places are the same bits", () => {
    const same = axes([3, -4, 12], [100, 0.5, -7]);
    expect(placeAgreement(same, same)).toEqual({
      compared: 6,
      unmatched: 0,
      medianUlps: 0,
      p99Ulps: 0,
      largestUlps: 0,
      largestRelative: 0,
    });
  });

  it("counts a difference in units in the last place of the place's distance, an axis near 0 among them", () => {
    // |(3, 4, 12)| = 13, whose binade's unit is 2⁻²⁰.
    const unit = ulpOf(13);
    const got = placeAgreement(axes([3, 4, 12]), axes([3 + unit, 4, 12 + 2 * unit]));
    expect([unit, got.compared, got.largestUlps, got.medianUlps]).toEqual([2 ** -20, 3, 2, 1]);
    expect(placeAgreement(axes([0, 4, 3]), axes([unit, 4, 3])).largestUlps).toBeCloseTo(
      unit / ulpOf(5),
    );
  });

  it("counts a pixel one probe alone drew as unmatched, and one neither drew not at all", () => {
    const none = [Uint32Array.of(ID_NONE), Uint32Array.of(ID_NONE), Uint32Array.of(ID_NONE)];
    expect([placeAgreement(axes([1, 2, 3]), none), placeAgreement(none, none)]).toEqual([
      { compared: 0, unmatched: 1, medianUlps: 0, p99Ulps: 0, largestUlps: 0, largestRelative: 0 },
      { compared: 0, unmatched: 0, medianUlps: 0, p99Ulps: 0, largestUlps: 0, largestRelative: 0 },
    ]);
  });
});
