import { METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { SystemPlace } from "./model";
import { barycentreAt } from "./place";

/** A place 1,000 km below its cell's upper x face, drifting +x at 200 km/s, stated at 1,000 s. */
const PLACE: SystemPlace = {
  system: "0200080020000000",
  designation: "Vorth AB-C e4-17",
  barycentre: { cell_ly: [-26_000, 12, 3], offset_m: [METRES_PER_LIGHT_YEAR - 1e6, 5e15, 0] },
  velocityMPerS: vec3(2e5, 0, -1e4),
  time: { seconds: 1_000, nanos: 0 },
};

describe("barycentreAt", () => {
  it("is the place's barycentre at the place's own time", () => {
    expect(barycentreAt(PLACE, PLACE.time ?? { seconds: 0, nanos: 0 })).toEqual(PLACE.barycentre);
  });

  it("drifts across a 1 ly cell boundary into the next cell, its offset renormalised", () => {
    // 10 s on at 200 km/s is 2,000 km: 1,000 km past the face. Along z, 100 km down from 0 m
    // crosses the lower face into the cell below.
    const at = barycentreAt(PLACE, { seconds: 1_010, nanos: 0 });
    expect(at?.cell_ly).toEqual([-25_999, 12, 2]);
    expect(at?.offset_m[0]).toBeCloseTo(1e6, -1);
    expect(at?.offset_m[1]).toBe(5e15);
    expect(at?.offset_m[2]).toBeCloseTo(METRES_PER_LIGHT_YEAR - 1e5, -1);
    for (const offset of at?.offset_m ?? []) {
      expect(offset >= 0 && offset < METRES_PER_LIGHT_YEAR).toBe(true);
    }
  });

  it("drifts backwards for a time before the place's", () => {
    const at = barycentreAt(PLACE, { seconds: 990, nanos: 0 });
    expect(at?.cell_ly).toEqual([-26_000, 12, 3]);
    expect(at?.offset_m[0]).toBeCloseTo(METRES_PER_LIGHT_YEAR - 3e6, -1);
    expect(at?.offset_m[2]).toBeCloseTo(1e5, -1);
  });

  it("gives a place known only from the chart unchanged, and none where nothing is known", () => {
    const charted: SystemPlace = { ...PLACE, velocityMPerS: null, time: null };
    expect(barycentreAt(charted, { seconds: 1e9, nanos: 0 })).toBe(PLACE.barycentre);
    expect(barycentreAt({ ...charted, barycentre: null }, { seconds: 0, nanos: 0 })).toBeNull();
  });
});
