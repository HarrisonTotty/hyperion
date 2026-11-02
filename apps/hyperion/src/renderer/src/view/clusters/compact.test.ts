/**
 * The cluster path's stable compaction (plan R14, R14.T2.a): its parameters as `compact.wgsl` lays
 * them out, its two kernel paths, and its twin's lists in the draw set's order and its scan's
 * prefixes over random counts. The kernel against its twin, both paths byte for byte, is the smoke
 * harness's (`smoke/clusterCull.ts`).
 */

import { describe, expect, it } from "vitest";

import { declaredStructLayout } from "../engine/webgpu/uniforms";
import {
  CLUSTER_COMPACT,
  COMPACT_PARAMS_BYTES,
  COMPACT_TEMPLATES,
  compactTwin,
  compactWorkgroups,
  packCompactParams,
  scanTwin,
} from "./compact";
import { NO_MARK } from "./cull";

/** A seeded uniform generator in [0, 1) (mulberry32). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** Random marks: each cluster refused with probability `refused`, else a random template. */
function randomMarks(
  next: () => number,
  instances: number,
  clusters: number,
  refused: number,
): Uint32Array {
  return Uint32Array.from({ length: instances * clusters }, () =>
    next() < refused ? NO_MARK : Math.floor(next() * COMPACT_TEMPLATES),
  );
}

/** A source from its scan on: what the two paths share. */
function tail(source: string): string {
  return source.slice(source.indexOf("fn scan("));
}

describe("the stable compaction (plan R14, R14.T2.a)", () => {
  it("lays its parameters out in 96 B as compact.wgsl's CompactParams, and packs them there", () => {
    const layout = declaredStructLayout(CLUSTER_COMPACT.reference, "CompactParams");
    expect([layout?.members.map((m) => [m.name, m.offsetBytes]), layout?.sizeBytes]).toEqual([
      [
        ["counts", 0],
        ["place", 16],
        ["listOffsets0", 32],
        ["listOffsets1", 48],
        ["listOffsets2", 64],
        ["listOffsets3", 80],
      ],
      COMPACT_PARAMS_BYTES,
    ]);
    const words = packCompactParams({
      instances: 7,
      clustersPerPage: 96,
      templates: 8,
      phase: "scatter",
      bucketBase: 40,
      listBase: 1000,
      templateOffsets: [0, 10, 20, 30, 40, 50, 60, 70],
    });
    expect([...words]).toEqual([
      7,
      96,
      8,
      2,
      40,
      1000,
      0,
      0,
      0,
      10,
      20,
      30,
      40,
      50,
      60,
      70,
      ...Array<number>(8).fill(0),
    ]);
    expect(() =>
      packCompactParams({
        instances: 1,
        clustersPerPage: 129,
        templates: 8,
        phase: "count",
        bucketBase: 0,
        listBase: 0,
        templateOffsets: [0, 0, 0, 0, 0, 0, 0, 0],
      }),
    ).toThrow(RangeError);
  });

  it("is bit-exact, its subgroup variant the reference but its count's subgroup sums", () => {
    const subgroup = CLUSTER_COMPACT.subgroup ?? "";
    expect([
      CLUSTER_COMPACT.readback,
      CLUSTER_COMPACT.reference.includes("enable subgroups"),
      subgroup.startsWith("enable subgroups;"),
      subgroup.includes("subgroupAdd("),
      tail(subgroup)
        .replace(", lane", "")
        .replace(", @builtin(subgroup_invocation_id) lane : u32", ""),
    ]).toEqual(["bit-exact", false, true, true, tail(CLUSTER_COMPACT.reference)]);
  });

  it("dispatches a workgroup an instance to count and to scatter, and one to scan", () => {
    expect([
      compactWorkgroups("count", 37),
      compactWorkgroups("scan", 37),
      compactWorkgroups("scatter", 37),
      compactWorkgroups("count", 0),
    ]).toEqual([
      [37, 1, 1],
      [1, 1, 1],
      [37, 1, 1],
      [1, 1, 1],
    ]);
  });

  it("writes each template's kept clusters in the draw set's order, instance by instance and cluster by cluster", () => {
    const next = random(0x14_2a);
    const instances = 23;
    const clusters = 96;
    const marks = randomMarks(next, instances, clusters, 0.4);
    const { lists, counts } = compactTwin(marks, instances, clusters, COMPACT_TEMPLATES);
    // The brute force: every kept (instance, cluster), sorted by its entry, grouped by template.
    const expected = Array.from({ length: COMPACT_TEMPLATES }, (_, t) =>
      [...marks.keys()]
        .filter((at) => marks[at] === t)
        .map((at) => Math.floor(at / clusters) * 256 + (at % clusters)),
    );
    expect(lists).toEqual(expected);
    expect(counts).toEqual(expected.map((list) => list.length));
    expect(counts.reduce((a, b) => a + b, 0)).toBe(marks.filter((m) => m !== NO_MARK).length);
  });

  it("scans each template's counts over random instances to their exclusive prefixes", () => {
    const next = random(7);
    for (const instances of [1, 2, 127, 128, 129, 1_208, 2_049]) {
      const tallies = Uint32Array.from({ length: instances * COMPACT_TEMPLATES }, () =>
        Math.floor(next() * 97),
      );
      const prefixes = scanTwin(tallies, instances);
      for (let t = 0; t < COMPACT_TEMPLATES; t += 1) {
        let running = 0;
        for (let n = 0; n < instances; n += 1) {
          expect(prefixes[n * COMPACT_TEMPLATES + t]).toBe(running);
          running += tallies[n * COMPACT_TEMPLATES + t] ?? 0;
        }
      }
    }
  });
});
