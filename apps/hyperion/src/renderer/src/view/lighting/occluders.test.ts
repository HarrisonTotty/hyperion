import { describe, expect, it } from "vitest";

import { universeTimeFromYears } from "@hyperion/protocol";

import { vec3 } from "../../geometry/vec3";
import type { SceneBodyFrame } from "../../lib/scene/apparent";
import { ZERO_SPAN } from "../../lib/scene/lightTime";
import {
  type LightingBody,
  lightingBodyOf,
  occludersFor,
  penumbraRadius,
  umbraRadius,
} from "./occluders";

/** Radii, km: the Sun (IAU 2015 B3 nominal), Earth (mean) and the Moon (mean). */
const SUN_KM = 695_700;
const EARTH_KM = 6_371;
const MOON_KM = 1_737.4;
/** One au and the Moon's mean distance, km. */
const AU_KM = 149_597_870.7;
const MOON_DISTANCE_KM = 384_400;

const KM = 1_000;

describe("the shadow cones' radii", () => {
  it("gives Earth's umbra about 4,600 km across the Moon's orbit", () => {
    expect(umbraRadius(EARTH_KM, SUN_KM, AU_KM, MOON_DISTANCE_KM)).toBeCloseTo(4_599.7, 0);
  });

  it("gives Earth's penumbra about 8,175 km there", () => {
    expect(penumbraRadius(EARTH_KM, SUN_KM, AU_KM, MOON_DISTANCE_KM)).toBeCloseTo(8_175.0, 0);
  });

  it("ends the Moon's umbra short of Earth's centre, so a central eclipse there is annular", () => {
    expect(umbraRadius(MOON_KM, SUN_KM, AU_KM, MOON_DISTANCE_KM)).toBeLessThan(0);
    expect(penumbraRadius(MOON_KM, SUN_KM, AU_KM, MOON_DISTANCE_KM)).toBeCloseTo(3_529.6, 0);
  });

  it("starts both cones at the occluder's own radius", () => {
    expect(umbraRadius(MOON_KM, SUN_KM, AU_KM, 0)).toBe(MOON_KM);
    expect(penumbraRadius(MOON_KM, SUN_KM, AU_KM, 0)).toBe(MOON_KM);
  });
});

describe("occludersFor", () => {
  const sun = { centreM: vec3(0, 0, 0), radiusM: SUN_KM * KM };
  const earth: LightingBody = {
    id: "earth",
    centreM: vec3(AU_KM * KM, 0, 0),
    radiusM: EARTH_KM * KM,
  };
  const moonAt = (y: number): LightingBody => ({
    id: "moon",
    centreM: vec3((AU_KM + MOON_DISTANCE_KM) * KM, y * KM, 0),
    radiusM: MOON_KM * KM,
  });

  it("lists Earth for a Moon in its shadow", () => {
    const moon = moonAt(0);
    expect(occludersFor(moon, [sun], [earth, moon]).map((body) => body.id)).toEqual(["earth"]);
  });

  it("lists Earth while the Moon's limb is still in the penumbra", () => {
    const moon = moonAt(8_175 + MOON_KM - 100);
    expect(occludersFor(moon, [sun], [earth, moon]).map((body) => body.id)).toEqual(["earth"]);
  });

  it("is empty for a Moon clear of every cone", () => {
    const moon = moonAt(8_175 + MOON_KM + 100);
    expect(occludersFor(moon, [sun], [earth, moon])).toEqual([]);
  });

  it("never lists a body on the far side of the lit one", () => {
    const moon = moonAt(0);
    expect(occludersFor(earth, [sun], [earth, moon])).toEqual([]);
  });
});

describe("lightingBodyOf", () => {
  const placed: SceneBodyFrame = {
    kind: "placed",
    id: "0200080020000000.0103",
    geometricM: vec3(1, 2, 3),
    apparentM: vec3(1, 2, 3),
    emitted: universeTimeFromYears(0),
    lightTime: ZERO_SPAN,
    level: "bulk",
    hillRadiusM: null,
  };

  it("places a body at its geometric centre with its radius", () => {
    expect(lightingBodyOf(placed, 6.371e6)).toEqual({
      id: "0200080020000000.0103",
      centreM: vec3(1, 2, 3),
      radiusM: 6.371e6,
    });
  });

  it("leaves out a body of unknown radius", () => {
    expect(lightingBodyOf(placed, null)).toBeNull();
  });

  it("never makes a contact an occluder (decisions-r06-r07, item 4)", () => {
    const contact: SceneBodyFrame = {
      kind: "contact",
      id: "0200080020000000.0104",
      apparentM: vec3(1, 2, 3),
      emitted: universeTimeFromYears(0),
      level: "contact",
    };
    expect(lightingBodyOf(contact, 1e6)).toBeNull();
  });
});
