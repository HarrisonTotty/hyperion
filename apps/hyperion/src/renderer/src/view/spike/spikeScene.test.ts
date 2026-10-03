import { describe, expect, it } from "vitest";

import { dot, normalise } from "../../geometry/vec3";
import { rotateToBody } from "../coords/rotation";
import { isDescending } from "../terrain/grounded";
import { spheroidNormal, spheroidPoint } from "../terrain/planet";
import { datumDirection, DescentProfile, landingSiteOf } from "./descentProfile";
import { testPlanetRotationAt } from "./rotation";
import {
  contactRule,
  GROUNDED_CLEARANCE_M,
  siteDirection,
  SPIKE_CRAFT,
  SPIKE_CRAFT_RADIUS_M,
  SPIKE_PLANET,
  spikeContactAt,
  spikeScene,
  sunDirectionBody,
  SYNTHETIC_FIELD_BYTES,
  syntheticField,
  TEST_PLANET_FIGURE,
} from "./spikeScene";

const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n), {
  siteHeightM: 812.5,
  trackMaxHeightM: 1_400,
});

describe("the craft as a contact", () => {
  const rule = contactRule(PROFILE);

  it("becomes one exactly when it is first descending on the scripted path", () => {
    let descended = false;
    // The script at the fixed step of Design note 19 (64 Hz), up to the first onset.
    for (let n = 0; n / 64 <= (rule.firstOnsetS ?? Number.NaN); n += 1) {
      const t = n / 64;
      const pose = PROFILE.poseAt(t);
      descended ||= isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps);
      expect(spikeContactAt(PROFILE, rule, t) !== null).toBe(descended);
    }
    const first = rule.firstOnsetS ?? Number.NaN;
    const before = PROFILE.poseAt(first - 1e-6);
    const at = PROFILE.poseAt(first);
    expect(isDescending(before.heightAboveFloorM, before.verticalSpeedMps)).toBe(false);
    expect(isDescending(at.heightAboveFloorM, at.verticalSpeedMps)).toBe(true);
    expect(spikeContactAt(PROFILE, rule, first)).not.toBeNull();
  });

  it("is one while descending, and held from its last descent to the end of the script", () => {
    const hold = rule.holdFromS ?? Number.NaN;
    for (let n = 0; n / 4 <= PROFILE.durationS; n += 1) {
      const t = n / 4;
      const pose = PROFILE.poseAt(t);
      const descending = isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps);
      expect(spikeContactAt(PROFILE, rule, t) !== null).toBe(descending || t >= hold);
    }
    expect(spikeContactAt(PROFILE, rule, PROFILE.durationS)).not.toBeNull();
  });

  it("is one while hovering at zero vertical speed near the ground", () => {
    const hover = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n), {
      siteHeightM: 812.5,
    });
    const t = hover.durationS;
    const pose = hover.poseAt(t);
    expect(pose.verticalSpeedMps).toBe(0);
    expect(isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps)).toBe(false);
    // The rule alone, without the hold from the last descent.
    const unheld = { firstOnsetS: null, holdFromS: null };
    expect(spikeContactAt(hover, unheld, t)).not.toBeNull();
    expect(pose.clearanceM).toBeLessThanOrEqual(GROUNDED_CLEARANCE_M);
  });

  it("is released over the level low pass", () => {
    const span = PROFILE.segmentSpans().find((each) => each.name === "low fast pass");
    if (span === undefined) {
      throw new Error("the script has no low fast pass");
    }
    const mid = (span.startS + span.endS) / 2;
    expect(spikeContactAt(PROFILE, rule, mid)).toBeNull();
    expect(rule.holdFromS ?? 0).toBeGreaterThan(span.endS - 5);
  });

  it("stands at the site's terrain beneath the craft with the hull's bounding radius", () => {
    const t = PROFILE.durationS;
    expect(spikeContactAt(PROFILE, rule, t)).toEqual({
      positionM: PROFILE.poseAt(t).groundPointM,
      radiusM: SPIKE_CRAFT_RADIUS_M,
    });
    // The stern's corners, (±4, ±2.5, 10) m from the reference point.
    expect(SPIKE_CRAFT_RADIUS_M).toBeCloseTo(Math.hypot(4, 2.5, 10), 12);
  });
});

describe("the datum direction", () => {
  it("is the bake's direction d of a datum point M·d, not the point's geocentric direction", () => {
    const d = normalise({ x: 0.3, y: -0.5, z: 0.81 });
    const p = spheroidPoint(TEST_PLANET_FIGURE, [d.x, d.y, d.z]);
    const back = datumDirection(TEST_PLANET_FIGURE, { x: p[0], y: p[1], z: p[2] });
    expect(back.x).toBeCloseTo(d.x, 14);
    expect(back.y).toBeCloseTo(d.y, 14);
    expect(back.z).toBeCloseTo(d.z, 14);
    // The geocentric direction of the same point differs, here by about 0.05° in z alone.
    const geocentric = normalise({ x: p[0], y: p[1], z: p[2] });
    expect(Math.abs(geocentric.z - d.z)).toBeGreaterThan(5e-4);
  });

  it("gives the site's own direction for its height query", () => {
    const site = landingSiteOf(5n);
    const datum = new DescentProfile(TEST_PLANET_FIGURE, site);
    const [x, y, z] = siteDirection(datum);
    const ground = datum.poseAt(datum.durationS).groundPointM;
    const p = spheroidPoint(TEST_PLANET_FIGURE, [x, y, z]);
    expect(Math.hypot(p[0] - ground.x, p[1] - ground.y, p[2] - ground.z)).toBeLessThan(1e-6);
  });
});

describe("the light", () => {
  it("stands 29.9° or more above the landing site's horizon at touchdown", () => {
    for (const seed of [1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n]) {
      const profile = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(seed));
      const end = profile.durationS;
      const g = profile.poseAt(end).groundDir;
      const normal = spheroidNormal(TEST_PLANET_FIGURE, [g.x, g.y, g.z]);
      const up = normalise(
        rotateToBody(testPlanetRotationAt(end), { x: normal[0], y: normal[1], z: normal[2] }),
      );
      expect(dot(up, sunDirectionBody(profile))).toBeGreaterThanOrEqual(
        Math.sin((29.9 * Math.PI) / 180),
      );
    }
  });
});

describe("the scene", () => {
  it("turns the planet and flies the craft on the script", () => {
    const t = 1_000;
    const scene = spikeScene(PROFILE).sceneAt(t);
    const planet = scene.bodies.find((body) => body.id === SPIKE_PLANET);
    const craft = scene.craft.find((each) => each.id === SPIKE_CRAFT);
    expect(planet?.rotation).toEqual(testPlanetRotationAt(t));
    expect(craft?.pose.position).toEqual({
      kind: "body_fixed",
      body: SPIKE_PLANET,
      m: PROFILE.poseAt(t).positionM,
    });
    expect(scene.ownShip).toBe(SPIKE_CRAFT);
    expect(scene.provenance).toEqual({ kind: "kept", name: "DESCENT SPIKE" });
  });

  it("puts the planet 1 au from the star, on the side the light comes from", () => {
    const scene = spikeScene(PROFILE).sceneAt(0);
    const centre = scene.bodies.find((body) => body.id === SPIKE_PLANET)?.centreM;
    if (centre === undefined) {
      throw new Error("the scene has no planet");
    }
    expect(Math.hypot(centre.x, centre.y, centre.z)).toBeCloseTo(149_597_870_700, 0);
    expect(dot(normalise(centre), sunDirectionBody(PROFILE))).toBeCloseTo(-1, 12);
  });
});

describe("the synthetic field", () => {
  it("is the brainstorm's 15 MB, every page written", () => {
    const field = new Uint8Array(syntheticField());
    expect(field.length).toBe(SYNTHETIC_FIELD_BYTES);
    expect(SYNTHETIC_FIELD_BYTES).toBe(15_000_000);
    for (let page = 0; page < field.length; page += 4096) {
      expect(field.subarray(page, page + 4096).some((b) => b !== 0)).toBe(true);
    }
  });
});
