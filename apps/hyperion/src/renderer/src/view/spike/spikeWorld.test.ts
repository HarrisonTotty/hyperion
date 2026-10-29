import { describe, expect, it } from "vitest";

import type { SpikeLaunch } from "../../../../preload/api";
import { KEPT_SYSTEM, keptBody } from "../scenes/kept";
import {
  TERRAIN_HOST_DISCS,
  TERRAIN_MOON_HILL_M,
  TERRAIN_MOON_POLE,
  terrainMoonRotationAt,
  terrainSceneAt,
  terrainStar,
  terrainSystem,
  terrainTestGolden,
} from "../scenes/terrainScene";
import { norm } from "../../geometry/vec3";
import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { sceneModelOf, stateNearEarth } from "../../test/sceneFixture";
import { type ServedBody, servedBodyOf } from "../scene/servedBody";
import { testPlanetRotationAt } from "./rotation";
import {
  servedSpikeWorld,
  type SpikeWorldName,
  spikeWorldNamed,
  TEST_PLANET_WORLD,
  terrainTestWorld,
  timeAfter,
} from "./spikeWorld";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

/** The slice's Earth, served, from the ship's scene in its frame. */
function servedEarth(): ServedBody {
  const read = servedBodyOf(sceneModelOf(stateNearEarth()), "000000000000002a", FIXTURE_EARTH);
  if (read.kind !== "served") {
    throw new Error(`the slice's Earth is a served world, not ${read.gap}`);
  }
  return read.served;
}

describe("the test planet's world", () => {
  it("holds R05's bodies, figure, Hill radius and rotation, with no appearance, seed or disc", () => {
    expect(TEST_PLANET_WORLD).toEqual({
      name: "test-planet",
      system: KEPT_SYSTEM,
      star: keptBody(0),
      body: keptBody(1),
      starDesignation: "TEST STAR",
      bodyDesignation: "TEST PLANET",
      bodyKind: "planet",
      figure: TEST_PLANET_FIGURE,
      hillRadiusM: 1.5e9,
      rotationAt: testPlanetRotationAt,
      appearance: null,
    });
    expect("detailSeed" in TEST_PLANET_WORLD).toBe(false);
    expect("hostDiscs" in TEST_PLANET_WORLD).toBe(false);
  });
});

describe("the terrain test's world", () => {
  const world = terrainTestWorld();
  const golden = terrainTestGolden();

  it("is TERRAIN TEST's star and moon, with the golden's body and seed", () => {
    expect([world.system, world.star, world.body, world.detailSeed]).toEqual([
      terrainSystem(),
      terrainStar(),
      golden.body,
      golden.seed,
    ]);
    expect([world.starDesignation, world.bodyDesignation, world.bodyKind]).toEqual([
      "TEST STAR",
      "TEST MOON",
      "moon",
    ]);
    expect(world.hillRadiusM).toBe(TERRAIN_MOON_HILL_M);
    expect(world.hostDiscs).toBe(TERRAIN_HOST_DISCS);
  });

  it("takes the golden header's figure about the scene's tilted pole", () => {
    expect(world.figure).toEqual({
      equatorialRadiusM: golden.header.equatorial_radius_m,
      polarRadiusM: golden.header.polar_radius_m,
      pole: TERRAIN_MOON_POLE,
    });
  });

  it("turns as TEST MOON does, a scene second a script second", () => {
    for (const tS of [0, 600, 1_230]) {
      expect(world.rotationAt(tS)).toEqual(terrainMoonRotationAt(tS));
    }
  });

  it("shades the moon as TERRAIN TEST's scene does", () => {
    const moon = terrainSceneAt(0).bodies.find((body) => body.id === golden.body);
    expect(world.appearance).toEqual(moon?.appearance);
  });
});

describe("a served world (R10.T15.b)", () => {
  const served = servedEarth();
  const discs = TERRAIN_HOST_DISCS;
  const world = servedSpikeWorld(served, discs);

  it("is the served body and its star, as the server's scene draws them", () => {
    expect(world).toMatchObject({
      name: "served",
      system: served.system,
      star: served.star.id,
      body: FIXTURE_EARTH,
      starDesignation: served.star.designation,
      bodyDesignation: served.body.designation,
      bodyKind: "planet",
      hillRadiusM: served.hillRadiusM,
      starRadiusM: served.star.radiusM,
      starDistanceM: norm(served.towardStarM),
    });
    expect(world.figure).toBe(served.figure);
    expect(world.appearance).toBe(served.appearance);
    expect(world.hostDiscs).toBe(discs);
  });

  it("carries no detail seed, which its session asks the server for", () => {
    expect("detailSeed" in world).toBe(false);
  });

  it("turns by its rotation law from the scene's time, a scene second a script second", () => {
    expect(world.rotationAt(0)).toEqual(served.body.rotation);
    expect(world.rotationAt(600.25)).toEqual(served.rotationAt(timeAfter(served.epoch, 600.25)));
  });

  it("counts its scene's time from the scene's when it was read", () => {
    expect(world.epoch).toEqual(served.epoch);
  });
});

describe("the time after an instant", () => {
  it("carries the nanoseconds into the seconds", () => {
    expect(timeAfter({ seconds: 10, nanos: 900_000_000 }, 1.25)).toEqual({
      seconds: 12,
      nanos: 150_000_000,
    });
    expect(timeAfter({ seconds: 10, nanos: 0 }, 0)).toEqual({ seconds: 10, nanos: 0 });
  });

  it("goes back before the instant for a negative span", () => {
    expect(timeAfter({ seconds: 10, nanos: 100_000_000 }, -0.5)).toEqual({
      seconds: 9,
      nanos: 600_000_000,
    });
  });
});

describe("the world --world names", () => {
  it("is one object a name, the test planet's R05's own", () => {
    expect(spikeWorldNamed("test-planet")).toBe(TEST_PLANET_WORLD);
    const terrain = spikeWorldNamed("terrain-test");
    expect(spikeWorldNamed("terrain-test")).toBe(terrain);
    expect(terrain).toEqual(terrainTestWorld());
  });

  it("takes the launch's names but the served world's, which is read from the server", () => {
    const names: ReadonlyArray<Exclude<SpikeLaunch["world"], "served">> = [
      "test-planet",
      "terrain-test",
    ];
    const worlds: ReadonlyArray<SpikeWorldName> = names;
    expect(worlds).toEqual(names);
    expect(names.map((name) => spikeWorldNamed(name).name)).toEqual(names);
  });
});
