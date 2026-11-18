/**
 * The rock layers a terrain pass keeps of its resident finest bakes on the render thread (plan
 * R11, T4.a): each patch's rocks held while its bake is resident, taken with it and forgotten with
 * it, so that the rocks ride in the patch cache and are never shed apart from their patch.
 *
 * @remarks
 * The pass hands it each bake it stores ({@link ResidentRocks.take}) and each key it forgets
 * ({@link ResidentRocks.forget}); a bake with no rock layer (a coarser level's, the test planet's)
 * forgets any rocks its key held. {@link ResidentRocks.revision} rises with every change, so that
 * the rocks' meshes (`rockPages.ts`) are made again only where the layers changed.
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { PatchKey } from "../../terrain/patchKey";
import type { BakedPatch } from "../../terrain/workers/messages";
import { type RockLayer, rockLayerBytes } from "./rockLayer";

/** One resident patch's rocks, with the origin its own vertices are placed about. */
export interface ResidentRockLayer {
  readonly key: PatchKey;
  /** The patch's origin, body-fixed m: its rocks are placed relative to it. */
  readonly originM: Vec3;
  readonly layer: RockLayer;
  /** The bake's generation, which changes with a rebake. */
  readonly generation: number;
}

/** What a reader sees of a pass's resident rocks. */
export interface ResidentRocksView {
  /** The rocks of the resident patch `keyString`, if it carries a rock layer. */
  get(keyString: string): ResidentRockLayer | undefined;
  /** Every resident patch's rocks, in no particular order. */
  values(): IterableIterator<ResidentRockLayer>;
  /** How many resident patches carry a rock layer. */
  readonly size: number;
  /** Rises with every layer taken or forgotten. */
  readonly revision: number;
  /** The bytes the layers hold, for the memory line. */
  readonly bytes: number;
}

/** A pass's resident rock layers (see the module's remarks). */
export class ResidentRocks implements ResidentRocksView {
  readonly #layers = new Map<string, ResidentRockLayer>();
  #revision = 0;
  #bytes = 0;

  get size(): number {
    return this.#layers.size;
  }

  get revision(): number {
    return this.#revision;
  }

  get bytes(): number {
    return this.#bytes;
  }

  get(keyString: string): ResidentRockLayer | undefined {
    return this.#layers.get(keyString);
  }

  values(): IterableIterator<ResidentRockLayer> {
    return this.#layers.values();
  }

  /** Holds `bake`'s rocks under `keyString`, or forgets the key's where it carries none. */
  take(keyString: string, bake: BakedPatch): void {
    const layer = bake.layers?.rocks;
    if (layer === undefined) {
      this.forget(keyString);
      return;
    }
    this.#drop(keyString);
    this.#layers.set(keyString, {
      key: bake.key,
      originM: bake.originM,
      layer,
      generation: bake.generation,
    });
    this.#bytes += rockLayerBytes(layer);
    this.#revision += 1;
  }

  /** Forgets `keyString`'s rocks, its patch no longer resident. */
  forget(keyString: string): void {
    if (this.#drop(keyString)) {
      this.#revision += 1;
    }
  }

  /** Forgets every patch's rocks. */
  clear(): void {
    if (this.#layers.size > 0) {
      this.#layers.clear();
      this.#bytes = 0;
      this.#revision += 1;
    }
  }

  #drop(keyString: string): boolean {
    const held = this.#layers.get(keyString);
    if (held === undefined) {
      return false;
    }
    this.#bytes -= rockLayerBytes(held.layer);
    this.#layers.delete(keyString);
    return true;
  }
}
