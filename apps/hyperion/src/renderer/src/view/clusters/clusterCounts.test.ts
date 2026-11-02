/**
 * The counting harness (plan R14, Design note 18; R14.T1.f) on a few of R05's descent's poses: each
 * lever's count as built, today's path's counts as the research's, and its record and summary.
 */

import { describe, expect, it } from "vitest";

import fixture from "../terrain/fixtures/level-bounds.json" with { type: "json" };
import { LIT_INTERIOR_INDICES, LIT_PATCH_INDICES } from "../terrain/gpu/resources";
import { type SampleWorld, sampleWorld } from "../terrain/levelBoundsCounts";
import {
  bandFigures,
  type ClusterCountCell,
  clusterCountsMarkdown,
  clusterCountsRecord,
  countClusterCell,
} from "./clusterCounts";

/** A sample world of the fixture by name. */
function world(name: string): SampleWorld {
  const found = fixture.worlds.find((each) => each.name === name);
  if (found === undefined) {
    throw new Error(`the fixture has no ${name}`);
  }
  return sampleWorld(found);
}

/** Three poses of the descent: from orbit, on the way down and below 1 km, s. */
const TIMES_S = [0, 700, 1220];

describe("the counting harness (plan R14, Design note 18)", () => {
  const moon = world("moon_like");
  const high: ClusterCountCell = countClusterCell({
    world: moon,
    setting: "high",
    timesS: TIMES_S,
  });
  const low = countClusterCell({ world: moon, setting: "low", timesS: TIMES_S });

  it("counts today's lit pass and the interior patches' draw as the lit mesh's and the interior mesh's triangles", () => {
    for (const pose of [...high.poses, ...low.poses]) {
      expect([pose.edgePatches, pose.litToday, pose.litInterior]).toEqual([
        0,
        (pose.interiorPatches * LIT_PATCH_INDICES) / 3,
        (pose.interiorPatches * LIT_INTERIOR_INDICES) / 3,
      ]);
    }
    // The high setting's budget binds above 1 km, as the research found: 603 or 604 patches.
    expect(high.poses[0]?.interiorPatches).toBeGreaterThan(590);
  });

  it("casts on high alone, each lever's count within the one before it, and the overlay's and the lit clusters' within the interior draw", () => {
    expect(low.poses.every((pose) => pose.casters.every((cast) => cast === null))).toBe(true);
    const cast = high.poses.flatMap((pose) => pose.casters.filter((each) => each !== null));
    expect(cast.length).toBeGreaterThan(0);
    for (const each of cast) {
      expect([
        each.clusters <= each.today,
        each.receivers <= each.clusters,
        each.strided <= each.receivers,
      ]).toEqual([true, true, true]);
    }
    for (const pose of [...high.poses, ...low.poses]) {
      expect([
        pose.overlayClusters <= pose.litInterior,
        // T2.a's lit pass: its frustum and the horizon refuse some of the interior draw's clusters.
        pose.litClusters > 0 && pose.litClusters < pose.litInterior,
      ]).toEqual([true, true]);
    }
  });

  it("summarises each cell by band, with each lever's ratio, in its record's markdown", () => {
    const bands = bandFigures(high);
    expect([bands[0]?.band, bands.at(-1)?.band]).toEqual(["orbit 100-1000 km", "below 1 km"]);
    expect(bands.every((band) => band.totalClusterPath < band.totalToday)).toBe(true);
    const markdown = clusterCountsMarkdown(
      clusterCountsRecord({
        startedAt: "2026-11-01T00:00:00Z",
        generatorVersion: 24,
        cells: [high, low],
      }),
    );
    expect(markdown).toContain(
      "| moon_like | high | orbit 100-1000 km | 2 | 6.48 | 5.24 (0.810) |",
    );
  });
});
