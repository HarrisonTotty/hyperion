/**
 * One view's draw of the authoritative rocks (plan R11, T4.b; plan R14, R14.T5.c): the rock meshes
 * of the finest patches its terrain draws, culled by the cluster path's shared cull into buckets
 * and drawn from them, lit or in the wireframe.
 *
 * @remarks
 * - **Instances**: one a resident page of each drawn patch's rock mesh, {@link ROCK_INSTANCE_BYTES}
 *   (`rockPage.wgsl`'s `RockInstance`): the patch's origin and its pages' origin less the camera,
 *   formed in `f64` in the body-fixed axes, turned into the non-rotating axes the passes are in and
 *   narrowed once to `f32`, with its pool slot, the cull's first four words; then its page's first
 *   word in the pool's pages, the patch's terrain slot, its first rock in the view's rock records,
 *   and its pages' origin about the patch's origin, body-fixed.
 * - **Passes**: two camera passes over the view's frustum and the body's horizon: pass 0 at the
 *   camera tolerance, where a `pixel` patch's rocks are drawn while they subtend it (each record's
 *   parent error its rock's largest diameter, `rockPages.ts`), and pass 1 at a tolerance of 0, the
 *   finest cut, where a `region` patch's are always drawn (R11 Design note 7; R14.T5.c's "the
 *   could-touch region's rocks at their finest cut whatever the setting"). A patch's instances
 *   enter one of the two by its rule (`rockSelection.ts`). The records' axes are the body's, turned
 *   by the dispatch's rotation.
 * - **Buffers**, category `scatter`: the instances and the view's rock records (each patch's
 *   records in turn), grown to a power of two and written each frame; the cull's buckets are the
 *   consumer path's.
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { BucketDrawBase } from "../../clusters/buckets";
import { type ClusterConsumer, ClusterConsumerPath } from "../../clusters/consumer";
import type { CullHorizon, CullPlane } from "../../clusters/cullPass";
import { MAX_PAGE_CLUSTERS } from "../../clusters/pageFormat";
import { pageSlotOffsetBytes } from "../../clusters/pages";
import { CLUSTER_SHAPE_CODE } from "../../clusters/record";
import { meshClusterMesh } from "../../clusters/templates";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  MeshHandle,
  RenderEngine,
} from "../../engine/types";
import { ROCK_RECORD_WORDS } from "./rockPages";
import type { RockMeshes } from "./rockMeshes";
import type { RockDrawRule } from "./rockSelection";
import rockPageWgsl from "./rockPage.wgsl?raw";

/** Bytes of a rock instance (see the module's remarks). */
export const ROCK_INSTANCE_BYTES = 48;

/** The rocks' cull's timed label: one dispatch a view-frame that draws rocks. */
export const ROCK_CULL_LABEL = "rock cull";

/** The mesh shape's cluster mesh, which every rock draw draws its buckets with. */
export const ROCK_CLUSTER_MESH = meshClusterMesh();

/** The rocks as a consumer of the cluster path: the pool's slots of 128 mesh clusters, one template. */
export const ROCK_CONSUMER: ClusterConsumer = {
  name: "rocks",
  templates: ROCK_CLUSTER_MESH.templates,
  clustersPerPage: MAX_PAGE_CLUSTERS,
  firstShape: CLUSTER_SHAPE_CODE.mesh,
  templateClusters: [MAX_PAGE_CLUSTERS],
  instanceBytes: ROCK_INSTANCE_BYTES,
  vertexWgsl: rockPageWgsl,
};

/** The pass a `pixel` patch's instances enter, and a `region` patch's. */
export const ROCK_PIXEL_PASS = 0;
export const ROCK_REGION_PASS = 1;

/** The `f32` bits of 1, an instance's unread error unit (every rock record is a DAG's). */
const UNIT_BITS = 0x3f80_0000;

/** A finest patch whose rocks a view draws this frame. */
export interface RockFramePatch {
  readonly keyString: string;
  /** Its terrain slot, whose class layers and horizon map shade its rocks. */
  readonly terrainSlot: number;
  readonly rule: RockDrawRule;
}

/** One view-frame's rocks (see the module's remarks). */
export interface RockFrameInput {
  readonly patches: ReadonlyArray<RockFramePatch>;
  readonly meshes: RockMeshes;
  /** The camera less the body's centre, body-fixed m (`f64`). */
  readonly cameraBodyFixedM: Vec3;
  /** The body's rotation's rows, body-fixed into the non-rotating axes. */
  readonly rotationRows: readonly [Vec3, Vec3, Vec3];
  /** The camera's frustum's planes and the body's horizon, in the non-rotating axes. */
  readonly planes: ReadonlyArray<CullPlane>;
  readonly horizon: CullHorizon | null;
  /** The camera pass's tolerance, rad (pass 0's). */
  readonly toleranceRad: number;
  /** The view axis, the camera's forward, in the non-rotating axes. */
  readonly viewAxis: Vec3;
  readonly kernel: ComputeHandle;
  /** The mesh shape's template ({@link ROCK_CLUSTER_MESH}) as the engine holds it. */
  readonly mesh: MeshHandle;
  /**
   * The draw its buckets take: its material, uniforms and textures; the rocks' storage is set
   * beside the base's own.
   */
  readonly base: BucketDrawBase;
}

/** What a view's last rock frame drew, for its counts. */
export interface RockFrameCounts {
  readonly patches: number;
  readonly instances: number;
  readonly rocks: number;
  /** Its rocks' triangles before the cull. */
  readonly triangles: number;
}

/** What a view's rock draw asks of the engine. */
export type RockViewEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "releaseBuffer" | "dispatch"
>;

/** A buffer of the view's, grown to a power of two. */
interface Grown {
  buffer: BufferHandle | null;
  bytes: number;
}

/** One view's rocks (see the module's remarks). */
export class RockView {
  readonly #engine: RockViewEngine;
  readonly #name: string;
  readonly #path: ClusterConsumerPath;
  readonly #instances: Grown = { buffer: null, bytes: 0 };
  readonly #records: Grown = { buffer: null, bytes: 0 };
  #base: BucketDrawBase | null = null;
  #heldBase: BucketDrawBase | null = null;
  #counts: RockFrameCounts = { patches: 0, instances: 0, rocks: 0, triangles: 0 };
  #disposed = false;

  constructor(engine: RockViewEngine, name: string) {
    this.#engine = engine;
    this.#name = name;
    this.#path = new ClusterConsumerPath(engine, ROCK_CONSUMER);
  }

  /** What its last frame drew. */
  get counts(): RockFrameCounts {
    return this.#counts;
  }

  /**
   * Culls `input`'s rocks and hands back their draws, pass 0's buckets then pass 1's; none where it
   * draws no rock.
   */
  frame(input: RockFrameInput): ReadonlyArray<DrawItem> {
    this.#counts = { patches: 0, instances: 0, rocks: 0, triangles: 0 };
    if (this.#disposed) {
      return [];
    }
    const { meshes } = input;
    const { pool } = meshes;
    const drawn = input.patches.flatMap((patch) => {
      const held = meshes.get(patch.keyString);
      const pageOrigin = held === undefined ? undefined : pool.originOf(held.mesh);
      return held === undefined || pageOrigin === undefined
        ? []
        : [{ patch, held, pageOrigin, pages: pool.residentPages(held.mesh) }];
    });
    const count = drawn.reduce((sum, each) => sum + each.pages.length, 0);
    if (count === 0) {
      return [];
    }
    const rocks = drawn.reduce((sum, each) => sum + each.held.rocks, 0);
    const instances = new ArrayBuffer(count * ROCK_INSTANCE_BYTES);
    const view = new DataView(instances);
    const records = new Uint8Array(rocks * ROCK_RECORD_WORDS * 4);
    const inputs = new Uint32Array(2 * count);
    const [r0, r1, r2] = input.rotationRows;
    const camera = input.cameraBodyFixedM;
    let n = 0;
    let firstRock = 0;
    for (const { patch, held, pageOrigin, pages } of drawn) {
      records.set(new Uint8Array(held.records), firstRock * ROCK_RECORD_WORDS * 4);
      // The pages' origin less the camera, body-fixed, in f64; then turned and narrowed once.
      const d = {
        x: held.originM.x + pageOrigin.x - camera.x,
        y: held.originM.y + pageOrigin.y - camera.y,
        z: held.originM.z + pageOrigin.z - camera.z,
      };
      const turned = [
        r0.x * d.x + r0.y * d.y + r0.z * d.z,
        r1.x * d.x + r1.y * d.y + r1.z * d.z,
        r2.x * d.x + r2.y * d.y + r2.z * d.z,
      ];
      for (const [, slot] of pages) {
        const at = n * ROCK_INSTANCE_BYTES;
        turned.forEach((x, k) => {
          view.setFloat32(at + 4 * k, x, true);
        });
        view.setUint32(at + 12, slot, true);
        view.setUint32(at + 16, pageSlotOffsetBytes(pool.layout, slot) / 4, true);
        view.setUint32(at + 20, patch.terrainSlot, true);
        view.setUint32(at + 24, firstRock, true);
        view.setFloat32(at + 32, pageOrigin.x, true);
        view.setFloat32(at + 36, pageOrigin.y, true);
        view.setFloat32(at + 40, pageOrigin.z, true);
        inputs[2 * n] = 1 << (patch.rule === "region" ? ROCK_REGION_PASS : ROCK_PIXEL_PASS);
        inputs[2 * n + 1] = UNIT_BITS;
        n += 1;
      }
      firstRock += held.rocks;
    }
    const instanceBuffer = this.#write(this.#instances, "instances", instances);
    const recordBuffer = this.#write(this.#records, "records", records.buffer);
    const camera0 = {
      kind: "camera" as const,
      planes: input.planes,
      horizon: input.horizon,
      order: "appended" as const,
      toleranceRad: input.toleranceRad,
      strideCap: 1 as const,
    };
    const rotationColumns: readonly [Vec3, Vec3, Vec3] = [
      { x: r0.x, y: r1.x, z: r2.x },
      { x: r0.y, y: r1.y, z: r2.y },
      { x: r0.z, y: r1.z, z: r2.z },
    ];
    this.#path.cull(
      input.kernel,
      {
        records: pool.records,
        lods: pool.lods,
        instances: instanceBuffer,
        count,
        inputs,
        passes: [camera0, { ...camera0, toleranceRad: 0 }],
        rotationColumns,
        viewAxis: input.viewAxis,
      },
      ROCK_CULL_LABEL,
    );
    const lists = this.#path.buckets?.lists;
    if (lists === undefined) {
      return [];
    }
    const base = this.#baseOf(input.base, {
      clusterLists: lists,
      rockPages: pool.pages,
      rockPageRecords: pool.records,
      rockInstances: instanceBuffer,
      rockRecords: recordBuffer,
    });
    this.#counts = {
      patches: drawn.length,
      instances: count,
      rocks,
      triangles: drawn.reduce((sum, each) => sum + each.held.triangles, 0),
    };
    return [
      ...this.#path.drawItems(ROCK_PIXEL_PASS, input.mesh, base, 1),
      ...this.#path.drawItems(ROCK_REGION_PASS, input.mesh, base, 1),
    ];
  }

  /** The draw base with the rocks' storage, the same object while nothing in it changes. */
  #baseOf(base: BucketDrawBase, storage: Readonly<Record<string, BufferHandle>>): BucketDrawBase {
    const held = this.#base;
    const same =
      held !== null &&
      this.#heldBase === base &&
      Object.entries(storage).every(([name, buffer]) => held.storageBuffers?.[name] === buffer);
    if (same) {
      return held;
    }
    const made = { ...base, storageBuffers: { ...base.storageBuffers, ...storage } };
    this.#base = made;
    this.#heldBase = base;
    return made;
  }

  /** `bytes` written to `grown`'s buffer, made again larger where it does not hold them. */
  #write(grown: Grown, what: string, bytes: ArrayBuffer): BufferHandle {
    const need = Math.max(16, bytes.byteLength);
    if (grown.buffer === null || grown.bytes < need) {
      if (grown.buffer !== null) {
        this.#engine.releaseBuffer(grown.buffer);
      }
      grown.bytes = 2 ** Math.ceil(Math.log2(need));
      grown.buffer = this.#engine.createBuffer({
        name: `${this.#name} rock ${what}`,
        bytes: grown.bytes,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "scatter",
      });
    }
    if (bytes.byteLength > 0) {
      this.#engine.writeBuffer(grown.buffer, 0, new Uint8Array(bytes));
    }
    return grown.buffer;
  }

  /** Releases its buffers and its path's. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#path.dispose();
    for (const grown of [this.#instances, this.#records]) {
      if (grown.buffer !== null) {
        this.#engine.releaseBuffer(grown.buffer);
        grown.buffer = null;
      }
    }
  }
}
