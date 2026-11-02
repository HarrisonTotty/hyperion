/**
 * The mesh regime's figure on the cluster path (plan R14, R14.T4.a): its templates draw R05's patch
 * mesh's own triangles, its records' spheres hold every vertex its draw places, morphs and skirts
 * included (the twin, `smoothPatchVertices`, R05's `FaceDifferences` at h = 0 in `f64`), and the
 * renderer culls each figure into its buckets and draws them, or its instanced draw on the way back
 * and until its kernel is made, again after a device restore.
 */

import { describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc } from "../../test/litFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import type { DrawItem } from "../engine/types";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import {
  CLUSTER_FLAG_MAY_DISCARD,
  CLUSTER_RECORD_BYTES,
  CLUSTER_SHAPE_CODE,
} from "../clusters/record";
import {
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  terrainClusterGrid,
  terrainClusterPlace,
  terrainTemplateRef,
} from "../clusters/terrain";
import { STRIDES } from "../clusters/templates";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { GEOMETRY_BEFORE_R14, SETTINGS } from "../quality/qualitySetting";
import { AU_M } from "../scenes/kept";
import { patchMeshData } from "../terrain/gpu/resources";
import { INSTANCE_RECORD_BYTES } from "../terrain/gpu/uniforms";
import type { BodyFigure } from "../terrain/planet";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import {
  type BodyFramePlan,
  FIGURE_CLUSTER_CULL_LABEL,
  FIGURE_CLUSTER_MESH,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "./draw";
import { sphereFootprint } from "./regime";
import { packSmoothClusterRecords, packSmoothMeshes, smoothPatchVertices } from "./smoothMesh";

const RAD = Math.PI / 180;
const VIEW: Viewport = { widthPx: 256, heightPx: 144 };
const EARTH: BodyFigure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null };
const SATURN: BodyFigure = {
  equatorialRadiusM: 6.0268e7,
  polarRadiusM: 6.0268e7 * (1 - 0.098),
  pole: normalise(vec3(0, 1, 0.4)),
};
const STRAIGHT: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
/** From 400 km up, pitched 15° down: the horizon in view, patches of many levels. */
const LOW_ORBIT: ProjectionCamera = {
  orientation: lookAlong(vec3(0, -Math.sin(15 * RAD), -Math.cos(15 * RAD)), vec3(0, 1, 0)),
  fovXRad: Math.PI / 3,
};

/** One body promoted to a mesh over a depth writer, lit by a Sun 1 au off. */
function meshPlan(figure: BodyFigure, centreM: Vec3, camera: ProjectionCamera): BodyFramePlan {
  const body: LitBodyInput = {
    id: "0200080020000000.0001",
    centreM,
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
  const writer = sphereFootprint(centreM, figure.equatorialRadiusM, camera, VIEW);
  const plan = planLitBodies(
    [body],
    [{ disc: aHostDisc(), centreM: add(centreM, scale(vec3(0.6, 0.6, 0.5), AU_M)) }],
    {
      camera,
      viewport: VIEW,
      presentedPx: VIEW,
      exposureScale: 1e-3,
      annuli: 4,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
      depthWriters: writer === null ? [] : [writer],
    },
    new Map([[body.id, "disc"]]),
  );
  if (plan.meshes.length !== 1 || (plan.meshes[0]?.mesh.patches.length ?? 0) === 0) {
    throw new Error("the body was not drawn as a mesh of patches");
  }
  return plan;
}

/** The three figures the tests draw: an Earth ahead, a tilted giant and an Earth from low orbit. */
function plans(): ReadonlyArray<readonly [string, BodyFramePlan]> {
  return [
    ["an Earth 40° across", meshPlan(EARTH, vec3(0, 0, -1.9e7), STRAIGHT)],
    ["a Saturn-like giant, its pole tilted", meshPlan(SATURN, vec3(2e7, 1e7, -3e8), STRAIGHT)],
    ["an Earth from 400 km", meshPlan(EARTH, vec3(0, -(6.371e6 + 4e5), 0), LOW_ORBIT)],
  ];
}

/** A template vertex's (lx, ly, code) in the figure's cluster mesh. */
function local(v: number): readonly [number, number, number] {
  const p = FIGURE_CLUSTER_MESH.mesh.positions;
  return [p[3 * v] ?? Number.NaN, p[3 * v + 1] ?? Number.NaN, p[3 * v + 2] ?? Number.NaN];
}

/** Template `t`'s triangles of `cluster`, each its vertices' R05 patch mesh (x, y, skirt). */
function clusterTriangles(t: number, cluster: number): string[] {
  const template = FIGURE_CLUSTER_MESH.templates[t];
  const indices = FIGURE_CLUSTER_MESH.mesh.indices ?? new Uint32Array();
  const out: string[] = [];
  for (let n = 0; n < (template?.indexCount ?? 0); n += 3) {
    const at = (template?.firstIndex ?? 0) + n;
    out.push(
      [0, 1, 2]
        .map((k) => terrainClusterGrid(cluster, local(indices[at + k] ?? 0)).join(" "))
        .join(" | "),
    );
  }
  return out;
}

/** What a test compares of draws: material, mesh, indirect offset and uniforms. */
function drawShapes(draws: ReadonlyArray<DrawItem>): unknown {
  return draws.map((d) => [d.material.name, d.mesh.name, d.indirect?.offsetBytes, d.uniforms]);
}

describe("the mesh regime's figure on the cluster path (plan R14, R14.T4.a)", () => {
  it("draws at stride 1 exactly R05's patch mesh's triangles, each in its own vertex order", () => {
    const patch = patchMeshData();
    const at = (v: number): string =>
      [0, 1, 2].map((k) => String(patch.positions[3 * v + k] ?? Number.NaN)).join(" ");
    const instanced: string[] = [];
    for (let n = 0; n < patch.indices.length; n += 3) {
      instanced.push([0, 1, 2].map((k) => at(patch.indices[n + k] ?? 0)).join(" | "));
    }
    const drawn: string[] = [];
    for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
      drawn.push(...clusterTriangles(cluster < TERRAIN_BLOCKS ? 0 : STRIDES.length, cluster));
    }
    expect(drawn.length).toBe(instanced.length);
    expect(new Set(drawn)).toEqual(new Set(instanced));
  });

  it("holds every vertex its draw places, morphed and skirted, inside its cluster's sphere", () => {
    for (const [name, plan] of plans()) {
      const meshes = plan.meshes.map(({ mesh }) => mesh);
      const records = new DataView(packSmoothClusterRecords(meshes));
      const instances = new DataView(packSmoothMeshes(meshes).instances.buffer);
      const patch = patchMeshData();
      let page = 0;
      let worst = -Infinity;
      for (const mesh of meshes) {
        for (const each of mesh.patches) {
          // The cull reads the instance's fourth word as its page.
          expect(instances.getUint32(page * INSTANCE_RECORD_BYTES + 12, true)).toBe(page);
          const vertices = smoothPatchVertices(mesh, each);
          const byPlace = new Map<string, Vec3>();
          vertices.forEach((v, n) => {
            const key = [0, 1, 2].map((k) => String(patch.positions[3 * n + k] ?? 0)).join(" ");
            byPlace.set(key, v);
          });
          for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
            const at = (page * TERRAIN_CLUSTERS_PER_PAGE + cluster) * CLUSTER_RECORD_BYTES;
            const o = each.originFromCameraM;
            const centre = vec3(
              o.x + records.getFloat32(at, true),
              o.y + records.getFloat32(at + 4, true),
              o.z + records.getFloat32(at + 8, true),
            );
            const radiusM = records.getFloat32(at + 12, true);
            const t = cluster < TERRAIN_BLOCKS ? 0 : STRIDES.length;
            for (const triangle of clusterTriangles(t, cluster)) {
              for (const corner of triangle.split(" | ")) {
                const v = byPlace.get(corner);
                if (v === undefined) {
                  throw new Error(`no vertex at ${corner}`);
                }
                worst = Math.max(
                  worst,
                  Math.hypot(v.x - centre.x, v.y - centre.y, v.z - centre.z) / radiusM,
                );
              }
            }
          }
          page += 1;
        }
      }
      expect([name, worst <= 1]).toEqual([name, true]);
    }
  });

  it("writes each record strided, may-discard and unmeasured, with terrain's template references", () => {
    const [, plan] = plans()[0] ?? [];
    if (plan === undefined) {
      throw new Error("no plan");
    }
    const records = new DataView(packSmoothClusterRecords(plan.meshes.map(({ mesh }) => mesh)));
    for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
      const at = cluster * CLUSTER_RECORD_BYTES;
      expect([
        records.getUint32(at + 16, true),
        records.getFloat32(at + 20, true),
        records.getUint32(at + 24, true),
        records.getUint32(at + 28, true),
      ]).toEqual([
        0,
        Infinity,
        terrainTemplateRef(cluster),
        CLUSTER_SHAPE_CODE[terrainClusterPlace(cluster).shape] | CLUSTER_FLAG_MAY_DISCARD,
      ]);
    }
  });
});

describe("the lit bodies' renderer on the figures' cluster path (plan R14, R14.T4.a)", () => {
  it("culls each figure into its pass's buckets and draws its stride-1 blocks and strips, one dispatch a plan", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    expect(await renderer.clustersReady()).toBe(true);
    const [, plan] = plans()[2] ?? [];
    if (plan === undefined) {
      throw new Error("no plan");
    }
    const patches = plan.meshes[0]?.mesh.patches.length ?? 0;
    const draws = renderer.meshDraws(plan);
    renderer.meshDraws(plan);
    const culls = engine.dispatched.filter((d) => d.pass === FIGURE_CLUSTER_CULL_LABEL);
    expect(culls.map((d) => d.workgroups)).toEqual([[patches, 1, 1]]);
    expect(
      draws.map((d) => [d.material.name, d.mesh.name, d.indirect?.offsetBytes, d.instanceCount]),
    ).toEqual([
      ["bodies:smoothMeshCluster", "figure clusters", 0, undefined],
      ["bodies:smoothMeshCluster", "figure clusters", 4 * 20, undefined],
    ]);
    expect(draws.map((d) => d.uniforms["firstInstance"]?.[0])).toEqual([0, 0]);
    expect(draws.every((d) => d.storageBuffers?.["clusterLists"] !== undefined)).toBe(true);
    renderer.dispose();
  });

  it.each([
    ["before its kernel is made", null],
    ["on the way back", GEOMETRY_BEFORE_R14],
    ["with the figures' lever off", { ...SETTINGS.high.geometry, figureClusters: false }],
    [
      "with the cluster path off, the figures' lever on",
      { ...SETTINGS.high.geometry, clusterPath: false },
    ],
  ] as const)("draws each figure instanced, culling nothing, %s", async (_, geometry) => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const [, plan] = plans()[0] ?? [];
    if (plan === undefined) {
      throw new Error("no plan");
    }
    const patches = plan.meshes[0]?.mesh.patches.length ?? 0;
    if (geometry !== null) {
      // Its kernel made, and the geometry asked.
      await renderer.clustersReady();
      renderer.setGeometry(geometry);
    }
    const draws = renderer.meshDraws(plan);
    expect([
      draws.map((d) => [d.material.name, d.instanceCount, d.indirect]),
      engine.dispatched.filter((d) => d.pass === FIGURE_CLUSTER_CULL_LABEL).length,
    ]).toEqual([[["bodies:smoothMesh", patches, undefined]], 0]);
    renderer.dispose();
  });

  it("makes its kernel and buckets again after a device restore, and draws a fresh renderer's items", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await renderer.clustersReady();
    const [, plan] = plans()[1] ?? [];
    if (plan === undefined) {
      throw new Error("no plan");
    }
    renderer.meshDraws(plan);
    engine.loseDevice();
    engine.restore();
    expect(await renderer.clustersReady()).toBe(true);
    const restored = renderer.meshDraws(plan);
    const fresh = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    await fresh.clustersReady();
    const made = fresh.meshDraws(plan);
    expect(drawShapes(restored)).toEqual(drawShapes(made));
    expect(engine.dispatched.filter((d) => d.pass === FIGURE_CLUSTER_CULL_LABEL)).toHaveLength(3);
    renderer.dispose();
    fresh.dispose();
  });
});
