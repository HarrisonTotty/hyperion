import { describe, expect, it } from "vitest";

import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  aHostDisc,
  MOON_GEOMETRIC_ALBEDO,
  photometryFor,
  planetPhotometry,
} from "../../test/litFixtures";
import type { BodyFigure } from "../appearance/fromWire";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import {
  type LightAtPoint,
  lightsAt,
  MAX_BODY_LIGHTS,
  type PlacedLight,
} from "../lighting/hostLights";
import { photopicIlluminance, STAR_CUT_RELATIVE } from "../lighting/illuminance";
import {
  litNeighbours,
  planetshineSources,
  type ReflectingBody,
  type SecondarySource,
} from "../lighting/planetshine";
import { SETTINGS } from "../quality/qualitySetting";
import { AU_M } from "../scenes/kept";
import { geodeticOf, type SpheroidFigure } from "./hillaire";
import { horizonDipRad, skySources, skySourcesWithSkips, twilightLimitRad } from "./sources";

const RAD = Math.PI / 180;

/** WGS 84 (NIMA TR8350.2), the body's datum. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/** The Earth–Moon distance, m: the Moon's semi-major axis (NASA GSFC, Moon Fact Sheet). */
const EARTH_MOON_M = 3.844e8;

const sphere = (radiusM: number): BodyFigure => ({
  equatorialRadiusM: radiusM,
  polarRadiusM: radiusM,
  pole: null,
});

/** The Sun at the origin, R07's shared fixture. */
const SUN: PlacedLight = { disc: aHostDisc(), centreM: vec3(0, 0, 0) };

/** A K dwarf 40 au out, a second light of the body's list. */
const COMPANION: PlacedLight = {
  disc: aHostDisc({ star: 1, absoluteV: 6.2 }),
  centreM: vec3(0, 40 * AU_M, 0),
};

const EARTH: ReflectingBody = {
  id: "0200080020000000.0003",
  centreM: vec3(AU_M, 0, 0),
  figure: sphere(6.371e6),
  photometry: planetPhotometry("Earth"),
  lighting: undefined,
};

/** The Moon 150° round Earth from the sunward direction, in the ecliptic: a gibbous Moon. */
const MOON: ReflectingBody = {
  id: "0200080020000000.0301",
  centreM: add(
    EARTH.centreM,
    scale(vec3(-Math.cos(150 * RAD), Math.sin(150 * RAD), 0), EARTH_MOON_M),
  ),
  figure: sphere(1.7374e6),
  photometry: photometryFor(
    [MOON_GEOMETRIC_ALBEDO, MOON_GEOMETRIC_ALBEDO, MOON_GEOMETRIC_ALBEDO],
    [0.626, 0.626, 0.626],
    "moon",
  ),
  lighting: undefined,
};

/** Earth's lights and planetshine as R07 builds them for the body (`planLitBodies`' functions). */
function r07Arrays(): {
  readonly stars: LightAtPoint[];
  readonly planetshine: SecondarySource[];
} {
  const hosts = [SUN, COMPANION];
  return {
    stars: lightsAt(EARTH.centreM, hosts, MAX_BODY_LIGHTS),
    planetshine: planetshineSources(
      EARTH,
      litNeighbours([MOON], hosts, DISC_ANNULI_HIGH),
      SETTINGS.high.photoreal.planetshineSources,
    ),
  };
}

describe("the sky's sources: R07's one list for the body (R08.T7)", () => {
  // A camera above Earth's air, where neither skip takes a source (no source stands above every
  // ground point's horizon, nor below the twilight limit), its body-fixed axes turned from the
  // lights' by 30° about an inclined axis.
  const toBodyFixed = quaternionFromAxisAngle(normalise(vec3(1, 2, 3)), 30 * RAD);
  const cameraLightsM = vec3(0, 0, 7_000_000);
  const camera = { positionM: rotate(toBodyFixed, cameraLightsM) };

  it("are the arrays R07 builds for the body, element for element, stars first", () => {
    const { stars, planetshine } = r07Arrays();
    expect([stars.length, planetshine.length]).toEqual([2, 1]);
    const listed = skySourcesWithSkips(
      stars,
      planetshine,
      camera,
      WGS84,
      { topHeightM: 100_000 },
      toBodyFixed,
    );
    expect(listed.map(({ source, skip }) => [source.kind, source.index, skip])).toEqual([
      ["star", 0, null],
      ["star", 1, null],
      ["planetshine", 0, null],
    ]);
    for (const [i, light] of stars.entries()) {
      const source = listed[i]?.source;
      const fromCamera = sub(light.toStarM, cameraLightsM);
      const expected = rotate(toBodyFixed, normalise(fromCamera));
      expect(norm(sub(source?.directionBodyFixed ?? vec3(0, 0, 0), expected))).toBeLessThan(1e-12);
      // Carried to the camera by the inverse square.
      const square = (norm(light.toStarM) / norm(fromCamera)) ** 2;
      for (const c of [0, 1, 2] as const) {
        expect((source?.illuminanceLx[c] ?? 0) / (light.illuminance[c] * square) - 1).toBeCloseTo(
          0,
          12,
        );
      }
      expect(source?.angularRadiusRad).toBeCloseTo(
        Math.asin(light.host.disc.radius_m / norm(fromCamera)),
        15,
      );
      expect([source?.curveSlot, source?.drawsDisc]).toEqual([i, false]);
    }
    const moon = planetshine[0];
    const source = listed[2]?.source;
    if (moon === undefined || source === undefined) {
      throw new Error("the fixture has Earth's Moon");
    }
    const fromCamera = sub(scale(moon.direction, moon.distanceM), cameraLightsM);
    expect(
      norm(sub(source.directionBodyFixed, rotate(toBodyFixed, normalise(fromCamera)))),
    ).toBeLessThan(1e-12);
    const square = (moon.distanceM / norm(fromCamera)) ** 2;
    expect(source.illuminanceLx[1] / (moon.illuminance[1] * square) - 1).toBeCloseTo(0, 12);
    // Light slot 0's curves and no disc of its own.
    expect([source.curveSlot, source.drawsDisc]).toEqual([0, false]);
    expect(source.angularRadiusRad).toBeCloseTo(Math.asin(moon.radiusM / norm(fromCamera)), 15);
  });

  it("takes the Moon's parallax from the camera, about 1° from Earth's ground", () => {
    const { planetshine } = r07Arrays();
    const moon = planetshine[0];
    if (moon === undefined) {
      throw new Error("the fixture has Earth's Moon");
    }
    // A camera on the ground, 90° from the Moon's direction.
    const offset = normalise(vec3(moon.direction.y, -moon.direction.x, 0));
    const ground = { positionM: scale(offset, 6_371_000) };
    const [source] = skySources(
      [],
      planetshine,
      ground,
      WGS84,
      { topHeightM: 100_000 },
      IDENTITY_QUATERNION,
    );
    const turned = Math.acos(source?.directionBodyFixed.x ?? 0) - Math.acos(moon.direction.x);
    expect(Math.abs(turned) / RAD).toBeGreaterThan(0.9);
    expect(Math.abs(turned) / RAD).toBeLessThan(1);
  });

  it("refuses more stars or sources than the kernels hold", () => {
    const { stars, planetshine } = r07Arrays();
    const top = { topHeightM: 100_000 };
    const five = [...stars, ...stars, stars[0]].flatMap((s) => (s === undefined ? [] : [s]));
    expect(() => skySources(five, [], camera, WGS84, top, toBodyFixed)).toThrow(/4 stars/u);
    const many = [...planetshine, ...planetshine, ...planetshine];
    expect(() => skySources([...stars, ...stars], many, camera, WGS84, top, toBodyFixed)).toThrow(
      /6 sources/u,
    );
  });
});

/** A camera 2 m above the equator at longitude 0; its normal is +x and north +z. */
const GROUND = { positionM: vec3(WGS84.equatorialRadiusM + 2, 0, 0) };

/** A direction at an elevation above the ground camera's horizon, towards the north. */
function elevated(elevationDeg: number): Vec3 {
  return vec3(Math.sin(elevationDeg * RAD), 0, Math.cos(elevationDeg * RAD));
}

/** The Sun as the lights see it from Earth's centre at an elevation for {@link GROUND}. */
function sunAt(elevationDeg: number): LightAtPoint {
  const toStarM = scale(elevated(elevationDeg), AU_M);
  const host = { disc: aHostDisc(), centreM: toStarM };
  return { host, toStarM, illuminance: [9.3e4, 1.07e5, 1.19e5] };
}

/**
 * A full Moon at an elevation for {@link GROUND}: a test's 0.32 lx, about Krisciunas and Schaefer
 * 1991's (PASP 103, 1033, eqs. 8–9 at m = −12.73) 0.313 lx without the opposition surge, and the
 * Moon's mean radius, 1,737.4 km (NASA's Moon fact sheet).
 */
function moonAt(elevationDeg: number): SecondarySource {
  return {
    body: "0200080020000000.0301",
    direction: elevated(elevationDeg),
    angularRadiusRad: Math.asin(1.7374e6 / EARTH_MOON_M),
    distanceM: EARTH_MOON_M,
    radiusM: 1.7374e6,
    illuminance: [0.3, 0.32, 0.33],
  };
}

describe("the sky's two skips inside the air (R08.T7, Design note 7)", () => {
  const top = { topHeightM: 100_000 };
  const limitDeg = twilightLimitRad(WGS84, 2, top.topHeightM) / RAD;

  it("puts the twilight limit 20.2° below the horizon on Earth's ground under a 100 km top", () => {
    const ground = twilightLimitRad(WGS84, 0, top.topHeightM) / RAD;
    expect(ground).toBeCloseTo(20.2, 1);
    // From 2 m and from 10 km up, the camera's own dip adds acos(R ÷ (R + h)): 0.0455° and 3.2°.
    expect(limitDeg - ground).toBeCloseTo(0.0455, 3);
    expect(twilightLimitRad(WGS84, 10_000, top.topHeightM) / RAD - ground).toBeCloseTo(3.2, 1);
  });

  it("skips the Sun 1° beyond the twilight limit and keeps a full Moon at 45°", () => {
    const listed = skySourcesWithSkips(
      [sunAt(-limitDeg - 1)],
      [moonAt(45)],
      GROUND,
      WGS84,
      top,
      IDENTITY_QUATERNION,
    );
    expect(listed.map(({ skip }) => skip)).toEqual(["below twilight limit", null]);
    const drawn = skySources(
      [sunAt(-limitDeg - 1)],
      [moonAt(45)],
      GROUND,
      WGS84,
      top,
      IDENTITY_QUATERNION,
    );
    expect(drawn.map((s) => s.kind)).toEqual(["planetshine"]);
  });

  it("keeps a Sun whose upper limb is still above the limit", () => {
    const listed = skySourcesWithSkips(
      [sunAt(-limitDeg + 0.2)],
      [moonAt(45)],
      GROUND,
      WGS84,
      top,
      IDENTITY_QUATERNION,
    );
    expect(listed.map(({ skip }) => skip)).toEqual([null, null]);
  });

  it("skips the Moon by day, under 10⁻⁴ of the Sun above the horizon", () => {
    const listed = skySourcesWithSkips(
      [sunAt(30)],
      [moonAt(45)],
      GROUND,
      WGS84,
      top,
      IDENTITY_QUATERNION,
    );
    expect(listed.map(({ skip }) => skip)).toEqual([null, "negligible"]);
    const [sun, moon] = listed.map(({ source }) => photopicIlluminance(source.illuminanceLx));
    expect((moon ?? 0) / (sun ?? 1)).toBeLessThan(STAR_CUT_RELATIVE);
  });

  it("keeps the Moon while the Sun is below the horizon, in twilight", () => {
    const listed = skySourcesWithSkips(
      [sunAt(-10)],
      [moonAt(45)],
      GROUND,
      WGS84,
      top,
      IDENTITY_QUATERNION,
    );
    expect(listed.map(({ skip }) => skip)).toEqual([null, null]);
  });

  it("counts a Sun as above the ground's horizon from its centre's elevation of acos(R ÷ r_c)", () => {
    // From 2 m the horizon dips 0.0455°: a Sun at 0.03° is under it and does not skip the Moon.
    expect(horizonDipRad(WGS84, 2) / RAD).toBeCloseTo(0.0455, 3);
    const skips = (sunDeg: number): ReturnType<typeof skySourcesWithSkips> =>
      skySourcesWithSkips([sunAt(sunDeg)], [moonAt(45)], GROUND, WGS84, top, IDENTITY_QUATERNION);
    expect(skips(0.03).map(({ skip }) => skip)).toEqual([null, null]);
    expect(skips(0.06).map(({ skip }) => skip)).toEqual([null, "negligible"]);
  });
});

/**
 * A camera on the equator at longitude 0, `distanceM` from the centre; its normal is +x, so that
 * {@link sunAt}'s and {@link moonAt}'s elevations are its own, less the Moon's parallax.
 */
function above(distanceM: number): { readonly positionM: Vec3 } {
  return { positionM: vec3(distanceM, 0, 0) };
}

describe("the sky's two skips outside the air (R08.T8.e, decision-r08-t8-leans.md §9)", () => {
  const top = { topHeightM: 100_000 };
  const places = [
    ["400 km", WGS84.equatorialRadiusM + 400_000],
    ["10⁷ m", 1e7],
  ] as const;

  it("puts the horizon's dip at 19.8° and the twilight limit at 40.1° from 400 km", () => {
    expect(horizonDipRad(WGS84, 0)).toBe(0);
    expect(horizonDipRad(WGS84, 400_000) / RAD).toBeCloseTo(19.85, 2);
    expect(twilightLimitRad(WGS84, 400_000, top.topHeightM) / RAD).toBeCloseTo(40.07, 2);
  });

  for (const [name, distanceM] of places) {
    const camera = above(distanceM);
    const heightM = geodeticOf(camera.positionM, WGS84).heightM;
    const limitDeg = twilightLimitRad(WGS84, heightM, top.topHeightM) / RAD;
    const dipDeg = horizonDipRad(WGS84, heightM) / RAD;

    it(`skips the Sun 1° beyond the twilight limit from ${name}, and keeps a full Moon at 45°`, () => {
      expect(heightM).toBeGreaterThan(top.topHeightM);
      const listed = skySourcesWithSkips(
        [sunAt(-limitDeg - 1)],
        [moonAt(45)],
        camera,
        WGS84,
        top,
        IDENTITY_QUATERNION,
      );
      expect(listed.map(({ skip }) => skip)).toEqual(["below twilight limit", null]);
      // Its upper limb still above the limit, it is kept.
      const kept = skySourcesWithSkips(
        [sunAt(-limitDeg + 0.2)],
        [moonAt(45)],
        camera,
        WGS84,
        top,
        IDENTITY_QUATERNION,
      );
      expect(kept.map(({ skip }) => skip)).toEqual([null, null]);
    });

    it(`skips a full Moon from ${name} with the Sun 1° above acos(R ÷ r_c), and keeps it 1° below`, () => {
      const skips = (sunDeg: number): ReturnType<typeof skySourcesWithSkips> =>
        skySourcesWithSkips([sunAt(sunDeg)], [moonAt(45)], camera, WGS84, top, IDENTITY_QUATERNION);
      expect(skips(dipDeg + 1).map(({ skip }) => skip)).toEqual([null, "negligible"]);
      // Under the dip some ground the camera sees has the Sun below its horizon.
      expect(skips(dipDeg - 1).map(({ skip }) => skip)).toEqual([null, null]);
    });
  }

  it("lists the same sources just below the medium's top and just above it", () => {
    const below = above(WGS84.equatorialRadiusM + top.topHeightM - 1);
    const over = above(WGS84.equatorialRadiusM + top.topHeightM + 1);
    // Each side of both rules' edges at the top, 0.05° off: the Sun's upper limb at the twilight
    // limit, and its centre at the horizon's dip.
    const limitDeg = twilightLimitRad(WGS84, top.topHeightM, top.topHeightM) / RAD;
    const dipDeg = horizonDipRad(WGS84, top.topHeightM) / RAD;
    const radiusDeg = Math.asin(aHostDisc().radius_m / AU_M) / RAD;
    const edge = -limitDeg - radiusDeg;
    for (const sunDeg of [
      -90,
      edge - 0.05,
      edge + 0.05,
      -10,
      0,
      dipDeg - 0.05,
      dipDeg + 0.05,
      90,
    ]) {
      const at = (camera: { readonly positionM: Vec3 }): ReturnType<typeof skySourcesWithSkips> =>
        skySourcesWithSkips([sunAt(sunDeg)], [moonAt(45)], camera, WGS84, top, IDENTITY_QUATERNION);
      expect(
        at(below).map(({ skip }) => skip),
        `the Sun at ${sunDeg}°`,
      ).toEqual(at(over).map(({ skip }) => skip));
    }
  });
});
