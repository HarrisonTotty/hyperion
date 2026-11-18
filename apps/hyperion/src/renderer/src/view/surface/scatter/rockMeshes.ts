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
 *   ({@link RockMeshes.refused}), its patch's rocks not drawn.
 * - **Builds**: {@link RockMeshes.sync} asks a build for each resident layer it holds no mesh of,
 *   or holds an older bake's, and removes each mesh whose patch has left; a build that lands after
 *   its layer left or was rebaked is dropped. Nothing is asked while the resident layers' revision
 *   is unchanged.
 */

import { vec3, type Vec3 } from "../../../geometry/vec3";
import type { RockPageBuilder } from "../../clusters/meshPageProducer";
import { readMeshIndex, readMeshPage, ROOT_PAGE_BYTES } from "../../clusters/pageFormat";
import { GeometryPagePool, type GeometryPagePoolEngine, pagePoolSlots } from "../../clusters/pages";
import { type PatchKey, patchKeyString } from "../../terrain/patchKey";
import type { ResidentRocksView } from "./residentRocks";
import { rocksOf } from "./rockLayer";
import type { RockPagesInput } from "./rockPages";

/** The share of the setting's `pageBytes` the rocks' pool takes: a half (lean, 2026-11-18). */
export const ROCK_POOL_SHARE = 0.5;

/** Bytes of a pool slot's records, LOD spheres and errors (`clusters/pages.ts`' own). */
const SLOT_RECORD_BYTES = 128 * (32 + 16 + 4);

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
  #revision = -1;
  #refused = 0;
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

  /** How many meshes the pool could not hold whole since it was made. */
  get refused(): number {
    return this.#refused;
  }

  /** Brings its meshes to `rocks`, the pass's resident layers (see the module's remarks). */
  sync(rocks: ResidentRocksView): void {
    if (this.#disposed || rocks.revision === this.#revision) {
      return;
    }
    this.#revision = rocks.revision;
    for (const [keyString, held] of this.#meshes) {
      if (rocks.get(keyString)?.generation !== held.generation) {
        this.pool.removeMesh(held.mesh);
        this.#meshes.delete(keyString);
      }
    }
    for (const resident of rocks.values()) {
      const keyString = patchKeyString(resident.key);
      const generation = resident.generation;
      if (
        resident.layer.count === 0 ||
        this.#meshes.has(keyString) ||
        this.#building.get(keyString) === generation
      ) {
        continue;
      }
      this.#building.set(keyString, generation);
      const largestRockM = rocksOf(resident.layer).reduce(
        (most, rock) => Math.max(most, 2 * Math.max(...rock.semiAxesM)),
        0,
      );
      const input = { key: resident.key, originM: resident.originM, layer: resident.layer };
      void this.#build(keyString, generation, input, largestRockM, rocks).catch(
        (error: unknown) => {
          this.#building.delete(keyString);
          console.warn(`the rocks of ${keyString} could not be built`, error);
        },
      );
    }
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

  /** Holds a built mesh whole, or refuses it where the pool cannot. */
  #hold(
    keyString: string,
    built: Omit<RockPatchMesh, "mesh"> & { readonly file: ArrayBuffer },
  ): void {
    const index = readMeshIndex(built.file);
    if (index.kind !== "read") {
      this.#refused += 1;
      return;
    }
    const mesh = this.pool.addMesh(index.value);
    for (const [p, entry] of index.value.pages.entries()) {
      const page = readMeshPage(
        built.file.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
        entry,
        p,
      );
      if (page.kind !== "read" || this.pool.insert(mesh, page.value).kind !== "stored") {
        this.pool.removeMesh(mesh);
        this.#refused += 1;
        return;
      }
    }
    const { file: _file, ...held } = built;
    this.#meshes.set(keyString, { ...held, mesh });
  }

  /** Releases the pool and stops the builder. */
  dispose(): void {
    this.#disposed = true;
    this.#builder.dispose();
    this.pool.dispose();
    this.#meshes.clear();
    this.#building.clear();
  }
}
