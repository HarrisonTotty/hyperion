import { describe, expect, it } from "vitest";

import { normalise, type Vec3 } from "../../../geometry/vec3";
import { lookAlong, rotate } from "../../camera/quaternion";
import { TEST_HULL } from "../../scene/hull";
import { datumGroundUnder } from "../../terrain/grounded";
import {
  boxReachM,
  craftBox,
  hullBox,
  intersectsBody,
  MAX_SCATTER_BODIES,
  meetsBox,
  nearGround,
  packScatterBoxes,
  SCATTER_BOX_BYTES,
  SCATTER_CULL_CLEARANCE_M,
  type ScatterBox,
} from "./scatterCull";

const AXES: readonly [Vec3, Vec3, Vec3] = [
  { x: 1, y: 0, z: 0 },
  { x: 0, y: 1, z: 0 },
  { x: 0, y: 0, z: 1 },
];

/** A body hovering `heightM` over the ground at z = 0, its box 2 m wide and 1 m tall. */
function hovering(heightM: number): ScatterBox {
  return { centreM: { x: 0, y: 0, z: heightM + 0.5 }, axes: AXES, halfExtentsM: [1, 1, 0.5] };
}

describe("the scatter's cull", () => {
  it("culls a pebble straddling a box's face and keeps one beside it", () => {
    const box = hovering(0);
    // Its +x face at x = 1: a 0.1 m sphere whose centre lies 0.05 m outside meets it.
    expect(meetsBox({ centreM: { x: 1.05, y: 0, z: 0.5 }, radiusM: 0.1 }, box)).toBe(true);
    expect(meetsBox({ centreM: { x: 1.15, y: 0, z: 0.5 }, radiusM: 0.1 }, box)).toBe(false);
    // Inside, whatever its size.
    expect(meetsBox({ centreM: { x: 0.2, y: -0.3, z: 0.4 }, radiusM: 0.001 }, box)).toBe(true);
    // On every face alike.
    for (const axis of AXES) {
      for (const sign of [-1, 1]) {
        const out = 1.2 * sign;
        const centreM = {
          x: axis.x * out,
          y: axis.y * out,
          z: 0.5 + axis.z * (sign * 0.7),
        };
        expect(meetsBox({ centreM, radiusM: 0.25 }, box)).toBe(true);
        expect(meetsBox({ centreM, radiusM: 0.15 }, box)).toBe(false);
      }
    }
  });

  it("culls under a body hovering 0.1 m up only the pebbles that reach its floor", () => {
    const box = hovering(0.1);
    // A pebble's centre on the ground: one of 0.12 m reaches the floor, one of 0.05 m does not.
    expect(intersectsBody({ centreM: { x: 0.3, y: 0.2, z: 0 }, radiusM: 0.12 }, [box])).toBe(true);
    expect(intersectsBody({ centreM: { x: 0.3, y: 0.2, z: 0 }, radiusM: 0.05 }, [box])).toBe(false);
    // And none beside it.
    expect(intersectsBody({ centreM: { x: 1.5, y: 0, z: 0 }, radiusM: 0.12 }, [box])).toBe(false);
  });

  it("tests a turned box in its own axes", () => {
    const turn = lookAlong(normalise({ x: 1, y: 1, z: 0 }), { x: 0, y: 0, z: 1 });
    const box = craftBox({ x: 0, y: 0, z: 0 }, (v) => rotate(turn, v), {
      centreM: { x: 0, y: 0, z: 0 },
      halfExtentsM: [2, 0.5, 0.5],
    });
    const along = rotate(turn, { x: 1.9, y: 0, z: 0 });
    const across = rotate(turn, { x: 0, y: 1.9, z: 0 });
    expect(meetsBox({ centreM: along, radiusM: 0.01 }, box)).toBe(true);
    expect(meetsBox({ centreM: across, radiusM: 0.01 }, box)).toBe(false);
  });

  it("takes only the bodies whose boxes come within 0.2 m of the ground", () => {
    const ground = datumGroundUnder({ equatorialRadiusM: 1e6, polarRadiusM: 1e6 });
    const at = (heightM: number): ScatterBox => ({
      centreM: { x: 0, y: 0, z: 1e6 + heightM + 0.5 },
      axes: AXES,
      halfExtentsM: [1, 1, 0.5],
    });
    expect(nearGround(at(0), ground)).toBe(true);
    expect(nearGround(at(0.1), ground)).toBe(true);
    expect(nearGround(at(SCATTER_CULL_CLEARANCE_M - 1e-6), ground)).toBe(true);
    expect(nearGround(at(0.3), ground)).toBe(false);
    // Below the ground counts.
    expect(nearGround(at(-0.4), ground)).toBe(true);
    // A tilted box reaches lower by its corner.
    const tilted = craftBox(
      { x: 0, y: 0, z: 1e6 + 0.8 },
      (v) => rotate(lookAlong(normalise({ x: 1, y: 0, z: 1 }), { x: 0, y: 1, z: 0 }), v),
      { centreM: { x: 0, y: 0, z: 0 }, halfExtentsM: [0.5, 0.5, 0.5] },
    );
    expect(boxReachM(tilted, { x: 0, y: 0, z: 1 })).toBeCloseTo(Math.SQRT2 * 0.5, 9);
    expect(nearGround(tilted, ground)).toBe(true);
  });

  it("boxes a hull's vertices in its own axes", () => {
    const box = hullBox(TEST_HULL);
    for (const v of TEST_HULL.vertices) {
      [v.x - box.centreM.x, v.y - box.centreM.y, v.z - box.centreM.z].forEach((d, k) => {
        expect(Math.abs(d)).toBeLessThanOrEqual((box.halfExtentsM[k] ?? 0) + 1e-12);
      });
    }
    expect(hullBox({ vertices: [] }).halfExtentsM).toEqual([0, 0, 0]);
  });

  it("packs at most eight boxes as the kernel's ScatterBox, zeros past them", () => {
    const out = new Float32Array((MAX_SCATTER_BODIES * SCATTER_BOX_BYTES) / 4);
    out.fill(7);
    const boxes = Array.from({ length: 10 }, (_, n) => hovering(n));
    expect(packScatterBoxes(boxes, out)).toBe(MAX_SCATTER_BODIES);
    expect(Array.from(out.subarray(0, 16))).toEqual([
      0, 0, 0.5, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0.5,
    ]);
    expect(packScatterBoxes(boxes.slice(0, 1), out)).toBe(1);
    expect(out.subarray(16).every((x) => x === 0)).toBe(true);
  });
});
