/**
 * The baked star cubes of one device, shared between its views (plan R06, T14; the brainstorm's
 * "Several views in one client").
 *
 * @remarks
 * A cube is baked once per sky and set of baked stars, and every view that bakes the same stars of
 * the same sky draws it. A cube's stars are placed from its sky's observer, so a view whose camera
 * has moved far enough for the parallax rule to call for a re-bake (Design note 20) needs a new
 * sky from its new place, which `useSky`'s request rule asks for; two views far apart, such as two
 * cameras across the nuclear disc, ask skies of their own and so hold a cube each. A cube is
 * released through T13.h's releases when its last view lets it go, and forgotten when a device
 * loss takes it.
 */

import type { SkyStars } from "@hyperion/protocol";

import type { RenderEngine } from "../engine/types";
import { type BakedCube, type BakeInput, bakeSkyCube, releaseBakedCube } from "./bake";

/** What a view asks the cache for. */
export interface CubeRequest {
  /** The sky's stars, by identity: another sky is another cube. */
  readonly stars: SkyStars;
  /** The baked stars' indices, brightest first. */
  readonly baked: Uint32Array;
  /** The bake's input, made only when a cube must be baked. */
  readonly bakeInput: () => BakeInput;
}

/** One cube and the views that draw it. */
interface Entry {
  readonly stars: SkyStars;
  readonly baked: Uint32Array;
  readonly cube: BakedCube;
  readonly views: Set<string>;
}

/** Whether two index lists hold the same stars in the same order. */
function sameStars(a: Uint32Array, b: Uint32Array): boolean {
  return a === b || (a.length === b.length && a.every((value, index) => value === b[index]));
}

/** The cubes of one engine's device, shared between its views. */
export class SkyCubeCache {
  readonly #engine: RenderEngine;
  readonly #entries: Entry[] = [];
  /** Which entry each view draws. */
  readonly #byView = new Map<string, Entry>();

  constructor(engine: RenderEngine) {
    this.#engine = engine;
    // A lost device took every cube with it: the views bake again on their next frames. The cache
    // lives as long as its engine, whose disposal drops the listener.
    engine.onRestored(() => {
      this.#entries.length = 0;
      this.#byView.clear();
    });
  }

  /** How many cubes the device holds. */
  get size(): number {
    return this.#entries.length;
  }

  /**
   * The cube a view draws, shared where one fits, baked where none does; `null` with no baked stars.
   *
   * @param view - The view's name, which holds its cube until released or given another.
   */
  acquire(view: string, request: CubeRequest): BakedCube | null {
    const held = this.#byView.get(view);
    if (held !== undefined && this.#fits(held, request)) {
      return held.cube;
    }
    this.release(view);
    if (request.baked.length === 0) {
      return null;
    }
    let entry = this.#entries.find((candidate) => this.#fits(candidate, request));
    if (entry === undefined) {
      entry = {
        stars: request.stars,
        baked: request.baked,
        cube: bakeSkyCube(this.#engine, request.bakeInput()),
        views: new Set(),
      };
      this.#entries.push(entry);
    }
    entry.views.add(view);
    this.#byView.set(view, entry);
    return entry.cube;
  }

  /** Lets a view's cube go, releasing it with its last view. */
  release(view: string): void {
    const entry = this.#byView.get(view);
    if (entry === undefined) {
      return;
    }
    this.#byView.delete(view);
    entry.views.delete(view);
    if (entry.views.size === 0) {
      this.#entries.splice(this.#entries.indexOf(entry), 1);
      releaseBakedCube(this.#engine, entry.cube);
    }
  }

  /** Whether an entry serves a request: the same sky's same baked stars. */
  #fits(entry: Entry, request: CubeRequest): boolean {
    return entry.stars === request.stars && sameStars(entry.baked, request.baked);
  }
}

/** Each engine's cache. */
const CACHES = new WeakMap<RenderEngine, SkyCubeCache>();

/** The cube cache of an engine's device, made on first use. */
export function skyCubeCacheOf(engine: RenderEngine): SkyCubeCache {
  let cache = CACHES.get(engine);
  if (cache === undefined) {
    cache = new SkyCubeCache(engine);
    CACHES.set(engine, cache);
  }
  return cache;
}
