/**
 * The DAG consumers' occluder depth in the wireframe (plan R14, R14.T5.f): each resident page of a
 * pool mesh one instance of the cull, its origin the mesh's plus its pages', and the wireframe
 * renderer's one cull and one bucket draw among the occluders, over the pool's pages and records,
 * at `meshTolerancePx.wireframeOccluder`; none off the cluster path, with no pool, or before the
 * kernel is made.
 */

import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { type MeshFileRead, readMeshFile } from "../../test/meshFlyby";
import { threeLevelMesh, writeMeshFile } from "../../test/meshPages";
import { aCameraPose, NO_TURN } from "../../test/viewFixtures";
import type { Viewport } from "../camera/projection";
import { GeometryPagePool, pagePoolSlots, pageSlotOffsetBytes } from "../clusters/pages";
import { SETTINGS } from "../quality/qualitySetting";
import type { DrawItem, FrameSubmission } from "../engine/types";
import { type DrawCamera, emptyDrawList, viewStrokesAt, type WireframeDrawList } from "./drawList";
import {
  MESH_OCCLUDER_INSTANCE_BYTES,
  type MeshOccluder,
  meshOccluderFrame,
  wireframeToleranceRad,
} from "./meshOccluders";
import { MESH_OCCLUDER_CULL_LABEL, MESH_OCCLUDER_MATERIAL, WireframeRenderer } from "./submit";

const VIEWPORT: Viewport = { widthPx: 640, heightPx: 360 };
const CAMERA: DrawCamera = { pose: aCameraPose({ orientation: NO_TURN }), fovXRad: Math.PI / 3 };

/** The three-level DAG's pages' origin in its frame, m, off the frame's origin. */
const PAGE_ORIGIN_M = { x: 1.5, y: -2, z: 4 } as const;

/** The three-level DAG read whole, its pages about {@link PAGE_ORIGIN_M}. */
function dag(): MeshFileRead {
  const read = readMeshFile(writeMeshFile({ ...threeLevelMesh(), originM: PAGE_ORIGIN_M }));
  if (read.kind === "refused") {
    throw new Error(read.reason);
  }
  return read.value;
}

/** `pages` of the three-level DAG stored in `pool` as a new mesh, its handle. */
function stored(pool: GeometryPagePool, pages: ReadonlyArray<number>): number {
  const file = dag();
  const mesh = pool.addMesh(file.index);
  for (const p of pages) {
    const page = file.pages[p];
    if (page === undefined || pool.insert(mesh, page).kind !== "stored") {
      throw new Error(`page ${String(p)} was not stored`);
    }
  }
  return mesh;
}

/** An occluder of `mesh` at `originM` from the camera. */
function occluder(mesh: number, originM: readonly [number, number, number]): MeshOccluder {
  return { id: `mesh ${String(mesh)}`, mesh, originF32: Float32Array.from(originM) };
}

/** A renderer on `engine` with its kernel made, drawing mesh occluders from `pool`. */
async function rendererOf(
  engine: Awaited<ReturnType<typeof countingRenderEngine>>,
  pool: GeometryPagePool | null,
): Promise<WireframeRenderer> {
  const renderer = new WireframeRenderer(engine);
  await renderer.clustersReady();
  renderer.setMeshPages(pool);
  return renderer;
}

/** The draw list of `occluders` alone. */
function listOf(occluders: ReadonlyArray<MeshOccluder>): WireframeDrawList {
  return { ...emptyDrawList(viewStrokesAt(1)), meshOccluders: occluders };
}

/** A frame's mesh occluder draw, if it has one. */
function meshDrawOf(frame: FrameSubmission): DrawItem | undefined {
  return frame.draws.find((d) => d.material.name === MESH_OCCLUDER_MATERIAL.name);
}

describe("the mesh occluders' instances (plan R14, R14.T5.f)", () => {
  it("makes an instance of each resident page: the mesh's origin plus its pages', its slot and its page's first word", async () => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const mesh = stored(pool, [0, 1, 3]);
    const frame = meshOccluderFrame(pool, [occluder(mesh, [1, 2, -30])]);
    if (frame === null) {
      throw new Error("no frame");
    }
    const view = new DataView(frame.instances);
    const seen = [0, 1, 2].map((n) => {
      const at = n * MESH_OCCLUDER_INSTANCE_BYTES;
      return [
        [0, 4, 8].map((k) => view.getFloat32(at + k, true)),
        view.getUint32(at + 12, true),
        view.getUint32(at + 16, true),
      ];
    });
    expect([frame.count, frame.meshes, seen]).toEqual([
      3,
      1,
      pool
        .residentPages(mesh)
        .map(([, slot]) => [[2.5, 0, -26], slot, pageSlotOffsetBytes(pool.layout, slot) / 4]),
    ]);
  });

  it("makes none for a mesh with no page resident", async () => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const mesh = pool.addMesh(dag().index);
    expect(meshOccluderFrame(pool, [occluder(mesh, [0, 0, -30])])).toBeNull();
  });
});

describe("the mesh occluders' tolerance (plan R14, R14.T5.f)", () => {
  it("takes a tolerance in px over the view's px per radian at its corners, the most anywhere in it", () => {
    const tanHalf = Math.tan(Math.PI / 6);
    const corner = tanHalf * tanHalf * (1 + (360 / 640) ** 2);
    expect(wireframeToleranceRad(0.5, Math.PI / 3, VIEWPORT)).toBeCloseTo(
      (0.5 * 2 * tanHalf) / 640 / (1 + corner),
      15,
    );
  });
});

describe("the mesh occluders' material (plan R14, R14.T5.f)", () => {
  it("writes depth alone", () => {
    expect([MESH_OCCLUDER_MATERIAL.colourWrites, MESH_OCCLUDER_MATERIAL.depthWrite]).toEqual([
      false,
      true,
    ]);
  });

  it("reads a bucket's list, the pool's pages, records and errors beside the hulls' corners", () => {
    expect(MESH_OCCLUDER_MATERIAL.storageBuffers?.map((b) => [b.name, b.binding])).toEqual([
      ["corners", 0],
      ["meshInstances", 1],
      ["clusterLists", 2],
      ["meshPages", 3],
      ["meshRecords", 4],
      ["meshErrors", 5],
    ]);
  });

  it("takes a bucket's base after the hull occluder's uniforms", () => {
    expect(MESH_OCCLUDER_MATERIAL.uniforms.at(-1)).toEqual({ name: "clusterBase", type: "u32" });
  });

  it("has one vertex stage, its own, and the hull occluder's fragment stage", () => {
    expect([
      MESH_OCCLUDER_MATERIAL.vertexWgsl.split("fn vertexMain(").length,
      MESH_OCCLUDER_MATERIAL.vertexWgsl.includes("mesh_instances[item >> 8u]"),
      MESH_OCCLUDER_MATERIAL.fragmentWgsl.includes("fn fragmentMain("),
    ]).toEqual([2, true, true]);
  });
});

describe("the wireframe renderer's mesh occluders (plan R14, R14.T5.f)", () => {
  it("culls every resident page once, a workgroup each", async () => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const renderer = await rendererOf(engine, pool);
    renderer.frame(listOf([occluder(stored(pool, [0, 1, 2]), [-8, 0, -30])]), CAMERA, VIEWPORT);
    expect([
      engine.dispatched.filter((d) => d.pass === MESH_OCCLUDER_CULL_LABEL).map((d) => d.workgroups),
      renderer.lastMeshOccluders?.instances,
    ]).toEqual([[[3, 1, 1]], 3]);
    renderer.dispose();
  });

  it("draws them by one bucket draw over the pool's pages, records and errors", async () => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const renderer = await rendererOf(engine, pool);
    const frame = renderer.frame(
      listOf([occluder(stored(pool, [0, 1, 2]), [-8, 0, -30])]),
      CAMERA,
      VIEWPORT,
    );
    const buffers = meshDrawOf(frame)?.storageBuffers;
    expect([buffers?.["meshPages"], buffers?.["meshRecords"], buffers?.["meshErrors"]]).toEqual([
      pool.pages,
      pool.records,
      pool.errors,
    ]);
    renderer.dispose();
  });

  it.each([
    ["with the cluster path off", "path"],
    ["with no pool set", "pool"],
    ["before its kernel is made", "kernel"],
  ] as const)("draws no mesh occluder %s", async (_, missing) => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const renderer =
      missing === "kernel" ? new WireframeRenderer(engine) : await rendererOf(engine, null);
    renderer.setMeshPages(missing === "pool" ? null : pool);
    if (missing === "path") {
      renderer.setGeometry({ ...SETTINGS.high.geometry, clusterPath: false });
    }
    const frame = renderer.frame(
      listOf([occluder(stored(pool, [0]), [0, 0, -30])]),
      CAMERA,
      VIEWPORT,
    );
    expect([
      engine.dispatched.filter((d) => d.pass === MESH_OCCLUDER_CULL_LABEL).length,
      meshDrawOf(frame) === undefined,
    ]).toEqual([0, true]);
    renderer.dispose();
  });

  it("drops its pool on a device restore, drawing no mesh until a new one is set", async () => {
    const engine = await countingRenderEngine();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const mesh = stored(pool, [0]);
    const renderer = await rendererOf(engine, pool);
    engine.loseDevice();
    engine.restore();
    await renderer.clustersReady();
    expect(
      meshDrawOf(renderer.frame(listOf([occluder(mesh, [0, 0, -30])]), CAMERA, VIEWPORT)),
    ).toBeUndefined();
    renderer.dispose();
  });

  it("draws a fresh renderer's mesh draw after a restore, with the new pool set", async () => {
    const engine = await countingRenderEngine();
    const renderer = await rendererOf(engine, new GeometryPagePool(engine, pagePoolSlots(2, 4)));
    engine.loseDevice();
    engine.restore();
    await renderer.clustersReady();
    const pool = new GeometryPagePool(engine, pagePoolSlots(2, 4));
    const list = listOf([occluder(stored(pool, [0, 1]), [0, 0, -30])]);
    renderer.setMeshPages(pool);
    const restored = meshDrawOf(renderer.frame(list, CAMERA, VIEWPORT));
    const fresh = await rendererOf(engine, pool);
    const made = meshDrawOf(fresh.frame(list, CAMERA, VIEWPORT));
    expect([
      restored?.storageBuffers?.["meshPages"] === pool.pages,
      restored?.uniforms,
      restored?.indirect,
    ]).toEqual([true, made?.uniforms, made?.indirect]);
    renderer.dispose();
    fresh.dispose();
  });
});
