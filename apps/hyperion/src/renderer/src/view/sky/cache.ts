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
 * loss takes it. A cube whose last view was disposed is kept as the device's one spare until a view
 * takes it up again or another is baked ({@link SkyCubeCache.letGo}).
 *
 * A view's frames ask through {@link SkyCubeCache.want} (lane perf-long-tasks-3): a cube that must
 * be baked is baked in steps ({@link startSkyBake}), its stars laid out and then its splats and
 * levels in tasks of their own that end past {@link SKY_BAKE_SLICE_MS}, between the frames, and its
 * uploads in the next frame that asks for it, rather than all in the frame that first draws its
 * sky, where a re-bake at high held the page's thread 260 to 540 ms on a pod. Until it is baked the
 * view keeps the cube it holds, which its frames draw with the sky they drew before
 * (`ViewFrameDrawer`); from the peak pass's end the two cubes are held together.
 * {@link SkyCubeCache.acquire} bakes at once, as the cache always did.
 */

import type { SkyStars } from "@hyperion/protocol";

import { nextTask } from "../../lib/nextTask";
import type { RenderEngine } from "../engine/types";
import {
  type BakedCube,
  releaseBakedCube,
  type SkyBakeJob,
  type SkyBakeSource,
  startSkyBake,
} from "./bake";

/** What a view asks the cache for. */
export interface CubeRequest {
  /** The sky's stars, by identity: another sky is another cube. */
  readonly stars: SkyStars;
  /** The baked stars' indices, brightest first. */
  readonly baked: Uint32Array;
  /** The bake's source, made only when a cube must be baked. */
  readonly bakeSource: () => SkyBakeSource;
}

/**
 * A view's cube as {@link SkyCubeCache.want} finds it: baked (`null` with no baked stars), being
 * baked, or failed to bake.
 */
export type CubeState =
  | { readonly kind: "baked"; readonly cube: BakedCube | null }
  | { readonly kind: "baking" }
  | { readonly kind: "failed"; readonly error: unknown };

/**
 * The time after which a task of bake steps ends, at the end of its step, ms (lane
 * perf-long-tasks-3): a task's steps, a tile's splat and levels or 16,384 stars' layout, take a few
 * ms on a pod. The kernels', uploads' and scratch's making is a frame's step, run by `want`.
 */
export const SKY_BAKE_SLICE_MS = 6;

/** Runs `task` in a task of its own, after what the page has queued (a frame among them). */
export type BakeTaskScheduler = (task: () => void) => void;

/** One cube and the views that draw it. */
interface Entry {
  readonly stars: SkyStars;
  readonly baked: Uint32Array;
  readonly cube: BakedCube;
  readonly views: Set<string>;
}

/** A bake under way, or finished and not yet taken up, and the views that wait on it. */
interface Bake {
  readonly stars: SkyStars;
  readonly baked: Uint32Array;
  readonly job: SkyBakeJob;
  readonly views: Set<string>;
  /** Its end: its cube, or why it failed; `null` while it runs. */
  outcome:
    | { readonly kind: "baked"; readonly cube: BakedCube }
    | { readonly kind: "failed"; readonly error: unknown }
    | null;
}

/** Whether a bake is under way and its next step is a frame's. */
function waitsForFrame(bake: Bake): boolean {
  return bake.outcome === null && bake.job.awaitsFrame;
}

/** Whether two index lists hold the same stars in the same order. */
function sameStars(a: Uint32Array, b: Uint32Array): boolean {
  return a === b || (a.length === b.length && a.every((value, index) => value === b[index]));
}

/** Whether an entry or a bake serves a request: the same sky's same baked stars. */
function fits(
  held: Pick<Entry, "stars" | "baked">,
  request: Pick<CubeRequest, "stars" | "baked">,
): boolean {
  return held.stars === request.stars && sameStars(held.baked, request.baked);
}

/** The options of a cache, for the tests. */
export interface SkyCubeCacheOptions {
  /** Runs the bake steps' tasks; `nextTask`, a channel's message, by default. */
  readonly schedule?: BakeTaskScheduler;
  /** The clock a task's steps are timed by, ms; `performance.now` by default. */
  readonly now?: () => number;
}

/** The cubes of one engine's device, shared between its views. */
export class SkyCubeCache {
  readonly #engine: RenderEngine;
  readonly #schedule: BakeTaskScheduler;
  readonly #now: () => number;
  readonly #entries: Entry[] = [];
  /** Which entry each view draws. */
  readonly #byView = new Map<string, Entry>();
  /** The cube a disposed view let go last, which no view draws ({@link SkyCubeCache.letGo}). */
  #spare: Entry | null = null;
  /** The bakes under way or not yet taken up, oldest first. */
  readonly #bakes: Bake[] = [];
  /** The bake each view waits on. */
  readonly #waiting = new Map<string, Bake>();
  /** Whether a task of steps is queued. */
  #queued = false;

  constructor(engine: RenderEngine, options: SkyCubeCacheOptions = {}) {
    this.#engine = engine;
    this.#schedule = options.schedule ?? nextTask;
    this.#now = options.now ?? (() => performance.now());
    // A lost device took every cube with it, and every bake's handles: the views bake again on
    // their next frames. The cache lives as long as its engine, whose disposal drops the listener.
    engine.onRestored(() => {
      this.#entries.length = 0;
      this.#byView.clear();
      this.#spare = null;
      for (const bake of this.#bakes) {
        try {
          // The lost engine's handles: their releases are dropped (`ResilientEngine`).
          bake.job.cancel();
        } catch {
          // A bake's handles that cannot be released died with the device all the same.
        }
      }
      this.#bakes.length = 0;
      this.#waiting.clear();
    });
  }

  /** How many cubes the device holds, the spare and the baked ones not yet taken up among them. */
  get size(): number {
    return (
      this.#entries.length +
      (this.#spare === null ? 0 : 1) +
      this.#bakes.filter((bake) => bake.outcome?.kind === "baked").length
    );
  }

  /** How many bakes are under way. */
  get baking(): number {
    return this.#bakes.filter((bake) => bake.outcome === null).length;
  }

  /**
   * The cube a view draws, shared where one fits, baked at once where none does; `null` with no
   * baked stars.
   *
   * @param view - The view's name, which holds its cube until released or given another.
   * @throws Whatever the bake throws.
   */
  acquire(view: string, request: CubeRequest): BakedCube | null {
    const held = this.#byView.get(view);
    if (held !== undefined && fits(held, request)) {
      return held.cube;
    }
    this.release(view);
    if (request.baked.length === 0) {
      return null;
    }
    const entry = this.#fitting(request) ?? this.#bakeAtOnce(request);
    this.#hold(view, entry);
    return entry.cube;
  }

  /**
   * A view's cube as its frame asks for it (lane perf-long-tasks-3): its own or another view's
   * where one fits, else a bake begun, or joined, in the tasks after the frame, the view keeping
   * the cube it holds meanwhile ({@link SkyCubeCache.held}); once baked, the view takes it up at
   * its next ask, and lets its old one go. A bake whose next step is a frame's runs it here, in
   * the asking frame: its uploads and scratch, or the CPU bake's cube and levels written.
   *
   * @param view - The view's name, which holds its cube until released or given another.
   */
  want(view: string, request: CubeRequest): CubeState {
    const held = this.#byView.get(view);
    if (held !== undefined && fits(held, request)) {
      this.#stopWaiting(view);
      return { kind: "baked", cube: held.cube };
    }
    if (request.baked.length === 0) {
      this.release(view);
      return { kind: "baked", cube: null };
    }
    const entry = this.#fitting(request);
    if (entry !== undefined) {
      this.release(view);
      this.#hold(view, entry);
      return { kind: "baked", cube: entry.cube };
    }
    return this.#bakeFor(view, request);
  }

  /** The cube a view holds, which its frames draw while its next is baked; `null` with none. */
  held(view: string): BakedCube | null {
    return this.#byView.get(view)?.cube ?? null;
  }

  /**
   * Whether a request is served with no bake: its stars are none, or a cube the view holds,
   * another view's, the spare or a bake ended fits it. Asks nothing, and changes nothing.
   */
  serves(view: string, request: Pick<CubeRequest, "stars" | "baked">): boolean {
    const held = this.#byView.get(view);
    return (
      request.baked.length === 0 ||
      (held !== undefined && fits(held, request)) ||
      this.#entries.some((entry) => fits(entry, request)) ||
      (this.#spare !== null && fits(this.#spare, request)) ||
      this.#bakes.some((bake) => bake.outcome?.kind === "baked" && fits(bake, request))
    );
  }

  /** Lets a view's cube go, releasing it with its last view, and stops its wait on a bake. */
  release(view: string): void {
    this.#stopWaiting(view);
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

  /**
   * Lets a disposed view's cube go: released with its last view, as {@link SkyCubeCache.release}
   * releases it, but kept as the device's one spare until a view asks for it again, another cube is
   * baked, or the device is lost. A stage remounted for a new scene of the same sky (the `PLANET
   * VIEWER`'s next planet, plan R15) then takes its cube up again rather than bake it anew; the
   * spare it replaces is released. Its wait on a bake stops: one under way is stopped, and one
   * ended is the spare in the held cube's place.
   */
  letGo(view: string): void {
    const entry = this.#byView.get(view);
    if (entry !== undefined) {
      this.#byView.delete(view);
      entry.views.delete(view);
      if (entry.views.size === 0) {
        this.#entries.splice(this.#entries.indexOf(entry), 1);
        this.#keepAsSpare(entry);
      }
    }
    // After the held cube: a bake it waited on that has ended is the newer, kept as the spare in
    // its place.
    this.#stopWaiting(view);
  }

  /**
   * Runs the bakes' steps a task may run, the oldest first, until `untilMs` (at least one step of
   * the first that has one), as each task does; a bake whose next step is a frame's waits for a
   * view's frame to ask ({@link SkyCubeCache.want}). The tests step it themselves.
   */
  step(untilMs: number): void {
    for (const bake of this.#bakes) {
      if (bake.outcome !== null || bake.job.awaitsFrame) {
        continue;
      }
      const ended = this.#stepBake(bake, untilMs, false);
      // A bake still under way in a task's steps had the task's time; one that ended, or now
      // waits for a frame, leaves what is left to the next.
      if ((!ended && !waitsForFrame(bake)) || this.#now() >= untilMs) {
        return;
      }
    }
  }

  /** Runs a bake's steps, noting its end, its cube or why it failed; whether it ended. */
  #stepBake(bake: Bake, untilMs: number, inFrame: boolean): boolean {
    try {
      const cube = bake.job.step(untilMs, this.#now, inFrame);
      if (cube !== null) {
        bake.outcome = { kind: "baked", cube };
        return true;
      }
      return false;
    } catch (error: unknown) {
      // A bake that fails (a lost device) is said by its views' next ask, which leave the
      // sprites; tried again on another sky.
      bake.outcome = { kind: "failed", error };
      return true;
    }
  }

  /** The entry, or the spare, that serves a request, held among the entries. */
  #fitting(request: CubeRequest): Entry | undefined {
    const entry = this.#entries.find((candidate) => fits(candidate, request));
    if (entry !== undefined) {
      return entry;
    }
    if (this.#spare !== null && fits(this.#spare, request)) {
      const spare = this.#spare;
      this.#spare = null;
      this.#entries.push(spare);
      return spare;
    }
    // A bake finished and not yet taken up serves it too.
    const bake = this.#bakes.find((each) => each.outcome?.kind === "baked" && fits(each, request));
    if (bake?.outcome?.kind === "baked") {
      this.#forget(bake);
      const taken: Entry = {
        stars: bake.stars,
        baked: bake.baked,
        cube: bake.outcome.cube,
        views: new Set(),
      };
      this.#entries.push(taken);
      return taken;
    }
    return undefined;
  }

  /**
   * The bake a view waits on for a request no cube serves: one under way joined, or one begun; or
   * why the one that fits failed.
   */
  #bakeFor(view: string, request: CubeRequest): CubeState {
    let bake = this.#bakes.find((each) => fits(each, request));
    if (bake?.outcome?.kind === "failed") {
      const { error } = bake.outcome;
      // Forgotten once no view waits to be told: each view that waited is told at its next ask.
      this.#stopWaiting(view);
      if (bake.views.size === 0) {
        this.#forget(bake);
      }
      return { kind: "failed", error };
    }
    if (bake === undefined) {
      this.#stopWaiting(view);
      // The spare goes before a bake, so that the device never holds more than the bake would.
      this.#dropSpare();
      let job: SkyBakeJob;
      try {
        job = startSkyBake(this.#engine, request.bakeSource());
      } catch (error: unknown) {
        return { kind: "failed", error };
      }
      bake = { stars: request.stars, baked: request.baked, job, views: new Set(), outcome: null };
      this.#bakes.push(bake);
      this.#queue();
    } else if (this.#waiting.get(view) !== bake) {
      this.#stopWaiting(view);
    }
    bake.views.add(view);
    this.#waiting.set(view, bake);
    if (waitsForFrame(bake)) {
      // The frame's step, the uploads, in this view's frame; the rest in the tasks after it.
      if (this.#stepBake(bake, this.#now(), true)) {
        return this.want(view, request);
      }
      this.#queue();
    }
    return { kind: "baking" };
  }

  /** Bakes a request's cube at once, a bake of it under way included, and holds it. */
  #bakeAtOnce(request: CubeRequest): Entry {
    const under = this.#bakes.find((each) => each.outcome === null && fits(each, request));
    let cube: BakedCube;
    if (under === undefined) {
      // The spare goes before a bake, so that the device never holds more than the bake would.
      this.#dropSpare();
      const baked = startSkyBake(this.#engine, request.bakeSource()).step(
        Number.POSITIVE_INFINITY,
        this.#now,
        true,
      );
      if (baked === null) {
        throw new Error("a sky bake run at once stopped before its end");
      }
      cube = baked;
    } else {
      this.#forget(under);
      const baked = under.job.step(Number.POSITIVE_INFINITY, this.#now, true);
      if (baked === null) {
        throw new Error("a sky bake run at once stopped before its end");
      }
      cube = baked;
    }
    const entry: Entry = { stars: request.stars, baked: request.baked, cube, views: new Set() };
    this.#entries.push(entry);
    return entry;
  }

  /** Has a view draw an entry. */
  #hold(view: string, entry: Entry): void {
    entry.views.add(view);
    this.#byView.set(view, entry);
  }

  /**
   * Stops a view's wait on its bake: a bake no view waits on any longer is stopped, or, finished,
   * kept as the spare.
   */
  #stopWaiting(view: string): void {
    const bake = this.#waiting.get(view);
    if (bake === undefined) {
      return;
    }
    this.#waiting.delete(view);
    bake.views.delete(view);
    if (bake.views.size > 0) {
      return;
    }
    this.#forget(bake);
    if (bake.outcome === null) {
      bake.job.cancel();
    } else if (bake.outcome.kind === "baked") {
      this.#keepAsSpare({
        stars: bake.stars,
        baked: bake.baked,
        cube: bake.outcome.cube,
        views: new Set(),
      });
    }
  }

  /** Drops a bake from the list, its waiting views with it. */
  #forget(bake: Bake): void {
    const at = this.#bakes.indexOf(bake);
    if (at >= 0) {
      this.#bakes.splice(at, 1);
    }
    for (const view of bake.views) {
      if (this.#waiting.get(view) === bake) {
        this.#waiting.delete(view);
      }
    }
  }

  /** Keeps an entry no view draws as the spare, releasing the one it replaces. */
  #keepAsSpare(entry: Entry): void {
    this.#dropSpare();
    this.#spare = entry;
  }

  /** Releases the spare, if any. */
  #dropSpare(): void {
    if (this.#spare !== null) {
      releaseBakedCube(this.#engine, this.#spare.cube);
      this.#spare = null;
    }
  }

  /** Queues a task of steps while a bake runs. */
  #queue(): void {
    if (this.#queued) {
      return;
    }
    this.#queued = true;
    this.#schedule(() => {
      this.#queued = false;
      this.step(this.#now() + SKY_BAKE_SLICE_MS);
      // A bake whose next step is a frame's queues the tasks again from that frame's ask.
      if (this.#bakes.some((bake) => bake.outcome === null && !bake.job.awaitsFrame)) {
        this.#queue();
      }
    });
  }
}

/** Each engine's cache. */
const CACHES = new WeakMap<RenderEngine, SkyCubeCache>();

/**
 * The cube cache of an engine's device, made on first use.
 *
 * @param options - The options of the cache made by this call, the first for the engine (a test's
 *   scheduler and clock); a later call's are not read.
 */
export function skyCubeCacheOf(
  engine: RenderEngine,
  options: SkyCubeCacheOptions = {},
): SkyCubeCache {
  let cache = CACHES.get(engine);
  if (cache === undefined) {
    cache = new SkyCubeCache(engine, options);
    CACHES.set(engine, cache);
  }
  return cache;
}
