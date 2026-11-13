/**
 * The mesh pages' fly-by (plan R14, R14.T5.b's acceptance) over the three-level DAG: 100
 * instances of three meshes flown past, every frame's cut whole, within the pool's bytes at both
 * settings' `pageBytes` and at a pool too small for every page wanted. `scripts/meshFlyby.mjs`
 * runs the same over `tools/meshbuild`'s test meshes by hand.
 */

import { beforeAll, describe, expect, it } from "vitest";

import { pagePoolLayout } from "../view/clusters/pages";
import { SETTINGS } from "../view/quality/qualitySetting";
import { type FlybyResult, runMeshFlyby } from "./meshFlyby";
import { threeLevelMesh, writeMeshFile } from "./meshPages";

/** 1 px of 1920 across 60°, rad. */
const ONE_PX_RAD = (2 * Math.tan(Math.PI / 6)) / 1920;

describe.each([
  ["high's", SETTINGS.high.geometry.pageBytes],
  ["low's", SETTINGS.low.geometry.pageBytes],
  // Six page slots for three meshes of five pages each.
  ["a pool of six page slots'", 2 ** 20],
])("the mesh pages' fly-by within %s bytes (plan R14, R14.T5.b)", (_, poolBytes) => {
  let result: FlybyResult;
  beforeAll(async () => {
    const bytes = writeMeshFile(threeLevelMesh());
    result = await runMeshFlyby({
      meshes: ["a", "b", "c"].map((name) => ({ name, bytes })),
      instances: 100,
      frames: 90,
      poolBytes,
      toleranceRad: ONE_PX_RAD,
      latencyFrames: 2,
      spacingM: 30,
      passM: 20,
      cracks: false,
      layout: "alternating",
    });
  });

  it("draws 100 instances with no hole and no overlap in any frame", () => {
    expect([result.holes, result.overlaps]).toEqual([0, 0]);
  });

  it("stays within the pool's bytes", () => {
    expect(result.peakPoolBytes).toBeLessThanOrEqual(poolBytes);
  });

  it("streams: pages arrive frames after they are wanted, their stand-ins drawn meanwhile", () => {
    expect(result.streaming).toBeGreaterThan(0);
  });

  it("stores what its slots hold, the 3 roots and up to the 15 pages, and asks for none it would refuse", () => {
    expect([result.stored, result.refused]).toEqual([
      3 + Math.min(15, pagePoolLayout(poolBytes).pageSlots),
      0,
    ]);
  });
});

// Six root slots and nine page slots, for six meshes of five pages each.
const RUNS_POOL_BYTES = 1_400_000;

describe("the mesh pages' fly-by past each mesh in a run, within nine page slots (plan R14, R14.T5.b)", () => {
  let result: FlybyResult;
  beforeAll(async () => {
    const bytes = writeMeshFile(threeLevelMesh());
    result = await runMeshFlyby({
      meshes: ["a", "b", "c", "d", "e", "f"].map((name) => ({ name, bytes })),
      instances: 100,
      frames: 90,
      poolBytes: RUNS_POOL_BYTES,
      toleranceRad: ONE_PX_RAD,
      latencyFrames: 2,
      spacingM: 30,
      passM: 20,
      cracks: false,
      layout: "runs",
    });
  });

  it("evicts the pages the camera has left behind for those it comes to", () => {
    expect(result.evicted).toBeGreaterThan(0);
  });

  it("draws every frame with no hole and no overlap as it evicts", () => {
    expect([result.holes, result.overlaps]).toEqual([0, 0]);
  });

  it("stays within the pool's bytes as it evicts", () => {
    expect(result.peakPoolBytes).toBeLessThanOrEqual(RUNS_POOL_BYTES);
  });
});
