import { describe, expect, it } from "vitest";

import { cross, dot, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { aBody, aViewScene, FIXTURE_PLANET, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { rotate } from "../camera/quaternion";
import { bodyKindSymbol, type ViewRing, type ViewScene } from "../scene/model";
import { barycentrePlacement, type PlacedGiant, placedPose, ringedGiantIn } from "./scriptedCamera";
import {
  APPROACH_END_HEIGHT_M,
  APPROACH_START_DISTANCE_M,
  APPROACH_START_ELEVATION_RAD,
  nuclearDiscSweep,
  ringedGiantApproach,
} from "./scripts";

const AU_M = 149_597_870_700;

/** The giant's ring plane's normal, tilted 30° from galactic z toward galactic y. */
const NORMAL = vec3(0, Math.sin(Math.PI / 6), Math.cos(Math.PI / 6));

/** A ring of the fixture's planet, its giant here. */
function aRing(innerRadiusM: number, outerRadiusM: number, body = FIXTURE_PLANET): ViewRing {
  return { body, innerRadiusM, outerRadiusM, normal: NORMAL };
}

/** A system of a star, at the barycentre unless given, and the giant 5 au out along galactic x. */
function giantScene(rings: ReadonlyArray<ViewRing>, starM: Vec3 = vec3(0, 0, 0)): ViewScene {
  return aViewScene({
    bodies: [
      aBody({
        id: `${FIXTURE_SYSTEM}.0000`,
        kind: "star",
        designation: "TEST STAR",
        radiusM: 6.957e8,
        hillRadiusM: null,
        centreM: starM,
        symbol: bodyKindSymbol("star"),
      }),
      aBody({ radiusM: 7.0e7, centreM: vec3(5 * AU_M, 0, 0) }),
    ],
    rings,
  });
}

/** The giant placed in `scene`, failing the test where it cannot be. */
function placed(scene: ViewScene): Extract<PlacedGiant, { readonly kind: "placed" }> {
  const found = ringedGiantIn(scene, FIXTURE_PLANET);
  if (found.kind === "gap") {
    throw new Error(found.reason);
  }
  return found;
}

/** Whether `a` and `b` agree within `tolerance` in each component. */
function near(a: Vec3, b: Vec3, tolerance: number): boolean {
  return norm(sub(a, b)) <= tolerance;
}

describe("the approach's giant in a served scene", () => {
  it("takes its widest ring's annulus, leaving other bodies' rings", () => {
    const scene = giantScene([
      aRing(8.0e7, 9.0e7),
      aRing(9.5e7, 1.4e8),
      aRing(1e7, 5e8, `${FIXTURE_SYSTEM}.0104`),
    ]);
    expect(placed(scene).giant).toEqual({ ringInnerM: 9.5e7, ringOuterM: 1.4e8 });
  });

  it("stands the script's z on the ring's normal and its x toward the star in the ring plane", () => {
    const [x, y, z] = placed(giantScene([aRing(9.5e7, 1.4e8)])).placement.axes;
    expect(near(z, NORMAL, 1e-15)).toBe(true);
    expect(Math.abs(dot(x, z))).toBeLessThan(1e-15);
    expect(norm(x)).toBeCloseTo(1, 15);
    // The star lies along −x from the giant: x is −x less its part along the normal.
    expect(dot(x, vec3(-1, 0, 0))).toBeGreaterThan(0.99);
    expect(near(y, cross(z, x), 1e-15)).toBe(true);
  });

  it("is placed about the giant in its own frame", () => {
    expect(placed(giantScene([aRing(9.5e7, 1.4e8)])).placement.frame).toEqual({
      kind: "body",
      body: FIXTURE_PLANET,
    });
  });

  it("names why a scene cannot be flown: no giant, or a giant with no ring", () => {
    expect(ringedGiantIn(giantScene([]), FIXTURE_PLANET)).toEqual({
      kind: "gap",
      reason: `the served scene gives ${FIXTURE_PLANET} no ring`,
      shown: "the served scene gives the pinned giant no ring, re-pin the scene",
    });
    expect(ringedGiantIn(giantScene([]), `${FIXTURE_SYSTEM}.0400`)).toEqual({
      kind: "gap",
      reason: `the served scene holds no body ${FIXTURE_SYSTEM}.0400`,
      shown: "the served scene holds no pinned giant, re-pin the scene",
    });
  });

  it("names a widest ring the approach cannot end inside, reaching past 10⁹ m", () => {
    expect(ringedGiantIn(giantScene([aRing(9.5e7, 2e9)]), FIXTURE_PLANET)).toEqual({
      kind: "gap",
      reason: `${FIXTURE_PLANET}'s widest ring, 95000000 m to 2000000000 m, is no annulus inside 10⁹ m`,
      shown: "the pinned giant's widest ring is too wide for the approach, re-pin the scene",
    });
  });

  it("turns the ring's normal to the star's side, so that the approach starts over the lit face", () => {
    // The star 1 au below the ring plane's side its normal points to.
    const { giant, placement } = placed(giantScene([aRing(9.5e7, 1.4e8)], vec3(0, 0, -AU_M)));
    const start = placedPose(ringedGiantApproach(giant).poseAt(0), placement).positionM;
    expect(dot(start, NORMAL)).toBeLessThan(0);
    expect(dot(placement.axes[2], NORMAL)).toBeCloseTo(-1, 15);
  });

  it("is placed the same to the bit from the same scene", () => {
    const scene = giantScene([aRing(9.5e7, 1.4e8)]);
    expect(placed(scene)).toStrictEqual(placed(scene));
  });
});

describe("a script's pose in the scene", () => {
  const { giant, placement } = placed(giantScene([aRing(9.5e7, 1.4e8)]));
  const script = ringedGiantApproach(giant);
  const [x, , z] = placement.axes;

  it("starts the approach 10¹⁰ m out, 20° above the sunlit face of the rings, looking at the giant", () => {
    const pose = placedPose(script.poseAt(0), placement);
    const p = pose.positionM;
    expect(norm(p) / APPROACH_START_DISTANCE_M).toBeCloseTo(1, 12);
    expect(dot(p, z) / norm(p)).toBeCloseTo(Math.sin(APPROACH_START_ELEVATION_RAD), 12);
    expect(dot(p, x)).toBeGreaterThan(0);
    const forward = rotate(pose.orientation, vec3(0, 0, -1));
    expect(near(forward, scale(p, -1 / norm(p)), 1e-12)).toBe(true);
  });

  it("ends the approach inside the annulus, its height above the ring plane", () => {
    const end = placedPose(script.poseAt(script.durationS), placement).positionM;
    const heightM = dot(end, z);
    const radialM = Math.sqrt(dot(end, end) - heightM * heightM);
    expect(heightM).toBeCloseTo(APPROACH_END_HEIGHT_M, 3);
    expect(radialM).toBeGreaterThan(giant.ringInnerM);
    expect(radialM).toBeLessThan(giant.ringOuterM);
  });

  it("leaves the sweep's poses on the galactic axes about the barycentre", () => {
    const sweep = nuclearDiscSweep();
    const pose = sweep.poseAt(42);
    expect(placedPose(pose, barycentrePlacement(FIXTURE_SYSTEM))).toEqual({
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: pose.positionM,
      orientation: pose.orientation,
    });
  });
});
