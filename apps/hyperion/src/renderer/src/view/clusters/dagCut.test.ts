/**
 * The cluster cull's DAG cut (plan R14, R14.T4.b): over random monotone DAGs with nested LOD
 * spheres the twin draws one consistent cut, exactly one cluster of each leaf's chain of groups,
 * at any camera and tolerance; the mesh records' and LOD spheres' packing; and the mesh shape's one
 * template, 128 triangles by index pulling.
 */

import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import { cullTwin, dagCutDrawn, type CullInstance } from "./cull";
import type { CameraCullPass, CascadeCullPass, CullPass } from "./cullPass";
import {
  CLUSTER_FLAG_DAG,
  CLUSTER_LOD_BYTES,
  CLUSTER_SHAPE_CODE,
  type ClusterLod,
  type ClusterRecord,
  clusterFlags,
  MESH_CLUSTER_TRIANGLES,
  meshTemplateRef,
  packClusterLod,
} from "./record";
import { meshClusterMesh, meshTemplate } from "./templates";

/** One group of a random DAG: its sphere, its own error, its parent, and whether it is a leaf. */
interface Group {
  readonly centreM: Vec3;
  readonly radiusM: number;
  readonly errorM: number;
  readonly parent: number | null;
  readonly leaf: boolean;
}

/**
 * A random tree of groups `levels` deep from its root, each child's sphere inside its parent's and
 * its error below its parent's (0 at the leaves): the monotone DAG with nested spheres that the cut
 * is consistent over (Karis 2021).
 */
function randomDag(random: () => number, levels: number): Group[] {
  const groups: Group[] = [
    {
      centreM: { x: 0, y: 0, z: 0 },
      radiusM: 1_000,
      errorM: 1_000 * random(),
      parent: null,
      leaf: levels === 1,
    },
  ];
  const grow = (at: number, level: number): void => {
    const parent = groups[at];
    if (parent === undefined || level >= levels) {
      return;
    }
    const children = 2 + Math.floor(3 * random());
    for (let k = 0; k < children; k += 1) {
      const radiusM = parent.radiusM * (0.2 + 0.5 * random());
      const reach = (parent.radiusM - radiusM) * random();
      const u = 2 * Math.PI * random();
      const v = Math.acos(2 * random() - 1);
      groups.push({
        centreM: {
          x: parent.centreM.x + reach * Math.sin(v) * Math.cos(u),
          y: parent.centreM.y + reach * Math.sin(v) * Math.sin(u),
          z: parent.centreM.z + reach * Math.cos(v),
        },
        radiusM,
        errorM: level + 1 === levels ? 0 : parent.errorM * random(),
        parent: at,
        leaf: level + 1 === levels,
      });
      grow(groups.length - 1, level + 1);
    }
  };
  grow(0, 1);
  return groups;
}

/** A group's record and LOD sphere as a DAG-cut cluster: own and parent errors and spheres. */
function recordOf(
  groups: ReadonlyArray<Group>,
  g: Group,
): { record: ClusterRecord; lod: ClusterLod } {
  const parent = g.parent === null ? null : (groups[g.parent] ?? null);
  return {
    record: {
      centreM: g.centreM,
      radiusM: g.radiusM,
      mode: "dag",
      errorsM: [g.errorM, parent?.errorM ?? Infinity],
      templateRef: meshTemplateRef(0, MESH_CLUSTER_TRIANGLES),
      shape: "mesh",
      mayDiscard: false,
      material: 0,
    },
    lod: {
      parentCentreM: parent?.centreM ?? g.centreM,
      parentRadiusM: parent?.radiusM ?? g.radiusM,
    },
  };
}

/** A camera pass with no planes, which keeps every sphere but the DAG's cut. */
function cameraPass(toleranceRad: number): CameraCullPass {
  return {
    kind: "camera",
    planes: [],
    horizon: null,
    order: "appended",
    toleranceRad,
    strideCap: 1,
  };
}

/** The twin's drawn groups of `groups` from a camera at `cameraM`, as one page of one instance. */
function drawnGroups(groups: ReadonlyArray<Group>, cameraM: Vec3, pass: CullPass): Set<number> {
  const instance: CullInstance = {
    originM: { x: -cameraM.x, y: -cameraM.y, z: -cameraM.z },
    page: 0,
    passMask: 1,
    unitM: 0,
  };
  const buckets = cullTwin(
    {
      instances: [instance],
      records: (_, cluster) => {
        const g = groups[cluster];
        if (g === undefined) {
          throw new Error(`no group ${String(cluster)}`);
        }
        return recordOf(groups, g).record;
      },
      lods: (_, cluster) => {
        const g = groups[cluster];
        if (g === undefined) {
          throw new Error(`no group ${String(cluster)}`);
        }
        return recordOf(groups, g).lod;
      },
      clustersPerPage: groups.length,
      rotationColumns: [
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 1, z: 0 },
        { x: 0, y: 0, z: 1 },
      ],
      passes: [pass],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 1,
      viewAxis: { x: 0, y: 0, z: -1 },
      firstShape: CLUSTER_SHAPE_CODE.mesh,
    },
    0,
  );
  return new Set((buckets.get(0) ?? []).map((entry) => entry & 0xff));
}

describe("the cluster cull's DAG cut (plan R14, R14.T4.b)", () => {
  it("draws exactly one cluster of each leaf's chain of groups, over random monotone DAGs, cameras and tolerances", () => {
    const random = seededRandom(44);
    let cases = 0;
    let mixed = 0;
    for (let n = 0; n < 200; n += 1) {
      const groups = randomDag(random, 2 + Math.floor(3 * random()));
      // The twin's lists hold eight bits of cluster: a page of at most 256.
      if (groups.length > 256) {
        continue;
      }
      const distanceM = 10 ** (1 + 5 * random());
      const cameraM = {
        x: distanceM * (random() - 0.5),
        y: distanceM * (random() - 0.5),
        z: distanceM * (random() - 0.5),
      };
      const pass =
        random() < 0.8
          ? cameraPass(10 ** (-5 + 3 * random()))
          : ({
              kind: "cascade",
              axes: [
                { x: 1, y: 0, z: 0 },
                { x: 0, y: 1, z: 0 },
                { x: 0, y: 0, z: 1 },
              ],
              centreFromCameraLightM: { x: -cameraM.x, y: -cameraM.y, z: -cameraM.z },
              radiusM: 1e6,
              depthFarM: 1e6,
              toleranceM: 1_000 * random(),
              strideCap: 1,
              receivers: null,
            } satisfies CascadeCullPass);
      const drawn = drawnGroups(groups, cameraM, pass);
      groups.forEach((g, leaf) => {
        if (!g.leaf) {
          return;
        }
        const chain: number[] = [];
        for (let at: number | null = leaf; at !== null; at = groups[at]?.parent ?? null) {
          chain.push(at);
        }
        expect(chain.filter((at) => drawn.has(at))).toHaveLength(1);
      });
      cases += 1;
      mixed += drawn.size > 1 && !groups.every((g, at) => g.leaf === drawn.has(at)) ? 1 : 0;
    }
    // The cut falls between the leaves and the root in many of them, not only at either end.
    expect([cases > 150, mixed > 20]).toEqual([true, true]);
  });

  it("draws a cluster where its own error is within the tolerance and its parent's is not, each at its group's sphere", () => {
    const pass = cameraPass(1e-3);
    const near = { x: 0, y: 0, z: -110 };
    // 100 m to the group's nearest point: 0.1 m is its error at 1 mrad.
    expect([
      dagCutDrawn(pass, [0.1, 0.2], near, 10, near, 10),
      dagCutDrawn(pass, [0.1, 0.1], near, 10, near, 10),
      dagCutDrawn(pass, [0.11, Infinity], near, 10, near, 10),
      // Inside its sphere only an error of 0 is within any tolerance: a leaf there is drawn, and
      // its parent group not.
      dagCutDrawn(pass, [0, 1e-9], { x: 0, y: 0, z: -5 }, 10, { x: 0, y: 0, z: -5 }, 10),
      dagCutDrawn(pass, [1e-9, Infinity], { x: 0, y: 0, z: -5 }, 10, near, 10),
    ]).toEqual([true, false, false, true, false]);
  });

  it("refuses a record of no cluster, its radius negative, in every pass", () => {
    const lone: Group = {
      centreM: { x: 0, y: 0, z: 0 },
      radiusM: 1,
      errorM: 0,
      parent: null,
      leaf: true,
    };
    const none = { ...recordOf([lone], lone).record, radiusM: -1 };
    expect(
      cullTwin(
        {
          instances: [{ originM: { x: 0, y: 0, z: -10 }, page: 0, passMask: 1, unitM: 0 }],
          records: () => none,
          lods: () => ({ parentCentreM: none.centreM, parentRadiusM: 1 }),
          clustersPerPage: 1,
          rotationColumns: [
            { x: 1, y: 0, z: 0 },
            { x: 0, y: 1, z: 0 },
            { x: 0, y: 0, z: 1 },
          ],
          passes: [cameraPass(1)],
          places: [{ bucketBase: 0, listBase: 0 }],
          templates: 1,
          viewAxis: { x: 0, y: 0, z: -1 },
          firstShape: CLUSTER_SHAPE_CODE.mesh,
        },
        2,
      ).size,
    ).toBe(0);
  });
});

describe("the mesh shape's records and template (plan R14, R14.T4.b)", () => {
  it("packs a template reference as its first triangle in bits 0–23 and its count in 24–31, as tools/meshbuild writes it", () => {
    expect([
      meshTemplateRef(0, 128).toString(16),
      meshTemplateRef(0xab_cdef, 7).toString(16),
    ]).toEqual(["80000000", "7abcdef"]);
    expect(() => meshTemplateRef(1 << 24, 1)).toThrow(RangeError);
    expect(() => meshTemplateRef(0, 129)).toThrow(RangeError);
    expect(() => meshTemplateRef(0, 0)).toThrow(RangeError);
  });

  it("flags a DAG record's mode and packs its LOD sphere in 16 B", () => {
    const lone: Group = {
      centreM: { x: 1, y: 2, z: 3 },
      radiusM: 4,
      errorM: 0,
      parent: null,
      leaf: true,
    };
    const { record, lod } = recordOf([lone], lone);
    const out = new DataView(new ArrayBuffer(2 * CLUSTER_LOD_BYTES));
    packClusterLod(out, CLUSTER_LOD_BYTES, lod);
    expect([clusterFlags(record), [16, 20, 24, 28].map((o) => out.getFloat32(o, true))]).toEqual([
      CLUSTER_SHAPE_CODE.mesh | CLUSTER_FLAG_DAG,
      [1, 2, 3, 4],
    ]);
    expect(() => {
      packClusterLod(out, CLUSTER_LOD_BYTES + 1, lod);
    }).toThrow(RangeError);
  });

  it("draws 128 triangles by index pulling, each vertex its corner and triangle", () => {
    const { templates, mesh } = meshClusterMesh();
    const positions = Array.from({ length: 384 }, (_, k) => [
      mesh.positions[3 * k],
      mesh.positions[3 * k + 1],
      mesh.positions[3 * k + 2],
    ]);
    expect([
      templates,
      [meshTemplate()],
      mesh.indices?.length,
      positions[0],
      positions[5],
      positions[383],
    ]).toEqual([
      [
        {
          shape: "mesh",
          stride: 1,
          triangles: 128,
          firstIndex: 0,
          indexCount: 384,
          errorScale: 1,
          mayDiscard: false,
        },
      ],
      templates,
      384,
      [0, 0, 0],
      [2, 1, 0],
      [2, 127, 0],
    ]);
  });
});
