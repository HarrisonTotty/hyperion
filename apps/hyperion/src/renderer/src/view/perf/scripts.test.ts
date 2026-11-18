import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { rotate } from "../camera/quaternion";
import { DescentProfile, landingSiteOf } from "../spike/descentProfile";
import { TEST_PLANET_FIGURE } from "../spike/testPlanetFigure";
import {
  APPROACH_BOUNDARY_M,
  APPROACH_DURATION_S,
  APPROACH_END_HEIGHT_M,
  APPROACH_START_DISTANCE_M,
  ARRIVAL_HELD_S,
  arrivalHold,
  AU_M,
  nuclearDiscSweep,
  type PerfPose,
  type PerfScript,
  ringedGiantApproach,
  SWEEP_DURATION_S,
  SWEEP_LENGTH_M,
} from "./scripts";

/** Saturn's main rings, the C ring's inner edge to the A ring's outer, m (NASA's Saturnian Rings Fact Sheet). */
const SATURN_RINGS = { ringInnerM: 74_658e3, ringOuterM: 136_775e3 };

/** R05's fixed-step mode's step, s: one frame at 60 Hz (`fixedStep.ts`). */
const STEP_S = 1 / 60;

function length(v: Vec3): number {
  return Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
}

/** Every number of a pose, in a fixed order, to compare two to the bit. */
function numbers(pose: PerfPose): ReadonlyArray<number> {
  const { positionM: p, orientation: q } = pose;
  return [pose.tS, p.x, p.y, p.z, q.w, q.x, q.y, q.z];
}

/** Whether two evaluations of `script` at each fixed step agree to the bit. */
function agreeToTheBit(make: () => PerfScript): boolean {
  const first = make();
  const second = make();
  const steps = Math.ceil(first.durationS / STEP_S) + 2;
  for (let k = 0; k <= steps; k += 1) {
    const a = numbers(first.poseAt(k * STEP_S));
    const b = numbers(second.poseAt(k * STEP_S));
    const again = numbers(first.poseAt(k * STEP_S));
    if (!a.every((value, i) => Object.is(value, b[i]) && Object.is(value, again[i]))) {
      return false;
    }
  }
  return true;
}

/** The camera's look direction, its −z axis, in the script's frame. */
function lookOf(pose: PerfPose): Vec3 {
  return rotate(pose.orientation, { x: 0, y: 0, z: -1 });
}

/** R05's descent over its test planet from seed 7's site. */
function descent(): DescentProfile {
  return new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(7n));
}

describe("the perf scenes' scripts", () => {
  it("agree to the bit at every fixed step, evaluated twice", () => {
    expect(agreeToTheBit(() => ringedGiantApproach(SATURN_RINGS))).toBe(true);
    expect(agreeToTheBit(() => nuclearDiscSweep())).toBe(true);
    const [a, b] = [arrivalHold(descent()), arrivalHold(descent())];
    expect([...numbers(a.arrival), ...numbers(a.deepest)]).toEqual([
      ...numbers(b.arrival),
      ...numbers(b.deepest),
    ]);
  });

  it("approach the giant from 10¹⁰ m to inside its annulus in 300 s, across 10⁹ m", () => {
    const script = ringedGiantApproach(SATURN_RINGS);
    expect(script.durationS).toBe(APPROACH_DURATION_S);
    expect(script.frame).toBe("body-fixed");
    const start = script.poseAt(0);
    expect(length(start.positionM)).toBeCloseTo(APPROACH_START_DISTANCE_M, -2);
    const end = script.poseAt(APPROACH_DURATION_S);
    const inPlane = Math.hypot(end.positionM.x, end.positionM.y);
    expect(inPlane).toBeGreaterThan(SATURN_RINGS.ringInnerM);
    expect(inPlane).toBeLessThan(SATURN_RINGS.ringOuterM);
    expect(end.positionM.z).toBeCloseTo(APPROACH_END_HEIGHT_M, 3);
    expect(script.segments.map(({ name }) => name)).toEqual(["far", "near", "annulus"]);
    const near = script.segments[1];
    expect(near).toBeDefined();
    expect(length(script.poseAt(near?.startS ?? 0).positionM)).toBeCloseTo(APPROACH_BOUNDARY_M, -3);
    let last = Infinity;
    for (let t = 0; t <= APPROACH_DURATION_S; t += 1) {
      const distance = length(script.poseAt(t).positionM);
      expect(distance).toBeLessThanOrEqual(last);
      last = distance;
    }
  });

  it("look at the giant's centre throughout the approach", () => {
    const script = ringedGiantApproach(SATURN_RINGS);
    for (const t of [0, 75, 150, 225, APPROACH_DURATION_S]) {
      const pose = script.poseAt(t);
      const look = lookOf(pose);
      const inward = length(pose.positionM);
      expect(look.x).toBeCloseTo(-pose.positionM.x / inward, 9);
      expect(look.z).toBeCloseTo(-pose.positionM.z / inward, 9);
    }
  });

  it("refuse rings outside 10⁹ m or with their edges the wrong way round", () => {
    expect(() => ringedGiantApproach({ ringInnerM: 2e9, ringOuterM: 3e9 })).toThrow(RangeError);
    expect(() => ringedGiantApproach({ ringInnerM: 2e8, ringOuterM: 1e8 })).toThrow(RangeError);
  });

  it("hold the descent's first pose for the arrival and its deepest a minute after", () => {
    const profile = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(7n));
    const hold = arrivalHold(profile);
    expect(hold.heldS).toBe(ARRIVAL_HELD_S);
    expect(hold.arrival.positionM).toEqual(profile.poseAt(0).positionM);
    expect(hold.deepest.positionM).toEqual(profile.poseAt(profile.durationS).positionM);
    expect(profile.poseAt(profile.durationS).clearanceM).toBeCloseTo(1, 6);
  });

  it("sweep 300 au about the nuclear disc's barycentre in 120 s, turning once", () => {
    const script = nuclearDiscSweep();
    expect(script.durationS).toBe(SWEEP_DURATION_S);
    expect(script.frame).toBe("system");
    const start = script.poseAt(0);
    const end = script.poseAt(SWEEP_DURATION_S);
    expect(SWEEP_LENGTH_M).toBe(300 * AU_M);
    expect(start.positionM.x).toBeCloseTo(-150 * AU_M, -3);
    expect(end.positionM.x).toBeCloseTo(150 * AU_M, -3);
    const half = lookOf(script.poseAt(SWEEP_DURATION_S / 2));
    expect(half.x).toBeCloseTo(-1, 9);
    expect(lookOf(start).x).toBeCloseTo(1, 9);
  });
});
