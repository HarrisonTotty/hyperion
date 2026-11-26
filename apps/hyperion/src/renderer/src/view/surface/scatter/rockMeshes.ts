/**
 * The rock meshes of a terrain pass's resident finest patches (plan R11, T4.b; plan R14,
 * R14.T5.c): each patch's rock layer built into its mesh by the procedural builder off the render
 * thread (`rockPages.ts`, through a {@link RockPageBuilder}), held whole in the rocks' page pool
 * while the patch is resident, and let go with it.
 *
 * @remarks
 * - **The pool** is the rocks' own, category `scatter`, of root slots alone: every page of a rock
 *   mesh is a root page (a one-level DAG), pinned while its mesh is in the pool, so that a resident
 *   patch's rocks are never shed apart from it (the research's "rock lists ride in the patch cache,
 *   never shed"). Its bytes are {@link ROCK_POOL_SHARE} of the setting's `pageBytes` (lean,
 *   2026-11-18). A mesh the pool cannot hold whole is refused and counted
 *   ({@link RockMeshes.refused}), its patch's rocks not drawn; a refused patch in a could-touch
 *   region raises `TERRAIN: DETAIL LIMITED` ({@link RockMeshes.isRefused}).
 * - **The region's precedence** (Design note 7's "every authoritative rock is drawn" within it):
 *   each sync is handed the could-touch region ({@link RockPoolRegion}), and a patch is the
 *   region's where its bounding sphere meets one of its discs, `rockSelection.ts`' `inRegion`, as
 *   the draw's rule reads it. A region patch's mesh that finds no room evicts the meshes of patches
 *   outside the region, farthest from the region first, and is admitted; it is refused only where
 *   the region's own meshes leave no room. A mesh outside the region is refused while a refused
 *   region patch waits on the room it would take. The rest of the pool is first come.
 * - **A refused patch is retried** (its mesh built again) once room frees, a mesh let go with its
 *   patch, or once the region comes to hold it, where its pool then has the room it needed; never
 *   twice for one freeing.
 * - **Builds**: {@link RockMeshes.sync} asks a build for each resident layer it holds no mesh of,
 *   or holds an older bake's, and removes each mesh whose patch has left; a build that lands after
 *   its layer left or was rebaked is dropped. Nothing new is asked while the resident layers'
 *   revision is unchanged, but a refused patch's retry.
 * - **Its revision** ({@link RockMeshes.revision}; plan R14, R14.T6.c): it rises whenever a mesh is
 *   held or let go, the only times the pool's pages and records change, so that a cascade the
 *   rocks cast into is reused while it and their instances are as at its last render (T6.a). The
 *   patch cache's rock lists change only with a bake.
 */

import { norm, sub, vec3, type Vec3 } from "../../../geometry/vec3";
import type { RockPageBuilder } from "../../clusters/meshPageProducer";
import {
  type MeshPage,
  readMeshIndex,
  readMeshPage,
  ROOT_PAGE_BYTES,
} from "../../clusters/pageFormat";
import { GeometryPagePool, type GeometryPagePoolEngine, pagePoolSlots } from "../../clusters/pages";
import type { GroundContact } from "../../terrain/grounded";
import { type PatchKey, patchKeyString } from "../../terrain/patchKey";
import type { ResidentRockLayer, ResidentRocksView } from "./residentRocks";
import { rocksOf } from "./rockLayer";
import type { RockPagesInput } from "./rockPages";
import { inRegion, type RockPatchSphere } from "./rockSelection";

/** The share of the setting's `pageBytes` the rocks' pool takes: a half (lean, 2026-11-18). */
export const ROCK_POOL_SHARE = 0.5;

/** Bytes of a pool slot's records, LOD spheres and errors (`clusters/pages.ts`' own). */
const SLOT_RECORD_BYTES = 128 * (32 + 16 + 4);

/**
 * The could-touch region as the pool's precedence reads it (see the module's remarks): its discs,
 * the draw's (`rockSelection.ts`' `couldTouch` of each grounded or swept contact), and a resident
 * patch's bounding sphere, as the draw's rule tests it against them.
 */
export interface RockPoolRegion {
  readonly discs: ReadonlyArray<GroundContact>;
  /** Resident patch `keyString`'s bounding sphere, body-fixed, or `undefined` where none is known. */
  sphereOf(keyString: string): RockPatchSphere | undefined;
}

/** No region: the pool first come throughout. */
export const NO_ROCK_REGION: RockPoolRegion = { discs: [], sphereOf: () => undefined };

/** A patch whose mesh the pool refused, as its retry reads it. */
interface Refusal {
  /** The bake's generation refused. */
  readonly generation: number;
  /** The root slots its mesh needs, `Infinity` for a file that could not be read. */
  readonly slots: number;
  /** The pool's freeings ({@link RockMeshes}' `#freed`) when it was refused. */
  readonly freed: number;
  /** Whether it was the region's when it was refused. */
  readonly region: boolean;
}

/** A resident patch's rock mesh in the pool. */
export interface RockPatchMesh {
  readonly key: PatchKey;
  readonly generation: number;
  /** The patch's origin, body-fixed m, about which the mesh and its records are placed. */
  readonly originM: Vec3;
  /** Its handle in the pool. */
  readonly mesh: number;
  /** Its rocks' records (`rockPages.ts`' ROCK_RECORD_WORDS words a rock). */
  readonly records: ArrayBuffer;
  readonly rocks: number;
  readonly triangles: number;
  /** Its rocks' largest diameter, m. */
  readonly largestRockM: number;
}

/** A pass's rock meshes (see the module's remarks). */
export class RockMeshes {
  readonly pool: GeometryPagePool;
  readonly #builder: RockPageBuilder;
  readonly #meshes = new Map<string, RockPatchMesh>();
  /** Each key's build in flight, by the generation it builds. */
  readonly #building = new Map<string, number>();
  /** The resident layers' revision it last brought its meshes to. */
  #synced = -1;
  #revision = 0;
  #refused = 0;
  /** Each patch whose mesh the pool last refused, its retry's terms. */
  readonly #refusedKeys = new Map<string, Refusal>();
  /** Each held mesh's root slots, its pages' spans. */
  readonly #slots = new Map<string, number>();
  /** How many times room freed: a mesh let go with its patch, not one evicted for the region. */
  #freed = 0;
  /** The region the last sync was handed. */
  #region: RockPoolRegion = NO_ROCK_REGION;
  /** The region, freeings and refusals its retries last read, which a sync reads again only once changed. */
  #retried: {
    readonly region: RockPoolRegion;
    readonly freed: number;
    readonly refused: number;
  } | null = null;
  #disposed = false;

  /**
   * @param pageBytes - The setting's `GeometrySettings.pageBytes`, of which the pool takes
   *   {@link ROCK_POOL_SHARE}.
   */
  constructor(engine: GeometryPagePoolEngine, builder: RockPageBuilder, pageBytes: number) {
    const rootSlots = Math.floor(
      (ROCK_POOL_SHARE * pageBytes) / (ROOT_PAGE_BYTES + SLOT_RECORD_BYTES),
    );
    this.pool = new GeometryPagePool(engine, pagePoolSlots(rootSlots, 0), {
      category: "scatter",
      name: "rock",
    });
    this.#builder = builder;
  }

  /** The rock mesh of resident patch `keyString`, once built and held. */
  get(keyString: string): RockPatchMesh | undefined {
    return this.#meshes.get(keyString);
  }

  /** How many meshes it holds, and how many builds are in flight. */
  get size(): number {
    return this.#meshes.size;
  }

  get building(): number {
    return this.#building.size;
  }

  /**
   * How many times a mesh was held or let go since it was made (see the module's remarks): the
   * pool's pages and records are as they were wherever it is unchanged.
   */
  get revision(): number {
    return this.#revision;
  }

  /** How many meshes the pool could not hold whole since it was made. */
  get refused(): number {
    return this.#refused;
  }

  /** Whether resident patch `keyString`'s mesh is being built now, asked by a sync. */
  isBuilding(keyString: string): boolean {
    return this.#building.has(keyString);
  }

  /**
   * Whether the pool refused resident patch `keyString`'s mesh, its rocks then not drawn, until a
   * later sync holds it or its patch leaves.
   */
  isRefused(keyString: string): boolean {
    return this.#refusedKeys.has(keyString);
  }

  /**
   * Brings its meshes to `rocks`, the pass's resident layers, with `region` the could-touch region
   * whose patches it holds first (see the module's remarks), and retries each refused patch its
   * pool now has room for.
   */
  sync(rocks: ResidentRocksView, region: RockPoolRegion = NO_ROCK_REGION): void {
    if (this.#disposed) {
      return;
    }
    this.#region = region;
    if (rocks.revision !== this.#synced) {
      this.#synced = rocks.revision;
      for (const [keyString, refusal] of this.#refusedKeys) {
        if (rocks.get(keyString)?.generation !== refusal.generation) {
          this.#refusedKeys.delete(keyString);
        }
      }
      for (const [keyString, held] of this.#meshes) {
        if (rocks.get(keyString)?.generation !== held.generation) {
          this.#let(keyString, held.mesh);
          this.#freed += 1;
        }
      }
      for (const resident of rocks.values()) {
        const keyString = patchKeyString(resident.key);
        const generation = resident.generation;
        if (
          resident.layer.count === 0 ||
          this.#meshes.has(keyString) ||
          this.#building.get(keyString) === generation ||
          this.#refusedKeys.get(keyString)?.generation === generation
        ) {
          continue;
        }
        this.#ask(keyString, resident, rocks);
      }
    }
    this.#retry(rocks);
  }

  /**
   * Asks a build again of each refused patch whose room has freed, or which the region has come to
   * hold, since its refusal, where the pool now has the root slots it needs: the free ones, with,
   * for a region patch, those of the meshes outside the region, or, for any other, the free ones
   * less what the region's refused patches wait on.
   */
  #retry(rocks: ResidentRocksView): void {
    const read = this.#retried;
    if (
      this.#refusedKeys.size === 0 ||
      (read?.region === this.#region &&
        read.freed === this.#freed &&
        read.refused === this.#refused)
    ) {
      return;
    }
    this.#retried = { region: this.#region, freed: this.#freed, refused: this.#refused };
    const due: Array<readonly [string, Refusal, boolean]> = [];
    for (const [keyString, refusal] of this.#refusedKeys) {
      if (this.#building.has(keyString)) {
        continue;
      }
      const region = this.#inRegion(keyString);
      if (this.#freed > refusal.freed || (region && !refusal.region)) {
        due.push([keyString, refusal, region]);
      }
    }
    if (due.length === 0) {
      return;
    }
    const free = this.#freeSlots();
    const waiting = this.#regionWaiting(null);
    let outside = 0;
    for (const keyString of this.#meshes.keys()) {
      outside += this.#inRegion(keyString) ? 0 : (this.#slots.get(keyString) ?? 0);
    }
    for (const [keyString, refusal, region] of due) {
      const resident = rocks.get(keyString);
      const room = region ? free + outside : free - waiting;
      if (resident?.generation === refusal.generation && refusal.slots <= room) {
        this.#ask(keyString, resident, rocks);
      }
    }
  }

  /** Asks the build of `resident`, resident patch `keyString`'s layer. */
  #ask(keyString: string, resident: ResidentRockLayer, rocks: ResidentRocksView): void {
    const generation = resident.generation;
    this.#building.set(keyString, generation);
    const largestRockM = rocksOf(resident.layer).reduce(
      (most, rock) => Math.max(most, 2 * Math.max(...rock.semiAxesM)),
      0,
    );
    const input = { key: resident.key, originM: resident.originM, layer: resident.layer };
    void this.#build(keyString, generation, input, largestRockM, rocks).catch((error: unknown) => {
      this.#building.delete(keyString);
      console.warn(`the rocks of ${keyString} could not be built`, error);
    });
  }

  /**
   * Builds `input`, `keyString`'s layer of `generation`, and holds it, unless its layer has left or
   * been rebaked by then (`rocks`' revision of it).
   */
  async #build(
    keyString: string,
    generation: number,
    input: RockPagesInput,
    largestRockM: number,
    rocks: ResidentRocksView,
  ): Promise<void> {
    const built = await this.#builder.build(input);
    if (this.#building.get(keyString) === generation) {
      this.#building.delete(keyString);
    }
    if (this.#disposed || typeof built === "string" || built === null) {
      return;
    }
    // Dropped where its layer has left or been rebaked since it was asked.
    if (rocks.get(keyString)?.generation !== generation || this.#meshes.has(keyString)) {
      return;
    }
    this.#hold(keyString, {
      key: input.key,
      generation,
      originM: vec3(input.originM.x, input.originM.y, input.originM.z),
      file: built.file,
      records: built.records,
      rocks: built.rocks,
      triangles: built.triangles,
      largestRockM,
    });
  }

  /**
   * Holds a built mesh whole, or refuses it where the pool cannot (see the module's remarks): a
   * region patch's evicting the meshes outside the region, farthest first, until it is held; any
   * other's refused where it does not fit, or where it would take room a refused region patch
   * waits on.
   */
  #hold(
    keyString: string,
    built: Omit<RockPatchMesh, "mesh"> & { readonly file: ArrayBuffer },
  ): void {
    const index = readMeshIndex(built.file);
    if (index.kind !== "read") {
      this.#refuse(keyString, built.generation, Infinity);
      return;
    }
    const pages: MeshPage[] = [];
    for (const [p, entry] of index.value.pages.entries()) {
      const page = readMeshPage(
        built.file.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
        entry,
        p,
      );
      if (page.kind !== "read") {
        this.#refuse(keyString, built.generation, Infinity);
        return;
      }
      pages.push(page.value);
    }
    const slots = pages.reduce(
      (sum, page) => sum + Math.max(1, Math.ceil(page.body.byteLength / ROOT_PAGE_BYTES)),
      0,
    );
    const region = this.#inRegion(keyString);
    if (!region && this.#freeSlots() - slots < this.#regionWaiting(keyString)) {
      this.#refuse(keyString, built.generation, slots);
      return;
    }
    // Room for a region patch: the meshes outside the region let go until its slots are free, then
    // one more at each try a fragmented pool refuses.
    if (region) {
      while (this.#freeSlots() < slots) {
        if (!this.#evictOne()) {
          break;
        }
      }
    }
    for (;;) {
      const mesh = this.#tryHold(index.value, pages);
      if (mesh !== null) {
        const { file: _file, ...held } = built;
        this.#meshes.set(keyString, { ...held, mesh });
        this.#slots.set(keyString, slots);
        this.#refusedKeys.delete(keyString);
        this.#revision += 1;
        return;
      }
      if (!region || !this.#evictOne()) {
        this.#refuse(keyString, built.generation, slots);
        return;
      }
    }
  }

  /** Stores `pages` of a mesh of `index` whole, or none of them: its handle, or `null`. */
  #tryHold(index: Parameters<GeometryPagePool["addMesh"]>[0], pages: MeshPage[]): number | null {
    const mesh = this.pool.addMesh(index);
    for (const page of pages) {
      if (this.pool.insert(mesh, page).kind !== "stored") {
        this.pool.removeMesh(mesh);
        return null;
      }
    }
    return mesh;
  }

  /**
   * Lets go the held mesh outside the region farthest from it, its patch refused until room frees
   * or the region holds it: whether there was one.
   */
  #evictOne(): boolean {
    let victim: { readonly keyString: string; readonly held: RockPatchMesh } | null = null;
    let farthestM = -Infinity;
    for (const [keyString, held] of this.#meshes) {
      if (this.#inRegion(keyString)) {
        continue;
      }
      const distanceM = this.#fromRegionM(keyString, held);
      if (
        distanceM > farthestM ||
        (distanceM === farthestM && victim !== null && keyString < victim.keyString)
      ) {
        victim = { keyString, held };
        farthestM = distanceM;
      }
    }
    if (victim === null) {
      return false;
    }
    const slots = this.#slots.get(victim.keyString) ?? Infinity;
    this.#let(victim.keyString, victim.held.mesh);
    this.#refusedKeys.set(victim.keyString, {
      generation: victim.held.generation,
      slots,
      freed: this.#freed,
      region: false,
    });
    return true;
  }

  /** Lets go held mesh `mesh` of `keyString`. */
  #let(keyString: string, mesh: number): void {
    this.pool.removeMesh(mesh);
    this.#meshes.delete(keyString);
    this.#slots.delete(keyString);
    this.#revision += 1;
  }

  /** Refuses `keyString`'s mesh of `generation`, of `slots` root slots. */
  #refuse(keyString: string, generation: number, slots: number): void {
    this.#refused += 1;
    this.#refusedKeys.set(keyString, {
      generation,
      slots,
      freed: this.#freed,
      region: this.#inRegion(keyString),
    });
  }

  /** Whether resident patch `keyString` is the last sync's region's (see the module's remarks). */
  #inRegion(keyString: string): boolean {
    const { discs } = this.#region;
    if (discs.length === 0) {
      return false;
    }
    const sphere = this.#region.sphereOf(keyString);
    return sphere !== undefined && inRegion(sphere, discs);
  }

  /**
   * How far held patch `keyString`'s sphere stands from the region's nearest disc, m: its mesh's
   * origin alone where its sphere is not known.
   */
  #fromRegionM(keyString: string, held: RockPatchMesh): number {
    const sphere = this.#region.sphereOf(keyString) ?? { centreM: held.originM, radiusM: 0 };
    let nearestM = Infinity;
    for (const disc of this.#region.discs) {
      nearestM = Math.min(
        nearestM,
        norm(sub(sphere.centreM, disc.positionM)) - disc.radiusM - sphere.radiusM,
      );
    }
    return nearestM;
  }

  /** The pool's free root slots. */
  #freeSlots(): number {
    return this.pool.layout.rootSlots - this.pool.pressure().rootSlotsUsed;
  }

  /** The root slots the region's refused patches wait on, `except`'s left out. */
  #regionWaiting(except: string | null): number {
    let waiting = 0;
    for (const [keyString, refusal] of this.#refusedKeys) {
      if (keyString !== except && Number.isFinite(refusal.slots) && this.#inRegion(keyString)) {
        waiting += refusal.slots;
      }
    }
    return waiting;
  }

  /** Releases the pool and stops the builder. */
  dispose(): void {
    this.#disposed = true;
    this.#builder.dispose();
    this.pool.dispose();
    this.#meshes.clear();
    this.#building.clear();
    this.#refusedKeys.clear();
    this.#slots.clear();
  }
}
