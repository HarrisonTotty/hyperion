import { describe, expect, it } from "vitest";

import { normalise, norm, sub } from "../../geometry/vec3";
import { surfacePoint } from "../terrain/planet";
import { rotateToBody } from "../coords/rotation";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import {
  blendS,
  datumDirection,
  DESCENT_SEGMENTS,
  DescentProfile,
  landingSiteOf,
} from "./descentProfile";
import { TEST_PLANET_RATE_RAD_PER_S, testPlanetRotationAt } from "./rotation";

const PROFILE = new DescentProfile(WGS84_FIGURE, landingSiteOf(7n));

describe("the scripted descent", () => {
  it("lasts the sum of Design note 19's durations", () => {
    expect(PROFILE.durationS).toBe(DESCENT_SEGMENTS.reduce((sum, s) => sum + s.durationS, 0));
    expect(PROFILE.durationS).toBe(1230);
  });

  it("meets every segment's boundary altitudes and speeds, blends included", () => {
    for (const span of PROFILE.segmentSpans()) {
      const segment = DESCENT_SEGMENTS.find(({ name }) => name === span.name);
      const start = PROFILE.poseAt(span.startS);
      const end = PROFILE.poseAt(span.endS);
      expect(start.altitudeM).toBeCloseTo(segment?.startAltitudeM ?? NaN, 6);
      expect(end.altitudeM).toBeCloseTo(segment?.endAltitudeM ?? NaN, 6);
      expect(start.horizontalSpeedMps).toBeCloseTo(segment?.startSpeedMps ?? NaN, 9);
      expect(end.horizontalSpeedMps).toBeCloseTo(segment?.endSpeedMps ?? NaN, 9);
    }
  });

  it("keeps position and velocity continuous across every boundary and blend", () => {
    const edges = PROFILE.segmentSpans().flatMap(({ endS, name }) => {
      const segment = DESCENT_SEGMENTS.find((s) => s.name === name);
      return [endS, endS - blendS(segment?.durationS ?? 0)];
    });
    const h = 1e-4;
    for (const t of edges.filter((e) => e > h && e < PROFILE.durationS - h)) {
      const before = PROFILE.poseAt(t - h);
      const after = PROFILE.poseAt(t + h);
      // A step of 0.2 ms moves the camera at most 7.67 km/s × 0.2 ms ≈ 1.6 m.
      expect(norm(sub(after.positionM, before.positionM))).toBeLessThan(2);
      expect(norm(sub(after.velocityMps, before.velocityMps))).toBeLessThan(0.05);
      expect(Math.abs(after.verticalSpeedMps - before.verticalSpeedMps)).toBeLessThan(0.05);
      expect(Math.abs(after.horizontalSpeedMps - before.horizontalSpeedMps)).toBeLessThan(0.01);
    }
  });

  it("gives a velocity whose size is the ground-track and vertical speeds' near the ground", () => {
    // The horizontal speed is the ground track's; a camera h above it moves (R + h) ÷ R faster, so
    // the check is made where h ≪ R.
    for (const t of [1100, 1130, 1150, 1225]) {
      const pose = PROFILE.poseAt(t);
      const speed = Math.hypot(pose.horizontalSpeedMps, pose.verticalSpeedMps);
      // The track's arc is measured on the mean of a and c, so the ground speed is within 0.4%.
      expect(Math.abs(norm(pose.velocityMps) - speed)).toBeLessThanOrEqual(0.004 * speed + 1e-6);
    }
  });

  it("is identical call to call at a fixed seed, and two seeds give two sites", () => {
    const again = new DescentProfile(WGS84_FIGURE, landingSiteOf(7n));
    expect(again.poseAt(812.25)).toEqual(PROFILE.poseAt(812.25));
    expect(landingSiteOf(7n)).toEqual(landingSiteOf(7n));
    const other = new DescentProfile(WGS84_FIGURE, landingSiteOf(8n));
    expect(
      norm(sub(other.poseAt(1230).groundPointM, PROFILE.poseAt(1230).groundPointM)),
    ).toBeGreaterThan(1e5);
  });

  it("ends at a metre above the landing site, looking straight down", () => {
    const end = PROFILE.poseAt(PROFILE.durationS);
    expect(end.altitudeM).toBeCloseTo(1, 9);
    expect(norm(sub(end.positionM, end.groundPointM))).toBeCloseTo(1, 6);
    expect(end.segment).toBe("hover and touchdown");
  });

  it("holds a hovering pose still in the body-fixed axes while its body-frame position moves at ω × r", () => {
    const tS = 1229;
    const a = PROFILE.poseAt(tS);
    const b = PROFILE.poseAt(tS + 0.5);
    expect(norm(sub(b.positionM, a.positionM))).toBe(0);
    const inBodyA = rotateToBody(testPlanetRotationAt(tS), a.positionM);
    const inBodyB = rotateToBody(testPlanetRotationAt(tS + 0.5), b.positionM);
    const axisDistanceM = Math.hypot(a.positionM.x, a.positionM.y);
    const speed = norm(sub(inBodyB, inBodyA)) / 0.5;
    expect(speed).toBeCloseTo(TEST_PLANET_RATE_RAD_PER_S * axisDistanceM, 3);
  });

  it("flies above the site's terrain and the low pass above the track's highest", () => {
    const site = landingSiteOf(7n);
    const raised = new DescentProfile(WGS84_FIGURE, site, {
      siteHeightM: 1200,
      trackMaxHeightM: 1500,
    });
    const end = raised.poseAt(raised.durationS);
    expect(end.altitudeM).toBeCloseTo(1201, 6);
    expect(end.clearanceM).toBeCloseTo(1, 9);
    expect(norm(sub(end.positionM, end.groundPointM))).toBeCloseTo(1, 6);
    const pass = raised.segmentSpans().find(({ name }) => name === "low fast pass");
    // It starts at 300 m above the track's highest and climbs gently, never below, so that its
    // blend into the slowdown leaves the slowdown's start exact (the re-fit of Design note 19).
    expect(raised.poseAt(pass?.startS ?? NaN).altitudeM).toBeCloseTo(1800, 6);
    for (let t = pass?.startS ?? NaN; t <= (pass?.endS ?? NaN); t += 0.5) {
      expect(raised.poseAt(t).altitudeM).toBeGreaterThanOrEqual(1800 - 1e-6);
    }
    // The ground track does not move with the terrain.
    const flat = PROFILE.poseAt(600);
    const lifted = raised.poseAt(600);
    expect(norm(sub(lifted.groundPointM, flat.groundPointM))).toBeCloseTo(1200, 3);
  });

  it("stands every pose over its ground direction d, the spheroid point M·d's", () => {
    const raised = new DescentProfile(WGS84_FIGURE, landingSiteOf(7n), { siteHeightM: -1845 });
    for (const t of [30, 600, 1000, 1100, 1150, 1225, 1230]) {
      const pose = raised.poseAt(t);
      const d = [pose.groundDir.x, pose.groundDir.y, pose.groundDir.z] as const;
      const ground = surfacePoint(WGS84_FIGURE, d, -1845);
      expect(
        norm(sub(pose.groundPointM, { x: ground[0], y: ground[1], z: ground[2] })),
      ).toBeLessThan(1e-6);
      expect(norm(pose.groundDir)).toBeCloseTo(1, 15);
    }
    expect(raised.poseAt(raised.durationS).groundDir).toEqual(raised.siteDir);
  });

  it("recovers d from a spheroid point, where the geocentric direction is off by up to f ÷ 2", () => {
    const d = PROFILE.siteDir;
    const p = surfacePoint(WGS84_FIGURE, [d.x, d.y, d.z], 0);
    const point = { x: p[0], y: p[1], z: p[2] };
    expect(norm(sub(datumDirection(WGS84_FIGURE, point), d))).toBeLessThan(1e-15);
    // p ÷ |p| is off by about f sin 2β ÷ 2: 0.036° at seed 7's site, 4 km along the ground.
    const offRad = norm(sub(normalise(point), d));
    expect(offRad).toBeGreaterThan(5e-4);
    expect(offRad).toBeLessThan(1 / 298.257223563 / 2);
  });

  it("refuses a seed outside u64", () => {
    expect(() => landingSiteOf(-1n)).toThrow(RangeError);
    expect(() => landingSiteOf(1n << 64n)).toThrow(RangeError);
  });
});
