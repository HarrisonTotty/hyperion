import { describe, expect, it, vi } from "vitest";

import terrainGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test.golden?raw";
import type * as TerrainScene from "./terrainScene";

/** The scene's module afresh, so that no test sees the whole body's field another provided. */
async function freshScene(): Promise<typeof TerrainScene> {
  vi.resetModules();
  return import("./terrainScene");
}

describe("the whole body's field (R10.T14.c)", () => {
  it("is refused before a --whole launch provides it, naming why", async () => {
    const scene = await freshScene();
    expect(() => scene.terrainTestGolden("terrain-test-whole")).toThrow(
      "the whole body's field (whole.golden) was not provided: a --terrain --whole launch reads it first",
    );
  });

  it("serves TERRAIN TEST's moon once provided, its body, seed and header the patch's", async () => {
    const scene = await freshScene();
    scene.provideWholeTerrainField(terrainGoldenText);
    const whole = scene.terrainTestGolden("terrain-test-whole");
    const patch = scene.terrainTestGolden();
    expect([whole.body, whole.seed, whole.header]).toEqual([patch.body, patch.seed, patch.header]);
    expect(scene.terrainMoon("terrain-test-whole")).toBe(patch.body);
  });

  it("refuses a field of another moon", async () => {
    const scene = await freshScene();
    const other = terrainGoldenText.replace(
      "dto radius_m 0x413a82b800000000  # 1737400.0",
      "dto radius_m 0x413a82b900000000  # 1737400.0000000002",
    );
    expect(other).not.toBe(terrainGoldenText);
    expect(() => scene.provideWholeTerrainField(other)).toThrow(
      "whole.golden is not TERRAIN TEST's moon: its body, seed or header differ from terrain_test.golden's",
    );
  });
});
