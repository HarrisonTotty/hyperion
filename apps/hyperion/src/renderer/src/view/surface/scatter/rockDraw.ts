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
 * - **The cascades** (plan R14, R14.T6.c): where the frame casts the rocks, the same dispatch culls
 *   them into each shadow cascade it renders, cascade c the pass {@link ROCK_CASCADE_PASS} + c, as
 *   the terrain's casters are culled (`clusterCull.wgsl`'s cascade box and T1.c's receivers' cull,
 *   each cluster by its sphere), each rock where its largest diameter is above the cascade's texel,
 *   the DAG cut at the cascade's tolerance. A patch's instances enter the cascades its caster
 *   sphere is admitted to by the terrain's own test of a patch (`terrainPass.ts`' `castsInto` and
 *   the receivers' `receiversRefuse`), whatever its rule; a cascade's pass then draws its buckets
 *   by the rocks' caster ({@link RockView.casterDraws}).
 * - **Buffers**, category `scatter`: the instances and the view's rock records (each patch's
 *   records in turn), grown to a power of two and written each frame; the cull's buckets are the
 *   consumer path's.
 * - **By the visibility buffer** (plan R14, R14.T5.e; Design note 9): where the frame shades by
 *   IDs, the same buckets drawn by the ID raster's base into the view's IDs, and one resolve, the
 *   rocks' camera passes' lists their range of the view's index space ({@link RockVisibility}),
 *   numbered from 0 until the frame moves them past its terrain's (`frameTerrain.ts`'
 *   `rangedVisibility`); nothing drawn forward.
 * - **A frame's steps**: {@link RockView.pack} lays its instances out on the CPU, so that the
 *   terrain pass can compare them as its cascades' caster inputs (plan R14, R14.T6.a) before it
 *   knows which cascades it renders; {@link RockView.cull} writes and culls them; then
 *   {@link RockView.draws} and {@link RockView.casterDraws} draw the cull's buckets.
 *   {@link RockView.frame} takes the three steps at once, with no cascade.
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { BucketDrawBase } from "../../clusters/buckets";
import { resolveUniforms } from "../../clusters/resolve";
import { type ClusterConsumer, ClusterConsumerPath } from "../../clusters/consumer";
import type { CascadeCullPass, CullHorizon, CullPass, CullPlane } from "../../clusters/cullPass";
import type { ClusterStats } from "../../clusters/stats";
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

/** The pass a `pixel` patch's instances enter: the camera's tolerance. */
export const ROCK_PIXEL_PASS = 0;

/** The pass a `region` patch's instances enter: a tolerance of 0, the finest cut. */
export const ROCK_REGION_PASS = 1;

/** Cascade 0's pass (plan R14, R14.T6.c): cascade c's is this plus c. */
export const ROCK_CASCADE_PASS = 2;

/** Each pass's name for the rocks' counters, in the cull's order (plan R14, R14.T6.c). */
export const ROCK_PASS_NAMES: ReadonlyArray<string> = [
  "rocks pixel",
  "rocks region",
  "rocks cascade 0",
  "rocks cascade 1",
  "rocks cascade 2",
  "rocks cascade 3",
];

/** The `f32` bits of 1, an instance's unread error unit (every rock record is a DAG's). */
const UNIT_BITS = 0x3f80_0000;

/** A finest patch whose rocks a view draws this frame. */
export interface RockFramePatch {
  readonly keyString: string;
  /** Its terrain slot, whose class layers and horizon map shade its rocks. */
  readonly terrainSlot: number;
  readonly rule: RockDrawRule;
  /**
   * The shadow cascades its rocks may cast into this frame, a bit a cascade, nearest first (plan
   * R14, R14.T6.c); absent or 0, none.
   */
  readonly cascades?: number;
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
  /** Where the frame shades by the visibility buffer, its ID raster's and resolve's draws; else `null`. */
  readonly ids: RockIdsInput | null;
  /**
   * The shadow cascades' passes of the cull, nearest first, `null` for one the frame does not
   * render (plan R14, R14.T6.c; `receivers.ts`' `cascadeCullPasses`, in the non-rotating axes);
   * absent or none, the rocks cast into no cascade.
   */
  readonly cascades?: ReadonlyArray<CascadeCullPass | null>;
}

/** What {@link RockView.pack} lays out: a frame's patches about its camera. */
export type RockPackInput = Pick<
  RockFrameInput,
  "patches" | "meshes" | "cameraBodyFixedM" | "rotationRows"
>;

/** What {@link RockView.cull} culls the packed instances by: the passes and the kernel. */
export type RockCullInput = Pick<
  RockFrameInput,
  | "meshes"
  | "rotationRows"
  | "planes"
  | "horizon"
  | "toleranceRad"
  | "viewAxis"
  | "kernel"
  | "cascades"
>;

/** What the rocks' draws take: the template mesh, the draw base and the visibility buffer's. */
export type RockDrawInput = Pick<RockFrameInput, "mesh" | "base" | "ids">;

/**
 * A frame's instances as {@link RockView.pack} laid them out (see the module's remarks), before
 * they are written: one a resident page of each drawn patch, {@link ROCK_INSTANCE_BYTES} each, and
 * each one's cull input, two words.
 */
export interface RockPack {
  /** The instances' bytes, as the cull and the draws read them. */
  readonly instances: ArrayBuffer;
  /** Each instance's cull input: the passes it may enter, a bit a pass, and its error unit's bits. */
  readonly inputs: Uint32Array;
  /** The instances. */
  readonly count: number;
  /** The view's rock records, each drawn patch's in turn. */
  readonly records: Uint8Array<ArrayBuffer>;
  readonly counts: RockFrameCounts;
}

/** What a frame by the visibility buffer gives the rocks' draws (plan R14, R14.T5.e). */
export interface RockIdsInput {
  /** The ID raster's base: the lit base's uniforms with the rocks' ID material. */
  readonly idBase: BucketDrawBase;
  /**
   * The resolve's draw: its material, the full-screen mesh, the lit base's uniforms and textures
   * with the view's resolve frame; the rocks' storage and the resolve's lists are set beside them.
   */
  readonly resolve: DrawItem;
  /** The device's sub-pixel grid's bits (`GpuCapabilities.subpixelBits`). */
  readonly subpixelBits: number;
}

/** The rocks by the visibility buffer in a frame: their raster, their resolve and their range's size. */
export interface RockVisibility {
  readonly raster: ReadonlyArray<DrawItem>;
  readonly resolve: ReadonlyArray<DrawItem>;
  /** The entries of their range of the view's index space: their camera passes' of their lists. */
  readonly entries: number;
}

/** A view-frame's rock draws: forward, or by the visibility buffer. */
export interface RockDraws {
  readonly forward: ReadonlyArray<DrawItem>;
  readonly byIds: RockVisibility | null;
}

/** No rock drawn. */
const NO_ROCK_DRAWS: RockDraws = { forward: [], byIds: null };

/** No patch, instance, rock or triangle drawn. */
const NO_COUNTS: RockFrameCounts = { patches: 0, instances: 0, rocks: 0, triangles: 0 };

/** The cascades a cull may take, after the camera's two passes. */
const CASCADE_PASSES = 4;

/** A patch's cascades' bits, one a cascade. */
const CASCADE_BITS = (1 << CASCADE_PASSES) - 1;

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

/** A draw base with the rocks' storage set beside its own, and the base it was made from. */
interface HeldBase<Base> {
  from: Base | null;
  made: Base | null;
}

/** A buffer of the view's, grown to a power of two. */
interface Grown {
  buffer: BufferHandle | null;
  bytes: number;
}

/** The storage a cull's draws bind: its lists, the pool's pages and the view's instances and records. */
type RockStorage = Readonly<Record<string, BufferHandle>>;

/** One view's rocks (see the module's remarks). */
export class RockView {
  readonly #engine: RockViewEngine;
  readonly #name: string;
  readonly #path: ClusterConsumerPath;
  readonly #instances: Grown = { buffer: null, bytes: 0 };
  readonly #records: Grown = { buffer: null, bytes: 0 };
  readonly #forward: HeldBase<BucketDrawBase> = { from: null, made: null };
  readonly #resolve: HeldBase<DrawItem> = { from: null, made: null };
  readonly #caster: HeldBase<BucketDrawBase> = { from: null, made: null };
  #resolveDraw: DrawItem | null = null;
  #counts: RockFrameCounts = NO_COUNTS;
  /** The last cull's storage and its passes, `null` before it and where the frame drew none. */
  #culled: { readonly storage: RockStorage; readonly passes: number } | null = null;
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
   * The passes its last cull culled ({@link ROCK_PASS_NAMES}' order): 2, or 2 and each cascade up
   * to the farthest it cast into; 0 where its frame drew none.
   */
  get passes(): number {
    return this.#culled?.passes ?? 0;
  }

  /**
   * Culls `input`'s rocks and hands back their draws, pass 0's buckets then pass 1's, forward or,
   * where `input.ids` is given, by the visibility buffer; none where it draws no rock. Its
   * cascades' draws, where `input.cascades` names any, are {@link RockView.casterDraws}'.
   */
  frame(input: RockFrameInput): RockDraws {
    const pack = this.pack(input);
    if (pack === null) {
      return NO_ROCK_DRAWS;
    }
    this.cull(pack, input);
    return this.draws(input);
  }

  /**
   * Lays out `input`'s instances about its camera and their cull inputs, the passes of each
   * instance its patch's rule's and its cascades' (see the module's remarks), writing nothing;
   * `null` where no patch's mesh is held, when the frame draws no rock. It forgets the last cull.
   */
  pack(input: RockPackInput): RockPack | null {
    this.#counts = NO_COUNTS;
    this.#culled = null;
    if (this.#disposed) {
      return null;
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
      return null;
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
      const passes =
        (1 << (patch.rule === "region" ? ROCK_REGION_PASS : ROCK_PIXEL_PASS)) |
        (((patch.cascades ?? 0) & CASCADE_BITS) << ROCK_CASCADE_PASS);
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
        inputs[2 * n] = passes;
        inputs[2 * n + 1] = UNIT_BITS;
        n += 1;
      }
      firstRock += held.rocks;
    }
    return {
      instances,
      inputs,
      count,
      records,
      counts: {
        patches: drawn.length,
        instances: count,
        rocks,
        triangles: drawn.reduce((sum, each) => sum + each.held.triangles, 0),
      },
    };
  }

  /**
   * Writes `pack`'s instances and records and culls them by one dispatch into the camera's two
   * passes and, after them, each of `input.cascades` given (see the module's remarks), the passes up
   * to the farthest cascade given and no more; its submission precedes every draw encoded after
   * the call.
   */
  cull(pack: RockPack, input: RockCullInput): void {
    if (this.#disposed) {
      return;
    }
    const { pool } = input.meshes;
    const instanceBuffer = this.#write(this.#instances, "instances", pack.instances);
    const recordBuffer = this.#write(this.#records, "records", pack.records.buffer);
    const camera0 = {
      kind: "camera" as const,
      planes: input.planes,
      horizon: input.horizon,
      order: "appended" as const,
      toleranceRad: input.toleranceRad,
      strideCap: 1 as const,
    };
    const passes: Array<CullPass | null> = [
      camera0,
      { ...camera0, toleranceRad: 0 },
      ...(input.cascades ?? []).slice(0, CASCADE_PASSES),
    ];
    while (passes.length > ROCK_CASCADE_PASS && passes.at(-1) === null) {
      passes.pop();
    }
    const [r0, r1, r2] = input.rotationRows;
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
        count: pack.count,
        inputs: pack.inputs,
        passes,
        rotationColumns,
        viewAxis: input.viewAxis,
      },
      ROCK_CULL_LABEL,
    );
    const buckets = this.#path.buckets;
    if (buckets === null) {
      return;
    }
    this.#culled = {
      storage: {
        clusterLists: buckets.lists,
        rockPages: pool.pages,
        rockInstances: instanceBuffer,
        rockRecords: recordBuffer,
      },
      passes: passes.length,
    };
    this.#counts = pack.counts;
  }

  /**
   * The last cull's camera draws, pass 0's buckets then pass 1's, forward or, where `input.ids` is
   * given, by the visibility buffer; none where it culled none.
   */
  draws(input: RockDrawInput): RockDraws {
    const culled = this.#culled;
    const buckets = this.#path.buckets;
    if (culled === null || buckets === null || this.#disposed) {
      return NO_ROCK_DRAWS;
    }
    const { storage } = culled;
    const { ids } = input;
    const drawBase = ids === null ? input.base : ids.idBase;
    const base = this.#baseOf(this.#forward, drawBase, storage);
    const draws = [
      ...this.#path.drawItems(ROCK_PIXEL_PASS, input.mesh, base, 1),
      ...this.#path.drawItems(ROCK_REGION_PASS, input.mesh, base, 1),
    ];
    if (ids === null) {
      return { forward: draws, byIds: null };
    }
    const resolve = this.#baseOf(this.#resolve, ids.resolve, storage);
    // The camera's two passes alone, the lists' first: the cascades' after them write no ID.
    const entries = ROCK_CASCADE_PASS * buckets.passEntries;
    const whole = resolveUniforms(buckets, ids.subpixelBits).resolveLists;
    const resolveLists = Float32Array.of(whole[0] ?? 0, whole[1] ?? 0, entries, whole[3] ?? 0);
    const held = this.#resolveDraw;
    const resolveDraw =
      held !== null &&
      held.storageBuffers === resolve.storageBuffers &&
      held.uniforms["resolveLists"]?.every((x, k) => x === resolveLists[k]) === true
        ? held
        : { ...resolve, uniforms: { ...resolve.uniforms, resolveLists } };
    this.#resolveDraw = resolveDraw;
    return {
      forward: [],
      byIds: {
        raster: draws,
        resolve: [resolveDraw],
        entries,
      },
    };
  }

  /**
   * Cascade `index`'s draws of the last cull (plan R14, R14.T6.c): its pass's buckets by `base`,
   * the rocks' caster's, with `mesh`, the rocks' storage beside its own; none where the cull took
   * no pass of it.
   */
  casterDraws(index: number, mesh: MeshHandle, base: BucketDrawBase): ReadonlyArray<DrawItem> {
    const culled = this.#culled;
    const pass = ROCK_CASCADE_PASS + index;
    if (culled === null || pass >= culled.passes || this.#disposed) {
      return [];
    }
    return this.#path.drawItems(pass, mesh, this.#baseOf(this.#caster, base, culled.storage), 1);
  }

  /**
   * Samples its last cull's buckets' counts into `stats` ({@link ROCK_PASS_NAMES}' passes), behind
   * the caller's measurement switch (plan R14, R14.T6.c; `clusters/stats.ts`); nothing where it
   * culled none.
   */
  sample(stats: ClusterStats): void {
    const buckets = this.#path.buckets;
    const passes = this.passes;
    if (buckets === null || passes === 0 || this.#disposed) {
      return;
    }
    stats.sample(buckets, ROCK_CONSUMER.templates, ROCK_PASS_NAMES.slice(0, passes));
  }

  /**
   * `base` with the rocks' `storage` beside its own, held in `held`: the same object while nothing
   * in it changes, so that the buckets' draws are kept.
   */
  #baseOf<Base extends BucketDrawBase>(
    held: HeldBase<Base>,
    base: Base,
    storage: Readonly<Record<string, BufferHandle>>,
  ): Base {
    const made = held.made;
    if (
      made !== null &&
      held.from === base &&
      Object.entries(storage).every(([name, buffer]) => made.storageBuffers?.[name] === buffer)
    ) {
      return made;
    }
    const next = { ...base, storageBuffers: { ...base.storageBuffers, ...storage } };
    held.made = next;
    held.from = base;
    return next;
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
