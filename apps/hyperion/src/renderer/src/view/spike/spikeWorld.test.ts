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
import { testPlanetRotationAt } from "./rotation";
import {
  type SpikeWorldName,
  spikeWorldNamed,
  TEST_PLANET_WORLD,
  terrainTestWorld,
} from "./spikeWorld";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

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

describe("the world --world names", () => {
  it("is one object a name, the test planet's R05's own", () => {
    expect(spikeWorldNamed("test-planet")).toBe(TEST_PLANET_WORLD);
    const terrain = spikeWorldNamed("terrain-test");
    expect(spikeWorldNamed("terrain-test")).toBe(terrain);
    expect(terrain).toEqual(terrainTestWorld());
  });

  it("takes the launch's names", () => {
    const names: ReadonlyArray<SpikeLaunch["world"]> = ["test-planet", "terrain-test"];
    const worlds: ReadonlyArray<SpikeWorldName> = names;
    expect(worlds.map((name) => spikeWorldNamed(name).name)).toEqual(names);
  });
});
