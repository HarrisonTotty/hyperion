/**
 * The descent's light (plan R10, R10.T14.c, the owner's request): `TERRAIN TEST`'s moon lit from
 * the track's start to touchdown, and served worlds' sites drawn where their own star lights the
 * descent, a tidally locked body's among them, with the star's elevation taken over the profile's
 * own poses.
 */

import { describe, expect, it } from "vitest";

import { dot, normalise, type Vec3, vec3 } from "../../geometry/vec3";
import {
  type Rotation3,
  rotateToBody,
  rotateToBodyFixed,
  rotation3FromRows,
} from "../coords/rotation";
import { finestLevel } from "../terrain/cube";
import type { PatchKey } from "../terrain/patchKey";
import { bodySiteOf, siteCandidates } from "./bodyDescent";
import { DescentProfile, landingSiteOf } from "./descentProfile";
import {
  descentStarOf,
  litBodySite,
  litSiteCandidates,
  SITE_STAR_ELEVATION_DEG,
  START_STAR_ELEVATION_DEG,
  starElevationRad,
} from "./siteLight";
import { spikeScene, sunDirectionBody } from "./spikeScene";
import { servedBodyOf } from "../scene/servedBody";
import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { sceneModelOf, stateNearEarth } from "../../test/sceneFixture";
import {
  servedSpikeWorld,
  type SpikeWorld,
  terrainTestWorld,
  TEST_PLANET_WORLD,
} from "./spikeWorld";

const DEG = 180 / Math.PI;

/** The star's elevation over the craft's ground at `tS`, degrees, `sun` in the body's axes. */
function craftElevationDeg(
  profile: DescentProfile,
  world: SpikeWorld,
  sun: Vec3,
  tS: number,
): number {
  const star = rotateToBodyFixed(world.rotationAt(tS), sun);
  return starElevationRad(world.figure, profile.poseAt(tS).groundDir, star) * DEG;
}

/** The least elevation over the craft's ground through the script from `fromS`, degrees, every 5 s. */
function leastElevationDeg(
  profile: DescentProfile,
  world: SpikeWorld,
  sun: Vec3,
  fromS = 0,
): number {
  let least = Infinity;
  for (let t = fromS; t <= profile.durationS; t += 5) {
    least = Math.min(least, craftElevationDeg(profile, world, sun, t));
  }
  return least;
}

describe("the light over TERRAIN TEST's moon", () => {
  const world = terrainTestWorld();
  // Sites in the survey's middle rows, 28° N, 45° E (`terrainScene.ts`' track), approached from
  // each quarter.
  const profiles = [0, 40, 90, 180, 270].map(
    (azimuthDeg) =>
      new DescentProfile(
        world.figure,
        { latitudeRad: 28 / DEG, longitudeRad: 45 / DEG, azimuthRad: azimuthDeg / DEG },
        { siteHeightM: 1_900 },
      ),
  );

  it("lit the track's start no more under T13.b's noon over the site, 29° to 83° below the horizon", () => {
    const noonStartsDeg = profiles.map((profile) => {
      const end = profile.durationS;
      const rotation = world.rotationAt(end);
      const site = rotateToBody(rotation, profile.poseAt(end).groundPointM);
      const pole = rotateToBody(rotation, vec3(0, 0, 1));
      const noon = normalise(
        vec3(
          site.x - pole.x * dot(site, pole),
          site.y - pole.y * dot(site, pole),
          site.z - pole.z * dot(site, pole),
        ),
      );
      return Math.round(craftElevationDeg(profile, world, noon, 0));
    });
    expect(noonStartsDeg).toEqual([-29, -35, -52, -83, -44]);
  });

  it("stands the star 20° to 45° over the site at touchdown, and 8° or more over the whole track", () => {
    for (const profile of profiles) {
      const sun = sunDirectionBody(profile, world);
      const siteDeg = craftElevationDeg(profile, world, sun, profile.durationS);
      expect(siteDeg).toBeGreaterThanOrEqual(SITE_STAR_ELEVATION_DEG.lowest - 0.05);
      expect(siteDeg).toBeLessThanOrEqual(SITE_STAR_ELEVATION_DEG.highest);
      expect(leastElevationDeg(profile, world, sun)).toBeGreaterThan(8);
    }
  });

  it("stands the star 10° over the start where the site keeps it 20° or more high", () => {
    const [north] = profiles;
    if (north === undefined) {
      throw new Error("no profile");
    }
    const sun = sunDirectionBody(north, world);
    expect(craftElevationDeg(north, world, sun, 0)).toBeCloseTo(START_STAR_ELEVATION_DEG, 0);
  });
});

/** A spin about a pole tilted `obliquityDeg` about +x, `periodS` a turn, as `TEST MOON`'s turns. */
function spinAt(periodS: number, obliquityDeg: number): (tS: number) => Rotation3 {
  const ce = Math.cos(obliquityDeg / DEG);
  const se = Math.sin(obliquityDeg / DEG);
  return (tS) => {
    const theta = 0.7 + (2 * Math.PI * tS) / periodS;
    const c = Math.cos(theta);
    const s = Math.sin(theta);
    return rotation3FromRows([vec3(c, -s, 0), vec3(ce * s, ce * c, -se), vec3(se * s, se * c, ce)]);
  };
}

/** A served world of radius `radiusM` turning as `spinAt` gives, lit from `star`. */
function servedWorld(
  radiusM: number,
  periodS: number,
  obliquityDeg: number,
  star: Vec3,
): SpikeWorld {
  const rotationAt = spinAt(periodS, obliquityDeg);
  return {
    ...terrainTestWorld(),
    name: "served",
    figure: {
      equatorialRadiusM: radiusM,
      polarRadiusM: radiusM * (1 - 1 / 300),
      pole: rotateToBody(rotationAt(0), vec3(0, 0, 1)),
    },
    rotationAt,
    starDirection: normalise(star),
  };
}

/** Every cell of level 6 surveyed: `just place-ship --survey`'s whole body. */
const WHOLE_SURVEY = { level: 6, resolutionAtM: (_cell: PatchKey) => 1 } as const;

/** The profile over `world` the measurement draws for `seed`: its lit site and its approach. */
function servedProfile(world: SpikeWorld, seed: bigint): DescentProfile {
  const star = descentStarOf(
    world,
    new DescentProfile(world.figure, { latitudeRad: 0, longitudeRad: 0, azimuthRad: 0 }),
  );
  if (star === null) {
    throw new Error("a served world has a star");
  }
  const candidates = litSiteCandidates(siteCandidates(WHOLE_SURVEY), star);
  const drawn = bodySiteOf(candidates, seed, 0, finestLevel(world.figure.equatorialRadiusM));
  if (drawn === null) {
    throw new Error(`no lit site for seed ${String(seed)}`);
  }
  return new DescentProfile(world.figure, litBodySite(drawn, star).site);
}

const SEEDS = [1n, 2n, 3n, 7n, 11n];

describe("a served world's site", () => {
  it.each([
    {
      name: "an Earth",
      radiusM: 6.378e6,
      periodS: 86_164,
      obliquityDeg: 23.4,
      star: vec3(1, 0.3, -0.2),
    },
    {
      name: "a Mars",
      radiusM: 3.396e6,
      periodS: 88_643,
      obliquityDeg: 25.2,
      star: vec3(-0.4, 0.8, 0.45),
    },
    {
      name: "a super-Earth turning in 3 h",
      radiusM: 8e6,
      periodS: 10_800,
      obliquityDeg: 10,
      star: vec3(0.2, -0.9, 0.1),
    },
    {
      name: "a moon of TEST MOON's size",
      radiusM: 1.736e6,
      periodS: 86_400,
      obliquityDeg: 5,
      star: vec3(0.3, 0.3, -0.9),
    },
  ])(
    "over $name is drawn where its star stands 20° to 45° high at the descent's start and end, its start lit",
    ({ radiusM, periodS, obliquityDeg, star }) => {
      const world = servedWorld(radiusM, periodS, obliquityDeg, star);
      for (const seed of SEEDS) {
        const profile = servedProfile(world, seed);
        const sun = sunDirectionBody(profile, world);
        expect(sun).toBe(world.starDirection);
        const atSite = [0, profile.durationS].map(
          (t) =>
            starElevationRad(
              world.figure,
              profile.siteDir,
              rotateToBodyFixed(world.rotationAt(t), sun),
            ) * DEG,
        );
        for (const deg of atSite) {
          expect(deg).toBeGreaterThanOrEqual(SITE_STAR_ELEVATION_DEG.lowest);
          expect(deg).toBeLessThanOrEqual(SITE_STAR_ELEVATION_DEG.highest);
        }
        expect(craftElevationDeg(profile, world, sun, 0)).toBeGreaterThanOrEqual(
          START_STAR_ELEVATION_DEG - 0.5,
        );
        expect(leastElevationDeg(profile, world, sun)).toBeGreaterThan(
          START_STAR_ELEVATION_DEG - 0.5,
        );
      }
    },
  );

  it("over the slice's served Earth is drawn where its Sun stands 20° to 45° high, its start lit", () => {
    const read = servedBodyOf(sceneModelOf(stateNearEarth()), "000000000000002a", FIXTURE_EARTH);
    if (read.kind !== "served") {
      throw new Error(`the slice's Earth is a served world, not ${read.gap}`);
    }
    const world = servedSpikeWorld(read.served, []);
    for (const seed of SEEDS) {
      const profile = servedProfile(world, seed);
      const sun = sunDirectionBody(profile, world);
      for (const t of [0, profile.durationS]) {
        const deg =
          starElevationRad(
            world.figure,
            profile.siteDir,
            rotateToBodyFixed(world.rotationAt(t), sun),
          ) * DEG;
        expect(deg).toBeGreaterThanOrEqual(SITE_STAR_ELEVATION_DEG.lowest);
        expect(deg).toBeLessThanOrEqual(SITE_STAR_ELEVATION_DEG.highest);
      }
      expect(leastElevationDeg(profile, world, sun)).toBeGreaterThan(
        START_STAR_ELEVATION_DEG - 0.5,
      );
    }
  });

  it("over a tidally locked body lies on its day hemisphere, its star still over its ground", () => {
    // A planet turning once an orbit, 14 d, lit from its equator's direction at longitude 0.7 rad.
    const world = servedWorld(5e6, 1_209_600, 0, vec3(Math.cos(0.7), Math.sin(0.7), 0));
    for (const seed of SEEDS) {
      const profile = servedProfile(world, seed);
      const sun = sunDirectionBody(profile, world);
      const atSite = [0, profile.durationS].map(
        (t) =>
          starElevationRad(
            world.figure,
            profile.siteDir,
            rotateToBodyFixed(world.rotationAt(t), sun),
          ) * DEG,
      );
      expect(atSite.every((deg) => deg >= 20 && deg <= 45)).toBe(true);
      // The star moves 0.37° over its ground in the script.
      expect(Math.abs((atSite[1] ?? 0) - (atSite[0] ?? 0))).toBeLessThan(0.4);
      expect(leastElevationDeg(profile, world, sun)).toBeGreaterThan(
        START_STAR_ELEVATION_DEG - 0.5,
      );
    }
  });

  it("over a body its track runs 213° round keeps the site's band, its start dark and the light reached 160 s in", () => {
    const world = servedWorld(1.2e6, 86_400, 0, vec3(1, 0, 0));
    const profile = servedProfile(world, 7n);
    const sun = sunDirectionBody(profile, world);
    const siteDeg = craftElevationDeg(profile, world, sun, profile.durationS);
    expect(siteDeg).toBeGreaterThanOrEqual(SITE_STAR_ELEVATION_DEG.lowest);
    expect(siteDeg).toBeLessThanOrEqual(SITE_STAR_ELEVATION_DEG.highest);
    expect(Math.round(profile.trackArcRad * DEG)).toBe(213);
    expect(Math.round(craftElevationDeg(profile, world, sun, 0))).toBe(-56);
    // Early in the 900 s descent arc, which starts at 60 s.
    expect(leastElevationDeg(profile, world, sun, 160)).toBeGreaterThan(0);
  });

  it("is lit from its star's own direction, the scene standing the body across from it", () => {
    const world = servedWorld(6.378e6, 86_164, 23.4, vec3(1, 0.3, -0.2));
    const profile = servedProfile(world, 7n);
    const centre = spikeScene(profile, world).sceneAt(0).bodies[1]?.centreM;
    if (centre === undefined || world.starDirection === undefined) {
      throw new Error("the scene has no body");
    }
    expect(dot(normalise(centre), world.starDirection)).toBeCloseTo(-1, 12);
  });
});

describe("the test planet's light", () => {
  it("stays T13.b's, which lights seed 7's start 42.9° high", () => {
    const profile = new DescentProfile(TEST_PLANET_WORLD.figure, landingSiteOf(7n));
    const sun = sunDirectionBody(profile);
    expect(craftElevationDeg(profile, TEST_PLANET_WORLD, sun, 0)).toBeCloseTo(42.9, 1);
  });
});
