import { type GalacticPosition, METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { SystemPlace } from "./model";
import { barycentreAt } from "./place";

const SYSTEM = "0200080020000000";
const DESIGNATION = "Vorth AB-C e4-17";

/** 1,000 km below its cell's upper x face and on its lower z face. */
const BARYCENTRE: GalacticPosition = {
  cell_ly: [-26_000, 12, 3],
  offset_m: [METRES_PER_LIGHT_YEAR - 1e6, 5e15, 0],
};

/** A place drifting +x at 200 km/s and −z at 10 km/s, stated at 1,000 s. */
const STATED = {
  kind: "stated",
  system: SYSTEM,
  designation: DESIGNATION,
  barycentre: BARYCENTRE,
  velocityMPerS: vec3(2e5, 0, -1e4),
  time: { seconds: 1_000, nanos: 0 },
} as const satisfies SystemPlace;

describe("barycentreAt", () => {
  it("is a stated place's barycentre at the place's own time", () => {
    expect(barycentreAt(STATED, STATED.time)).toEqual(BARYCENTRE);
  });

  it("drifts across a 1 ly cell boundary into the next cell, its offset renormalised", () => {
    // 10 s on at 200 km/s is 2,000 km: 1,000 km past the upper x face. Along z, 100 km down
    // from 0 m crosses the lower face into the cell below.
    const at = barycentreAt(STATED, { seconds: 1_010, nanos: 0 });
    expect(at?.cell_ly).toEqual([-25_999, 12, 2]);
    expect(at?.offset_m[0]).toBeCloseTo(1e6, -1);
    expect(at?.offset_m[1]).toBe(5e15);
    expect(at?.offset_m[2]).toBeCloseTo(METRES_PER_LIGHT_YEAR - 1e5, -1);
    for (const offset of at?.offset_m ?? []) {
      expect(offset >= 0 && offset < METRES_PER_LIGHT_YEAR).toBe(true);
    }
  });

  it("drifts backwards for a time before the place's", () => {
    const at = barycentreAt(STATED, { seconds: 990, nanos: 0 });
    expect(at?.cell_ly).toEqual([-26_000, 12, 3]);
    expect(at?.offset_m[0]).toBeCloseTo(METRES_PER_LIGHT_YEAR - 3e6, -1);
    expect(at?.offset_m[2]).toBeCloseTo(1e5, -1);
  });

  it("gives a charted place's barycentre unchanged, and none for an unknown place", () => {
    const charted: SystemPlace = {
      kind: "charted",
      system: SYSTEM,
      designation: DESIGNATION,
      barycentre: BARYCENTRE,
    };
    const unknown: SystemPlace = { kind: "unknown", system: SYSTEM, designation: SYSTEM };
    expect(barycentreAt(charted, { seconds: 1e9, nanos: 0 })).toBe(BARYCENTRE);
    expect(barycentreAt(unknown, { seconds: 0, nanos: 0 })).toBeNull();
  });
});
