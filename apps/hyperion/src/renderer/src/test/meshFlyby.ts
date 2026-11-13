/**
 * The mesh pages' fly-by (plan R14, R14.T5.b's acceptance): instances of mesh files flown past
 * through a {@link MeshResidency} and its {@link GeometryPagePool}, each frame's cut taken by the
 * cull's twin over the very bytes the pool wrote, and checked for holes and overlaps.
 *
 * @remarks
 * - **The engine** is `mirrorEngine.ts`' {@link MirrorEngine}, which holds each buffer's bytes as the GPU would, so
 *   that the twin reads the records as rewritten, and tallies each category's bytes.
 * - **The cut** of each instance is the twin's camera pass with no planes, at the demand's
 *   tolerance: every resident page of its mesh an instance of the cull, its origin the instance's
 *   less the camera's ({@link drawnClusters}).
 * - **Holes and overlaps** ({@link cutDefects}): over the whole DAG, read from the file, every
 *   path from a leaf up to a root must cross exactly one drawn cluster; none is a hole, two an
 *   overlap. Where the mesh is a closed or bounded surface, {@link cutCracks} also checks the
 *   drawn triangles watertight as `tools/meshbuild`'s tests do: no directed edge twice, and every
 *   edge matched by its reverse but on the leaves' open boundary.
 * - **A frame**: the camera moves along the instances; each instance requests its pages
 *   ({@link meshPageDemand}); the residency updates; fetches finish `latencyFrames` later.
 *
 * `scripts/meshFlyby.mjs` runs it by hand over `tools/meshbuild`'s procedural test meshes; its
 * test runs it over `meshPages.ts`' three-level DAG.
 */

import type { Vec3 } from "../geometry/vec3";
import { type CullInstance, cullTwin } from "../view/clusters/cull";
import type { CameraCullPass } from "../view/clusters/cullPass";
import { InThreadMeshPageProducer } from "../view/clusters/meshPageProducer";
import {
  type FormatRead,
  MAX_PAGE_CLUSTERS,
  type MeshIndex,
  type MeshPage,
  type MeshPageEntry,
  NO_GROUP,
  PAGE_LOD_BYTES,
  PAGE_RECORD_BYTES,
  pageClusterCorners,
  type PageSphere,
  readMeshIndex,
  readMeshPage,
} from "../view/clusters/pageFormat";
import { GeometryPagePool } from "../view/clusters/pages";
import {
  CLUSTER_SHAPE_CODE,
  type ClusterLod,
  type ClusterRecord,
  CLUSTER_FLAG_DAG,
} from "../view/clusters/record";
import { MeshResidency, meshPageDemand, type MeshPageProducer } from "../view/clusters/residency";
import { MirrorEngine } from "./mirrorEngine";

/** The record of `cluster` in `slot`, as the cull reads it from the pool's records. */
function recordAt(view: DataView, slot: number, cluster: number): ClusterRecord {
  const at = (slot * MAX_PAGE_CLUSTERS + cluster) * PAGE_RECORD_BYTES;
  const f32 = (k: number): number => view.getFloat32(at + k, true);
  const flags = view.getUint32(at + 28, true);
  const radiusM = f32(12);
  if (radiusM >= 0 && (flags & 0xf) !== CLUSTER_SHAPE_CODE.mesh) {
    throw new Error(`slot ${String(slot)}'s cluster ${String(cluster)} is not a mesh cluster`);
  }
  if (radiusM >= 0 && (flags & CLUSTER_FLAG_DAG) === 0) {
    throw new Error(`slot ${String(slot)}'s cluster ${String(cluster)} is not a DAG-cut record`);
  }
  return {
    centreM: { x: f32(0), y: f32(4), z: f32(8) },
    radiusM,
    mode: "dag",
    errorsM: [f32(16), f32(20)],
    templateRef: view.getUint32(at + 24, true),
    shape: "mesh",
    mayDiscard: false,
    material: (flags >>> 8) & 0xff,
  };
}

/** The LOD sphere of `cluster` in `slot`. */
function lodAt(view: DataView, slot: number, cluster: number): ClusterLod {
  const at = (slot * MAX_PAGE_CLUSTERS + cluster) * PAGE_LOD_BYTES;
  const f32 = (k: number): number => view.getFloat32(at + k, true);
  return { parentCentreM: { x: f32(0), y: f32(4), z: f32(8) }, parentRadiusM: f32(12) };
}

/** A view of `bytes`. */
function viewOf(bytes: Uint8Array): DataView {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

/** An instance as the fly-by draws it: its mesh in the pool and its pages' origin less the camera. */
export interface DrawnInstance {
  readonly mesh: number;
  readonly originM: Vec3;
}

/**
 * Each instance's drawn clusters by the cull's twin over `engine`'s copy of `pool`'s records (see
 * the module's remarks), as page × {@link MAX_PAGE_CLUSTERS} + cluster.
 */
export function drawnClusters(
  pool: GeometryPagePool,
  engine: MirrorEngine,
  instances: ReadonlyArray<DrawnInstance>,
  toleranceRad: number,
): Array<Set<number>> {
  const records = viewOf(engine.bytes(pool.records));
  const lods = viewOf(engine.bytes(pool.lods));
  const cull: CullInstance[] = [];
  const owners: Array<{ readonly instance: number; readonly page: number }> = [];
  instances.forEach((instance, i) => {
    for (const [page, slot] of pool.residentPages(instance.mesh)) {
      cull.push({ originM: instance.originM, page: slot, passMask: 1, unitM: 0 });
      owners.push({ instance: i, page });
    }
  });
  const pass: CameraCullPass = {
    kind: "camera",
    planes: [],
    horizon: null,
    order: "appended",
    toleranceRad,
    strideCap: 1,
  };
  const buckets = cullTwin({
    instances: cull,
    records: (slot, cluster) => recordAt(records, slot, cluster),
    lods: (slot, cluster) => lodAt(lods, slot, cluster),
    clustersPerPage: MAX_PAGE_CLUSTERS,
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
  });
  const out = instances.map(() => new Set<number>());
  for (const entries of buckets.values()) {
    for (const entry of entries) {
      const owner = owners[Math.floor(entry / 256)];
      if (owner !== undefined) {
        out[owner.instance]?.add(owner.page * MAX_PAGE_CLUSTERS + (entry & 0xff));
      }
    }
  }
  return out;
}

/** A mesh file read whole: its index and every page. */
export interface MeshFileRead {
  readonly index: MeshIndex;
  readonly pages: ReadonlyArray<MeshPage>;
}

/** `bytes` read whole, or why not. */
export function readMeshFile(bytes: ArrayBuffer): FormatRead<MeshFileRead> {
  const index = readMeshIndex(bytes);
  if (index.kind === "refused") {
    return index;
  }
  const pages: MeshPage[] = [];
  for (const [p, entry] of index.value.pages.entries()) {
    const page = readMeshPage(
      bytes.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
      entry,
      p,
    );
    if (page.kind === "refused") {
      return page;
    }
    pages.push(page.value);
  }
  return { kind: "read", value: { index: index.value, pages } };
}

/** What a cut is missing or holds twice (see the module's remarks). */
export interface CutDefects {
  /** Leaves with a path to a root that crosses no drawn cluster. */
  readonly holes: number;
  /** Leaves with a path that crosses two or more. */
  readonly overlaps: number;
}

/** The checker of a file's cuts, its DAG's paths read once. */
export class CutChecker {
  readonly #file: MeshFileRead;
  /** The clusters each group made, as page × 128 + cluster. */
  readonly #made = new Map<number, number[]>();
  readonly #leaves: number[] = [];
  #open: Set<string> | null = null;

  constructor(file: MeshFileRead) {
    this.#file = file;
    file.pages.forEach((page, p) => {
      for (let c = 0; c < page.clusters; c += 1) {
        const key = p * MAX_PAGE_CLUSTERS + c;
        const gen = page.generatingGroups[c] ?? NO_GROUP;
        if (gen === NO_GROUP) {
          this.#leaves.push(key);
        } else {
          const made = this.#made.get(gen) ?? [];
          made.push(key);
          this.#made.set(gen, made);
        }
      }
    });
  }

  /** The cut `drawn`'s holes and overlaps (see the module's remarks). */
  defects(drawn: ReadonlySet<number>): CutDefects {
    const memo = new Map<number, readonly [number, number]>();
    const above = (key: number): readonly [number, number] => {
      const known = memo.get(key);
      if (known !== undefined) {
        return known;
      }
      const page = this.#file.pages[Math.floor(key / MAX_PAGE_CLUSTERS)];
      const con = page?.consumingGroups[key % MAX_PAGE_CLUSTERS] ?? NO_GROUP;
      let [least, most] = [0, 0];
      if (con !== NO_GROUP) {
        const spans = (this.#made.get(con) ?? []).map(above);
        least = Math.min(...spans.map((s) => s[0]));
        most = Math.max(...spans.map((s) => s[1]));
      }
      const own = drawn.has(key) ? 1 : 0;
      const span = [least + own, most + own] as const;
      memo.set(key, span);
      return span;
    };
    let holes = 0;
    let overlaps = 0;
    for (const leaf of this.#leaves) {
      const [least, most] = above(leaf);
      holes += least === 0 ? 1 : 0;
      overlaps += most > 1 ? 1 : 0;
    }
    return { holes, overlaps };
  }

  /**
   * The drawn triangles' cracks and doubled edges (see the module's remarks): directed edges
   * drawn twice, and edges unmatched by their reverse off the leaves' open boundary.
   */
  cracks(drawn: ReadonlySet<number>): number {
    const edges = this.#edges(drawn);
    this.#open ??= this.#openPoints();
    let bad = 0;
    for (const [edge, count] of edges) {
      const [a = "", b = ""] = edge.split("|");
      if (count > 1) {
        bad += 1;
      } else if (!edges.has(`${b}|${a}`) && !(this.#open.has(a) && this.#open.has(b))) {
        bad += 1;
      }
    }
    return bad;
  }

  /** The drawn triangles' directed edges, counted. */
  #edges(drawn: Iterable<number>): Map<string, number> {
    const edges = new Map<string, number>();
    for (const key of drawn) {
      const page = this.#file.pages[Math.floor(key / MAX_PAGE_CLUSTERS)];
      if (page === undefined) {
        continue;
      }
      const corners = pageClusterCorners(page, key % MAX_PAGE_CLUSTERS);
      for (let t = 0; t < corners.length; t += 9) {
        const point = (k: number): string =>
          `${String(corners[t + 3 * k])},${String(corners[t + 3 * k + 1])},${String(corners[t + 3 * k + 2])}`;
        const [a, b, c] = [point(0), point(1), point(2)];
        for (const edge of [`${a}|${b}`, `${b}|${c}`, `${c}|${a}`]) {
          edges.set(edge, (edges.get(edge) ?? 0) + 1);
        }
      }
    }
    return edges;
  }

  /** The points on the leaves' open boundary. */
  #openPoints(): Set<string> {
    const edges = this.#edges(this.#leaves);
    const open = new Set<string>();
    for (const edge of edges.keys()) {
      const [a = "", b = ""] = edge.split("|");
      if (!edges.has(`${b}|${a}`)) {
        open.add(a);
        open.add(b);
      }
    }
    return open;
  }
}

/**
 * A producer whose pages finish `latencyFrames` ticks after they are asked for, once
 * {@link LatentProducer.flying}; before, as the meshes load with their roots, at once.
 */
class LatentProducer implements MeshPageProducer {
  readonly #inner: MeshPageProducer;
  readonly #latencyFrames: number;
  #queue: Array<{ readonly due: number; readonly run: () => void }> = [];
  #frame = 0;
  flying = false;

  constructor(inner: MeshPageProducer, latencyFrames: number) {
    this.#inner = inner;
    this.#latencyFrames = latencyFrames;
  }

  index(url: string): Promise<FormatRead<MeshIndex>> {
    return this.#inner.index(url);
  }

  page(url: string, entry: MeshPageEntry, page: number): Promise<FormatRead<MeshPage>> {
    if (!this.flying) {
      return this.#inner.page(url, entry, page);
    }
    return new Promise((resolve, reject) => {
      this.#queue.push({
        due: this.#frame + this.#latencyFrames,
        run: () => {
          this.#inner.page(url, entry, page).then(resolve, reject);
        },
      });
    });
  }

  forget(url: string): void {
    this.#inner.forget(url);
  }

  /** Ends a frame: the fetches due now finish. */
  tick(): void {
    this.#frame += 1;
    const due = this.#queue.filter((job) => job.due <= this.#frame);
    this.#queue = this.#queue.filter((job) => job.due > this.#frame);
    for (const job of due) {
      job.run();
    }
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/** A mesh file the fly-by flies past, by name. */
export interface FlybyMesh {
  readonly name: string;
  readonly bytes: ArrayBuffer;
}

/** The fly-by's run (see the module's remarks). */
export interface FlybyOptions {
  readonly meshes: ReadonlyArray<FlybyMesh>;
  /** Instances, of the meshes in turn. */
  readonly instances: number;
  readonly frames: number;
  readonly poolBytes: number;
  /** The camera pass's and the demand's tolerance, rad. */
  readonly toleranceRad: number;
  /** The frames a page's fetch takes. */
  readonly latencyFrames: number;
  /** The instances' spacing along the path, m. */
  readonly spacingM: number;
  /** The camera's least distance from the instances' line, m. */
  readonly passM: number;
  /** Whether each cut's triangles are checked watertight as well ({@link CutChecker.cracks}). */
  readonly cracks: boolean;
  /**
   * How the meshes take turns along the path: one instance each in turn (`alternating`), or each
   * mesh's instances in a run of their own (`runs`), so that the camera leaves a mesh's pages
   * behind and a small pool must evict them.
   */
  readonly layout: "alternating" | "runs";
}

/** What the fly-by saw. */
export interface FlybyResult {
  readonly frames: number;
  readonly instances: number;
  /** Instance-frames whose cut had a hole, an overlap, or a crack. */
  readonly holes: number;
  readonly overlaps: number;
  readonly cracks: number;
  /** The pool's `geometry-pages` bytes at their most, and the bound. */
  readonly peakPoolBytes: number;
  readonly poolBytes: number;
  /** The pool's slots. */
  readonly rootSlots: number;
  readonly pageSlots: number;
  /** Instance-frames that drew a page's stand-in: a wanted page not yet resident. */
  readonly streaming: number;
  /** The residency's counters at the end, and the pool's refusals. */
  readonly fetched: number;
  readonly stored: number;
  readonly evicted: number;
  readonly refused: number;
  readonly rootsRefused: number;
  /** Drawn triangles a frame over every instance, the median and the largest. */
  readonly trianglesP50: number;
  readonly trianglesMax: number;
}

/** Lets the producer's settled fetches reach the residency. */
async function settle(): Promise<void> {
  await new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

/** A sphere holding the root pages' bounds: the mesh's, since the spheres nest. */
function meshBounds(index: MeshIndex): PageSphere {
  const roots = index.pages.slice(0, index.rootPages).map((entry) => entry.bounds);
  const first = roots[0];
  if (first === undefined) {
    throw new Error("a mesh with no root page");
  }
  let radiusM = 0;
  for (const b of roots) {
    radiusM = Math.max(
      radiusM,
      Math.hypot(
        b.centreM.x - first.centreM.x,
        b.centreM.y - first.centreM.y,
        b.centreM.z - first.centreM.z,
      ) + b.radiusM,
    );
  }
  return { centreM: first.centreM, radiusM };
}

/** Flies past `options.instances` instances (see the module's remarks). */
export async function runMeshFlyby(options: FlybyOptions): Promise<FlybyResult> {
  const engine = new MirrorEngine();
  const pool = new GeometryPagePool(engine, options.poolBytes);
  const byUrl = new Map(options.meshes.map((m) => [`mesh:${m.name}`, m.bytes]));
  const producer = new LatentProducer(
    new InThreadMeshPageProducer((url) => {
      const bytes = byUrl.get(url);
      return bytes === undefined
        ? Promise.reject(new Error(`no mesh ${url}`))
        : Promise.resolve(bytes);
    }),
    options.latencyFrames,
  );
  const residency = new MeshResidency(pool, producer);
  const loaded = await Promise.all(
    options.meshes.map(async (m) => {
      const file = readMeshFile(m.bytes);
      const load = await residency.load(`mesh:${m.name}`);
      if (file.kind === "refused" || load.kind === "refused") {
        throw new Error(`${m.name} could not be read`);
      }
      return {
        mesh: load.mesh,
        file: file.value,
        checker: new CutChecker(file.value),
        bounds: meshBounds(file.value.index),
        pageOriginM: file.value.pages[0]?.originM ?? { x: 0, y: 0, z: 0 },
      };
    }),
  );
  producer.flying = true;
  const placed = Array.from({ length: options.instances }, (_, i) => {
    const mesh =
      loaded[
        options.layout === "runs"
          ? Math.floor((i * loaded.length) / options.instances)
          : i % loaded.length
      ];
    if (mesh === undefined) {
      throw new Error("no mesh to place");
    }
    const side = i % 2 === 0 ? 1 : -1;
    return {
      mesh,
      positionM: { x: options.spacingM * i, y: side * (options.passM + 10 * (i % 3)), z: 0 },
    };
  });
  const lengthM = options.spacingM * options.instances;
  let [holes, overlaps, cracks, streaming, peakPoolBytes] = [0, 0, 0, 0, 0];
  const triangles: number[] = [];
  const memo = new Map<string, { defects: CutDefects; cracks: number; triangles: number }>();
  for (let frame = 0; frame < options.frames; frame += 1) {
    const s = frame / Math.max(1, options.frames - 1);
    const cameraM = { x: -0.1 * lengthM + 1.2 * lengthM * s, y: 0, z: 5 };
    const drawn: DrawnInstance[] = [];
    const wanted: number[][] = [];
    for (const { mesh, positionM } of placed) {
      const at = {
        x: positionM.x + mesh.pageOriginM.x,
        y: positionM.y + mesh.pageOriginM.y,
        z: positionM.z + mesh.pageOriginM.z,
      };
      const nearestM = Math.max(
        0,
        Math.hypot(
          at.x + mesh.bounds.centreM.x - cameraM.x,
          at.y + mesh.bounds.centreM.y - cameraM.y,
          at.z + mesh.bounds.centreM.z - cameraM.z,
        ) - mesh.bounds.radiusM,
      );
      const demand = meshPageDemand(mesh.file.index, nearestM, options.toleranceRad);
      for (const want of demand) {
        residency.request({ mesh: mesh.mesh, page: want.page }, want.priorityRad);
      }
      wanted.push(demand.map((want) => want.page));
      drawn.push({
        mesh: mesh.mesh,
        originM: { x: at.x - cameraM.x, y: at.y - cameraM.y, z: at.z - cameraM.z },
      });
    }
    residency.update();
    placed.forEach(({ mesh }, i) => {
      streaming += (wanted[i] ?? []).some((page) => pool.slotOf(mesh.mesh, page) === undefined)
        ? 1
        : 0;
    });
    peakPoolBytes = Math.max(peakPoolBytes, engine.heldBytes("geometry-pages"));
    const cuts = drawnClusters(pool, engine, drawn, options.toleranceRad);
    let frameTriangles = 0;
    cuts.forEach((cut, i) => {
      const mesh = placed[i]?.mesh;
      if (mesh === undefined) {
        return;
      }
      const key = `${String(mesh.mesh)}:${[...cut].toSorted((a, b) => a - b).join(",")}`;
      let seen = memo.get(key);
      if (seen === undefined) {
        seen = {
          defects: mesh.checker.defects(cut),
          cracks: options.cracks ? mesh.checker.cracks(cut) : 0,
          triangles: [...cut].reduce((sum, k) => {
            const page = mesh.file.pages[Math.floor(k / MAX_PAGE_CLUSTERS)];
            const at = (page?.sections.records ?? 0) + PAGE_RECORD_BYTES * (k % MAX_PAGE_CLUSTERS);
            return (
              sum +
              (page === undefined ? 0 : new DataView(page.body).getUint32(at + 24, true) >>> 24)
            );
          }, 0),
        };
        memo.set(key, seen);
      }
      holes += seen.defects.holes > 0 ? 1 : 0;
      overlaps += seen.defects.overlaps > 0 ? 1 : 0;
      cracks += seen.cracks > 0 ? 1 : 0;
      frameTriangles += seen.triangles;
    });
    triangles.push(frameTriangles);
    producer.tick();
    // Each frame's arrivals reach the residency before the next frame: the frames are in order.
    // oxlint-disable-next-line eslint/no-await-in-loop
    await settle();
  }
  const counts = residency.counts();
  const pressure = pool.pressure();
  residency.dispose();
  pool.dispose();
  const sorted = triangles.toSorted((a, b) => a - b);
  return {
    frames: options.frames,
    instances: options.instances,
    holes,
    overlaps,
    cracks,
    peakPoolBytes,
    poolBytes: options.poolBytes,
    rootSlots: pool.layout.rootSlots,
    pageSlots: pool.layout.pageSlots,
    streaming,
    fetched: counts.fetched,
    stored: counts.stored,
    evicted: counts.evicted,
    refused: counts.refused,
    rootsRefused: pressure.rootsRefused,
    trianglesP50: sorted[Math.floor(sorted.length / 2)] ?? 0,
    trianglesMax: sorted.at(-1) ?? 0,
  };
}
