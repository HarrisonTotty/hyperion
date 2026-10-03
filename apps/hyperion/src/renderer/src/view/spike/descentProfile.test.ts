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
  type DescentTerrain,
  DescentUnclearable,
  FLOOR_TOLERANCE_M,
  type TrackStretch,
  landingSiteOf,
  trackStretches,
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

/**
 * FNV-1a over the bits of every 64 Hz pose's position and clearance: a profile's fingerprint, to
 * pin today's profile against the code that made it.
 */
function poseHash(profile: DescentProfile): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = (1n << 64n) - 1n;
  const view = new DataView(new ArrayBuffer(8));
  const steps = Math.round(profile.durationS * 64);
  for (let n = 0; n <= steps; n += 1) {
    const pose = profile.poseAt(n / 64);
    for (const value of [pose.positionM.x, pose.positionM.y, pose.positionM.z, pose.clearanceM]) {
      view.setFloat64(0, value);
      hash = ((hash ^ view.getBigUint64(0)) * prime) & mask;
    }
  }
  return hash.toString(16).padStart(16, "0");
}

/** Every 64 Hz sample time of a profile. */
function grid(profile: DescentProfile): number[] {
  return Array.from({ length: Math.round(profile.durationS * 64) + 1 }, (_, n) => n / 64);
}

describe("the descent over the stretches' floors (decision-r05-descent-clearance.md)", () => {
  const site = landingSiteOf(7n);
  const SITE_M = -1845.8;
  const plain = new DescentProfile(WGS84_FIGURE, site, { siteHeightM: SITE_M });
  const stretches = trackStretches(plain);
  const index = (piece: string): number => stretches.findIndex((s) => s.piece === piece);
  /** Floors at the site, with `raised` pieces' floors that many metres above it. */
  const floors = (raised: Readonly<Record<string, number>>): number[] =>
    stretches.map(({ piece }) => SITE_M + (raised[piece] ?? 0));
  const RIDGES: Readonly<Record<string, number>> = {
    "approach and flare 9": 2000,
    "approach and flare 10": 2000,
    "approach and flare 11": 2000,
    "slowdown 3": 800,
    "final approach": 180,
  };
  const rough = new DescentProfile(WGS84_FIGURE, site, {
    siteHeightM: SITE_M,
    stretchMaxHeightsM: floors(RIDGES),
  });

  it("plans the same stretches for every terrain, tiling the track from the orbit to the site", () => {
    expect(trackStretches(rough)).toEqual(stretches);
    expect(stretches[0]?.fromRemainingM).toBeGreaterThan(4e6);
    expect(stretches.at(-1)?.toRemainingM).toBe(0);
    for (let k = 1; k < stretches.length; k += 1) {
      expect(stretches[k]?.fromRemainingM).toBe(stretches[k - 1]?.toRemainingM);
      expect(stretches[k]?.startS).toBe(stretches[k - 1]?.endS);
      expect(Number.isInteger(stretches[k]?.startS)).toBe(true);
    }
    expect(stretches.map(({ piece }) => piece)).toContain("slowdown 5");
    expect(stretches[index("final approach")]).toMatchObject({ level: 16, clearanceM: 100 });
    expect(stretches[index("final approach")]?.fromRemainingM).toBeCloseTo(250, 6);
    expect(stretches[index("low fast pass")]).toMatchObject({ level: 14, clearanceM: 300 });
    expect(stretches[index("approach and flare 1")]).toMatchObject({ level: 12, clearanceM: 200 });
    expect(stretches[index("approach and flare 12")]).toMatchObject({ level: 14, clearanceM: 200 });
    expect(stretches[index("descent arc 9")]).toMatchObject({ level: 4, clearanceM: 1000 });
  });

  it("is the profile before the ruling, bit for bit, where no floor lifts it", () => {
    // Fingerprints of the 64 Hz poses that 003a6a3's profile (before the ruling) gave. A lifted low
    // pass (the default track maximum, 0 m, above a site below the datum) now holds level, so
    // those profiles changed on purpose.
    const before: ReadonlyArray<readonly [DescentTerrain, string]> = [
      [{}, "97246927f5515e5c"],
      [{ siteHeightM: 1845.8 }, "0464f94febbca6d9"],
      [{ siteHeightM: 1200, trackMaxHeightM: 1000 }, "4a035319bc790f00"],
    ];
    for (const [terrain, hash] of before) {
      expect(poseHash(new DescentProfile(WGS84_FIGURE, site, terrain))).toBe(hash);
    }
  });

  it("is today's profile, bit for bit, with every floor at the site's height", () => {
    const atSite = new DescentProfile(WGS84_FIGURE, site, {
      siteHeightM: SITE_M,
      stretchMaxHeightsM: floors({}),
    });
    const today = new DescentProfile(WGS84_FIGURE, site, {
      siteHeightM: SITE_M,
      trackMaxHeightM: SITE_M,
    });
    expect(atSite.segments).toEqual(today.segments);
    for (const t of grid(today)) {
      expect(atSite.poseAt(t).positionM).toEqual(today.poseAt(t).positionM);
    }
    expect(atSite.minFloorMarginM).toBeGreaterThanOrEqual(-FLOOR_TOLERANCE_M);
  });

  it("keeps every pose its piece's clearance above the floor", () => {
    expect(rough.minFloorMarginM).toBeGreaterThanOrEqual(-FLOOR_TOLERANCE_M);
    for (const [k, stretch] of rough.stretches.entries()) {
      for (let t = stretch.startS; t <= stretch.endS; t += 1 / 64) {
        const pose = rough.poseAt(t);
        const floorM = floors(RIDGES)[k] ?? NaN;
        expect(pose.altitudeM - floorM).toBeGreaterThanOrEqual(
          stretch.clearanceM - FLOOR_TOLERANCE_M,
        );
      }
      const inside = rough.poseAt((stretch.startS + stretch.endS) / 2);
      expect(inside.floorM).toBe(floors(RIDGES)[k]);
      expect(inside.heightAboveFloorM).toBeCloseTo(inside.altitudeM - inside.floorM, 6);
    }
    const end = rough.poseAt(rough.durationS);
    expect(end.floorM).toBe(SITE_M);
    expect(end.heightAboveFloorM).toBeCloseTo(1, 9);
  });

  it("keeps position and velocity continuous over the lifted pieces", () => {
    const h = 1e-4;
    const edges = [
      ...rough.stretches.flatMap(({ endS }) => [endS, endS - 1, endS - 5]),
      ...rough.segmentSpans().map(({ endS }) => endS),
    ];
    for (const t of edges.filter((e) => e > h && e < rough.durationS - h)) {
      const before = rough.poseAt(t - h);
      const after = rough.poseAt(t + h);
      expect(norm(sub(after.positionM, before.positionM))).toBeLessThan(2);
      expect(norm(sub(after.velocityMps, before.velocityMps))).toBeLessThan(0.05);
    }
  });

  it("keeps the ground track and the horizontal speed whatever the floors", () => {
    for (const t of grid(plain)) {
      const a = plain.poseAt(t);
      const b = rough.poseAt(t);
      expect(b.groundDir).toEqual(a.groundDir);
      expect(b.horizontalSpeedMps).toBe(a.horizontalSpeedMps);
    }
  });

  it("flies the low pass level at its floor plus 300 m where its neighbours do not bind", () => {
    const pass = new DescentProfile(WGS84_FIGURE, site, {
      siteHeightM: SITE_M,
      stretchMaxHeightsM: floors({ "low fast pass": 500 }),
    });
    const span = pass.segmentSpans().find(({ name }) => name === "low fast pass");
    const levelM = SITE_M + 800;
    const heights: number[] = [];
    for (let t = span?.startS ?? NaN; t <= (span?.endS ?? NaN); t += 1 / 64) {
      heights.push(pass.poseAt(t).altitudeM);
    }
    expect(Math.min(...heights)).toBeCloseTo(levelM, 3);
    expect(Math.max(...heights) - Math.min(...heights)).toBeLessThanOrEqual(1e-3);
  });

  it("stretches the vertical descent from a raised top, keeping its 20 m/s", () => {
    const span = rough.segmentSpans().find(({ name }) => name === "vertical descent");
    const top = rough.poseAt(span?.startS ?? NaN);
    expect(top.clearanceM).toBeCloseTo(280, 6);
    expect((span?.endS ?? NaN) - (span?.startS ?? NaN)).toBeCloseTo((10 * 278) / 198, 9);
    for (let t = (span?.startS ?? NaN) + 0.25; t < (span?.endS ?? NaN) - 1; t += 0.25) {
      expect(rough.poseAt(t).verticalSpeedMps).toBeGreaterThan(-21);
      expect(rough.poseAt(t).verticalSpeedMps).toBeLessThan(-19);
    }
  });

  it("leaves the last 50 s above the site as they were", () => {
    for (let x = 0; x <= 50; x += 1 / 64) {
      const a = plain.poseAt(plain.durationS - x);
      const b = rough.poseAt(rough.durationS - x);
      expect(b.clearanceM).toBeCloseTo(a.clearanceM, 9);
      expect(norm(sub(b.positionM, a.positionM))).toBeLessThan(1e-6);
    }
  });

  /** The follow-up's cases: a lifted low pass beside a binding slowdown 3, and a valley. */
  const FOLLOW_UP: Readonly<Record<string, Readonly<Record<string, number>>>> = {
    ridges: RIDGES,
    "lifted low pass": { "low fast pass": 1500, "slowdown 3": 800 },
    valley: { "slowdown 2": 800, "slowdown 5": 800 },
  };

  for (const [name, raised] of Object.entries(FOLLOW_UP)) {
    it(`has no valley inside a segment, clears every floor and stays continuous (${name})`, () => {
      const profile = new DescentProfile(WGS84_FIGURE, site, {
        siteHeightM: SITE_M,
        stretchMaxHeightsM: floors(raised),
      });
      expect(profile.minFloorMarginM).toBeGreaterThanOrEqual(-FLOOR_TOLERANCE_M);
      for (const span of profile.segmentSpans()) {
        if (!profile.stretches.some(({ segment }) => segment === span.name)) {
          continue;
        }
        const heights: number[] = [];
        for (let t = span.startS; t <= span.endS; t += 1 / 64) {
          heights.push(profile.poseAt(t).clearanceM);
        }
        // Running maxima from each end: a pose below both is in a valley.
        const after = [...heights];
        for (let i = after.length - 2; i >= 0; i -= 1) {
          after[i] = Math.max(after[i] ?? -Infinity, after[i + 1] ?? -Infinity);
        }
        let before = -Infinity;
        let worst = Infinity;
        for (const [i, h] of heights.entries()) {
          before = Math.max(before, h);
          worst = Math.min(worst, h - Math.min(before, after[i] ?? h));
        }
        expect({ segment: span.name, ok: worst >= -0.5 }).toEqual({ segment: span.name, ok: true });
      }
      for (const t of profile.stretches.map(({ endS }) => endS).slice(0, -1)) {
        const a = profile.poseAt(t - 1e-4);
        const b = profile.poseAt(t + 1e-4);
        expect(norm(sub(b.positionM, a.positionM))).toBeLessThan(2);
        expect(norm(sub(b.velocityMps, a.velocityMps))).toBeLessThan(0.05);
      }
    });
  }

  it("descends from a lifted low pass to a binding slowdown 3 without dropping to the table", () => {
    const profile = new DescentProfile(WGS84_FIGURE, site, {
      siteHeightM: SITE_M,
      stretchMaxHeightsM: floors(FOLLOW_UP["lifted low pass"] ?? {}),
    });
    const at = (piece: string): TrackStretch | undefined =>
      profile.stretches.find((s) => s.piece === piece);
    const levelM = profile.poseAt(at("low fast pass")?.startS ?? NaN).clearanceM;
    expect(levelM).toBeCloseTo(1800, 6);
    const third = profile.poseAt(at("slowdown 3")?.startS ?? NaN).clearanceM;
    // Its floor plus C, or the slowdown's shape re-anchored to the lifted start where higher.
    expect(third).toBeGreaterThanOrEqual(1000 - FLOOR_TOLERANCE_M);
    const boundaries = ["slowdown 1", "slowdown 2"].flatMap((piece) => [
      profile.poseAt(at(piece)?.startS ?? NaN).clearanceM,
      profile.poseAt(at(piece)?.endS ?? NaN).clearanceM,
    ]);
    for (const [i, h] of boundaries.entries()) {
      expect(h).toBeGreaterThanOrEqual(Math.min(levelM, third) - FLOOR_TOLERANCE_M);
      expect(h).toBeLessThanOrEqual((boundaries[i - 1] ?? Infinity) + FLOOR_TOLERANCE_M);
    }
  });

  it("names a floor it cannot clear by its own RangeError", () => {
    const error = new DescentUnclearable("the descent cannot clear its floors");
    expect(error).toBeInstanceOf(RangeError);
    expect(error.name).toBe("DescentUnclearable");
  });

  // The fourth lift's RangeError is a guard: each lift raises a short piece's two boundaries, and so
  // the whole piece, by its deficit, and no floor met in testing needed a second round.
  it("refuses floors that do not match the plan, or are not finite", () => {
    expect(
      () => new DescentProfile(WGS84_FIGURE, site, { stretchMaxHeightsM: floors({}).slice(1) }),
    ).toThrow(RangeError);
    expect(
      () =>
        new DescentProfile(WGS84_FIGURE, site, {
          stretchMaxHeightsM: floors({ "slowdown 2": Infinity }),
        }),
    ).toThrow(RangeError);
  });
});
