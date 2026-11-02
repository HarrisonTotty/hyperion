/**
 * The hulls' occluder faces on the cluster path (plan R14, R14.T4.b): each hull a page of mesh
 * clusters whose spheres hold every corner, a one-level DAG of (0, +∞), its unused records refused;
 * and the wireframe renderer's one cull and one bucket draw in the hull draws' place, the instanced
 * draws on the way back, for hulls of more than one fill and until the kernel is made, again after
 * a device restore.
 */

import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aCameraPose, NO_TURN } from "../../test/viewFixtures";
import type { Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import {
  CLUSTER_FLAG_DAG,
  CLUSTER_LOD_BYTES,
  CLUSTER_RECORD_BYTES,
  CLUSTER_SHAPE_CODE,
  meshTemplateRef,
} from "../clusters/record";
import type { DrawItem } from "../engine/types";
import { GEOMETRY_BEFORE_R14, SETTINGS } from "../quality/qualitySetting";
import { TEST_HULL } from "../scene/hull";
import { type DrawCamera, emptyDrawList, type OccluderMesh, viewStrokesAt } from "./drawList";
import { hullFaces } from "./hulls";
import { HULL_CLUSTERS_PER_PAGE, HULL_INSTANCE_BYTES, hullClusterFrame } from "./hullClusters";
import { HULL_CLUSTER_CULL_LABEL, WireframeRenderer } from "./submit";

const VIEWPORT: Viewport = { widthPx: 640, heightPx: 360 };
const CAMERA: DrawCamera = { pose: aCameraPose({ orientation: NO_TURN }), fovXRad: Math.PI / 3 };

/** `TEST_HULL`'s opaque faces at `originM` from the camera, as the draw list holds them. */
function hullAt(
  id: string,
  originM: readonly [number, number, number],
  fill: string | null,
): OccluderMesh {
  const faces = hullFaces(TEST_HULL, IDENTITY_QUATERNION);
  const triangles = new Float32Array(faces.length * 9);
  faces.forEach((corners, i) => {
    corners.forEach((corner, j) => {
      triangles.set([corner.x, corner.y, corner.z], i * 9 + j * 3);
    });
  });
  return { id, originF32: Float32Array.from(originM), triangles, twoSided: true, fill };
}

/** A mesh of `n` triangles fanned about its origin, each a little apart. */
function fan(n: number): OccluderMesh {
  const triangles = new Float32Array(9 * n);
  for (let t = 0; t < n; t += 1) {
    const a = (2 * Math.PI * t) / n;
    triangles.set(
      [0, 0, 0, Math.cos(a), Math.sin(a), 0.1 * t, Math.cos(a + 0.1), Math.sin(a + 0.1), 0],
      9 * t,
    );
  }
  return {
    id: "fan",
    originF32: new Float32Array([0, 0, -50]),
    triangles,
    twoSided: true,
    fill: null,
  };
}

/** The draw list of `meshes` alone. */
function listOf(meshes: ReadonlyArray<OccluderMesh>): ReturnType<typeof emptyDrawList> {
  return { ...emptyDrawList(viewStrokesAt(1)), occluderMeshes: meshes };
}

/** What a test compares of draws: material, mesh, instances and indirect offset. */
function drawShapes(draws: ReadonlyArray<DrawItem>): unknown {
  return draws.map((d) => [d.material.name, d.mesh.name, d.instanceCount, d.indirect?.offsetBytes]);
}

describe("the hulls' cluster pages (plan R14, R14.T4.b)", () => {
  it("bound every corner of each cluster by its sphere, 128 faces a cluster, the page's rest refused", () => {
    const meshes = [hullAt("a", [0, 0, -40], null), fan(300)];
    const frame = hullClusterFrame(meshes);
    if (frame === null) {
      throw new Error("no frame");
    }
    const records = new DataView(frame.records);
    const lods = new DataView(frame.lods);
    const instances = new DataView(frame.instances);
    const faces = meshes.map((mesh) => mesh.triangles.length / 9);
    expect([frame.hulls, frame.clusters, frame.triangles]).toEqual([2, 4, (faces[0] ?? 0) + 300]);
    let worst = 0;
    const empties: number[] = [];
    const seen: unknown[] = [];
    const wanted: unknown[] = [];
    meshes.forEach((mesh, page) => {
      const at = page * HULL_INSTANCE_BYTES;
      expect([
        instances.getUint32(at + 12, true),
        instances.getUint32(at + 16, true),
        instances.getUint32(at + 20, true),
      ]).toEqual([page, page === 0 ? 0 : faces[0], faces[page]]);
      for (let c = 0; c < HULL_CLUSTERS_PER_PAGE; c += 1) {
        const r = (page * HULL_CLUSTERS_PER_PAGE + c) * CLUSTER_RECORD_BYTES;
        const first = 128 * c;
        const count = Math.min(128, (faces[page] ?? 0) - first);
        if (count <= 0) {
          empties.push(records.getFloat32(r + 12, true));
          continue;
        }
        const centre = [0, 4, 8].map((o) => records.getFloat32(r + o, true));
        const radius = records.getFloat32(r + 12, true);
        for (let n = 9 * first; n < 9 * (first + count); n += 3) {
          const d = Math.hypot(
            (mesh.triangles[n] ?? 0) - (centre[0] ?? 0),
            (mesh.triangles[n + 1] ?? 0) - (centre[1] ?? 0),
            (mesh.triangles[n + 2] ?? 0) - (centre[2] ?? 0),
          );
          worst = Math.max(worst, d / radius);
        }
        const l = (page * HULL_CLUSTERS_PER_PAGE + c) * CLUSTER_LOD_BYTES;
        seen.push([
          records.getFloat32(r + 16, true),
          records.getFloat32(r + 20, true),
          records.getUint32(r + 24, true),
          records.getUint32(r + 28, true),
          [0, 4, 8, 12].map((o) => lods.getFloat32(l + o, true)),
        ]);
        wanted.push([
          0,
          Infinity,
          meshTemplateRef(first, count),
          CLUSTER_SHAPE_CODE.mesh | CLUSTER_FLAG_DAG,
          [...centre, radius],
        ]);
      }
    });
    expect(seen).toEqual(wanted);
    // Of the two pages' 16 records, 4 clusters: 12 refused by their negative radii.
    expect([empties.length, empties.every((radius) => radius < 0)]).toEqual([12, true]);
    expect(worst).toBeLessThanOrEqual(1);
  });

  it("leaves a hull of more faces than a page holds to the instanced draws", () => {
    expect([hullClusterFrame([fan(1024)]) === null, hullClusterFrame([fan(1025)])]).toEqual([
      false,
      null,
    ]);
  });
});

describe("the wireframe renderer's hulls on the cluster path (plan R14, R14.T4.b)", () => {
  it("culls every hull once and draws them by one bucket draw in the first hull draw's place", async () => {
    const engine = await countingRenderEngine();
    const renderer = new WireframeRenderer(engine);
    expect(await renderer.clustersReady()).toBe(true);
    const list = listOf([hullAt("a", [0, 0, -40], null), hullAt("b", [5, 0, -60], null)]);
    const frame = renderer.frame(list, CAMERA, VIEWPORT);
    const culls = engine.dispatched.filter((d) => d.pass === HULL_CLUSTER_CULL_LABEL);
    expect([culls.map((d) => d.workgroups), drawShapes(frame.draws)]).toEqual([
      [[2, 1, 1]],
      [["wireframe:occluderHullCluster", "mesh clusters", undefined, 0]],
    ]);
    expect(renderer.lastHullClusters?.clusters).toBe(2);
    const filled = renderer.frame(
      listOf([hullAt("a", [0, 0, -40], "#101418"), hullAt("b", [5, 0, -60], "#101418")]),
      CAMERA,
      VIEWPORT,
    );
    expect(drawShapes(filled.draws)).toEqual([
      ["wireframe:hullSilhouetteCluster", "mesh clusters", undefined, 0],
    ]);
    renderer.dispose();
  });

  it.each([
    ["before its kernel is made", null],
    ["on the way back", GEOMETRY_BEFORE_R14],
    ["with the hulls' lever off", { ...SETTINGS.high.geometry, hullClusters: false }],
    [
      "with the cluster path off, the hulls' lever on",
      { ...SETTINGS.high.geometry, clusterPath: false },
    ],
  ] as const)("draws each hull instanced, culling nothing, %s", async (_, geometry) => {
    const engine = await countingRenderEngine();
    const renderer = new WireframeRenderer(engine);
    const list = listOf([hullAt("a", [0, 0, -40], null), hullAt("b", [5, 0, -60], null)]);
    const faces = (list.occluderMeshes[0]?.triangles.length ?? 0) / 9;
    if (geometry !== null) {
      // Its kernel made, and the geometry asked.
      await renderer.clustersReady();
      renderer.setGeometry(geometry);
    }
    const frame = renderer.frame(list, CAMERA, VIEWPORT);
    expect([
      drawShapes(frame.draws),
      engine.dispatched.filter((d) => d.pass === HULL_CLUSTER_CULL_LABEL).length,
    ]).toEqual([
      [
        ["wireframe:occluderHull", "wireframe:triangle", faces, undefined],
        ["wireframe:occluderHull", "wireframe:triangle", faces, undefined],
      ],
      0,
    ]);
    renderer.dispose();
  });

  it("draws hulls of two fills instanced, the depth's and the silhouette's", async () => {
    const engine = await countingRenderEngine();
    const renderer = new WireframeRenderer(engine);
    await renderer.clustersReady();
    const list = listOf([hullAt("a", [0, 0, -40], null), hullAt("b", [5, 0, -60], "#101418")]);
    const faces = (list.occluderMeshes[0]?.triangles.length ?? 0) / 9;
    expect(drawShapes(renderer.frame(list, CAMERA, VIEWPORT).draws)).toEqual([
      ["wireframe:occluderHull", "wireframe:triangle", faces, undefined],
      ["wireframe:hullSilhouette", "wireframe:triangle", faces, undefined],
    ]);
    renderer.dispose();
  });

  it("makes its kernel and buckets again after a device restore, and draws a fresh renderer's items", async () => {
    const engine = await countingRenderEngine();
    const renderer = new WireframeRenderer(engine);
    await renderer.clustersReady();
    const list = listOf([hullAt("a", [0, 0, -40], null)]);
    renderer.frame(list, CAMERA, VIEWPORT);
    engine.loseDevice();
    engine.restore();
    expect(await renderer.clustersReady()).toBe(true);
    const restored = renderer.frame(list, CAMERA, VIEWPORT);
    const fresh = new WireframeRenderer(engine);
    await fresh.clustersReady();
    const made = fresh.frame(list, CAMERA, VIEWPORT);
    expect(drawShapes(restored.draws)).toEqual(drawShapes(made.draws));
    expect(restored.draws[0]?.uniforms).toEqual(made.draws[0]?.uniforms);
    expect(engine.dispatched.filter((d) => d.pass === HULL_CLUSTER_CULL_LABEL)).toHaveLength(3);
    renderer.dispose();
    fresh.dispose();
  });

  it("releases its cluster buffers when disposed", async () => {
    const engine = await countingRenderEngine();
    const renderer = new WireframeRenderer(engine);
    await renderer.clustersReady();
    renderer.frame(listOf([hullAt("a", [0, 0, -40], null)]), CAMERA, VIEWPORT);
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    renderer.dispose();
    expect(released.toSorted()).toEqual(
      [
        "wireframe:hull clusters lists",
        "wireframe:hull clusters arguments",
        "wireframe:hull clusters cull inputs",
        "wireframe:hull clusters marks",
        "wireframe:hull cluster records",
        "wireframe:hull cluster lods",
        "wireframe:hull cluster instances",
        // The mesh occluders' instances (plan R14, R14.T5.f); their buckets, made at their first
        // cull, are not made here.
        "wireframe:mesh occluder instances",
      ].toSorted(),
    );
  });
});
